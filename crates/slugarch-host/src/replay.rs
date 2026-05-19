//! Software EMC/HJ layer for the Plan 4 CXL endpoint.
//!
//! This is intentionally a boundary recorder: it sees CXL messages and
//! endpoint-local workload hints, not the private RTL state inside the IP.

use crate::GemmResult;
use serde::{Deserialize, Serialize};
use slugarch_cxl_wire::{
    encode, CxlMsg, D2HReqOp, D2HRespOp, H2DReqOp, H2DRespOp, M2SReqOp, M2SRwDOp, MsgClass,
    S2MDRSOp, S2MNDROp, FLIT_BYTES,
};
use slugarch_ir::types::IpId;

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CxlRecordMode {
    Validation,
    Delta,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CxlDirection {
    HostToDevice,
    DeviceToHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CxlEndpoint {
    Host,
    Device(IpId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CxlTransactionClass {
    CxlMemRead,
    CxlMemWrite,
    CxlMemWritePartial,
    CxlMemEviction,
    CxlMemData,
    CxlCompletion,
    CxlCacheRequest,
    CxlCacheResponse,
    CxlSnoopRequest,
    CxlSnoopResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayloadCapture {
    None,
    Hash64(u64),
    Delta {
        baseline_hash: u64,
        changed_bytes: Vec<(u8, u8)>,
    },
    Full(Vec<u8>),
}

impl PayloadCapture {
    fn from_payload(mode: CxlRecordMode, payload: Option<&[u8]>) -> Self {
        let Some(bytes) = payload else {
            return PayloadCapture::None;
        };

        match mode {
            CxlRecordMode::Validation => PayloadCapture::Hash64(stable_hash64(bytes)),
            CxlRecordMode::Delta => {
                let changed_bytes = bytes
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, byte)| {
                        if *byte == 0 {
                            None
                        } else {
                            Some((idx as u8, *byte))
                        }
                    })
                    .collect();
                PayloadCapture::Delta {
                    baseline_hash: stable_hash64(&[0u8; 32]),
                    changed_bytes,
                }
            }
            CxlRecordMode::Full => PayloadCapture::Full(bytes.to_vec()),
        }
    }

    pub fn captured_bytes(&self) -> u64 {
        match self {
            PayloadCapture::None => 0,
            PayloadCapture::Hash64(_) => 8,
            PayloadCapture::Delta {
                baseline_hash: _,
                changed_bytes,
            } => 8 + (changed_bytes.len() as u64 * 2),
            PayloadCapture::Full(bytes) => bytes.len() as u64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CxlRecordPolicy {
    pub mode: CxlRecordMode,
    pub endpoint: IpId,
}

impl CxlRecordPolicy {
    pub fn gemm(mode: CxlRecordMode) -> Self {
        Self {
            mode,
            endpoint: IpId::SlugCxl4x4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CxlReplayRecord {
    pub sequence: u64,
    pub source: CxlEndpoint,
    pub destination: CxlEndpoint,
    pub epoch: u64,
    pub direction: CxlDirection,
    pub transaction_class: CxlTransactionClass,
    pub msg_class: MsgClass,
    pub opcode: String,
    pub tag: u16,
    pub address: Option<u64>,
    pub dependencies: Vec<u16>,
    pub fence: bool,
    pub payload: PayloadCapture,
    pub provenance: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CxlReplayArtifact {
    pub policy: CxlRecordPolicy,
    pub records: Vec<CxlReplayRecord>,
    pub final_result_commitment: u64,
    pub summary: CxlReplaySummary,
}

impl CxlReplayArtifact {
    pub fn validate_equivalent(&self, other: &Self) -> CxlReplayValidation {
        let records_compared = self.records.len().min(other.records.len());
        let record_mismatches = self
            .records
            .iter()
            .zip(&other.records)
            .filter(|(left, right)| *left != *right)
            .count()
            + self.records.len().abs_diff(other.records.len());

        CxlReplayValidation {
            records_compared,
            record_count_matches: self.records.len() == other.records.len(),
            record_mismatches,
            final_commitment_matches: self.final_result_commitment == other.final_result_commitment,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CxlReplaySummary {
    pub record_count: u64,
    pub epoch_count: u64,
    pub application_flit_bytes: u64,
    pub replay_record_bytes: u64,
    pub payload_capture_bytes: u64,
    pub hash_payload_records: u64,
    pub delta_payload_records: u64,
    pub full_payload_records: u64,
}

impl CxlReplaySummary {
    pub fn replay_bytes_per_app_gib(&self) -> f64 {
        if self.application_flit_bytes == 0 {
            return 0.0;
        }
        self.replay_record_bytes as f64 * GIB / self.application_flit_bytes as f64
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CxlReplayValidation {
    pub records_compared: usize,
    pub record_count_matches: bool,
    pub record_mismatches: usize,
    pub final_commitment_matches: bool,
}

impl CxlReplayValidation {
    pub fn is_equivalent(&self) -> bool {
        self.record_count_matches && self.record_mismatches == 0 && self.final_commitment_matches
    }
}

#[derive(Debug, Clone)]
pub struct CxlRecordedRun {
    pub result: GemmResult,
    pub artifact: CxlReplayArtifact,
}

pub(crate) struct CxlTraceRecorder {
    policy: CxlRecordPolicy,
    records: Vec<CxlReplayRecord>,
    last_response_tag: Option<u16>,
}

impl CxlTraceRecorder {
    pub(crate) fn new(policy: CxlRecordPolicy) -> Self {
        Self {
            policy,
            records: Vec::new(),
            last_response_tag: None,
        }
    }

    pub(crate) fn record_gemm_msg(
        &mut self,
        request_index: usize,
        direction: CxlDirection,
        msg: &CxlMsg,
    ) {
        let sequence = self.records.len() as u64;
        let dependencies = match direction {
            CxlDirection::HostToDevice => self.last_response_tag.into_iter().collect(),
            CxlDirection::DeviceToHost => vec![msg.tag()],
        };
        let payload = PayloadCapture::from_payload(self.policy.mode, payload_bytes(msg));
        let (source, destination) = match direction {
            CxlDirection::HostToDevice => {
                (CxlEndpoint::Host, CxlEndpoint::Device(self.policy.endpoint))
            }
            CxlDirection::DeviceToHost => {
                (CxlEndpoint::Device(self.policy.endpoint), CxlEndpoint::Host)
            }
        };

        self.records.push(CxlReplayRecord {
            sequence,
            source,
            destination,
            epoch: gemm_epoch(request_index),
            direction,
            transaction_class: classify(msg),
            msg_class: msg.class(),
            opcode: opcode_name(msg).to_string(),
            tag: msg.tag(),
            address: address(msg),
            dependencies,
            fence: request_index == 32,
            payload,
            provenance: provenance_label(request_index),
        });

        if matches!(direction, CxlDirection::DeviceToHost) {
            self.last_response_tag = Some(msg.tag());
        }
    }

    pub(crate) fn finish(self, result: &GemmResult) -> CxlReplayArtifact {
        let final_result_commitment = result_commitment(result);
        let summary = summarize(&self.records);
        CxlReplayArtifact {
            policy: self.policy,
            records: self.records,
            final_result_commitment,
            summary,
        }
    }
}

fn summarize(records: &[CxlReplayRecord]) -> CxlReplaySummary {
    let mut summary = CxlReplaySummary {
        record_count: records.len() as u64,
        epoch_count: records
            .iter()
            .map(|record| record.epoch)
            .max()
            .map_or(0, |epoch| epoch + 1),
        application_flit_bytes: records.len() as u64 * FLIT_BYTES as u64,
        ..CxlReplaySummary::default()
    };

    for record in records {
        summary.payload_capture_bytes += record.payload.captured_bytes();
        match record.payload {
            PayloadCapture::Hash64(_) => summary.hash_payload_records += 1,
            PayloadCapture::Delta { .. } => summary.delta_payload_records += 1,
            PayloadCapture::Full(_) => summary.full_payload_records += 1,
            PayloadCapture::None => {}
        }
        summary.replay_record_bytes +=
            bincode::serialized_size(record).unwrap_or(FLIT_BYTES as u64);
    }

    summary
}

fn classify(msg: &CxlMsg) -> CxlTransactionClass {
    match msg {
        CxlMsg::M2SReq {
            opcode: M2SReqOp::MemRd | M2SReqOp::MemRdData,
            ..
        } => CxlTransactionClass::CxlMemRead,
        CxlMsg::M2SReq {
            opcode: M2SReqOp::MemInv,
            ..
        } => CxlTransactionClass::CxlCacheRequest,
        CxlMsg::M2SRwD {
            opcode: M2SRwDOp::MemWr,
            ..
        } => CxlTransactionClass::CxlMemWrite,
        CxlMsg::M2SRwD {
            opcode: M2SRwDOp::MemWrPtl,
            ..
        } => CxlTransactionClass::CxlMemWritePartial,
        CxlMsg::M2SRwD {
            opcode: M2SRwDOp::MemClnEvct,
            ..
        } => CxlTransactionClass::CxlMemEviction,
        CxlMsg::S2MDRS { .. } => CxlTransactionClass::CxlMemData,
        CxlMsg::S2MNDR { .. } => CxlTransactionClass::CxlCompletion,
        CxlMsg::D2HReq { .. } => CxlTransactionClass::CxlCacheRequest,
        CxlMsg::D2HResp { .. } => CxlTransactionClass::CxlCacheResponse,
        CxlMsg::H2DReq { .. } => CxlTransactionClass::CxlSnoopRequest,
        CxlMsg::H2DResp { .. } => CxlTransactionClass::CxlSnoopResponse,
    }
}

fn opcode_name(msg: &CxlMsg) -> &'static str {
    match msg {
        CxlMsg::M2SReq {
            opcode: M2SReqOp::MemRd,
            ..
        } => "MemRd",
        CxlMsg::M2SReq {
            opcode: M2SReqOp::MemRdData,
            ..
        } => "MemRdData",
        CxlMsg::M2SReq {
            opcode: M2SReqOp::MemInv,
            ..
        } => "MemInv",
        CxlMsg::M2SRwD {
            opcode: M2SRwDOp::MemWr,
            ..
        } => "MemWr",
        CxlMsg::M2SRwD {
            opcode: M2SRwDOp::MemWrPtl,
            ..
        } => "MemWrPtl",
        CxlMsg::M2SRwD {
            opcode: M2SRwDOp::MemClnEvct,
            ..
        } => "MemClnEvct",
        CxlMsg::S2MDRS {
            opcode: S2MDRSOp::MemData,
            ..
        } => "MemData",
        CxlMsg::S2MDRS {
            opcode: S2MDRSOp::MemDataNxm,
            ..
        } => "MemDataNxm",
        CxlMsg::S2MNDR {
            opcode: S2MNDROp::Cmp,
            ..
        } => "Cmp",
        CxlMsg::S2MNDR {
            opcode: S2MNDROp::CmpS,
            ..
        } => "CmpS",
        CxlMsg::S2MNDR {
            opcode: S2MNDROp::CmpE,
            ..
        } => "CmpE",
        CxlMsg::S2MNDR {
            opcode: S2MNDROp::CmpI,
            ..
        } => "CmpI",
        CxlMsg::S2MNDR {
            opcode: S2MNDROp::MemPassDirty,
            ..
        } => "MemPassDirty",
        CxlMsg::S2MNDR {
            opcode: S2MNDROp::DispatchFailed,
            ..
        } => "DispatchFailed",
        CxlMsg::D2HReq {
            opcode: D2HReqOp::RdShared,
            ..
        } => "RdShared",
        CxlMsg::D2HReq {
            opcode: D2HReqOp::RdOwn,
            ..
        } => "RdOwn",
        CxlMsg::D2HReq {
            opcode: D2HReqOp::Inval,
            ..
        } => "Inval",
        CxlMsg::D2HResp {
            opcode: D2HRespOp::RspIFwdM,
            ..
        } => "RspIFwdM",
        CxlMsg::D2HResp {
            opcode: D2HRespOp::RspSFwdM,
            ..
        } => "RspSFwdM",
        CxlMsg::D2HResp {
            opcode: D2HRespOp::RspV,
            ..
        } => "RspV",
        CxlMsg::H2DReq {
            opcode: H2DReqOp::SnpData,
            ..
        } => "SnpData",
        CxlMsg::H2DReq {
            opcode: H2DReqOp::SnpInv,
            ..
        } => "SnpInv",
        CxlMsg::H2DReq {
            opcode: H2DReqOp::SnpCur,
            ..
        } => "SnpCur",
        CxlMsg::H2DResp {
            opcode: H2DRespOp::GoWritePull,
            ..
        } => "GoWritePull",
        CxlMsg::H2DResp {
            opcode: H2DRespOp::GoErr,
            ..
        } => "GoErr",
        CxlMsg::H2DResp {
            opcode: H2DRespOp::Go,
            ..
        } => "Go",
    }
}

fn payload_bytes(msg: &CxlMsg) -> Option<&[u8]> {
    match msg {
        CxlMsg::M2SRwD { data, .. } | CxlMsg::S2MDRS { data, .. } => Some(data),
        CxlMsg::D2HResp { data, .. } => data.as_ref().map(|bytes| &bytes[..]),
        CxlMsg::M2SReq { .. }
        | CxlMsg::S2MNDR { .. }
        | CxlMsg::D2HReq { .. }
        | CxlMsg::H2DReq { .. }
        | CxlMsg::H2DResp { .. } => None,
    }
}

fn address(msg: &CxlMsg) -> Option<u64> {
    match msg {
        CxlMsg::M2SReq { addr, .. }
        | CxlMsg::M2SRwD { addr, .. }
        | CxlMsg::D2HReq { addr, .. }
        | CxlMsg::H2DReq { addr, .. } => Some(*addr),
        CxlMsg::S2MDRS { .. }
        | CxlMsg::S2MNDR { .. }
        | CxlMsg::D2HResp { .. }
        | CxlMsg::H2DResp { .. } => None,
    }
}

fn gemm_epoch(request_index: usize) -> u64 {
    match request_index {
        0..=15 => 0,
        16..=31 => 1,
        32 => 2,
        _ => 3,
    }
}

fn provenance_label(request_index: usize) -> Option<String> {
    let label = match request_index {
        0..=15 => "gemm.load_a",
        16..=31 => "gemm.load_b",
        32 => "gemm.compute",
        _ => "gemm.readback",
    };
    Some(label.to_string())
}

fn result_commitment(result: &GemmResult) -> u64 {
    let mut bytes = Vec::with_capacity(4 * 4 * 4 + 24);
    for row in result.c {
        for cell in row {
            bytes.extend_from_slice(&cell.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&result.cycles.to_le_bytes());
    bytes.extend_from_slice(&result.flits_sent.to_le_bytes());
    bytes.extend_from_slice(&result.flits_received.to_le_bytes());
    stable_hash64(&bytes)
}

fn stable_hash64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

#[allow(dead_code)]
fn flit_commitment(msg: &CxlMsg) -> u64 {
    stable_hash64(&encode(msg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CxlHost, GemmJob};

    fn sample_job() -> GemmJob {
        GemmJob {
            a: [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
            b: [
                [2, 3, 4, 5],
                [6, 7, 8, 9],
                [10, 11, 12, 13],
                [14, 15, 16, 17],
            ],
        }
    }

    #[test]
    fn recorded_run_has_one_record_per_boundary_flit() {
        let mut host = CxlHost::new();
        let run = host
            .run_gemm_recorded(
                &sample_job(),
                CxlRecordPolicy::gemm(CxlRecordMode::Validation),
            )
            .unwrap();

        assert_eq!(run.artifact.summary.record_count, 98);
        assert_eq!(run.artifact.summary.epoch_count, 4);
        assert_eq!(run.artifact.summary.application_flit_bytes, 98 * 64);
        assert_eq!(run.artifact.records[0].epoch, 0);
        assert_eq!(run.artifact.records[64].epoch, 2);
        assert_eq!(run.artifact.records[97].epoch, 3);
    }

    #[test]
    fn full_mode_captures_more_payload_bytes_than_validation() {
        let job = sample_job();
        let validation = CxlHost::new()
            .run_gemm_recorded(&job, CxlRecordPolicy::gemm(CxlRecordMode::Validation))
            .unwrap();
        let full = CxlHost::new()
            .run_gemm_recorded(&job, CxlRecordPolicy::gemm(CxlRecordMode::Full))
            .unwrap();

        assert!(full.artifact.summary.payload_capture_bytes > 0);
        assert!(
            full.artifact.summary.payload_capture_bytes
                > validation.artifact.summary.payload_capture_bytes
        );
    }

    #[test]
    fn equivalent_recordings_validate() {
        let job = sample_job();
        let left = CxlHost::new()
            .run_gemm_recorded(&job, CxlRecordPolicy::gemm(CxlRecordMode::Delta))
            .unwrap();
        let right = CxlHost::new()
            .run_gemm_recorded(&job, CxlRecordPolicy::gemm(CxlRecordMode::Delta))
            .unwrap();

        let validation = left.artifact.validate_equivalent(&right.artifact);
        assert!(validation.is_equivalent(), "{validation:?}");
    }
}
