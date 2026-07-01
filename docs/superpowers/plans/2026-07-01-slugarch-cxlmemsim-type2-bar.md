# SlugArch CXLMemSim Type-2 BAR Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Evaluate SlugArch through the guest-visible CXLMemSim QEMU Type-2 BAR2 path, with `targets/qemu-type2/` as the stable SlugArch target surface.

**Architecture:** SlugArch exports and validates 64-byte SlugCXL FLIT streams. A QEMU `cxl-type2` BAR2 bridge accepts one FLIT per command, executes a deterministic software SlugCXL 4x4 GEMM endpoint model, and returns one response FLIT per request. A guest C helper maps BAR2, submits the exported requests, captures responses, and the host harness stores evidence under `artifact/slugarch_cxlmemsim/<run-id>/`.

**Tech Stack:** Rust 2021, Cargo, Clap, serde/serde_json, SlugArch crates, C, QEMU CXL Type-2 device model, CXLMemSim qemu_integration shell scripts, SSH for existing-guest runs.

## Global Constraints

- Do not run Quartus, generate SOF files, or depend on FPGA hardware-JIT collateral.
- Preserve SlugArch's current v1 64-byte FLIT layout; do not redesign the wire format.
- Keep `targets/qemu-type2/` as the SlugArch target-specific entrypoint for this simulator path.
- Keep CXLMemSim edits path-limited because `/home/victoryang00/CXLMemSim` is already dirty.
- Ignore malformed `launch_qemu_type2_gpu.sh` and `launch_qemu_type2_hetgpu.sh`.
- Use the supported CXLMemSim Type-2 surfaces: `qemu_integration/smoke_type2_endpoint.sh`, `qemu_integration/launch_qemu_vcs_dcd_gfam.sh`, and BAR2 on PCI `8086:0d92`.
- The first live run may use `--existing-guest`; `--launch` may be added only after local kernel/image/SSH paths are known.

---

## File Structure

SlugArch files:

- Create `targets/qemu-type2/README.md`: target documentation and command sequence.
- Create `targets/qemu-type2/identity_times_const.json`: default GEMM job for simulator proof.
- Create `targets/qemu-type2/run_existing_guest.sh`: host-side harness for an already running QEMU guest.
- Create `crates/slugarch-host/src/qemu_type2.rs`: export and validation helpers for request/response FLIT artifacts.
- Modify `crates/slugarch-host/src/lib.rs`: export the qemu_type2 module API.
- Modify `crates/slugarch-cli/src/main.rs`: add `export-cxlmemsim` and `validate-cxlmemsim` subcommands.
- Test `crates/slugarch-host/tests/qemu_type2_artifacts.rs`: artifact export/validation tests.

CXLMemSim files:

- Modify `/home/victoryang00/CXLMemSim/lib/qemu/include/hw/cxl/cxl_type2_gpu_cmd.h`: add Slug command IDs and register result conventions.
- Modify `/home/victoryang00/CXLMemSim/qemu_integration/guest_libcuda/cxl_gpu_cmd.h`: keep guest command IDs in sync.
- Modify `/home/victoryang00/CXLMemSim/lib/qemu/include/hw/cxl/cxl_type2.h`: add `CXLType2SlugBridgeState`.
- Modify `/home/victoryang00/CXLMemSim/lib/qemu/hw/cxl/cxl_type2.c`: implement Slug bridge commands and deterministic endpoint model.
- Create `/home/victoryang00/CXLMemSim/qemu_integration/slugarch_type2_guest.c`: guest BAR2 submit/capture helper.
- Create `/home/victoryang00/CXLMemSim/qemu_integration/slugarch_type2_bar_run.sh`: guest-side wrapper used by SlugArch harness.

---

### Task 1: SlugArch Artifact Export and Validation

**Files:**
- Create: `crates/slugarch-host/src/qemu_type2.rs`
- Modify: `crates/slugarch-host/src/lib.rs`
- Test: `crates/slugarch-host/tests/qemu_type2_artifacts.rs`

**Interfaces:**
- Consumes: `GemmJob`, `GemmResult`, `dispatch::build_gemm_dispatch_stream`, `result::decode_results`, `slugarch_cxl_wire::{encode, decode, CxlMsg, S2MDRSOp, S2MNDROp, FLIT_BYTES}`.
- Produces:
  - `pub struct QemuType2Expected`
  - `pub struct QemuType2Summary`
  - `pub fn expected_for_job(job: &GemmJob) -> QemuType2Expected`
  - `pub fn export_requests(job: &GemmJob, out_dir: &Path) -> Result<QemuType2Expected, HostError>`
  - `pub fn validate_responses(job: &GemmJob, responses_path: &Path, out_dir: &Path) -> Result<QemuType2Summary, HostError>`

- [ ] **Step 1: Write failing artifact tests**

Create `crates/slugarch-host/tests/qemu_type2_artifacts.rs`:

