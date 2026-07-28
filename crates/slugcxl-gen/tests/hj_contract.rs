use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use slugcxl_gen::{generate, GenerateOptions};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let serial = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "slugcxl-hj-contract-{}-{serial}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn generated_hj_has_atomic_runtime_policy_and_record_ports() {
    let dir = TestDir::new();
    generate(&GenerateOptions {
        out: dir.path().to_path_buf(),
        hardware_jit: true,
        quartus_project: None,
        policy_path: None,
    })
    .unwrap();

    let pipeline = std::fs::read_to_string(dir.path().join("slugcxl_hj_pipeline.sv")).unwrap();
    for token in [
        "policy_load_begin",
        "policy_load_valid",
        "policy_load_ready",
        "policy_load_index",
        "policy_load_word",
        "policy_load_commit",
        "policy_load_abort",
        "active_bank",
        "policy_digest",
        "policy_ready",
        "policy_error",
        "record_valid",
        "record_ready",
        "record_length",
        "record_data",
        "policy_words_bank0 [0:31]",
        "policy_words_bank1 [0:31]",
        "range_words_bank0 [0:3]",
        "range_words_bank1 [0:3]",
    ] {
        assert!(pipeline.contains(token), "missing {token}");
    }
    assert!(!pipeline.contains("parameter integer RECORD_MODE"));
    assert!(pipeline.contains("record_valid && !record_ready"));
    assert!(pipeline.contains("SLUG_JIT_ERR_DIGEST_MISMATCH"));
    assert!(pipeline.contains("SLUG_JIT_ERR_DROP"));

    let top = std::fs::read_to_string(dir.path().join("slugcxl_4x4_hj_top.sv")).unwrap();
    assert!(top.contains(".policy_load_begin(policy_load_begin)"));
    assert!(top.contains(".record_data(record_data)"));

    let fit = std::fs::read_to_string(dir.path().join("slugcxl_hj_fit_top.sv")).unwrap();
    assert!(fit.contains("default_policy_word"));
    assert!(fit.contains("policy_load_commit"));
    assert!(fit.contains("policy_ready"));

    let runtime: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("slugcxl_endpoint_runtime.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        runtime["flit_layout"]["event_id_bytes"],
        serde_json::json!([43, 50])
    );
    assert_eq!(
        runtime["flit_layout"]["phase_id_bytes"],
        serde_json::json!([51, 58])
    );
    assert_eq!(
        runtime["flit_layout"]["status_bytes"],
        serde_json::json!([59, 62])
    );
    assert_eq!(runtime["flit_layout"]["control_byte"], 63);
    assert_eq!(runtime["record_layout"]["record_bytes"], 128);
    assert_eq!(runtime["transport_limits"]["max_payload_bytes"], 32);
    assert_eq!(runtime["transport_limits"]["max_delta_pairs"], 16);
}
