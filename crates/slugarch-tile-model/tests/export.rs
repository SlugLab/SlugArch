use slugarch_tile_model::{export_corpus, CorpusConfig, FaultKind, RecordMode, WORKLOAD_SEED};
use std::fs;

#[test]
fn identical_configs_produce_byte_identical_canonical_json_lines() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let first_path = directory.path().join("first.json");
    let second_path = directory.path().join("second.json");
    let config = CorpusConfig {
        tiles: 4,
        record_mode: RecordMode::Validation,
        seed: WORKLOAD_SEED,
        fault: None,
    };

    let first = export_corpus(&config, &first_path).expect("first export");
    let second = export_corpus(&config, &second_path).expect("second export");

    assert_eq!(first.sha256, second.sha256);
    assert_eq!(
        fs::read(&first_path).expect("first bytes"),
        fs::read(&second_path).expect("second bytes")
    );
    assert_eq!(first.case_count, 4);
}

#[test]
fn every_exported_case_preserves_the_model_proof_boundary() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("corpus.json");
    let config = CorpusConfig {
        tiles: 2,
        record_mode: RecordMode::Delta,
        seed: WORKLOAD_SEED,
        fault: None,
    };
    export_corpus(&config, &path).expect("export");

    let text = fs::read_to_string(path).expect("corpus text");
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 4);
    for line in lines {
        let value: serde_json::Value = serde_json::from_str(line).expect("canonical JSON object");
        assert_eq!(value["evidence_kind"], "qemu_event_level_home_agent_model");
        assert_eq!(value["physical_cxl_cache"], false);
        assert_eq!(value["tiles"], 2);
        assert_eq!(value["record_mode"], 2);
        assert_eq!(value["warmup_events"], 200);
        assert_eq!(value["measured_events"], 20_000);
        assert!(value["warmup_sha256"].as_str().is_some());
        assert!(value["measured_sha256"].as_str().is_some());
        assert!(value.get("timestamp").is_none());
        assert!(value.get("pointer").is_none());
    }
}

#[test]
fn exporter_rejects_a_nonexistent_parent_directory() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("missing").join("corpus.json");
    let config = CorpusConfig {
        tiles: 1,
        record_mode: RecordMode::Full,
        seed: WORKLOAD_SEED,
        fault: None,
    };
    let error = export_corpus(&config, &path).unwrap_err();
    assert_eq!(error.code, 0x0009);
}

#[test]
fn requested_fault_adds_one_attributed_corpus_case() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("fault-corpus.json");
    let config = CorpusConfig {
        tiles: 2,
        record_mode: RecordMode::Validation,
        seed: WORKLOAD_SEED,
        fault: Some(FaultKind::StaleLineVersion),
    };
    let exported = export_corpus(&config, &path).expect("fault export");
    assert_eq!(exported.case_count, 5);

    let text = fs::read_to_string(path).expect("fault corpus text");
    let value: serde_json::Value =
        serde_json::from_str(text.lines().last().expect("fault line")).expect("fault object");
    assert_eq!(value["case_kind"], "fault");
    assert_eq!(value["fault_code"], 0x1002);
    assert!(value["injected_tile_id"].as_u64().is_some());
    assert!(value["injected_event_id"].as_u64().is_some());
    assert!(value["first_failure_event_id"].as_u64().is_some());
}
