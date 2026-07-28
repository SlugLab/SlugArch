use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use slugcxl_gen::{generate, GenerateOptions};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let serial = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "slugcxl-hj-opcode-contract-{}-{serial}",
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

fn generated_pipeline() -> String {
    let dir = TestDir::new();
    generate(&GenerateOptions {
        out: dir.path().to_path_buf(),
        hardware_jit: true,
        quartus_project: None,
        policy_path: None,
    })
    .unwrap();
    std::fs::read_to_string(dir.path().join("slugcxl_hj_pipeline.sv")).unwrap()
}

#[test]
fn generated_hj_interpreter_has_the_normative_opcode_and_state_contract() {
    let rtl = generated_pipeline();

    for (name, value) in [
        ("HJ_OP_HALT", "8'h00"),
        ("HJ_OP_MATCH_CLASS", "8'h01"),
        ("HJ_OP_MATCH_DIRECTION", "8'h02"),
        ("HJ_OP_MATCH_STATUS", "8'h03"),
        ("HJ_OP_MATCH_RANGE", "8'h04"),
        ("HJ_OP_SAMPLE", "8'h05"),
        ("HJ_OP_CAPTURE", "8'h06"),
        ("HJ_OP_EMIT", "8'h07"),
        ("HJ_OP_EPOCH_INCREMENT", "8'h08"),
        ("HJ_OP_EPOCH_FROM_PHASE", "8'h09"),
        ("HJ_OP_REJECT", "8'h0a"),
    ] {
        let declaration = format!("{name} = {value}");
        assert!(rtl.contains(&declaration), "missing {declaration}");
        assert!(
            rtl.contains(&format!("{name}: begin")),
            "missing decode arm for {name}"
        );
    }

    for state in [
        "HJ_STATE_IDLE",
        "HJ_STATE_FETCH",
        "HJ_STATE_DECODE",
        "HJ_STATE_MATCH",
        "HJ_STATE_CAPTURE",
        "HJ_STATE_EMIT_WAIT",
        "HJ_STATE_REJECT",
        "HJ_STATE_DONE",
        "HJ_STATE_ERROR",
    ] {
        assert!(rtl.contains(state), "missing interpreter state {state}");
    }
}

#[test]
fn generated_hj_interpreter_is_bounded_and_fail_stop() {
    let rtl = generated_pipeline();

    for token in [
        "SLUG_JIT_ERR_INVALID_CONTROL_FLOW",
        "SLUG_JIT_ERR_UNSUPPORTED",
        "SLUG_JIT_ERR_TIMEOUT",
        "SLUG_JIT_ERR_BACKEND",
        "execution_steps == 6'd31",
        "branch_target >= active_instruction_count",
        "{24'd0, instruction_range_index}\n                       >= active_range_count",
        "instruction_stride == 32'd0",
        "chosen_payload_len > 6'd32",
        "record_valid && !record_ready",
        "record_data <= pending_record_data",
        "record_length <= pending_record_length",
        "hj_state <= HJ_STATE_ERROR",
        "ep_flit_valid  = h2d_flit_valid && engine_idle\n      && !ep_resp_valid",
        "load_failed",
    ] {
        assert!(rtl.contains(token), "missing fail-stop guard {token}");
    }
}

#[test]
fn generated_hj_interpreter_covers_capture_epoch_and_loader_edges() {
    let rtl = generated_pipeline();

    for token in [
        "HJ_CAPTURE_VALIDATION",
        "HJ_CAPTURE_DELTA",
        "HJ_CAPTURE_FULL",
        "hash_payload(event_payload, event_payload_len)",
        "event_payload[delta_scan_index * 8 +: 8]",
        "event_epoch <= event_phase_id",
        "event_epoch <= event_epoch + 64'd1",
        "policy_load_abort",
        "load_word_count != POLICY_IMAGE_WORDS",
        "policy_load_index >= 16'd40",
        "load_seen[policy_load_index[5:0]]",
        "if (load_failed) begin",
    ] {
        assert!(rtl.contains(token), "missing interpreter behavior {token}");
    }
}
