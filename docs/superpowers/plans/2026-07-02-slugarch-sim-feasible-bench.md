# SlugArch Simulator-Feasible Benchmark Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a truthful simulator-feasible benchmark pass that measures SlugArch replay metadata, BAR2 overhead evidence, CXL.mem/DAX reachability, and blocked-claim boundaries.

**Architecture:** Keep live probing and artifact summarization in SlugArch rather than changing CXLMemSim. Add a small host-side measurement module that reuses the existing GEMM replay recorder, reads the committed QEMU Type-2 evidence, probes the current machine for DAX/CXL memory reachability, and emits a compact JSON/Markdown claim ledger. Update the paper only from generated artifacts.

**Tech Stack:** Rust workspace crates `slugarch-host` and `slugarch-cli`, existing `serde`/`serde_json`/`bincode`, shell commands for live probes, existing CXLMemSim artifacts, LaTeX paper repo at `/root/Concordia/64fa450c44d0cdf46c7c3a7d`.

## Global Constraints

- The pass must not imply that BAR2 command traffic proves CXL.cache coherence, CXL.mem pooling, DMA, ATS, page migration, switch ordering, or FPGA feasibility.
- Measured paper wording must stay tied to the path that actually produced the data.
- A blocked artifact is successful evidence if it proves that the current substrate cannot exercise a claim.
- If a process is started by the runner, it must record the PID, log path, and cleanup status.
- If the runner attaches to an existing guest, it must record that fact and must not claim ownership of the VM lifecycle.
- Raw logs and binary artifacts stay under `artifact/`; compact JSON/Markdown summaries go under `docs/evaluation/`.
- The final verification commands are `cargo fmt --check --package slugarch-host --package slugarch-cli --package slugcxl-gen`, `env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include cargo test -p slugarch-host --test qemu_type2_artifacts`, `cargo test -p slugcxl-gen`, and `jq empty docs/evaluation/sim-feasible-bench-20260702.json`.

---

## File Structure

- Create `crates/slugarch-host/src/sim_feasible.rs`: replay metadata measurement, DAX/CXL probe data structures, QEMU Type-2 repeatability summarization, claim ledger assembly, JSON/Markdown report writing.
- Modify `crates/slugarch-host/src/lib.rs`: export the new `sim_feasible` module and public report types.
- Modify `crates/slugarch-cli/src/main.rs`: add `measure-sim-feasible` CLI command that calls the host module.
- Create `crates/slugarch-host/tests/sim_feasible.rs`: focused tests for replay metadata, DAX probe classification, QEMU repeatability summarization, and claim ledger boundaries.
- Keep raw/generated run logs under `artifact/slugarch_cxlmemsim/sim-feasible-20260702-<time>/` if a live runner produces them.
- Create the compact claim-ledger summaries only as `docs/evaluation/sim-feasible-bench-20260702.json` and `docs/evaluation/sim-feasible-bench-20260702.md`.
- Modify paper file `/root/Concordia/64fa450c44d0cdf46c7c3a7d/eval.tex` only after the artifact summary exists.

## Task 1: Replay Metadata Measurement API

**Files:**
- Create: `crates/slugarch-host/src/sim_feasible.rs`
- Modify: `crates/slugarch-host/src/lib.rs`
- Test: `crates/slugarch-host/tests/sim_feasible.rs`

**Interfaces:**
- Consumes: `CxlHost::run_gemm_recorded(&GemmJob, CxlRecordPolicy) -> Result<CxlRecordedRun, HostError>`
- Produces: `pub fn measure_replay_metadata(job: &GemmJob, repeats: usize) -> Result<ReplayMetadataReport, HostError>`
- Produces: `pub struct ReplayMetadataReport { pub workload: String, pub repeats: usize, pub modes: Vec<ReplayModeMeasurement> }`
- Produces: `pub struct ReplayModeMeasurement` with serialized fields `mode`, `record_count`, `epoch_count`, `application_flit_bytes`, `replay_record_bytes`, `payload_capture_bytes`, `metadata_bytes_per_app_gib`, `payload_record_counts`, `payload_compression_ratio_vs_full`, `equivalent_validation_ns`, `mismatch_validation_ns`, `equivalent`, `mismatch_detected`, `provenance_labels`, and `uncovered_records`.

- [ ] **Step 1: Write failing replay metadata tests**

