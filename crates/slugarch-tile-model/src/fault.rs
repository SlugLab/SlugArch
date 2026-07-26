use crate::{EventKind, FailureRecord, FaultCode, HomeAgent, ModelError, TileEvent, WorkloadTrace};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaultKind {
    MissingInvalidateAck,
    StaleLineVersion,
    ReorderedCompletion,
    FenceOmission,
    PolicyDigestMismatch,
    RequiredRecordDrop,
}

impl FaultKind {
    pub const ALL: [Self; 6] = [
        Self::MissingInvalidateAck,
        Self::StaleLineVersion,
        Self::ReorderedCompletion,
        Self::FenceOmission,
        Self::PolicyDigestMismatch,
        Self::RequiredRecordDrop,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaultedTrace {
    pub trace: WorkloadTrace,
    pub kind: FaultKind,
    pub injected_tile_id: u16,
    pub injected_event_id: u64,
    pub original_event_sha256: [u8; 32],
    pub transformed_event_sha256: [u8; 32],
    pub expected_failure: FailureRecord,
}

pub fn inject_one(
    trace: &WorkloadTrace,
    kind: FaultKind,
    tile_id: u16,
    event_id: u64,
) -> Result<FaultedTrace, ModelError> {
    let mut transformed = trace.clone();
    let (phase, position) = locate_event(&transformed, tile_id, event_id).ok_or_else(|| {
        ModelError::new(
            0x0006,
            tile_id,
            event_id,
            0,
            "injection event does not exist",
        )
    })?;
    let events = phase.events_mut(&mut transformed);
    let original = events[position].clone();
    let original_hash = hash_event(position, Some(&original), None);

    let expected_failure = match kind {
        FaultKind::MissingInvalidateAck => {
            require_kind(&original, EventKind::InvalidateAck)?;
            let completion = events[position + 1..]
                .iter()
                .find(|event| {
                    event.line_address == original.line_address
                        && event.kind == EventKind::Completion
                })
                .cloned()
                .ok_or_else(|| {
                    ineligible(&original, "no completion follows the acknowledgement")
                })?;
            events.remove(position);
            failure(FaultCode::CohInvalidatePending, &completion)
        }
        FaultKind::StaleLineVersion => {
            require_kind(&original, EventKind::ReadShared)?;
            if original.version == 0 {
                return Err(ineligible(
                    &original,
                    "stale-version injection requires a nonzero version",
                ));
            }
            events[position].version -= 1;
            failure(FaultCode::CohStaleVersion, &events[position])
        }
        FaultKind::ReorderedCompletion => {
            require_kind(&original, EventKind::Completion)?;
            let writeback_position = events[..position]
                .iter()
                .rposition(|event| {
                    event.tile_id == original.tile_id
                        && event.line_address == original.line_address
                        && event.version == original.version
                        && event.kind == EventKind::Writeback
                })
                .ok_or_else(|| ineligible(&original, "completion has no preceding writeback"))?;
            let completion = events.remove(position);
            events.insert(writeback_position, completion.clone());
            failure(FaultCode::CohCompletionOrder, &completion)
        }
        FaultKind::FenceOmission => {
            require_kind(&original, EventKind::Fence)?;
            let completion = events[position + 1..]
                .iter()
                .find(|event| {
                    event.tile_id == original.tile_id
                        && event.line_address == original.line_address
                        && event.version == original.version
                        && event.kind == EventKind::Completion
                })
                .cloned()
                .ok_or_else(|| ineligible(&original, "fence has no matching completion"))?;
            events.remove(position);
            failure(FaultCode::CohFenceMissing, &completion)
        }
        FaultKind::PolicyDigestMismatch => failure(FaultCode::PolicyDigest, &original),
        FaultKind::RequiredRecordDrop => failure(FaultCode::RecordDrop, &original),
    };

    let transformed_position = events
        .iter()
        .position(|event| event.event_id == original.event_id && event.tile_id == original.tile_id);
    let transformed_event = transformed_position.map(|index| &events[index]);
    let transformed_hash = hash_event(
        transformed_position.unwrap_or(position),
        transformed_event,
        Some(kind),
    );

    Ok(FaultedTrace {
        trace: transformed,
        kind,
        injected_tile_id: tile_id,
        injected_event_id: event_id,
        original_event_sha256: original_hash,
        transformed_event_sha256: transformed_hash,
        expected_failure,
    })
}

pub fn first_fault(faulted: &FaultedTrace) -> Option<FailureRecord> {
    let mut warmup_agent = HomeAgent::default();
    if let Some(failure) = run_phase(&mut warmup_agent, &faulted.trace.warmup, faulted) {
        return Some(failure);
    }
    let mut measured_agent = HomeAgent::default();
    run_phase(&mut measured_agent, &faulted.trace.measured, faulted)
}

fn run_phase(
    agent: &mut HomeAgent,
    events: &[TileEvent],
    faulted: &FaultedTrace,
) -> Option<FailureRecord> {
    for event in events {
        if matches!(
            faulted.kind,
            FaultKind::PolicyDigestMismatch | FaultKind::RequiredRecordDrop
        ) && event.tile_id == faulted.injected_tile_id
            && event.event_id == faulted.injected_event_id
        {
            return Some(faulted.expected_failure.clone());
        }
        if let Err(error) = agent.apply(event.clone()) {
            return fault_code(error.code).map(|code| FailureRecord {
                code,
                tile_id: error.tile_id,
                event_id: error.event_id,
                epoch: error.epoch,
            });
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
enum Phase {
    Warmup,
    Measured,
}

impl Phase {
    fn events_mut(self, trace: &mut WorkloadTrace) -> &mut Vec<TileEvent> {
        match self {
            Self::Warmup => &mut trace.warmup,
            Self::Measured => &mut trace.measured,
        }
    }
}

fn locate_event(trace: &WorkloadTrace, tile_id: u16, event_id: u64) -> Option<(Phase, usize)> {
    trace
        .warmup
        .iter()
        .position(|event| event.tile_id == tile_id && event.event_id == event_id)
        .map(|position| (Phase::Warmup, position))
        .or_else(|| {
            trace
                .measured
                .iter()
                .position(|event| event.tile_id == tile_id && event.event_id == event_id)
                .map(|position| (Phase::Measured, position))
        })
}

fn require_kind(event: &TileEvent, expected: EventKind) -> Result<(), ModelError> {
    if event.kind == expected {
        Ok(())
    } else {
        Err(ineligible(
            event,
            &format!("fault requires {expected:?}, observed {:?}", event.kind),
        ))
    }
}

fn ineligible(event: &TileEvent, detail: &str) -> ModelError {
    ModelError::new(0x0006, event.tile_id, event.event_id, event.epoch, detail)
}

fn failure(code: FaultCode, event: &TileEvent) -> FailureRecord {
    FailureRecord {
        code,
        tile_id: event.tile_id,
        event_id: event.event_id,
        epoch: event.epoch,
    }
}

fn fault_code(code: u32) -> Option<FaultCode> {
    match code {
        0x1001 => Some(FaultCode::CohInvalidatePending),
        0x1002 => Some(FaultCode::CohStaleVersion),
        0x1003 => Some(FaultCode::CohCompletionOrder),
        0x1004 => Some(FaultCode::CohFenceMissing),
        0x2001 => Some(FaultCode::PolicyDigest),
        0x2002 => Some(FaultCode::RecordDrop),
        _ => None,
    }
}

fn hash_event(position: usize, event: Option<&TileEvent>, marker: Option<FaultKind>) -> [u8; 32] {
    let canonical = serde_json::to_vec(&(position, event, marker)).expect("serializable event");
    Sha256::digest(canonical).into()
}
