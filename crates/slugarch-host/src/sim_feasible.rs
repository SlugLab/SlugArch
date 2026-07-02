use crate::{CxlHost, CxlRecordMode, CxlRecordPolicy, GemmJob, HostError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    Measured,
    PartiallyMeasured,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimLedgerEntry {
    pub claim: String,
    pub status: ClaimStatus,
    pub evidence_artifact: String,
    pub measured_substrate: String,
    pub paper_safe_wording: String,
    pub limitation: String,
    pub checked: Vec<String>,
    pub missing_substrate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaxProbeStatus {
    Measured,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaxProbe {
    pub status: DaxProbeStatus,
    pub dev_root: String,
    pub devices: Vec<String>,
    pub checked: Vec<String>,
    pub limitation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bar2EvidenceSummary {
    pub status: ClaimStatus,
    pub runs: usize,
    pub pass_runs: usize,
    pub request_count: u64,
    pub response_count: u64,
    pub tag_mismatches: u64,
    pub dispatch_failures: u64,
    pub guest_elapsed_ms: Vec<u64>,
    pub source_dir: Option<String>,
    pub limitation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimFeasibleReport {
    pub generated_utc: String,
    pub workload: String,
    pub replay_metadata: ReplayMetadataReport,
    pub dax_probe: DaxProbe,
    pub bar2_evidence: Bar2EvidenceSummary,
    pub claims: Vec<ClaimLedgerEntry>,
}

pub struct SimFeasibleInput<'a> {
    pub job: &'a GemmJob,
    pub replay_repeats: usize,
    pub qemu_repeatability_dir: Option<&'a Path>,
    pub dev_root: &'a Path,
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

pub fn probe_dax_devices(dev_root: &Path) -> Result<DaxProbe, HostError> {
    let mut devices = Vec::new();
    let checked = vec![format!("{}/dax*", dev_root.display())];
    let entries = fs::read_dir(dev_root).map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("read {}: {e}", dev_root.display()),
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| HostError::DispatchFailed {
            tag: 0,
            reason: format!("read {} entry: {e}", dev_root.display()),
        })?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("dax") {
            devices.push(format!("{}/{}", dev_root.display(), name));
        }
    }
    devices.sort();
    let status = if devices.is_empty() {
        DaxProbeStatus::Blocked
    } else {
        DaxProbeStatus::Measured
    };
    let limitation = if devices.is_empty() {
        format!(
            "No /dev/dax* device was visible under {}",
            dev_root.display()
        )
    } else {
        format!(
            "DAX devices were visible, but this probe does not run a streaming workload by itself: {}",
            devices.join(", ")
        )
    };
    Ok(DaxProbe {
        status,
        dev_root: dev_root.display().to_string(),
        devices,
        checked,
        limitation,
    })
}

pub fn summarize_bar2_repeatability(dir: Option<&Path>) -> Result<Bar2EvidenceSummary, HostError> {
    let Some(dir) = dir else {
        return Ok(Bar2EvidenceSummary {
            status: ClaimStatus::Blocked,
            runs: 0,
            pass_runs: 0,
            request_count: 0,
            response_count: 0,
            tag_mismatches: 0,
            dispatch_failures: 0,
            guest_elapsed_ms: Vec::new(),
            source_dir: None,
            limitation: "No QEMU Type-2 repeatability artifact directory was supplied".to_string(),
        });
    };
    let mut runs = 0usize;
    let mut pass_runs = 0usize;
    let mut request_count = 0u64;
    let mut response_count = 0u64;
    let mut tag_mismatches = 0u64;
    let mut dispatch_failures = 0u64;
    let mut guest_elapsed_ms = Vec::new();
    for idx in 1..=5 {
        let run_dir = dir.join(format!("run-{idx}"));
        let summary_path = run_dir.join("summary.json");
        if !summary_path.exists() {
            continue;
        }
        runs += 1;
        let summary: serde_json::Value =
            serde_json::from_slice(&fs::read(&summary_path).map_err(|e| {
                HostError::DispatchFailed {
                    tag: 0,
                    reason: format!("read {}: {e}", summary_path.display()),
                }
            })?)
            .map_err(|e| HostError::DispatchFailed {
                tag: 0,
                reason: format!("parse {}: {e}", summary_path.display()),
            })?;
        if summary.get("status").and_then(|v| v.as_str()) == Some("pass") {
            pass_runs += 1;
        }
        request_count += summary
            .get("request_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        response_count += summary
            .get("response_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        tag_mismatches += summary
            .get("tag_mismatches")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        dispatch_failures += summary
            .get("dispatch_failures")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let guest_path = run_dir.join("guest-summary.json");
        if guest_path.exists() {
            let guest: serde_json::Value =
                serde_json::from_slice(&fs::read(&guest_path).map_err(|e| {
                    HostError::DispatchFailed {
                        tag: 0,
                        reason: format!("read {}: {e}", guest_path.display()),
                    }
                })?)
                .map_err(|e| HostError::DispatchFailed {
                    tag: 0,
                    reason: format!("parse {}: {e}", guest_path.display()),
                })?;
            if let Some(ms) = guest.get("elapsed_ms").and_then(|v| v.as_u64()) {
                guest_elapsed_ms.push(ms);
            }
        }
    }
    Ok(Bar2EvidenceSummary {
        status: if runs > 0 && runs == pass_runs {
            ClaimStatus::Measured
        } else {
            ClaimStatus::Blocked
        },
        runs,
        pass_runs,
        request_count,
        response_count,
        tag_mismatches,
        dispatch_failures,
        guest_elapsed_ms,
        source_dir: Some(dir.display().to_string()),
        limitation: "This is Type-2 BAR2 command-path evidence, not CXL link latency or hardware endpoint latency".to_string(),
    })
}

pub fn build_sim_feasible_report(
    input: SimFeasibleInput<'_>,
) -> Result<SimFeasibleReport, HostError> {
    let replay_metadata = measure_replay_metadata(input.job, input.replay_repeats)?;
    let dax_probe = probe_dax_devices(input.dev_root)?;
    let bar2_evidence = summarize_bar2_repeatability(input.qemu_repeatability_dir)?;
    let generated_utc = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("unix_epoch_seconds:{}", duration.as_secs()))
        .unwrap_or_else(|_| "unix_epoch_seconds:0".to_string());
    let mut report = SimFeasibleReport {
        generated_utc,
        workload: "slugcxl_gemm_4x4".to_string(),
        replay_metadata,
        dax_probe,
        bar2_evidence,
        claims: Vec::new(),
    };
    report.claims = claim_ledger(&report);
    Ok(report)
}

fn claim_ledger(report: &SimFeasibleReport) -> Vec<ClaimLedgerEntry> {
    let bar2_status = report.bar2_evidence.status.clone();
    let dax_status = match report.dax_probe.status {
        DaxProbeStatus::Measured => ClaimStatus::PartiallyMeasured,
        DaxProbeStatus::Blocked => ClaimStatus::Blocked,
    };
    vec![
        claim(
            "QEMU Type-2 BAR2 command/replay boundary",
            bar2_status,
            report
                .bar2_evidence
                .source_dir
                .clone()
                .unwrap_or_else(|| "none".to_string()),
            "CXLMemSim QEMU Type-2 BAR2 carried the SlugArch command stream when the supplied artifact has all runs passing.",
            &report.bar2_evidence.limitation,
            vec!["artifact/slugarch_cxlmemsim/qemu-type2-repeatability-20260702-0627".to_string()],
            None,
        ),
        claim(
            "CXL.mem/DAX simulator traffic",
            dax_status,
            report.dax_probe.dev_root.clone(),
            "CXL.mem/DAX is reported only as a device-reachability result unless a streaming workload is present.",
            &report.dax_probe.limitation,
            report.dax_probe.checked.clone(),
            if report.dax_probe.devices.is_empty() {
                Some("/dev/dax* device or guest DAX path".to_string())
            } else {
                None
            },
        ),
        claim(
            "CXL.cache coherence",
            ClaimStatus::Blocked,
            "none".to_string(),
            "CXL.cache coherence is not measured by BAR2 command traffic or software replay metadata.",
            "No live CXLMemSim GPU coherency statistic or CXL.cache transaction artifact is produced by this pass.",
            vec!["/home/victoryang00/CXLMemSim/qemu_integration/guest_libcuda/cpu_gpu_hitm_benchmark.c".to_string()],
            Some("live CXL.cache or coherency-stat workload".to_string()),
        ),
        claim(
            "DMA",
            ClaimStatus::Blocked,
            "none".to_string(),
            "DMA replay remains a benchmark slot.",
            "No real DMA path is exercised or logged by this pass.",
            vec!["QEMU Type-2 BAR2 SlugArch command path".to_string()],
            Some("live DMA transaction source".to_string()),
        ),
        claim(
            "ATS",
            ClaimStatus::Blocked,
            "none".to_string(),
            "ATS replay remains a benchmark slot.",
            "No simulator or kernel ATS event path is exposed by this pass.",
            vec!["CXLMemSim Type-2 and Type-3 helper inventory".to_string()],
            Some("ATS event source and translation log".to_string()),
        ),
        claim(
            "Page migration",
            ClaimStatus::Blocked,
            "none".to_string(),
            "Page migration replay remains a benchmark slot.",
            "No migration event source is exercised or logged by this pass.",
            vec!["/home/victoryang00/CXLMemSim/include/cxlcounter.h".to_string()],
            Some("live migrate_in/migrate_out counter source".to_string()),
        ),
        claim(
            "Switch ordering",
            ClaimStatus::Blocked,
            "none".to_string(),
            "Switch ordering replay remains a benchmark slot.",
            "No two-host or switch-lock workload is run by this pass.",
            vec!["/home/victoryang00/CXLMemSim/microbench/cxl_switch_lock_bench.c".to_string()],
            Some("two-host switch-ordering workload".to_string()),
        ),
        claim(
            "Runtime overhead",
            ClaimStatus::PartiallyMeasured,
            "artifact/slugarch_cxlmemsim/qemu-type2-repeatability-20260702-0627".to_string(),
            "Runtime overhead is partially measured for the QEMU Type-2 BAR2 command path only.",
            "This is not a CXL link, hardware endpoint, or production continuous-overhead measurement.",
            vec!["guest-summary.json elapsed_ms".to_string()],
            Some("hardware endpoint timing and continuous workload timing".to_string()),
        ),
        claim(
            "Compression",
            ClaimStatus::PartiallyMeasured,
            "software replay metadata report".to_string(),
            "Compression is measured for software replay artifact modes only.",
            "Validation, delta, and full payload accounting do not prove a hardware compression engine.",
            vec!["crates/slugarch-host/src/replay.rs".to_string()],
            Some("hardware compression datapath".to_string()),
        ),
        claim(
            "Replay latency",
            ClaimStatus::PartiallyMeasured,
            "software replay metadata report".to_string(),
            "Replay latency is measured as software replay validation only.",
            "The pass times validation of replay artifacts, not hardware replay execution.",
            vec!["CxlReplayArtifact::validate_equivalent".to_string()],
            Some("hardware replay executor".to_string()),
        ),
        claim(
            "Provenance",
            ClaimStatus::PartiallyMeasured,
            "software replay metadata report".to_string(),
            "Provenance is measured for software labels on the GEMM trace only.",
            "The pass does not prove fabric-wide endpoint or protection-domain provenance.",
            vec!["gemm.load_a, gemm.load_b, gemm.compute, gemm.readback".to_string()],
            Some("fabric-wide provenance labels".to_string()),
        ),
        claim(
            "FPGA resource cost",
            ClaimStatus::Blocked,
            "targets/agilex-vr2/generated/slugcxl_hj_overhead.json".to_string(),
            "FPGA resource cost is blocked for post-fit resources; model-side metadata estimates may be cited separately.",
            "No Quartus synthesis, fit, timing, or resource report is produced by this simulator pass.",
            vec!["targets/agilex-vr2/generated/slugcxl_hj_overhead.json".to_string()],
            Some("post-fit Quartus resource report".to_string()),
        ),
    ]
}

fn claim(
    claim: &str,
    status: ClaimStatus,
    evidence_artifact: String,
    paper_safe_wording: &str,
    limitation: &str,
    checked: Vec<String>,
    missing_substrate: Option<String>,
) -> ClaimLedgerEntry {
    ClaimLedgerEntry {
        claim: claim.to_string(),
        status,
        evidence_artifact,
        measured_substrate: "SlugArch/CXLMemSim simulator-feasible pass".to_string(),
        paper_safe_wording: paper_safe_wording.to_string(),
        limitation: limitation.to_string(),
        checked,
        missing_substrate,
    }
}

pub fn write_sim_feasible_report(
    report: &SimFeasibleReport,
    out_dir: &Path,
) -> Result<(), HostError> {
    fs::create_dir_all(out_dir).map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("create {}: {e}", out_dir.display()),
    })?;
    fs::write(
        out_dir.join("sim-feasible-bench-20260702.json"),
        serde_json::to_vec_pretty(report).map_err(|e| HostError::DispatchFailed {
            tag: 0,
            reason: format!("serialize sim feasible report: {e}"),
        })?,
    )
    .map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("write sim-feasible-bench-20260702.json: {e}"),
    })?;
    fs::write(
        out_dir.join("sim-feasible-bench-20260702.md"),
        report_markdown(report),
    )
    .map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("write sim-feasible-bench-20260702.md: {e}"),
    })?;
    Ok(())
}

