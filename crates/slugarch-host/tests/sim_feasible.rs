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