```rust
use slugarch_host::qemu_type2::{export_requests, validate_responses};
use slugarch_host::GemmJob;
use slugarch_cxl_wire::{encode, CxlMsg, S2MDRSOp, S2MNDROp, FLIT_BYTES};
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

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "slugarch-qemu-type2-{}-{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn export_writes_49_request_flits_and_expected_json() {
    let dir = temp_dir("export");
    let expected = export_requests(&job(), &dir).unwrap();
    let requests = fs::read(dir.join("requests.bin")).unwrap();
    assert_eq!(requests.len(), 49 * FLIT_BYTES);
    assert_eq!(expected.request_count, 49);
    assert_eq!(expected.expected_c[0], [2, 3, 4, 5]);
    assert!(dir.join("expected.json").exists());
}

#[test]
fn validate_accepts_known_good_response_stream() {
    let dir = temp_dir("validate-good");
    export_requests(&job(), &dir).unwrap();
    let response_bytes = known_good_response_stream();
    fs::write(dir.join("responses.bin"), response_bytes).unwrap();
    let summary = validate_responses(&job(), &dir.join("responses.bin"), &dir).unwrap();
    assert_eq!(summary.status, "pass");
    assert_eq!(summary.response_count, 49);
    assert_eq!(summary.tag_mismatches, 0);
    assert_eq!(summary.result_c[3], [14, 15, 16, 17]);
}

#[test]
fn validate_rejects_truncated_responses() {
    let dir = temp_dir("truncated");
    fs::write(dir.join("responses.bin"), vec![0u8; FLIT_BYTES - 1]).unwrap();
    let err = validate_responses(&job(), &dir.join("responses.bin"), &dir).unwrap_err();
    assert!(format!("{err}").contains("multiple of 64"));
}

fn known_good_response_stream() -> Vec<u8> {
    let mut bytes = Vec::new();
    for tag in 0..33u16 {
        bytes.extend_from_slice(&encode(&CxlMsg::S2MNDR {
            tag,
            opcode: S2MNDROp::Cmp,
        }));
    }
    let expected = [
        [2u32, 3, 4, 5],
        [6, 7, 8, 9],
        [10, 11, 12, 13],
        [14, 15, 16, 17],
    ];
    for (i, value) in expected.iter().flatten().enumerate() {
        let mut data = [0u8; 32];
        data[0] = (*value & 0xff) as u8;
        data[1] = ((*value >> 8) & 0xff) as u8;
        data[2] = ((*value >> 16) & 0xff) as u8;
        bytes.extend_from_slice(&encode(&CxlMsg::S2MDRS {
            tag: 33 + i as u16,
            opcode: S2MDRSOp::MemData,
            data,
        }));
    }
    bytes
}
```

Expected initial result: compile fails because `qemu_type2` API does not exist.

- [ ] **Step 2: Run failing tests**

Run:

```bash
cargo test -p slugarch-host --test qemu_type2_artifacts
```

Expected: FAIL with unresolved import `slugarch_host::qemu_type2`.

- [ ] **Step 3: Implement `qemu_type2.rs`**

Create `crates/slugarch-host/src/qemu_type2.rs` with these exact public types and behavior:

```rust
use crate::dispatch::build_gemm_dispatch_stream;
use crate::result::decode_results;
use crate::{GemmJob, HostError};
use serde::{Deserialize, Serialize};
use slugarch_cxl_wire::{decode, encode, CxlMsg, S2MDRSOp, S2MNDROp, FLIT_BYTES};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QemuType2Expected {
    pub workload: String,
    pub request_count: usize,
    pub flit_bytes: usize,
    pub request_tags: Vec<u16>,
    pub expected_c: [[u32; 4]; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QemuType2Summary {
    pub status: String,
    pub request_count: usize,
    pub response_count: usize,
    pub tag_mismatches: usize,
    pub dispatch_failures: usize,
    pub result_c: [[u32; 4]; 4],
    pub expected_c: [[u32; 4]; 4],
    pub requests_path: String,
    pub responses_path: String,
}

pub fn expected_for_job(job: &GemmJob) -> QemuType2Expected {
    let stream = build_gemm_dispatch_stream(job, 0);
    QemuType2Expected {
        workload: "slugcxl_gemm_4x4".to_string(),
        request_count: stream.len(),
        flit_bytes: FLIT_BYTES,
        request_tags: stream.iter().map(CxlMsg::tag).collect(),
        expected_c: matmul(job),
    }
}

pub fn export_requests(job: &GemmJob, out_dir: &Path) -> Result<QemuType2Expected, HostError> {
    fs::create_dir_all(out_dir).map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("create {}: {e}", out_dir.display()),
    })?;
    let stream = build_gemm_dispatch_stream(job, 0);
    let mut bytes = Vec::with_capacity(stream.len() * FLIT_BYTES);
    for msg in &stream {
        bytes.extend_from_slice(&encode(msg));
    }
    fs::write(out_dir.join("requests.bin"), bytes).map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("write requests.bin: {e}"),
    })?;
    let expected = expected_for_job(job);
    let expected_json = serde_json::to_vec_pretty(&expected).map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("serialize expected.json: {e}"),
    })?;
    fs::write(out_dir.join("expected.json"), expected_json).map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("write expected.json: {e}"),
    })?;
    Ok(expected)
}

pub fn validate_responses(
    job: &GemmJob,
    responses_path: &Path,
    out_dir: &Path,
) -> Result<QemuType2Summary, HostError> {
    let bytes = fs::read(responses_path).map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("read {}: {e}", responses_path.display()),
    })?;
    if bytes.len() % FLIT_BYTES != 0 {
        return Err(HostError::DispatchFailed {
            tag: 0,
            reason: format!("responses.bin length must be a multiple of 64, got {}", bytes.len()),
        });
    }
    let response_count = bytes.len() / FLIT_BYTES;
    let requests = build_gemm_dispatch_stream(job, 0);
    let mut decoded = Vec::with_capacity(response_count);
    for chunk in bytes.chunks_exact(FLIT_BYTES) {
        let mut flit = [0u8; FLIT_BYTES];
        flit.copy_from_slice(chunk);
        decoded.push(decode(&flit)?);
    }
    let mut tag_mismatches = 0usize;
    let mut dispatch_failures = 0usize;
    let mut read_responses = Vec::new();
    for (idx, response) in decoded.iter().enumerate() {
        if let Some(request) = requests.get(idx) {
            if response.tag() != request.tag() {
                tag_mismatches += 1;
            }
        }
        match response {
            CxlMsg::S2MNDR { opcode: S2MNDROp::Cmp, .. } if idx < 33 => {}
            CxlMsg::S2MDRS { opcode: S2MDRSOp::MemData, .. } if idx >= 33 => {
                read_responses.push(response.clone());
            }
            CxlMsg::S2MNDR { opcode: S2MNDROp::DispatchFailed, .. } => {
                dispatch_failures += 1;
            }
            _ => {
                dispatch_failures += 1;
            }
        }
    }
    let result_c = if read_responses.len() == 16 {
        decode_results(&read_responses)?
    } else {
        [[0u32; 4]; 4]
    };
    let expected_c = matmul(job);
    let status = if response_count == requests.len()
        && tag_mismatches == 0
        && dispatch_failures == 0
        && result_c == expected_c
    {
        "pass"
    } else {
        "fail"
    }
    .to_string();
    let summary = QemuType2Summary {
        status,
        request_count: requests.len(),
        response_count,
        tag_mismatches,
        dispatch_failures,
        result_c,
        expected_c,
        requests_path: out_dir.join("requests.bin").display().to_string(),
        responses_path: responses_path.display().to_string(),
    };
    fs::create_dir_all(out_dir).map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("create {}: {e}", out_dir.display()),
    })?;
    fs::write(
        out_dir.join("summary.json"),
        serde_json::to_vec_pretty(&summary).map_err(|e| HostError::DispatchFailed {
            tag: 0,
            reason: format!("serialize summary.json: {e}"),
        })?,
    )
    .map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("write summary.json: {e}"),
    })?;
    fs::write(out_dir.join("summary.csv"), summary_csv(&summary)).map_err(|e| {
        HostError::DispatchFailed {
            tag: 0,
            reason: format!("write summary.csv: {e}"),
        }
    })?;
    Ok(summary)
}

fn matmul(job: &GemmJob) -> [[u32; 4]; 4] {
    let mut out = [[0u32; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            let mut sum = 0u32;
            for k in 0..4 {
                sum += job.a[r][k] as u32 * job.b[k][c] as u32;
            }
            out[r][c] = sum;
        }
    }
    out
}

fn summary_csv(summary: &QemuType2Summary) -> String {
    format!(
        "status,request_count,response_count,tag_mismatches,dispatch_failures\n{},{},{},{},{}\n",
        summary.status,
        summary.request_count,
        summary.response_count,
        summary.tag_mismatches,
        summary.dispatch_failures
    )
}
```

