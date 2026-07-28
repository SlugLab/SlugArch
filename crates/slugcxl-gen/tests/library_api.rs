use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use slugcxl_gen::{generate, GenerateOptions};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        let serial = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "slugcxl-gen-{name}-{}-{serial}",
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
fn library_emits_hj_artifacts_without_process_exit() {
    let dir = TestDir::new("library");
    let outputs = generate(&GenerateOptions {
        out: dir.path().to_path_buf(),
        hardware_jit: true,
        quartus_project: None,
        policy_path: None,
    })
    .unwrap();

    assert!(outputs
        .iter()
        .any(|path| path.ends_with("slugcxl_hj_pipeline.sv")));
    for name in [
        "slugcxl_endpoint.sv",
        "slugcxl_4x4_top.sv",
        "slugcxl_endpoint_runtime.json",
        "slugcxl_hj_pipeline.sv",
        "slugcxl_4x4_hj_top.sv",
        "slugcxl_hj_fit_top.sv",
        "slugcxl_hj_overhead.json",
        "slugcxl_hj_policy.hex",
        "slugcxl_hj_policy.json",
    ] {
        assert!(dir.path().join(name).is_file(), "missing {name}");
    }

    let runtime =
        std::fs::read_to_string(dir.path().join("slugcxl_endpoint_runtime.json")).unwrap();
    assert!(runtime.contains("\"policy_hex\": \"slugcxl_hj_policy.hex\""));
    assert!(runtime.contains("\"policy_json\": \"slugcxl_hj_policy.json\""));
}
