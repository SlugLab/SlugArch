use crate::{CxlHost, CxlRecordMode, CxlRecordPolicy, GemmJob, HostError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayMetadataReport {
    pub workload: String,
    pub repeats: usize,
    pub modes: Vec<ReplayModeMeasurement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PayloadRecordCounts {
    pub hash: u64,
    pub delta: u64,
    pub full: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayModeMeasurement {
    pub mode: String,
    pub record_count: u64,
    pub epoch_count: u64,
    pub application_flit_bytes: u64,
    pub replay_record_bytes: u64,
    pub payload_capture_bytes: u64,
    pub metadata_bytes_per_app_gib: f64,
    pub payload_record_counts: PayloadRecordCounts,
    pub payload_compression_ratio_vs_full: f64,
    pub equivalent_validation_ns: u128,
    pub mismatch_validation_ns: u128,
    pub equivalent: bool,
    pub mismatch_detected: bool,
    pub provenance_labels: BTreeMap<String, u64>,
    pub uncovered_records: u64,
}

pub fn measure_replay_metadata(
    job: &GemmJob,
    repeats: usize,
) -> Result<ReplayMetadataReport, HostError> {
    let mode_order = [
        ("validation", CxlRecordMode::Validation),
        ("delta", CxlRecordMode::Delta),
        ("full", CxlRecordMode::Full),
    ];
    let mut raw = Vec::new();
    for (name, mode) in mode_order {
        raw.push(measure_mode(job, repeats.max(1), name, mode)?);
    }
    let full_payload = raw
        .iter()
        .find(|m| m.mode == "full")
        .map(|m| m.payload_capture_bytes)
        .unwrap_or(0);
    for measurement in &mut raw {
        measurement.payload_compression_ratio_vs_full = if measurement.payload_capture_bytes == 0 {
            0.0
        } else {
            full_payload as f64 / measurement.payload_capture_bytes as f64
        };
    }
    Ok(ReplayMetadataReport {
        workload: "slugcxl_gemm_4x4".to_string(),
        repeats: repeats.max(1),
        modes: raw,
    })
}

fn measure_mode(
    job: &GemmJob,
    repeats: usize,
    mode_name: &str,
    mode: CxlRecordMode,
) -> Result<ReplayModeMeasurement, HostError> {
    let mut equivalent_validation_ns = 0u128;
    let mut mismatch_validation_ns = 0u128;
    let mut equivalent = true;
    let mut mismatch_detected = true;
    let mut first_summary = None;
    let mut first_records = None;

    for _ in 0..repeats {
        let policy = CxlRecordPolicy::gemm(mode);
        let left = CxlHost::new().run_gemm_recorded(job, policy.clone())?;
        let right = CxlHost::new().run_gemm_recorded(job, policy)?;

        let start = Instant::now();
        let validation = left.artifact.validate_equivalent(&right.artifact);
        equivalent_validation_ns += start.elapsed().as_nanos();
        equivalent &= validation.is_equivalent();

        let mut bad = right.artifact.clone();
        if let Some(record) = bad.records.first_mut() {
            record.tag = record.tag.wrapping_add(1);
        }
        let start = Instant::now();
        let bad_validation = left.artifact.validate_equivalent(&bad);
        mismatch_validation_ns += start.elapsed().as_nanos();
        mismatch_detected &= !bad_validation.is_equivalent();

        if first_summary.is_none() {
            first_summary = Some(left.artifact.summary.clone());
            first_records = Some(left.artifact.records.clone());
        }
    }

    let summary = first_summary.expect("at least one repeat");
    let records = first_records.expect("at least one repeat");
    let mut provenance_labels = BTreeMap::new();
    let mut uncovered_records = 0u64;
    for record in &records {
        if let Some(label) = &record.provenance {
            *provenance_labels.entry(label.clone()).or_insert(0) += 1;
        } else {
            uncovered_records += 1;
        }
    }

    Ok(ReplayModeMeasurement {
        mode: mode_name.to_string(),
        record_count: summary.record_count,
        epoch_count: summary.epoch_count,
        application_flit_bytes: summary.application_flit_bytes,
        replay_record_bytes: summary.replay_record_bytes,
        payload_capture_bytes: summary.payload_capture_bytes,
        metadata_bytes_per_app_gib: summary.replay_bytes_per_app_gib(),
        payload_record_counts: PayloadRecordCounts {
            hash: summary.hash_payload_records,
            delta: summary.delta_payload_records,
            full: summary.full_payload_records,
        },
        payload_compression_ratio_vs_full: 0.0,
        equivalent_validation_ns: equivalent_validation_ns / repeats as u128,
        mismatch_validation_ns: mismatch_validation_ns / repeats as u128,
        equivalent,
        mismatch_detected,
        provenance_labels,
        uncovered_records,
    })
}
