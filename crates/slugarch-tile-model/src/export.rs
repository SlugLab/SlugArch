use crate::{
    first_fault, generate_workload, inject_one, EventKind, FaultKind, ModelError, TileEvent,
    WorkloadKind, WorkloadTrace,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum RecordMode {
    Validation = 1,
    Delta = 2,
    Full = 3,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusConfig {
    pub tiles: u16,
    pub record_mode: RecordMode,
    pub seed: u64,
    pub fault: Option<FaultKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusExport {
    pub sha256: [u8; 32],
    pub case_count: usize,
}

#[derive(Debug, Serialize)]
struct CorpusCase {
    schema_version: u32,
    case_kind: &'static str,
    evidence_kind: &'static str,
    physical_cxl_cache: bool,
    tiles: u16,
    record_mode: u8,
    seed: u64,
    workload_id: u8,
    workload: &'static str,
    warmup_events: usize,
    measured_events: usize,
    warmup_sha256: String,
    measured_sha256: String,
    fault_code: Option<u32>,
    injected_tile_id: Option<u16>,
    injected_event_id: Option<u64>,
    first_failure_tile_id: Option<u16>,
    first_failure_event_id: Option<u64>,
}

pub fn export_corpus(config: &CorpusConfig, output: &Path) -> Result<CorpusExport, ModelError> {
    let parent = output
        .parent()
        .ok_or_else(|| export_error(output, "output has no parent"))?;
    if !parent.is_dir() {
        return Err(export_error(
            output,
            "output parent directory does not exist",
        ));
    }

    let mut cases = Vec::with_capacity(4 + usize::from(config.fault.is_some()));
    for workload in WorkloadKind::ALL {
        let trace = generate_workload(workload, config.tiles, 100, 10_000, config.seed)?;
        cases.push(workload_case(config, workload, &trace));
    }
    if let Some(kind) = config.fault {
        cases.push(fault_case(config, kind)?);
    }

    let mut bytes = Vec::new();
    for case in &cases {
        serde_json::to_writer(&mut bytes, case)
            .map_err(|error| export_error(output, &format!("serialize corpus case: {error}")))?;
        bytes.push(b'\n');
    }

    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| export_error(output, "output filename is not valid UTF-8"))?;
    let temporary = parent.join(format!(".{file_name}.tmp"));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| export_error(output, &format!("create temporary output: {error}")))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| export_error(output, &format!("write temporary output: {error}")))?;
    fs::rename(&temporary, output)
        .map_err(|error| export_error(output, &format!("commit output: {error}")))?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| export_error(output, &format!("sync output directory: {error}")))?;

    Ok(CorpusExport {
        sha256: Sha256::digest(&bytes).into(),
        case_count: cases.len(),
    })
}

fn workload_case(
    config: &CorpusConfig,
    workload: WorkloadKind,
    trace: &WorkloadTrace,
) -> CorpusCase {
    CorpusCase {
        schema_version: 1,
        case_kind: "workload",
        evidence_kind: "qemu_event_level_home_agent_model",
        physical_cxl_cache: false,
        tiles: config.tiles,
        record_mode: config.record_mode as u8,
        seed: config.seed,
        workload_id: workload.id(),
        workload: workload.name(),
        warmup_events: trace.warmup.len(),
        measured_events: trace.measured.len(),
        warmup_sha256: hash_events(&trace.warmup),
        measured_sha256: hash_events(&trace.measured),
        fault_code: None,
        injected_tile_id: None,
        injected_event_id: None,
        first_failure_tile_id: None,
        first_failure_event_id: None,
    }
}

fn fault_case(config: &CorpusConfig, kind: FaultKind) -> Result<CorpusCase, ModelError> {
    let workload = kind.workload();
    let trace = generate_workload(workload, config.tiles, 100, 10_000, config.seed)?;
    let target = trace
        .measured
        .iter()
        .find(|event| kind.eligible(event))
        .ok_or_else(|| ModelError::new(0x0006, 0, 0, 2, "no eligible fault event"))?;
    let faulted = inject_one(&trace, kind, target.tile_id, target.event_id)?;
    let failure = first_fault(&faulted).ok_or_else(|| {
        ModelError::new(0x0006, target.tile_id, target.event_id, 2, "fault missed")
    })?;

    Ok(CorpusCase {
        schema_version: 1,
        case_kind: "fault",
        evidence_kind: "qemu_event_level_home_agent_model",
        physical_cxl_cache: false,
        tiles: config.tiles,
        record_mode: config.record_mode as u8,
        seed: config.seed,
        workload_id: workload.id(),
        workload: workload.name(),
        warmup_events: faulted.trace.warmup.len(),
        measured_events: faulted.trace.measured.len(),
        warmup_sha256: hash_events(&faulted.trace.warmup),
        measured_sha256: hash_events(&faulted.trace.measured),
        fault_code: Some(failure.code as u32),
        injected_tile_id: Some(faulted.injected_tile_id),
        injected_event_id: Some(faulted.injected_event_id),
        first_failure_tile_id: Some(failure.tile_id),
        first_failure_event_id: Some(failure.event_id),
    })
}

fn hash_events(events: &[TileEvent]) -> String {
    let canonical = serde_json::to_vec(events).expect("events are serializable");
    hex(&Sha256::digest(canonical))
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn export_error(output: &Path, detail: &str) -> ModelError {
    ModelError::new(0x0009, 0, 0, 0, format!("{}: {detail}", output.display()))
}

impl WorkloadKind {
    pub const ALL: [Self; 4] = [
        Self::PrivatePartitions,
        Self::ReadSharedFanout,
        Self::ProducerConsumer,
        Self::HotLinePingPong,
    ];

    fn id(self) -> u8 {
        match self {
            Self::PrivatePartitions => 1,
            Self::ReadSharedFanout => 2,
            Self::ProducerConsumer => 3,
            Self::HotLinePingPong => 4,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::PrivatePartitions => "private_partitions",
            Self::ReadSharedFanout => "read_shared_fanout",
            Self::ProducerConsumer => "producer_consumer",
            Self::HotLinePingPong => "hot_line_ping_pong",
        }
    }
}

impl FaultKind {
    fn workload(self) -> WorkloadKind {
        match self {
            Self::MissingInvalidateAck | Self::ReorderedCompletion => WorkloadKind::HotLinePingPong,
            Self::StaleLineVersion | Self::FenceOmission => WorkloadKind::ProducerConsumer,
            Self::PolicyDigestMismatch | Self::RequiredRecordDrop => {
                WorkloadKind::PrivatePartitions
            }
        }
    }

    fn eligible(self, event: &TileEvent) -> bool {
        match self {
            Self::MissingInvalidateAck => event.kind == EventKind::InvalidateAck,
            Self::StaleLineVersion => event.kind == EventKind::ReadShared && event.version > 0,
            Self::ReorderedCompletion => event.kind == EventKind::Completion,
            Self::FenceOmission => event.kind == EventKind::Fence,
            Self::PolicyDigestMismatch | Self::RequiredRecordDrop => true,
        }
    }
}