- [ ] **Step 4: Export module from `lib.rs`**

Add to `crates/slugarch-host/src/lib.rs`:

```rust
pub mod qemu_type2;
```

- [ ] **Step 5: Fix test imports**

In `crates/slugarch-host/tests/qemu_type2_artifacts.rs`, make sure the import line uses:

```rust
use slugarch_cxl_wire::{encode, CxlMsg, S2MNDROp, FLIT_BYTES};
```

Remove unused imports after the module compiles.

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p slugarch-host --test qemu_type2_artifacts
cargo test -p slugarch-host
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/slugarch-host/src/lib.rs crates/slugarch-host/src/qemu_type2.rs crates/slugarch-host/tests/qemu_type2_artifacts.rs
git commit -m "feat: export qemu type2 cxlmemsim artifacts"
```

---

### Task 2: SlugArch CLI and `targets/qemu-type2`

**Files:**
- Create: `targets/qemu-type2/README.md`
- Create: `targets/qemu-type2/identity_times_const.json`
- Create: `targets/qemu-type2/run_existing_guest.sh`
- Modify: `crates/slugarch-cli/src/main.rs`

**Interfaces:**
- Consumes: Task 1 `slugarch_host::qemu_type2::{export_requests, validate_responses}`.
- Produces:
  - `slugarch export-cxlmemsim <job> --out <dir>`
  - `slugarch validate-cxlmemsim <job> --responses <responses.bin> --out <dir>`
  - `targets/qemu-type2/run_existing_guest.sh` host harness.

- [ ] **Step 1: Add target job fixture**

Create `targets/qemu-type2/identity_times_const.json`:

```json
{
  "a": [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
  "b": [[2, 3, 4, 5], [6, 7, 8, 9], [10, 11, 12, 13], [14, 15, 16, 17]]
}
```

- [ ] **Step 2: Add CLI subcommands**

Modify `crates/slugarch-cli/src/main.rs`:

```rust
    /// Export SlugCXL request FLITs for the CXLMemSim QEMU Type-2 BAR target.
    ExportCxlmemsim {
        /// Path to a GemmJob JSON file.
        job: PathBuf,
        /// Output directory for requests.bin and expected.json.
        #[arg(long)]
        out: PathBuf,
    },
    /// Validate CXLMemSim QEMU Type-2 BAR response FLITs.
    ValidateCxlmemsim {
        /// Path to a GemmJob JSON file.
        job: PathBuf,
        /// Path to responses.bin captured from the guest helper.
        #[arg(long)]
        responses: PathBuf,
        /// Output directory for summary.json and summary.csv.
        #[arg(long)]
        out: PathBuf,
    },
```

Add match arms:

```rust
        Cmd::ExportCxlmemsim { job, out } => export_cxlmemsim(&job, &out),
        Cmd::ValidateCxlmemsim {
            job,
            responses,
            out,
        } => validate_cxlmemsim(&job, &responses, &out),
```

Add helper functions:

```rust
fn read_gemm_job(job_path: &std::path::Path) -> Result<slugarch_host::GemmJob> {
    let text = std::fs::read_to_string(job_path)
        .with_context(|| format!("reading {}", job_path.display()))?;
    serde_json::from_str(&text).with_context(|| "parsing GemmJob JSON")
}

fn export_cxlmemsim(job_path: &std::path::Path, out: &std::path::Path) -> Result<()> {
    let job = read_gemm_job(job_path)?;
    let expected = slugarch_host::qemu_type2::export_requests(&job, out)
        .map_err(|e| anyhow!("export cxlmemsim: {}", e))?;
    println!("workload: {}", expected.workload);
    println!("requests: {}", expected.request_count);
    println!("flit_bytes: {}", expected.flit_bytes);
    println!("out: {}", out.display());
    Ok(())
}

fn validate_cxlmemsim(
    job_path: &std::path::Path,
    responses: &std::path::Path,
    out: &std::path::Path,
) -> Result<()> {
    let job = read_gemm_job(job_path)?;
    let summary = slugarch_host::qemu_type2::validate_responses(&job, responses, out)
        .map_err(|e| anyhow!("validate cxlmemsim: {}", e))?;
    println!("status: {}", summary.status);
    println!("requests: {}", summary.request_count);
    println!("responses: {}", summary.response_count);
    println!("tag_mismatches: {}", summary.tag_mismatches);
    println!("dispatch_failures: {}", summary.dispatch_failures);
    if summary.status != "pass" {
        return Err(anyhow!("CXLMemSim Type-2 validation failed"));
    }
    Ok(())
}
```

- [ ] **Step 3: Add target README**

Create `targets/qemu-type2/README.md`:

```markdown
# SlugArch QEMU Type-2 Target

This target evaluates SlugArch through the CXLMemSim QEMU `cxl-type2` BAR2
path. It is the simulator-backed replacement for `targets/agilex-vr2` hardware
JIT evaluation.

## One-Guest Existing-VM Flow

```bash
RUN_DIR=artifact/slugarch_cxlmemsim/$(date -u +%Y%m%d-%H%M%S)
cargo run -p slugarch-cli -- export-cxlmemsim \
  targets/qemu-type2/identity_times_const.json --out "$RUN_DIR"
CXLMEMSIM_GUEST_SSH="ssh root@GUEST" \
  targets/qemu-type2/run_existing_guest.sh "$RUN_DIR"
cargo run -p slugarch-cli -- validate-cxlmemsim \
  targets/qemu-type2/identity_times_const.json \
  --responses "$RUN_DIR/responses.bin" --out "$RUN_DIR"
```

The run passes only when `summary.json` reports `status: "pass"` and the QEMU
log shows Type-2 device realization plus Slug bridge activity.
```

- [ ] **Step 4: Add target harness**

Create `targets/qemu-type2/run_existing_guest.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 RUN_DIR" >&2
    exit 2
fi

RUN_DIR=$1
GUEST_SSH=${CXLMEMSIM_GUEST_SSH:?set CXLMEMSIM_GUEST_SSH, for example: ssh root@192.168.122.10}
CXLMEMSIM_ROOT=${CXLMEMSIM_ROOT:-/home/victoryang00/CXLMemSim}
GUEST_DIR=${CXLMEMSIM_GUEST_DIR:-/root/slugarch-qemu-type2}

mkdir -p "$RUN_DIR"
{
    echo "guest_ssh=$GUEST_SSH"
    echo "cxlmemsim_root=$CXLMEMSIM_ROOT"
    echo "guest_dir=$GUEST_DIR"
} >>"$RUN_DIR/commands.txt"

if [[ ! -f "$RUN_DIR/requests.bin" ]]; then
    echo "missing $RUN_DIR/requests.bin; run slugarch export-cxlmemsim first" >&2
    exit 1
fi

if [[ ! -f "$CXLMEMSIM_ROOT/qemu_integration/slugarch_type2_guest.c" ]]; then
    echo "missing CXLMemSim guest helper source; implement qemu_integration/slugarch_type2_guest.c first" >&2
    exit 1
fi

$GUEST_SSH "mkdir -p '$GUEST_DIR'"
scp "$RUN_DIR/requests.bin" "$CXLMEMSIM_ROOT/qemu_integration/slugarch_type2_guest.c" "$GUEST_SSH:$GUEST_DIR/"
$GUEST_SSH "cd '$GUEST_DIR' && gcc -O2 -Wall -Wextra -o slugarch_type2_guest slugarch_type2_guest.c && sudo ./slugarch_type2_guest --requests requests.bin --responses responses.bin --summary guest-summary.json"
scp "$GUEST_SSH:$GUEST_DIR/responses.bin" "$RUN_DIR/responses.bin"
scp "$GUEST_SSH:$GUEST_DIR/guest-summary.json" "$RUN_DIR/guest-summary.json"
```

Then make it executable:

```bash
chmod +x targets/qemu-type2/run_existing_guest.sh
```

- [ ] **Step 5: Run CLI smoke**

Run:

```bash
RUN_DIR=/tmp/slugarch-qemu-type2-cli-smoke
rm -rf "$RUN_DIR"
cargo run -p slugarch-cli -- export-cxlmemsim targets/qemu-type2/identity_times_const.json --out "$RUN_DIR"
test -s "$RUN_DIR/requests.bin"
test -s "$RUN_DIR/expected.json"
```

Expected: `requests: 49` is printed and both files exist.

- [ ] **Step 6: Commit**

```bash
git add crates/slugarch-cli/src/main.rs targets/qemu-type2/README.md targets/qemu-type2/identity_times_const.json targets/qemu-type2/run_existing_guest.sh
git commit -m "feat: add qemu type2 target"
```

---

### Task 3: CXLMemSim Slug BAR2 Command ABI

**Files:**
- Modify: `/home/victoryang00/CXLMemSim/lib/qemu/include/hw/cxl/cxl_type2_gpu_cmd.h`
- Modify: `/home/victoryang00/CXLMemSim/qemu_integration/guest_libcuda/cxl_gpu_cmd.h`
- Modify: `/home/victoryang00/CXLMemSim/lib/qemu/include/hw/cxl/cxl_type2.h`
- Modify: `/home/victoryang00/CXLMemSim/lib/qemu/hw/cxl/cxl_type2.c`

**Interfaces:**
- Consumes: SlugArch v1 FLIT layout: byte 0 class/opcode, bytes 1-2 tag LE, bytes 3-10 addr LE, bytes 11-42 data, bytes 43-63 zero.
- Produces: BAR2 commands `0xE0..0xE4` with deterministic one-request/one-response behavior.

- [ ] **Step 1: Add command IDs to both headers**

Add to both headers' `CXLGPUCommand` enum after existing commands:

```c
    CXL_GPU_CMD_SLUG_QUERY = 0xE0,
    CXL_GPU_CMD_SLUG_RESET = 0xE1,
    CXL_GPU_CMD_SLUG_SUBMIT_FLIT = 0xE2,
    CXL_GPU_CMD_SLUG_READ_FLIT = 0xE3,
    CXL_GPU_CMD_SLUG_GET_STATS = 0xE4,
```

Add constants to both headers:

```c
#define CXL_GPU_SLUG_VERSION 1
#define CXL_GPU_SLUG_FLIT_SIZE 64
#define CXL_GPU_SLUG_QUEUE_DEPTH 16
#define CXL_GPU_SLUG_CAP_GEMM4X4 (1 << 0)
```

- [ ] **Step 2: Add bridge state to `cxl_type2.h`**

Add before `typedef struct CXLType2State`:

```c
#define CXL_TYPE2_SLUG_FLIT_SIZE 64
#define CXL_TYPE2_SLUG_QUEUE_DEPTH 16

typedef struct CXLType2SlugBridgeState {
    uint8_t matrix_a[16][16];
    uint8_t matrix_b[16][16];
    uint32_t matrix_c[16][16];
    uint8_t responses[CXL_TYPE2_SLUG_QUEUE_DEPTH][CXL_TYPE2_SLUG_FLIT_SIZE];
    uint32_t response_head;
    uint32_t response_tail;
    uint32_t response_count;
    uint64_t submitted;
    uint64_t completed;
    uint64_t failed;
    uint64_t load_count;
    uint64_t compute_count;
    uint64_t read_count;
    uint64_t bad_tag_count;
} CXLType2SlugBridgeState;
```

Add inside `CXLType2State` near `gpu_cmd`:

```c
    CXLType2SlugBridgeState slug;
```

- [ ] **Step 3: Add helper functions to `cxl_type2.c`**

Add static helpers near the GPU command implementation:

```c
static uint16_t slug_get_tag(const uint8_t flit[64])
{
    return (uint16_t)flit[1] | ((uint16_t)flit[2] << 8);
}

static uint64_t slug_get_addr(const uint8_t flit[64])
{
    uint64_t v = 0;
    for (int i = 0; i < 8; i++) {
        v |= ((uint64_t)flit[3 + i]) << (8 * i);
    }
    return v;
}

static uint32_t slug_get_data_u32(const uint8_t flit[64])
{
    return (uint32_t)flit[11] |
           ((uint32_t)flit[12] << 8) |
           ((uint32_t)flit[13] << 16) |
           ((uint32_t)flit[14] << 24);
}

static void slug_put_common(uint8_t flit[64], uint8_t cls, uint8_t opcode, uint16_t tag)
{
    memset(flit, 0, 64);
    flit[0] = (uint8_t)((cls << 4) | (opcode & 0xf));
    flit[1] = (uint8_t)(tag & 0xff);
    flit[2] = (uint8_t)(tag >> 8);
}

static void slug_enqueue_response(CXLType2State *ct2d, const uint8_t flit[64])
{
    CXLType2SlugBridgeState *s = &ct2d->slug;
    if (s->response_count == CXL_TYPE2_SLUG_QUEUE_DEPTH) {
        s->failed++;
        return;
    }
    memcpy(s->responses[s->response_tail], flit, 64);
    s->response_tail = (s->response_tail + 1) % CXL_TYPE2_SLUG_QUEUE_DEPTH;
    s->response_count++;
}

static void slug_reset(CXLType2State *ct2d)
{
    memset(&ct2d->slug, 0, sizeof(ct2d->slug));
}
```

- [ ] **Step 4: Implement request execution**

Add:

```c
static void slug_compute(CXLType2State *ct2d)
{
    CXLType2SlugBridgeState *s = &ct2d->slug;
    memset(s->matrix_c, 0, sizeof(s->matrix_c));
    for (int r = 0; r < 16; r++) {
        for (int c = 0; c < 16; c++) {
            uint32_t sum = 0;
            for (int k = 0; k < 16; k++) {
                sum += (uint32_t)s->matrix_a[r][k] * (uint32_t)s->matrix_b[k][c];
            }
            s->matrix_c[r][c] = sum;
        }
    }
}

static bool slug_reserved_zero(const uint8_t flit[64])
{
    for (int i = 43; i < 64; i++) {
        if (flit[i] != 0) {
            return false;
        }
    }
    return true;
}

static void slug_execute_request(CXLType2State *ct2d, const uint8_t req[64])
{
    CXLType2SlugBridgeState *s = &ct2d->slug;
    uint8_t cls = req[0] >> 4;
    uint8_t op = req[0] & 0xf;
    uint16_t tag = slug_get_tag(req);
    uint64_t addr = slug_get_addr(req);
    uint8_t resp[64];
    bool ok = false;

    s->submitted++;

    if (!slug_reserved_zero(req) || (addr & 0xffff) != 0x2000) {
        goto fail;
    }

    if (cls == 0x2 && op == 0x0) {
        uint32_t token = slug_get_data_u32(req);
        bool load = (token >> 2) & 1;
        bool matrix_sel = (token >> 3) & 1;
        uint8_t load_addr = (token >> 4) & 0xff;
        uint8_t load_data = (token >> 12) & 0xff;
        bool compute = (token >> 20) & 1;
        if (load && !compute) {
            uint8_t row = load_addr / 16;
            uint8_t col = load_addr % 16;
            if (matrix_sel) {
                s->matrix_b[row][col] = load_data;
            } else {
                s->matrix_a[row][col] = load_data;
            }
            s->load_count++;
            ok = true;
        } else if (!load && compute) {
            slug_compute(ct2d);
            s->compute_count++;
            ok = true;
        }
        if (ok) {
            slug_put_common(resp, 0x4, 0x0, tag);
            slug_enqueue_response(ct2d, resp);
            s->completed++;
            return;
        }
    } else if (cls == 0x1 && op == 0x0) {
        uint32_t token = (uint32_t)(addr >> 32);
        bool read = (token >> 21) & 1;
        uint8_t read_addr = (token >> 22) & 0xff;
        if (read) {
            uint8_t row = read_addr / 16;
            uint8_t col = read_addr % 16;
            uint32_t value = s->matrix_c[row][col];
            slug_put_common(resp, 0x3, 0x0, tag);
            resp[11] = (uint8_t)(value & 0xff);
            resp[12] = (uint8_t)((value >> 8) & 0xff);
            resp[13] = (uint8_t)((value >> 16) & 0xff);
            s->read_count++;
            slug_enqueue_response(ct2d, resp);
            s->completed++;
            return;
        }
    }

fail:
    s->failed++;
    slug_put_common(resp, 0x4, 0xf, tag);
    slug_enqueue_response(ct2d, resp);
}
```

- [ ] **Step 5: Wire commands into `cxl_type2_gpu_execute_cmd`**

Add switch cases:

```c
    case CXL_GPU_CMD_SLUG_QUERY:
        ct2d->gpu_cmd.results[0] = CXL_GPU_SLUG_VERSION;
        ct2d->gpu_cmd.results[1] = CXL_GPU_SLUG_FLIT_SIZE;
        ct2d->gpu_cmd.results[2] = CXL_GPU_SLUG_QUEUE_DEPTH;
        ct2d->gpu_cmd.results[3] = CXL_GPU_SLUG_CAP_GEMM4X4;
        ct2d->gpu_cmd.cmd_result = CXL_GPU_SUCCESS;
        break;

    case CXL_GPU_CMD_SLUG_RESET:
        slug_reset(ct2d);
        ct2d->gpu_cmd.cmd_result = CXL_GPU_SUCCESS;
        break;

    case CXL_GPU_CMD_SLUG_SUBMIT_FLIT:
        slug_execute_request(ct2d, ct2d->gpu_cmd.data);
        ct2d->gpu_cmd.results[0] = ct2d->slug.response_count;
        ct2d->gpu_cmd.cmd_result = CXL_GPU_SUCCESS;
        qemu_log("CXL Type2 SlugBridge: submit=%lu complete=%lu failed=%lu\n",
                 (unsigned long)ct2d->slug.submitted,
                 (unsigned long)ct2d->slug.completed,
                 (unsigned long)ct2d->slug.failed);
        break;

    case CXL_GPU_CMD_SLUG_READ_FLIT:
        if (ct2d->slug.response_count == 0) {
            ct2d->gpu_cmd.cmd_result = CXL_GPU_ERROR_NOT_READY;
        } else {
            memcpy(ct2d->gpu_cmd.data,
                   ct2d->slug.responses[ct2d->slug.response_head],
                   CXL_TYPE2_SLUG_FLIT_SIZE);
            ct2d->slug.response_head =
                (ct2d->slug.response_head + 1) % CXL_TYPE2_SLUG_QUEUE_DEPTH;
            ct2d->slug.response_count--;
            ct2d->gpu_cmd.results[0] = ct2d->slug.response_count;
            ct2d->gpu_cmd.cmd_result = CXL_GPU_SUCCESS;
        }
        break;

    case CXL_GPU_CMD_SLUG_GET_STATS:
        ct2d->gpu_cmd.results[0] = ct2d->slug.submitted;
        ct2d->gpu_cmd.results[1] = ct2d->slug.completed;
        ct2d->gpu_cmd.results[2] = ct2d->slug.failed;
        ct2d->gpu_cmd.results[3] = ct2d->slug.read_count;
        if (ct2d->gpu_cmd.data_size >= 64) {
            uint64_t *stats = (uint64_t *)ct2d->gpu_cmd.data;
            stats[0] = ct2d->slug.load_count;
            stats[1] = ct2d->slug.compute_count;
            stats[2] = ct2d->slug.read_count;
            stats[3] = ct2d->slug.bad_tag_count;
        }
        ct2d->gpu_cmd.cmd_result = CXL_GPU_SUCCESS;
        break;
```

- [ ] **Step 6: Initialize bridge state on device realize**

In the device initialization block after `memset(&ct2d->gpu_cmd, 0, ...)`, add:

```c
    slug_reset(ct2d);
```

- [ ] **Step 7: Build QEMU**

Run:

```bash
ninja -C /home/victoryang00/CXLMemSim/lib/qemu/build qemu-system-x86_64
```

Expected: PASS. If the QEMU build directory is absent, run the existing CXLMemSim QEMU build setup used in this checkout; do not create a new build system.

- [ ] **Step 8: Run Type-2 smoke**

Run:

```bash
/home/victoryang00/CXLMemSim/qemu_integration/smoke_type2_endpoint.sh
```

Expected: PASS and prints `Type2 endpoint smoke passed`.

- [ ] **Step 9: Commit CXLMemSim QEMU changes**

From `/home/victoryang00/CXLMemSim`:

```bash
git add lib/qemu/include/hw/cxl/cxl_type2_gpu_cmd.h \
  lib/qemu/include/hw/cxl/cxl_type2.h \
  lib/qemu/hw/cxl/cxl_type2.c \
  qemu_integration/guest_libcuda/cxl_gpu_cmd.h
git status --short
git commit -m "feat: add slugarch type2 bar bridge"
```

Before committing, verify `git status --short` contains only the intended files. Do not stage unrelated dirty files.

---

### Task 4: CXLMemSim Guest Helper

**Files:**
- Create: `/home/victoryang00/CXLMemSim/qemu_integration/slugarch_type2_guest.c`
- Create: `/home/victoryang00/CXLMemSim/qemu_integration/slugarch_type2_bar_run.sh`

**Interfaces:**
- Consumes: BAR2 command IDs from `guest_libcuda/cxl_gpu_cmd.h`, `requests.bin`.
- Produces: `responses.bin`, `guest-summary.json`.

- [ ] **Step 1: Create guest helper C program**

Create `/home/victoryang00/CXLMemSim/qemu_integration/slugarch_type2_guest.c` with:

```c
#define _GNU_SOURCE
#include "guest_libcuda/cxl_gpu_cmd.h"
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

#define CXL_TYPE2_VENDOR 0x8086
#define CXL_TYPE2_DEVICE 0x0d92

static const char *requests_path = "requests.bin";
static const char *responses_path = "responses.bin";
static const char *summary_path = "guest-summary.json";

static uint32_t rd32(volatile uint8_t *bar, size_t off) {
    return *(volatile uint32_t *)(bar + off);
}

static uint64_t rd64(volatile uint8_t *bar, size_t off) {
    return *(volatile uint64_t *)(bar + off);
}

static void wr32(volatile uint8_t *bar, size_t off, uint32_t v) {
    *(volatile uint32_t *)(bar + off) = v;
}

static int issue(volatile uint8_t *bar, uint32_t cmd) {
    wr32(bar, CXL_GPU_REG_CMD, cmd);
    for (int i = 0; i < 1000000; i++) {
        uint32_t st = rd32(bar, CXL_GPU_REG_CMD_STATUS);
        if (st == CXL_GPU_CMD_STATUS_COMPLETE || st == CXL_GPU_CMD_STATUS_ERROR) {
            return rd32(bar, CXL_GPU_REG_CMD_RESULT) == CXL_GPU_SUCCESS ? 0 : -1;
        }
    }
    errno = ETIMEDOUT;
    return -1;
}

static int parse_args(int argc, char **argv) {
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--requests") && i + 1 < argc) {
            requests_path = argv[++i];
        } else if (!strcmp(argv[i], "--responses") && i + 1 < argc) {
            responses_path = argv[++i];
        } else if (!strcmp(argv[i], "--summary") && i + 1 < argc) {
            summary_path = argv[++i];
        } else {
            fprintf(stderr, "usage: %s [--requests FILE] [--responses FILE] [--summary FILE]\n", argv[0]);
            return -1;
        }
    }
    return 0;
}

