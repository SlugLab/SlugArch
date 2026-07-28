use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use slugcxl_gen::{generate, GenerateOptions};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let serial = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "slugcxl-hj-rtl-smoke-{}-{serial}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        if std::env::var_os("SLUGARCH_KEEP_RTL_TEST").is_none() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn verilated_policy_loader_interpreter_and_failures_are_exact() {
    let dir = TestDir::new();
    generate(&GenerateOptions {
        out: dir.path().to_path_buf(),
        hardware_jit: true,
        quartus_project: None,
        policy_path: None,
    })
    .unwrap();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let testbench = manifest_dir.join("tests/fixtures/hj_policy_tb.sv");
    let obj_dir = dir.path().join("obj");
    let status =
        Command::new(std::env::var("VERILATOR").unwrap_or_else(|_| "verilator".to_string()))
            .args([
                "--binary",
                "--timing",
                "--assert",
                "--Wall",
                "-Wno-fatal",
                "-Wno-UNUSEDSIGNAL",
                "--top-module",
                "hj_policy_tb",
                "--Mdir",
            ])
            .arg(&obj_dir)
            .arg(dir.path().join("slugcxl_hj_pipeline.sv"))
            .arg(&testbench)
            .env("CCACHE_DISABLE", "1")
            .env("TMPDIR", dir.path())
            .status()
            .expect("failed to invoke Verilator");
    assert!(status.success(), "Verilator failed to build HJ testbench");

    let output = Command::new(obj_dir.join("Vhj_policy_tb"))
        .arg(format!(
            "+POLICY_HEX={}",
            dir.path().join("slugcxl_hj_policy.hex").display()
        ))
        .output()
        .expect("failed to run Verilated HJ testbench");
    assert!(
        output.status.success(),
        "HJ RTL smoke failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("SLUGARCH_HJ_POLICY_RTL_PASS"),
        "HJ RTL smoke did not print its proof marker"
    );
}
