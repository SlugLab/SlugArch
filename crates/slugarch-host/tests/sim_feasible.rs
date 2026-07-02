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

#[test]
fn visible_dax_nodes_do_not_upgrade_cxl_mem_claim_without_streaming_artifact() {
    let out = std::env::temp_dir().join(format!("slugarch-sim-visible-dax-{}", std::process::id()));
    let dev = out.join("dev");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&dev).unwrap();
    fs::write(dev.join("dax0.0"), b"").unwrap();

    let report = build_sim_feasible_report(SimFeasibleInput {
        job: &job(),
        replay_repeats: 1,
        qemu_repeatability_dir: None,
        dev_root: &dev,
    })
    .unwrap();

    assert_eq!(report.dax_probe.status, DaxProbeStatus::Measured);
    let dax_claim = report
        .claims
        .iter()
        .find(|claim| claim.claim == "CXL.mem/DAX simulator traffic")
        .unwrap();
    assert!(report
        .dax_probe
        .devices
        .iter()
        .any(|path| path.ends_with("dax0.0")));
    assert_eq!(
        serde_json::to_value(&dax_claim.status).unwrap(),
        serde_json::json!("blocked")
    );
}

#[test]
fn sim_feasible_report_uses_measured_replay_workload() {
    let out = std::env::temp_dir().join(format!("slugarch-sim-workload-{}", std::process::id()));
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

    assert_eq!(report.workload, report.replay_metadata.workload);
}

#[test]
fn bar2_claims_follow_the_actual_repeatability_source_path() {
    let out = std::env::temp_dir().join(format!("slugarch-sim-bar2-source-{}", std::process::id()));
    let dev = out.join("dev");
    let fake_bar2 = out.join("custom-repeatability-artifact");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&dev).unwrap();
    fs::create_dir_all(fake_bar2.join("run-1")).unwrap();
    fs::write(
        fake_bar2.join("run-1/summary.json"),
        r#"{"status":"pass","request_count":1,"response_count":1,"tag_mismatches":0,"dispatch_failures":0}"#,
    )
    .unwrap();
    fs::write(
        fake_bar2.join("run-1/guest-summary.json"),
        r#"{"elapsed_ms":7}"#,
    )
    .unwrap();

    let blocked_report = build_sim_feasible_report(SimFeasibleInput {
        job: &job(),
        replay_repeats: 1,
        qemu_repeatability_dir: None,
        dev_root: &dev,
    })
    .unwrap();
    let measured_report = build_sim_feasible_report(SimFeasibleInput {
        job: &job(),
        replay_repeats: 1,
        qemu_repeatability_dir: Some(&fake_bar2),
        dev_root: &dev,
    })
    .unwrap();

    for report in [&blocked_report, &measured_report] {
        let serialized = serde_json::to_string(report).unwrap();
        assert!(!serialized.contains("qemu-type2-repeatability-20260702-0627"));
    }

    let bar2_claim = measured_report
        .claims
        .iter()
        .find(|claim| claim.claim == "QEMU Type-2 BAR2 command/replay boundary")
        .unwrap();
    let runtime_claim = measured_report
        .claims
        .iter()
        .find(|claim| claim.claim == "Runtime overhead")
        .unwrap();
    let fake_path = fake_bar2.display().to_string();
    assert_eq!(bar2_claim.evidence_artifact, fake_path);
    assert_eq!(bar2_claim.checked, vec![fake_path.clone()]);
    assert_eq!(runtime_claim.evidence_artifact, fake_path);
    assert_eq!(runtime_claim.checked, vec![fake_path]);

    let blocked_bar2_claim = blocked_report
        .claims
        .iter()
        .find(|claim| claim.claim == "QEMU Type-2 BAR2 command/replay boundary")
        .unwrap();
    let blocked_runtime_claim = blocked_report
        .claims
        .iter()
        .find(|claim| claim.claim == "Runtime overhead")
        .unwrap();
    assert_eq!(blocked_bar2_claim.evidence_artifact, "none");
    assert_eq!(blocked_bar2_claim.checked, vec!["none".to_string()]);
    assert_eq!(blocked_runtime_claim.evidence_artifact, "none");
    assert_eq!(blocked_runtime_claim.checked, vec!["none".to_string()]);
}