Append this test content to the new file `crates/slugarch-host/tests/sim_feasible.rs`:

```rust
use slugarch_host::sim_feasible::measure_replay_metadata;
use slugarch_host::GemmJob;

fn job() -> GemmJob {
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
fn replay_metadata_reports_all_modes_and_boundaries() {
    let report = measure_replay_metadata(&job(), 1).unwrap();

    assert_eq!(report.workload, "slugcxl_gemm_4x4");
    assert_eq!(report.repeats, 1);
    assert_eq!(report.modes.len(), 3);

    let validation = report.modes.iter().find(|m| m.mode == "validation").unwrap();
    let delta = report.modes.iter().find(|m| m.mode == "delta").unwrap();
    let full = report.modes.iter().find(|m| m.mode == "full").unwrap();

    assert_eq!(validation.record_count, 98);
    assert_eq!(validation.epoch_count, 4);
    assert_eq!(validation.application_flit_bytes, 98 * 64);
    assert!(validation.equivalent);
    assert!(validation.mismatch_detected);
    assert!(validation.provenance_labels.contains_key("gemm.load_a"));
    assert_eq!(validation.uncovered_records, 0);
    assert!(full.payload_capture_bytes >= validation.payload_capture_bytes);
    assert!(delta.payload_capture_bytes >= validation.payload_capture_bytes);
    assert!(validation.payload_compression_ratio_vs_full >= 1.0);
}
```

- [ ] **Step 2: Run the new test and verify it fails**

