use slugarch_host::sim_feasible::{
    build_sim_feasible_report, measure_replay_metadata, probe_dax_devices,
    write_sim_feasible_report, DaxProbeStatus, SimFeasibleInput,
};
use slugarch_host::GemmJob;
use std::fs;

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

    let validation = report
        .modes
        .iter()
        .find(|m| m.mode == "validation")
        .unwrap();
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