int main(int argc, char **argv) {
    if (parse_args(argc, argv)) return 2;
    fprintf(stderr, "device discovery must scan /sys/bus/pci/devices for 8086:0d92 before final run\n");
    return 1;
}
```

This first version intentionally fails after compiling so the next step can add discovery and MMIO safely.

- [ ] **Step 2: Build helper and verify expected fail**

Run from `/home/victoryang00/CXLMemSim/qemu_integration`:

```bash
gcc -O2 -Wall -Wextra -o /tmp/slugarch_type2_guest slugarch_type2_guest.c
/tmp/slugarch_type2_guest
```

Expected: compile PASS, runtime exits `1` with the discovery message.

- [ ] **Step 3: Implement PCI discovery and BAR2 mapping**

Replace the runtime stub with functions that:

- iterate `/sys/bus/pci/devices`,
- read `vendor` and `device`,
- match `0x8086` and `0x0d92`,
- open `resource2`,
- determine BAR2 size from `resource`,
- `mmap` BAR2 read/write shared,
- check `rd32(bar, CXL_GPU_REG_MAGIC) == CXL_GPU_MAGIC`.

Use this exact failure message on missing device:

```c
fprintf(stderr, "no CXL Type-2 device 8086:0d92 found\n");
```

- [ ] **Step 4: Implement request/response loop**

Implement:

```c
if (issue(bar, CXL_GPU_CMD_SLUG_QUERY)) die("SLUG_QUERY");
if (rd64(bar, CXL_GPU_REG_RESULT1) != CXL_GPU_SLUG_FLIT_SIZE) die("bad flit size");
if (issue(bar, CXL_GPU_CMD_SLUG_RESET)) die("SLUG_RESET");

