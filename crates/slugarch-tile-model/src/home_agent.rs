use crate::{AppliedEvent, EventKind, FaultCode, LineState, ModelError, TileEvent};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingWrite {
    tile_id: u16,
    version: u64,
    epoch: u64,
    fenced: bool,
}

#[derive(Debug, Default)]
pub struct HomeAgent {
    lines: BTreeMap<u64, LineState>,
    pending_writes: BTreeMap<u64, PendingWrite>,
    records: Vec<AppliedEvent>,
}

impl HomeAgent {
    pub fn apply(&mut self, event: TileEvent) -> Result<&AppliedEvent, ModelError> {
        if event.kind == EventKind::EpochSeal {
            self.seal_epoch(&event)?;
            self.records.push(AppliedEvent {
                event,
                line_before: None,
                line_after: None,
            });
            return Ok(self.records.last().expect("record was just appended"));
        }

        let address = event.line_address;
        let before = self.lines.get(&address).cloned();
        let mut line = before.clone().unwrap_or_default();
        let tile_bit = 1u64 << event.tile_id;

        match event.kind {
            EventKind::ReadShared => {
                if event.version < line.version {
                    return Err(fault(
                        FaultCode::CohStaleVersion,
                        &event,
                        "shared read observed an older line version",
                    ));
                }
                if event.version > line.version {
                    return Err(fault(
                        FaultCode::CohCompletionOrder,
                        &event,
                        "shared read observed a version not yet installed",
                    ));
                }
                line.sharers |= tile_bit;
            }
            EventKind::ReadExclusive => {
                let mut invalidations = line.sharers & !tile_bit;
                if let Some(owner) = line.owner_tile {
                    if owner != event.tile_id {
                        invalidations |= 1u64 << owner;
                    }
                }
                line.owner_tile = Some(event.tile_id);
                line.sharers = tile_bit;
                line.outstanding_invalidations = invalidations;
            }
            EventKind::Writeback => {
                if event.version <= line.version {
                    return Err(fault(
                        FaultCode::CohStaleVersion,
                        &event,
                        "writeback version must advance the line",
                    ));
                }
                line.version = event.version;
                line.owner_tile = Some(event.tile_id);
                line.sharers = tile_bit;
                line.last_writer_tile = Some(event.tile_id);
                self.pending_writes.insert(
                    address,
                    PendingWrite {
                        tile_id: event.tile_id,
                        version: event.version,
                        epoch: event.epoch,
                        fenced: false,
                    },
                );
            }
            EventKind::Invalidate => {
                if line.outstanding_invalidations & tile_bit == 0 {
                    return Err(ModelError::new(
                        0x0003,
                        event.tile_id,
                        event.event_id,
                        event.epoch,
                        "tile has no pending invalidation",
                    ));
                }
            }
            EventKind::InvalidateAck => {
                if line.outstanding_invalidations & tile_bit == 0 {
                    return Err(ModelError::new(
                        0x0004,
                        event.tile_id,
                        event.event_id,
                        event.epoch,
                        "tile acknowledged a non-pending invalidation",
                    ));
                }
                line.outstanding_invalidations &= !tile_bit;
            }
            EventKind::Fence => {
                let Some(pending) = self.pending_writes.get_mut(&address) else {
                    return Err(fault(
                        FaultCode::CohFenceMissing,
                        &event,
                        "fence has no matching pending write",
                    ));
                };
                if pending.tile_id != event.tile_id
                    || pending.version != event.version
                    || pending.epoch != event.epoch
                {
                    return Err(fault(
                        FaultCode::CohFenceMissing,
                        &event,
                        "fence does not match the pending write",
                    ));
                }
                pending.fenced = true;
                line.visible_epoch = event.epoch;
            }
            EventKind::Completion => {
                if line.outstanding_invalidations != 0 {
                    return Err(fault(
                        FaultCode::CohInvalidatePending,
                        &event,
                        "completion exposed before invalidation acknowledgements",
                    ));
                }
                if event.version > line.version {
                    return Err(fault(
                        FaultCode::CohCompletionOrder,
                        &event,
                        "completion exposed before its writeback",
                    ));
                }
                if let Some(pending) = self.pending_writes.get(&address) {
                    if pending.tile_id == event.tile_id
                        && pending.version == event.version
                        && !pending.fenced
                    {
                        return Err(fault(
                            FaultCode::CohFenceMissing,
                            &event,
                            "completion exposed before the producer fence",
                        ));
                    }
                }
                self.pending_writes.remove(&address);
            }
            EventKind::EpochSeal => unreachable!("handled before line transition"),
        }

        self.lines.insert(address, line.clone());
        self.records.push(AppliedEvent {
            event,
            line_before: before,
            line_after: Some(line),
        });
        Ok(self.records.last().expect("record was just appended"))
    }

    pub fn line(&self, address: u64) -> Option<&LineState> {
        self.lines.get(&address)
    }

    pub fn records(&self) -> &[AppliedEvent] {
        &self.records
    }

    fn seal_epoch(&self, event: &TileEvent) -> Result<(), ModelError> {
        if self
            .lines
            .values()
            .any(|line| line.outstanding_invalidations != 0)
        {
            return Err(fault(
                FaultCode::CohInvalidatePending,
                event,
                "epoch sealed with pending invalidations",
            ));
        }
        if self.pending_writes.values().any(|write| !write.fenced) {
            return Err(fault(
                FaultCode::CohFenceMissing,
                event,
                "epoch sealed with an unfenced write",
            ));
        }
        Ok(())
    }
}

fn fault(code: FaultCode, event: &TileEvent, detail: &str) -> ModelError {
    ModelError::new(
        code as u32,
        event.tile_id,
        event.event_id,
        event.epoch,
        detail,
    )
}
