//! Hardware-JIT overhead accounting for the v1 SlugCXL GEMM workload.

use crate::config::{HardwareJitConfig, HardwareJitRecordMode};
use serde::Serialize;

const GEMM_REQUEST_FLITS: u64 = 49;
const GEMM_RESPONSE_FLITS: u64 = 49;
const GEMM_PAYLOAD_FLITS: u64 = 49;
const FLIT_BYTES: u64 = 64;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HardwareJitOverhead {
    pub workload: &'static str,
    pub record_mode: HardwareJitRecordMode,
    pub request_flits: u64,
    pub response_flits: u64,
    pub record_count: u64,
    pub app_flit_bytes: u64,
    pub metadata_record_bytes: u64,
    pub payload_capture_bytes: u64,
    pub metadata_bytes: u64,
    pub metadata_bytes_per_app_gib: f64,
    pub inserted_data_path_cycles: u64,
    pub estimated_runtime_overhead_percent: f64,
}

pub fn estimate_gemm_4x4(hj: &HardwareJitConfig) -> HardwareJitOverhead {
    let all_flits = GEMM_REQUEST_FLITS + GEMM_RESPONSE_FLITS;
    let record_count = all_flits.div_ceil(hj.sample_stride as u64);
    let sampled_payload_flits = GEMM_PAYLOAD_FLITS.div_ceil(hj.sample_stride as u64);
    let payload_capture_bytes = sampled_payload_flits * payload_bytes_per_payload_flit(hj);
    let metadata_record_bytes = record_count * hj.metadata_record_bytes as u64;
    let metadata_bytes = metadata_record_bytes + payload_capture_bytes;
    let app_flit_bytes = all_flits * FLIT_BYTES;

    HardwareJitOverhead {
        workload: "slugcxl_gemm_4x4",
        record_mode: hj.record_mode,
        request_flits: GEMM_REQUEST_FLITS,
        response_flits: GEMM_RESPONSE_FLITS,
        record_count,
        app_flit_bytes,
        metadata_record_bytes,
        payload_capture_bytes,
        metadata_bytes,
        metadata_bytes_per_app_gib: metadata_bytes as f64 * 1024.0 * 1024.0 * 1024.0
            / app_flit_bytes as f64,
        inserted_data_path_cycles: 0,
        estimated_runtime_overhead_percent: 0.0,
    }
}

pub fn emit_report_json(hj: &HardwareJitConfig) -> String {
    serde_json::to_string_pretty(&estimate_gemm_4x4(hj)).expect("serialize HJ overhead report")
}

fn payload_bytes_per_payload_flit(hj: &HardwareJitConfig) -> u64 {
    match hj.record_mode {
        HardwareJitRecordMode::Validation => 8,
        // The software estimator is conservative for delta mode because
        // changed-byte count is workload-data dependent. The RTL computes
        // the exact nonzero-byte count per payload FLIT.
        HardwareJitRecordMode::Delta => 8 + 32 * 2,
        HardwareJitRecordMode::Full => 32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HardwareJitConfig, HardwareJitRecordMode};

    #[test]
    fn validation_mode_has_zero_inserted_cycles() {
        let hj = HardwareJitConfig::validation_gemm();
        let overhead = estimate_gemm_4x4(&hj);
        assert_eq!(overhead.record_count, 98);
        assert_eq!(overhead.app_flit_bytes, 98 * 64);
        assert_eq!(overhead.payload_capture_bytes, 49 * 8);
        assert_eq!(overhead.inserted_data_path_cycles, 0);
        assert_eq!(overhead.estimated_runtime_overhead_percent, 0.0);
    }

    #[test]
    fn full_mode_captures_more_payload_than_validation() {
        let validation = estimate_gemm_4x4(&HardwareJitConfig::validation_gemm());
        let mut full_cfg = HardwareJitConfig::validation_gemm();
        full_cfg.record_mode = HardwareJitRecordMode::Full;
        let full = estimate_gemm_4x4(&full_cfg);

        assert!(full.payload_capture_bytes > validation.payload_capture_bytes);
        assert!(full.metadata_bytes > validation.metadata_bytes);
    }
}