for each 64-byte request:
    memcpy(bar + CXL_GPU_DATA_OFFSET, request, 64);
    issue(bar, CXL_GPU_CMD_SLUG_SUBMIT_FLIT);
    issue(bar, CXL_GPU_CMD_SLUG_READ_FLIT);
    memcpy(response, bar + CXL_GPU_DATA_OFFSET, 64);
```

Write `guest-summary.json`:

```json
{
  "status": "pass",
  "device": "0000:00:00.0",
  "requests": 49,
  "responses": 49,
  "slug_submitted": 49,
  "slug_completed": 49,
  "slug_failed": 0
}
```

- [ ] **Step 5: Add wrapper script**

Create `/home/victoryang00/CXLMemSim/qemu_integration/slugarch_type2_bar_run.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

REQUESTS=${1:-requests.bin}
RESPONSES=${2:-responses.bin}
SUMMARY=${3:-guest-summary.json}

gcc -O2 -Wall -Wextra -o slugarch_type2_guest slugarch_type2_guest.c
sudo ./slugarch_type2_guest --requests "$REQUESTS" --responses "$RESPONSES" --summary "$SUMMARY"
```

Run:

```bash
chmod +x /home/victoryang00/CXLMemSim/qemu_integration/slugarch_type2_bar_run.sh
```

- [ ] **Step 6: Compile helper**

Run:

```bash
cd /home/victoryang00/CXLMemSim/qemu_integration
gcc -O2 -Wall -Wextra -o /tmp/slugarch_type2_guest slugarch_type2_guest.c
bash -n slugarch_type2_bar_run.sh
```

Expected: PASS.

- [ ] **Step 7: Commit helper**

From `/home/victoryang00/CXLMemSim`:

```bash
git add qemu_integration/slugarch_type2_guest.c qemu_integration/slugarch_type2_bar_run.sh
git status --short
git commit -m "feat: add slugarch type2 guest helper"
```

Verify no unrelated dirty files are staged.

---

### Task 5: Host Harness and Local Artifact Round Trip

**Files:**
- Modify: `targets/qemu-type2/run_existing_guest.sh`
- Test: no new file; uses CLI, harness, and helper compile.

**Interfaces:**
- Consumes: `targets/qemu-type2/identity_times_const.json`, Task 2 CLI, Task 4 guest helper.
- Produces: timestamped artifact directory and `commands.txt`.

- [ ] **Step 1: Harden harness command logging**

Modify `targets/qemu-type2/run_existing_guest.sh` to append exact commands before running them:

```bash
{
    printf 'scp_requests='
    printf '%q ' scp "$RUN_DIR/requests.bin" "$CXLMEMSIM_ROOT/qemu_integration/slugarch_type2_guest.c" "$GUEST_SSH:$GUEST_DIR/"
    printf '\n'
    printf 'guest_build_run='
    printf '%q ' "$GUEST_SSH" "cd '$GUEST_DIR' && gcc -O2 -Wall -Wextra -o slugarch_type2_guest slugarch_type2_guest.c && sudo ./slugarch_type2_guest --requests requests.bin --responses responses.bin --summary guest-summary.json"
    printf '\n'
} >>"$RUN_DIR/commands.txt"
```

- [ ] **Step 2: Add preflight checks**

Add checks:

```bash
command -v scp >/dev/null || { echo "scp not found" >&2; exit 1; }
command -v cargo >/dev/null || true
test -r "$RUN_DIR/requests.bin" || { echo "requests.bin is not readable" >&2; exit 1; }
```

- [ ] **Step 3: Local export preflight**

Run:

```bash
RUN_DIR=/tmp/slugarch-qemu-type2-artifact
rm -rf "$RUN_DIR"
cargo run -p slugarch-cli -- export-cxlmemsim targets/qemu-type2/identity_times_const.json --out "$RUN_DIR"
bash -n targets/qemu-type2/run_existing_guest.sh
test -s "$RUN_DIR/requests.bin"
test -s "$RUN_DIR/expected.json"
```

Expected: PASS.

- [ ] **Step 4: Commit harness hardening**

```bash
git add targets/qemu-type2/run_existing_guest.sh
git commit -m "chore: harden qemu type2 target harness"
```

---

### Task 6: Live End-to-End CXLMemSim QEMU Type-2 Proof

**Files:**
- No source changes expected.
- Artifact output under `artifact/slugarch_cxlmemsim/<UTC-run-id>/`.

**Interfaces:**
- Consumes: built QEMU with Slug bridge, running CXLMemSim Type-2 guest with SSH, SlugArch CLI.
- Produces: evidence artifact directory.

- [ ] **Step 1: Verify QEMU Type-2 smoke**

Run:

```bash
/home/victoryang00/CXLMemSim/qemu_integration/smoke_type2_endpoint.sh
```

Expected: `Type2 endpoint smoke passed`.

- [ ] **Step 2: Confirm guest is reachable**

Run with the actual guest address:

```bash
CXLMEMSIM_GUEST_SSH="ssh root@GUEST"
$CXLMEMSIM_GUEST_SSH 'uname -a; test -d /sys/bus/pci/devices'
```

Expected: SSH succeeds.

- [ ] **Step 3: Export requests**

Run from `/root/Concordia/SlugArch`:

```bash
RUN_DIR=artifact/slugarch_cxlmemsim/$(date -u +%Y%m%d-%H%M%S)
cargo run -p slugarch-cli -- export-cxlmemsim targets/qemu-type2/identity_times_const.json --out "$RUN_DIR"
```

Expected: `requests: 49`.

- [ ] **Step 4: Run guest BAR2 helper**

Run:

```bash
CXLMEMSIM_GUEST_SSH="ssh root@GUEST" \
CXLMEMSIM_ROOT=/home/victoryang00/CXLMemSim \
targets/qemu-type2/run_existing_guest.sh "$RUN_DIR"
```

Expected: `responses.bin` and `guest-summary.json` are copied back into `$RUN_DIR`.

- [ ] **Step 5: Validate responses**

Run:

```bash
cargo run -p slugarch-cli -- validate-cxlmemsim \
  targets/qemu-type2/identity_times_const.json \
  --responses "$RUN_DIR/responses.bin" \
  --out "$RUN_DIR"