Run:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test sim_feasible replay_metadata_reports_all_modes_and_boundaries
```

Expected: FAIL with an unresolved import for `slugarch_host::sim_feasible`.

- [ ] **Step 3: Add the replay metadata implementation**

Create `crates/slugarch-host/src/sim_feasible.rs` with these public types and functions:

```rust
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
        measurement.payload_compression_ratio_vs_full =
            if measurement.payload_capture_bytes == 0 {
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
```

Modify `crates/slugarch-host/src/lib.rs`:

```rust
pub mod sim_feasible;
```

and extend the public exports:

```rust
pub use sim_feasible::{
    measure_replay_metadata, PayloadRecordCounts, ReplayMetadataReport, ReplayModeMeasurement,
};
```

- [ ] **Step 4: Run the replay metadata test and verify it passes**

Run:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test sim_feasible replay_metadata_reports_all_modes_and_boundaries
```

Expected: PASS with `1 passed`.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add crates/slugarch-host/src/lib.rs crates/slugarch-host/src/sim_feasible.rs crates/slugarch-host/tests/sim_feasible.rs
git commit -m "feat: measure replay metadata modes"
```

## Task 2: Simulator-Feasible Report and Claim Ledger CLI

**Files:**
- Modify: `crates/slugarch-host/src/sim_feasible.rs`
- Modify: `crates/slugarch-cli/src/main.rs`
- Test: `crates/slugarch-host/tests/sim_feasible.rs`

**Interfaces:**
- Consumes: `measure_replay_metadata(job, repeats)`
- Produces: `pub fn build_sim_feasible_report(input: SimFeasibleInput<'_>) -> Result<SimFeasibleReport, HostError>`
- Produces: `pub fn write_sim_feasible_report(report: &SimFeasibleReport, out_dir: &Path) -> Result<(), HostError>`
- Produces CLI: `slugarch measure-sim-feasible <job> --out <dir> [--qemu-repeatability-dir <dir>] [--dev-root <dir>] [--replay-repeats <n>]`

- [ ] **Step 1: Add failing report tests**

Append these tests to `crates/slugarch-host/tests/sim_feasible.rs`:

```rust
use slugarch_host::sim_feasible::{
    build_sim_feasible_report, probe_dax_devices, write_sim_feasible_report, DaxProbeStatus,
    SimFeasibleInput,
};
use std::fs;

#[test]
fn dax_probe_reports_blocked_when_no_dax_devices_exist() {
    let root = std::env::temp_dir().join(format!("slugarch-empty-dev-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let probe = probe_dax_devices(&root).unwrap();
    assert_eq!(probe.status, DaxProbeStatus::Blocked);
    assert!(probe.devices.is_empty());
    assert!(probe.limitation.contains("/dev/dax"));
}

#[test]
fn sim_feasible_report_writes_json_and_markdown_with_blocked_claims() {
    let out = std::env::temp_dir().join(format!("slugarch-sim-report-{}", std::process::id()));
    let dev = out.join("dev");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&dev).unwrap();

    let report = build_sim_feasible_report(SimFeasibleInput {
        job: &job(),
        replay_repeats: 1,
        qemu_repeatability_dir: None,
        dev_root: &dev,
    })
    .unwrap();
    write_sim_feasible_report(&report, &out).unwrap();

    let json = fs::read_to_string(out.join("sim-feasible-bench-20260702.json")).unwrap();
    let md = fs::read_to_string(out.join("sim-feasible-bench-20260702.md")).unwrap();
    assert!(json.contains("\"claim\": \"CXL.cache coherence\""));
    assert!(json.contains("\"status\": \"blocked\""));
    assert!(md.contains("CXL.cache coherence"));
    assert!(md.contains("software replay validation only"));
}
```

- [ ] **Step 2: Run the report tests and verify they fail**

Run:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test sim_feasible
```

Expected: FAIL with unresolved imports for the report types and functions.

- [ ] **Step 3: Add report, DAX probe, and claim ledger types**

Extend `crates/slugarch-host/src/sim_feasible.rs` with these additional public types:

```rust
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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
```

Add these functions:

```rust
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
        format!("No /dev/dax* device was visible under {}", dev_root.display())
    } else {
        format!("DAX devices were visible, but this probe does not run a streaming workload by itself: {}", devices.join(", "))
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
            serde_json::from_slice(&fs::read(&summary_path).map_err(|e| HostError::DispatchFailed {
                tag: 0,
                reason: format!("read {}: {e}", summary_path.display()),
            })?)
            .map_err(|e| HostError::DispatchFailed {
                tag: 0,
                reason: format!("parse {}: {e}", summary_path.display()),
            })?;
        if summary.get("status").and_then(|v| v.as_str()) == Some("pass") {
            pass_runs += 1;
        }
        request_count += summary.get("request_count").and_then(|v| v.as_u64()).unwrap_or(0);
        response_count += summary.get("response_count").and_then(|v| v.as_u64()).unwrap_or(0);
        tag_mismatches += summary.get("tag_mismatches").and_then(|v| v.as_u64()).unwrap_or(0);
        dispatch_failures += summary.get("dispatch_failures").and_then(|v| v.as_u64()).unwrap_or(0);
        let guest_path = run_dir.join("guest-summary.json");
        if guest_path.exists() {
            let guest: serde_json::Value =
                serde_json::from_slice(&fs::read(&guest_path).map_err(|e| HostError::DispatchFailed {
                    tag: 0,
                    reason: format!("read {}: {e}", guest_path.display()),
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
```

Add `build_sim_feasible_report`, `claim_ledger`, `write_sim_feasible_report`, and `report_markdown` with this implementation:

```rust
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
    out.push_str("| Mode | Records | Payload bytes | Compression vs full | Equivalent ns | Mismatch ns |\n");
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
```

- [ ] **Step 4: Add the CLI command**

Modify `crates/slugarch-cli/src/main.rs`:

```rust
/// Build the simulator-feasible benchmark report and claim ledger.
MeasureSimFeasible {
    /// Path to a GemmJob JSON file.
    job: PathBuf,
    /// Output directory for sim-feasible-bench-20260702.json and .md.
    #[arg(long)]
    out: PathBuf,
    /// Existing qemu-type2 repeatability artifact directory.
    #[arg(long)]
    qemu_repeatability_dir: Option<PathBuf>,
    /// Device root to probe for dax* devices.
    #[arg(long, default_value = "/dev")]
    dev_root: PathBuf,
    /// Number of replay validation repetitions per mode.
    #[arg(long, default_value_t = 5)]
    replay_repeats: usize,
}
```

Add the match arm:

```rust
Cmd::MeasureSimFeasible {
    job,
    out,
    qemu_repeatability_dir,
    dev_root,
    replay_repeats,
} => measure_sim_feasible(&job, &out, qemu_repeatability_dir.as_deref(), &dev_root, replay_repeats),
```

Add the helper:

```rust
fn measure_sim_feasible(
    job_path: &std::path::Path,
    out: &std::path::Path,
    qemu_repeatability_dir: Option<&std::path::Path>,
    dev_root: &std::path::Path,
    replay_repeats: usize,
) -> Result<()> {
    let job = read_gemm_job(job_path)?;
    let report = slugarch_host::sim_feasible::build_sim_feasible_report(
        slugarch_host::sim_feasible::SimFeasibleInput {
            job: &job,
            replay_repeats,
            qemu_repeatability_dir,
            dev_root,
        },
    )
    .map_err(|e| anyhow!("measure sim feasible: {}", e))?;
    slugarch_host::sim_feasible::write_sim_feasible_report(&report, out)
        .map_err(|e| anyhow!("write sim feasible report: {}", e))?;
    println!("workload: {}", report.workload);
    println!("claims: {}", report.claims.len());
    println!("out: {}", out.display());
    Ok(())
}
```

- [ ] **Step 5: Run the report tests and CLI smoke**

Run:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test sim_feasible
cargo run -p slugarch-cli -- measure-sim-feasible \
  targets/qemu-type2/identity_times_const.json \
  --out /tmp/slugarch-sim-feasible-smoke \
  --qemu-repeatability-dir artifact/slugarch_cxlmemsim/qemu-type2-repeatability-20260702-0627 \
  --dev-root /dev \
  --replay-repeats 1
jq empty /tmp/slugarch-sim-feasible-smoke/sim-feasible-bench-20260702.json
```

Expected: tests pass, CLI prints `claims: 12`, and `jq empty` exits 0.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add crates/slugarch-host/src/sim_feasible.rs crates/slugarch-host/tests/sim_feasible.rs crates/slugarch-cli/src/main.rs
git commit -m "feat: emit simulator feasible benchmark report"
```

## Task 3: Generate Benchmark Artifacts and Evaluation Docs

**Files:**
- Create: `docs/evaluation/sim-feasible-bench-20260702.json`
- Create: `docs/evaluation/sim-feasible-bench-20260702.md`

**Interfaces:**
- Consumes: `slugarch measure-sim-feasible <job> --out <dir>`
- Produces: compact report files in `docs/evaluation/`

- [ ] **Step 1: Run the benchmark report generator**

Run:

```bash
SUMMARY_DIR="docs/evaluation"
mkdir -p "$SUMMARY_DIR"
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo run -p slugarch-cli -- measure-sim-feasible \
  targets/qemu-type2/identity_times_const.json \
  --out "$SUMMARY_DIR" \
  --qemu-repeatability-dir artifact/slugarch_cxlmemsim/qemu-type2-repeatability-20260702-0627 \
  --dev-root /dev \
  --replay-repeats 5
```

Expected: command exits 0 and prints `claims: 12`.

- [ ] **Step 2: Validate compact summaries in docs/evaluation**

Run:

```bash
jq empty docs/evaluation/sim-feasible-bench-20260702.json
```

Expected: `jq empty` exits 0.

- [ ] **Step 3: Verify claim boundaries in generated docs**

Run:

```bash
rg -n "CXL.cache coherence|CXL.mem/DAX simulator traffic|software replay validation only|blocked|Type-2 BAR2" \
  docs/evaluation/sim-feasible-bench-20260702.md
jq -r '.claims[] | [.claim, .status, .paper_safe_wording] | @tsv' \
  docs/evaluation/sim-feasible-bench-20260702.json
```

Expected: the Markdown mentions blocked claims, and the JSON prints 12 TSV rows.

- [ ] **Step 4: Commit Task 3**

Run:

```bash
git add docs/evaluation/sim-feasible-bench-20260702.json docs/evaluation/sim-feasible-bench-20260702.md
git commit -m "bench: add simulator feasible benchmark evidence"
```

## Task 4: Paper Update from Simulator-Feasible Evidence

**Files:**
- Modify: `/root/Concordia/64fa450c44d0cdf46c7c3a7d/eval.tex`

**Interfaces:**
- Consumes: `docs/evaluation/sim-feasible-bench-20260702.json`
- Produces: paper text that reports only measured or explicitly blocked simulator-feasible claims.

- [ ] **Step 1: Read the generated evidence before editing the paper**

Run:

```bash
jq -r '.claims[] | [.claim, .status, .paper_safe_wording, .limitation] | @tsv' \
  /root/Concordia/SlugArch/.worktrees/slugarch-sim-feasible-bench/docs/evaluation/sim-feasible-bench-20260702.json
```

Expected: 12 rows, including blocked entries for CXL.cache coherence, DMA, ATS, page migration, switch ordering, and FPGA resource cost.

- [ ] **Step 2: Update `eval.tex`**

In `/root/Concordia/64fa450c44d0cdf46c7c3a7d/eval.tex`, add a short subsection after the current QEMU Type-2 BAR first-results subsection. The text must state:

```latex
\subsection{Simulator-feasible claim audit}

The next artifact pass separates simulator-supported measurements from
architecture claims that require additional substrate. SlugArch's software
replay recorder measured validation, delta, and full payload-capture modes for
the same 98-record 4x4 GEMM boundary trace. These measurements report replay
metadata bytes, payload-capture bytes, validation latency, and provenance label
coverage for the software boundary recorder; they are not hardware compression
or fabric-wide provenance measurements.

The pass also audits the live simulator substrate for broader claims. The
existing Type-2 BAR2 repeatability artifact remains measured evidence for the
guest-visible command path. CXL.mem/DAX is reported only when a live DAX-backed
path is visible and exercised; otherwise the artifact records the checked device
paths and classifies the claim as blocked. CXL.cache coherence, DMA, ATS, page
migration, switch ordering, and post-fit FPGA resource cost remain blocked in
this pass unless their corresponding simulator or hardware path is present in
the artifact. This keeps the evaluation truthful: every row is either measured
on the stated substrate or named as a limitation with the missing substrate.
```

If the generated JSON has measured CXL.mem/DAX status, add one sentence with the
device path and measured bandwidth. If the generated JSON has blocked CXL.mem/DAX
status, add one sentence saying the DAX path was not visible in this run and the
claim remains a benchmark slot.

- [ ] **Step 3: Build and inspect the paper**

Run in `/root/Concordia/64fa450c44d0cdf46c7c3a7d`:

```bash
latexmk -pdf -interaction=nonstopmode main.tex
pdftotext main.pdf - | rg -n "Simulator-feasible|software boundary recorder|CXL.mem/DAX|blocked|FPGA resource" -C 2
```

Expected: `latexmk` exits 0 and `pdftotext` finds the new subsection text.

- [ ] **Step 4: Commit Task 4 in the paper repo**

Run in `/root/Concordia/64fa450c44d0cdf46c7c3a7d`:

```bash
git add eval.tex
git commit -m "paper: add simulator feasible claim audit"
git stash push -u -m "latex build byproducts after simulator feasible audit"
```

Expected: paper repo has a new commit and a clean status after the stash.

## Task 5: Final Verification and Local Merge

**Files:**
- No new source files beyond Tasks 1-4.

**Interfaces:**
- Consumes: all earlier task commits.
- Produces: verified branch ready for local merge.

- [ ] **Step 1: Run SlugArch final verification**

Run in `/root/Concordia/SlugArch/.worktrees/slugarch-sim-feasible-bench`:

```bash
cargo fmt --check --package slugarch-host --package slugarch-cli --package slugcxl-gen
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test qemu_type2_artifacts
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test sim_feasible
cargo test -p slugcxl-gen
jq empty docs/evaluation/sim-feasible-bench-20260702.json
```

Expected: every command exits 0.

- [ ] **Step 2: Run paper final verification**

Run in `/root/Concordia/64fa450c44d0cdf46c7c3a7d`:

```bash
latexmk -pdf -interaction=nonstopmode main.tex
pdftotext main.pdf - | rg -n "Simulator-feasible|software boundary recorder|blocked" -C 2
```

Expected: `latexmk` exits 0 and the PDF text contains the simulator-feasible claim audit.

- [ ] **Step 3: Merge SlugArch branch locally**

Run:

```bash
cd /root/Concordia/SlugArch
git merge --ff-only slugarch-sim-feasible-bench
git worktree remove /root/Concordia/SlugArch/.worktrees/slugarch-sim-feasible-bench
git branch -d slugarch-sim-feasible-bench
```

Expected: `main` fast-forwards and the temporary worktree/branch are removed.

- [ ] **Step 4: Merge paper branch locally if Task 4 used a paper feature branch**

If the paper changes were made directly on `master`, skip this step. If a paper
feature branch was created for Task 4, run:

```bash
cd /root/Concordia/64fa450c44d0cdf46c7c3a7d
git switch master
git merge --ff-only slugarch-sim-feasible-bench
git branch -d slugarch-sim-feasible-bench
```

Expected: paper `master` contains the simulator-feasible audit commit and has a clean status after stashing build byproducts.