fn report_markdown(report: &SimFeasibleReport) -> String {
    let mut out = String::new();
    out.push_str("# SlugArch Simulator-Feasible Benchmark Pass\n\n");
    out.push_str(&format!("- Workload: `{}`\n", report.workload));
    out.push_str(&format!(
        "- Replay modes measured: `{}`\n",
        report.replay_metadata.modes.len()
    ));
    out.push_str(&format!(
        "- BAR2 pass runs: `{}/{}`\n",
        report.bar2_evidence.pass_runs, report.bar2_evidence.runs
    ));
    out.push_str(&format!(
        "- DAX probe: `{:?}`; {}\n\n",
        report.dax_probe.status, report.dax_probe.limitation
    ));
    out.push_str("## Replay Metadata\n\n");
    out.push_str(
        "| Mode | Records | Payload bytes | Compression vs full | Equivalent ns | Mismatch ns |\n",
    );
    out.push_str("| --- | ---: | ---: | ---: | ---: | ---: |\n");
    for mode in &report.replay_metadata.modes {
        out.push_str(&format!(
            "| {} | {} | {} | {:.2} | {} | {} |\n",
            mode.mode,
            mode.record_count,
            mode.payload_capture_bytes,
            mode.payload_compression_ratio_vs_full,
            mode.equivalent_validation_ns,
            mode.mismatch_validation_ns
        ));
    }
    out.push_str("\n## Claim Ledger\n\n");
    out.push_str("| Claim | Status | Paper-safe wording | Limitation |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for claim in &report.claims {
        out.push_str(&format!(
            "| {} | {:?} | {} | {} |\n",
            claim.claim, claim.status, claim.paper_safe_wording, claim.limitation
        ));
    }
    out
}