```

Expected: `status: pass`.

- [ ] **Step 6: Capture logs**

Copy the relevant QEMU and CXLMemSim logs into the run directory. If using the standard smoke/launch paths:

```bash
cp /home/victoryang00/CXLMemSim/build/type2-smoke/qemu-type2-smoke.log "$RUN_DIR/qemu.log" 2>/dev/null || true
cp /home/victoryang00/CXLMemSim/build/type2-smoke/cxlmemsim-server.log "$RUN_DIR/cxlmemsim-server.log" 2>/dev/null || true
```

If the guest was launched by another script, copy its actual QEMU/server logs and record their source paths in `commands.txt`.

- [ ] **Step 7: Verify evidence**

Run:

```bash
test -s "$RUN_DIR/requests.bin"
test -s "$RUN_DIR/responses.bin"
test -s "$RUN_DIR/summary.json"
test -s "$RUN_DIR/summary.csv"
python3 -m json.tool "$RUN_DIR/summary.json" >/dev/null
rg -n '"status": "pass"|CXL Type2 SlugBridge|CXL Type2: Device realized' "$RUN_DIR" || true
```

Expected:

- `summary.json` contains `"status": "pass"`.
- `responses.bin` is exactly `3136` bytes.
- QEMU log includes `CXL Type2 SlugBridge` when available.

- [ ] **Step 8: Commit SlugArch evidence manifest only with user approval**

Do not commit raw logs by default. If a durable manifest is needed, create:

```text
artifact/slugarch_cxlmemsim/<run-id>/MANIFEST.txt
```

with artifact hashes and command lines, then ask before committing artifacts.

---

## Plan Self-Review

**Spec coverage:** The plan covers SlugArch export/validation, BAR2 command IDs, QEMU endpoint model, guest helper, `targets/qemu-type2`, host harness, error handling, and live evidence. It explicitly avoids Quartus and stale Type-2 scripts.

**Placeholder scan:** No task uses deferred-work placeholder language. Environment-specific values use explicit variables such as `CXLMEMSIM_GUEST_SSH="ssh root@GUEST"`.

**Type consistency:** Public Rust names are defined in Task 1 and consumed in Task 2. BAR command names and IDs are defined in Task 3 and consumed in Task 4. The artifact filenames match the design spec.
