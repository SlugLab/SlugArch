# SlugArch Type-2 CXL.mem Campaign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the SlugArch-side guest benchmark, durable 4-latency-by-5-boot campaign runner, evidence validator, and deterministic Figure 2 data export for the approved QEMU Type-2 CXL.mem/CXLMemSim experiment.

**Architecture:** Preserve the July BAR2 helper and its evidence unchanged. Extract the current Rust replay policy into a lightweight shared crate, use it in a statically linked guest benchmark that accesses only a validated devdax work window, and orchestrate fresh server/QEMU boots from a standard-library Python package. The runner durably registers, arms, validates, seals, and normalizes a campaign; no timing value is eligible until the complete 20-slot campaign and its registry/checksum chain verify.

**Tech Stack:** Rust 2021, Cargo, `serde`, `serde_json`, `bincode`, `libc`, `sha2`, Python 3 standard library, QEMU TCG, CXLMemSim TCP SLT2 v1 protocol, Linux devdax, SSH/SCP, JSON/JSONL/CSV, gzip, SHA-256, CRC32C, `flock`, and `fsync`.

---

## Scope and Preconditions

This plan implements the SlugArch repository side of
`docs/superpowers/specs/2026-07-24-slugarch-type2-cxlmem-experiment-design.md`.
It does not implement the companion CXLMemSim server or QEMU patches. Before
Task 14 starts, the companion implementation must provide these exact
interfaces and paths:

- QEMU binary:
  `/tmp/slugarch-type2-cxlmem-build/qemu/qemu-system-x86_64`
- CXLMemSim server:
  `/tmp/slugarch-type2-cxlmem-build/cxlmemsim_server`
- QEMU Type-2 properties:
  `sync-type2-wire=on`, `type2-wire-version=1`, and
  a `slugarch-event-log` property that accepts the runner’s absolute
  attempt-local JSONL path
- QEMU QOM observability:
  a writable `slugarch-phase-id` string plus read-only completed-operation,
  byte, failure, direct-CFMWS, aggregate BAR4-overlay, bulk-overlay,
  coherent-pool, local-shadow, and local-cache counters, all accessible
  through QMP `qom-get`/`qom-set` under the exact `slugarch-*` property names
  enumerated in Task 7 of the transport plan
- Server arguments:
  `--slugarch-type2-protocol=true`,
  `--slugarch-event-log` followed by an absolute attempt-local gzip JSONL
  path, and `--slugarch-shm-name` followed by a unique attempt-local POSIX
  shared-memory name
- QEMU completion JSONL fields:
  `client_id`, `request_id`, `server_sequence`, `operation`, `dpa`,
  `length`, `payload_sha256`, `status`, `returned_modeled_latency_ns`,
  `requested_delay_ns`, `applied_delay_ns`, `delay_overshoot_ns`,
  `delay_undershot`, `path`, and `phase_id`
- Server completion JSONL fields:
  `server_instance_id`, `client_role`, `client_id`, `request_id`,
  `server_sequence`, `operation`, `dpa`, `length`, `payload_sha256`,
  `status`, `configured_base_latency_ns`, `modeled_latency_ns`,
  `receive_monotonic_ns`, and `backend_complete_monotonic_ns`
- QEMU path-counter JSONL fields:
  `phase_id`, `direct_cfmws`, `bar4_overlay`, `local_shadow`,
  `local_cache`, `bulk_overlay`, and `coherent_pool`

Use these frozen current inputs:

- kernel: `/tmp/slugarch-type2-cxlmem-build/bzImage`
- base disk: `/tmp/slugarch-type2-cxlmem-build/slugarch-type2.img`
- reference job: `targets/qemu-type2/identity_times_const.json`
- design spec:
  `docs/superpowers/specs/2026-07-24-slugarch-type2-cxlmem-experiment-design.md`
- artifact root:
  `/root/Concordia/SlugArch/artifact/slugarch_type2_cxlmem`
- QEMU host CPUs: `0-3`
- server host CPUs: `4-5`
- guest benchmark CPU: vCPU `1`
- server TCP port: `10199`
- guest SSH port: `12022`

The runner must create a private qcow2 overlay and a fresh, uniquely named,
zeroed 256 MiB POSIX shared-memory backing object for every attempt under the
ownership of `runtime_root / attempt_id`. It must never boot the published
base image read-write or attach to a pre-existing shared-memory object.

## Existing Code to Reuse Without Widening Claims

- `crates/slugarch-host/src/dispatch.rs::build_gemm_dispatch_stream` is the
  source of the 49-request GEMM stream.
- `crates/slugarch-host/src/qemu_type2.rs::{export_requests,validate_responses}`
  provides the existing host export and response-validation boundary.
- `crates/slugarch-host/src/result.rs::decode_results` defines the expected
  4-by-4 matrix result.
- `crates/slugarch-host/src/replay.rs` defines the current validation, delta,
  and full policies.
- `targets/qemu-type2/slugarch_type2_guest.c::handle_request` is the behavioral
  reference for the guest-software GEMM interpreter.
- `crates/slugarch-host/tests/qemu_type2_artifacts.rs` supplies the existing
  bad-tag, missing, duplicate, wrong-data, and wrong-phase negative patterns.
- `targets/qemu-type2/run_live_knob_sweep.sh` supplies only launch/SSH/cleanup
  ideas. It is not a campaign runner because it runs five samples in one boot,
  uses a 64 MiB all-bypass aperture, can attach to an existing server, and
  writes the base disk directly.

Do not modify or reuse the July BAR2 artifact directories. Do not modify
`targets/qemu-type2/slugarch_type2_guest.c`; Figure 1 must continue to point to
the legacy helper and its saved evidence.

## Normative Count Vocabulary

The existing exporter emits 49 request FLITs. The existing SlugArch replay
recorder records both the request and its response, so a one-copy run contains
98 boundary records. The new data model must carry both values:

```text
request_record_count  = 49 * copy_count
response_record_count = request_record_count
boundary_record_count = request_record_count + response_record_count
```

The Figure 2 scale labels `49`, `196`, `784`, and `3,136` mean
`request_record_count`. Every machine-readable row also carries
`boundary_record_count` with values `98`, `392`, `1,568`, and `6,272`.
No code may call 49 requests “98 requests” or call 98 boundary records “49
boundary records.”

## File Map

### Rust replay core and guest

- Create `crates/slugarch-cxl-replay/Cargo.toml`
- Create `crates/slugarch-cxl-replay/src/lib.rs`
- Create `crates/slugarch-cxl-replay/tests/compat.rs`
- Create `crates/slugarch-type2-guest/Cargo.toml`
- Create `crates/slugarch-type2-guest/src/lib.rs`
- Create `crates/slugarch-type2-guest/src/main.rs`
- Create `crates/slugarch-type2-guest/src/schema.rs`
- Create `crates/slugarch-type2-guest/src/gemm.rs`
- Create `crates/slugarch-type2-guest/src/dax.rs`
- Create `crates/slugarch-type2-guest/src/measure.rs`
- Create `crates/slugarch-type2-guest/tests/offline.rs`
- Modify `Cargo.toml`
- Modify `crates/slugarch-host/Cargo.toml`
- Modify `crates/slugarch-host/src/replay.rs`
- Modify `crates/slugarch-host/src/qemu_type2.rs`
- Modify `crates/slugarch-host/tests/qemu_type2_artifacts.rs`
- Modify `crates/slugarch-cli/src/main.rs`

### Python campaign package

- Create `targets/qemu-type2/cxlmem_campaign/__init__.py`
- Create `targets/qemu-type2/cxlmem_campaign/contract.py`
- Create `targets/qemu-type2/cxlmem_campaign/provenance.py`
- Create `targets/qemu-type2/cxlmem_campaign/artifacts.py`
- Create `targets/qemu-type2/cxlmem_campaign/registry.py`
- Create `targets/qemu-type2/cxlmem_campaign/wire.py`
- Create `targets/qemu-type2/cxlmem_campaign/launch.py`
- Create `targets/qemu-type2/cxlmem_campaign/guest_session.py`
- Create `targets/qemu-type2/cxlmem_campaign/runner.py`
- Create `targets/qemu-type2/cxlmem_campaign/validate.py`
- Create `targets/qemu-type2/cxlmem_campaign/normalize.py`
- Create `targets/qemu-type2/cxlmem_campaign/cli.py`
- Create `targets/qemu-type2/run_cxlmem_campaign.py`
- Create `targets/qemu-type2/cxlmem-campaign-defaults.json`
- Create `targets/qemu-type2/tests/campaign_fixtures.py`
- Create `targets/qemu-type2/tests/test_contract.py`
- Create `targets/qemu-type2/tests/test_provenance.py`
- Create `targets/qemu-type2/tests/test_artifacts.py`
- Create `targets/qemu-type2/tests/test_registry.py`
- Create `targets/qemu-type2/tests/test_wire.py`
- Create `targets/qemu-type2/tests/test_runner_state.py`
- Create `targets/qemu-type2/tests/test_validation.py`
- Create `targets/qemu-type2/tests/test_normalize.py`
- Modify `targets/qemu-type2/README.md`
- Modify `.gitignore`

### Runtime outputs

These are generated, never hand-edited, and never staged:

```text
artifact/slugarch_type2_cxlmem/
  .campaign.lock
  campaign-registry.jsonl
  pilots/
  .{campaign_id}.inprogress/
  {campaign_id}/
  {campaign_id}.failed/
  exports/{campaign_id}-{campaign_checksum}/
```

The paper-facing export is generated as:

```text
artifact/slugarch_type2_cxlmem/exports/{campaign_id}-{campaign_checksum}/
  observations.csv
  observations.json
  slugarch-type2-cxlmem.json
  checksums.sha256
  EXPORT_COMPLETE
```

The export is outside the finalized campaign directory because finalized
campaigns are immutable. It contains and verifies the finalized campaign hash.

---

## Task 1: Extract the Lightweight Replay Policy Core

**Files:**
- Create: `crates/slugarch-cxl-replay/Cargo.toml`
- Create: `crates/slugarch-cxl-replay/src/lib.rs`
- Create: `crates/slugarch-cxl-replay/tests/compat.rs`
- Modify: `Cargo.toml`
- Modify: `crates/slugarch-host/Cargo.toml`
- Modify: `crates/slugarch-host/src/replay.rs`
- Modify: `crates/slugarch-host/src/lib.rs`

The new crate owns `CxlRecordMode`, `CxlDirection`, `CxlEndpoint`,
`CxlTransactionClass`, `PayloadCapture`, `CxlRecordPolicy`,
`CxlReplayRecord`, `CxlReplayArtifact`, `CxlReplaySummary`,
`CxlReplayValidation`, and `CxlTraceRecorder`. The host crate keeps
`CxlRecordedRun` and the adapter that derives a final commitment from
`GemmResult`.

- [ ] **Step 1: Write the compatibility test before moving code**

Create `crates/slugarch-cxl-replay/tests/compat.rs` with a complete golden
boundary test:

```rust
use slugarch_cxl_replay::{
    CxlDirection, CxlRecordMode, CxlRecordPolicy, CxlTraceRecorder,
};
use slugarch_cxl_wire::{CxlMsg, M2SRwDOp, S2MNDROp};
use slugarch_ir::types::IpId;

#[test]
fn one_49_request_trace_has_98_boundary_records() {
    let policy = CxlRecordPolicy {
        mode: CxlRecordMode::Validation,
        endpoint: IpId::SlugCxl4x4,
    };
    let mut recorder = CxlTraceRecorder::new(policy);
    for request_index in 0..49usize {
        let request = CxlMsg::M2SRwD {
            tag: request_index as u16,
            opcode: M2SRwDOp::MemWr,
            addr: 0x2000,
            data: [0u8; 32],
        };
        let response = CxlMsg::S2MNDR {
            tag: request_index as u16,
            opcode: S2MNDROp::Cmp,
        };
        recorder.record_gemm_msg(
            request_index,
            CxlDirection::HostToDevice,
            &request,
        );
        recorder.record_gemm_msg(
            request_index,
            CxlDirection::DeviceToHost,
            &response,
        );
    }
    let artifact = recorder.finish_with_commitment(0x534c_5547);
    assert_eq!(artifact.summary.record_count, 98);
    assert_eq!(artifact.summary.application_flit_bytes, 98 * 64);
    assert_eq!(artifact.records.first().unwrap().sequence, 0);
    assert_eq!(artifact.records.last().unwrap().sequence, 97);
}
```

- [ ] **Step 2: Run the new test and verify RED**

Run:

```bash
cargo test -p slugarch-cxl-replay --test compat
```

Expected: Cargo reports that package `slugarch-cxl-replay` does not exist.

- [ ] **Step 3: Add the crate manifests**

Add `"crates/slugarch-cxl-replay"` to the root workspace member list. Add
`libc = "0.2"` and `sha2 = "0.10"` to `[workspace.dependencies]`. Task 3
adds the guest member when its manifest exists.

Create `crates/slugarch-cxl-replay/Cargo.toml`:

```toml
[package]
name = "slugarch-cxl-replay"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
bincode.workspace = true
serde.workspace = true
slugarch-cxl-wire = { path = "../slugarch-cxl-wire" }
slugarch-ir = { path = "../slugarch-ir" }
```

Add this dependency to `crates/slugarch-host/Cargo.toml`:

```toml
slugarch-cxl-replay = { path = "../slugarch-cxl-replay" }
```

- [ ] **Step 4: Move policy-neutral code and add stable stream helpers**

Move the policy-neutral definitions without changing their serde shapes or
payload algorithms. Add these complete helpers to
`crates/slugarch-cxl-replay/src/lib.rs`:

```rust
pub fn serialize_records(records: &[CxlReplayRecord]) -> Result<Vec<u8>, String> {
    bincode::serialize(records).map_err(|error| error.to_string())
}

pub fn deserialize_records(bytes: &[u8]) -> Result<Vec<CxlReplayRecord>, String> {
    bincode::deserialize(bytes).map_err(|error| error.to_string())
}
```

Change the recorder completion interface to:

```rust
impl CxlTraceRecorder {
    pub fn finish_with_commitment(
        self,
        final_result_commitment: u64,
    ) -> CxlReplayArtifact {
        let summary = summarize(&self.records);
        CxlReplayArtifact {
            policy: self.policy,
            records: self.records,
            final_result_commitment,
            summary,
        }
    }
}
```

Make `CxlTraceRecorder::{new,record_gemm_msg}` public. Add
`record_gemm_msg_in_namespace(copy_index, request_index, direction, message)`;
the original method delegates to namespace zero. The namespaced method uses
epoch `copy_index * 4 + gemm_epoch(request_index)`, while sequence numbers
remain globally increasing. Preserve the current FNV-1a payload hash, delta
representation, provenance strings, record byte accounting, and equality
behavior exactly.

- [ ] **Step 5: Convert the host module into an adapter**

In `crates/slugarch-host/src/replay.rs`, publicly re-export the shared types:

```rust
pub use slugarch_cxl_replay::{
    deserialize_records, serialize_records, CxlDirection, CxlEndpoint,
    CxlRecordMode, CxlRecordPolicy, CxlReplayArtifact, CxlReplayRecord,
    CxlReplaySummary, CxlReplayValidation, CxlTraceRecorder,
    CxlTransactionClass, PayloadCapture,
};
```

Retain `CxlRecordedRun` and `result_commitment`. Change the host completion
call to:

```rust
let artifact = recorder.finish_with_commitment(result_commitment(result));
```

Keep the exports from `crates/slugarch-host/src/lib.rs` source-compatible.

- [ ] **Step 6: Run compatibility and existing host tests**

Run:

```bash
cargo test -p slugarch-cxl-replay

env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --lib replay

env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test sim_feasible replay_metadata_reports_all_modes_and_boundaries
```

Expected: all three commands pass; the existing host test still reports 98
records for one GEMM trace.

- [ ] **Step 7: Commit the replay-core extraction**

Run:

```bash
git add Cargo.toml Cargo.lock \
  crates/slugarch-cxl-replay \
  crates/slugarch-host/Cargo.toml \
  crates/slugarch-host/src/replay.rs \
  crates/slugarch-host/src/lib.rs
git commit -m "refactor: share SlugArch CXL replay policy"
```

## Task 2: Add Scaled Trace Export and Independent Response Validation

**Files:**
- Modify: `crates/slugarch-host/src/qemu_type2.rs`
- Modify: `crates/slugarch-host/tests/qemu_type2_artifacts.rs`
- Modify: `crates/slugarch-cli/src/main.rs`

The host must remain an independent checker of the guest-produced response
files. Tags are namespaced as `copy_index * 49 + local_tag`; the maximum tag is
3,135 and therefore fits in `u16`.

- [ ] **Step 1: Write failing scaled-count and namespace tests**

Add:

```rust
#[test]
fn scaled_export_distinguishes_requests_from_boundary_records() {
    let expected = expected_scaled_for_job(&job(), 64).unwrap();
    assert_eq!(expected.copy_count, 64);
    assert_eq!(expected.request_record_count, 3_136);
    assert_eq!(expected.response_record_count, 3_136);
    assert_eq!(expected.boundary_record_count, 6_272);
    assert_eq!(expected.request_tags.first().copied(), Some(0));
    assert_eq!(expected.request_tags.last().copied(), Some(3_135));
}

#[test]
fn scaled_validator_rejects_a_cross_copy_tag() {
    let dir = temp_dir("scaled-cross-copy-tag");
    let mut responses = known_good_scaled_responses(4);
    responses[49].set_tag_for_test(0);
    fs::write(dir.join("responses.bin"), encode_messages(&responses)).unwrap();
    let summary =
        validate_scaled_responses(&job(), &dir.join("responses.bin"), &dir, 4)
            .unwrap();
    assert_eq!(summary.status, "fail");
    assert_eq!(summary.tag_mismatches, 1);
}
```

Add a test-only `set_tag_for_test` helper in the test module by reconstructing
the enum variant; do not add a mutation API to `slugarch-cxl-wire`.

- [ ] **Step 2: Run one test and verify RED**

Run:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test qemu_type2_artifacts \
  scaled_export_distinguishes_requests_from_boundary_records
```

Expected: unresolved imports for `expected_scaled_for_job`.

- [ ] **Step 3: Add the scaled expected schema and builder**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QemuType2ScaledExpected {
    pub workload: String,
    pub copy_count: usize,
    pub request_record_count: usize,
    pub response_record_count: usize,
    pub boundary_record_count: usize,
    pub flit_bytes: usize,
    pub request_tags: Vec<u16>,
    pub expected_c_per_copy: Vec<[[u32; 4]; 4]>,
}

pub fn expected_scaled_for_job(
    job: &GemmJob,
    copy_count: usize,
) -> Result<QemuType2ScaledExpected, HostError> {
    if !matches!(copy_count, 1 | 4 | 16 | 64) {
        return Err(HostError::DispatchFailed {
            tag: 0,
            reason: format!("unsupported trace copy count {copy_count}"),
        });
    }
    let request_record_count = 49 * copy_count;
    let request_tags = (0..request_record_count)
        .map(|tag| tag as u16)
        .collect::<Vec<_>>();
    Ok(QemuType2ScaledExpected {
        workload: "slugcxl_gemm_4x4".to_string(),
        copy_count,
        request_record_count,
        response_record_count: request_record_count,
        boundary_record_count: request_record_count * 2,
        flit_bytes: FLIT_BYTES,
        request_tags,
        expected_c_per_copy: vec![matmul(job); copy_count],
    })
}
```

Add `export_scaled_requests` by calling
`build_gemm_dispatch_stream(job, (copy_index * 49) as u16)` for every copy.
Write `requests.bin` and `expected.json`, and reject all copy counts except
1, 4, 16, and 64.

- [ ] **Step 4: Add scaled response validation**

Implement:

```rust
pub fn validate_scaled_responses(
    job: &GemmJob,
    responses_path: &Path,
    out_dir: &Path,
    copy_count: usize,
) -> Result<QemuType2Summary, HostError>
```

Split decoded responses into 49-response copies. For each copy, require 33
completion responses, 16 data responses, exact namespaced tags, and the
expected matrix. Aggregate counts into the existing summary without weakening
the one-copy validator.

- [ ] **Step 5: Add exact CLI commands**

Add:

```text
slugarch export-cxlmemsim-scaled targets/qemu-type2/identity_times_const.json \
  --copies 4 --out /tmp/slugarch-scaled-export
slugarch validate-cxlmemsim-scaled \
  targets/qemu-type2/identity_times_const.json \
  --copies 4 \
  --responses /tmp/slugarch-scaled-export/responses.bin \
  --out /tmp/slugarch-scaled-validation
```

The validation command exits nonzero when `summary.status != "pass"`.

- [ ] **Step 6: Run all artifact tests and freeze the 49-request trace**

Run:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test qemu_type2_artifacts

rm -rf /tmp/slugarch-trace-check
cargo run -q -p slugarch-cli -- export-cxlmemsim-scaled \
  targets/qemu-type2/identity_times_const.json \
  --copies 1 \
  --out /tmp/slugarch-trace-check
test "$(stat -c %s /tmp/slugarch-trace-check/requests.bin)" -eq 3136
sha256sum /tmp/slugarch-trace-check/requests.bin
```

Expected: all host artifact tests pass; the final command prints:

```text
f9f05b04d9352de8e0213c42e5efb46f56b05863e077d9cf1ce47a9ddef2b75c  /tmp/slugarch-trace-check/requests.bin
```

- [ ] **Step 7: Commit scaled trace support**

Run:

```bash
git add crates/slugarch-host/src/qemu_type2.rs \
  crates/slugarch-host/tests/qemu_type2_artifacts.rs \
  crates/slugarch-cli/src/main.rs
git commit -m "feat: validate scaled Type-2 GEMM traces"
```

## Task 3: Define the Guest Schema and Fixed Condition Plan

**Files:**
- Create: `crates/slugarch-type2-guest/Cargo.toml`
- Create: `crates/slugarch-type2-guest/src/lib.rs`
- Create: `crates/slugarch-type2-guest/src/schema.rs`
- Create: `crates/slugarch-type2-guest/src/main.rs`
- Create: `crates/slugarch-type2-guest/tests/offline.rs`

- [ ] **Step 1: Write the condition-count and slot-layout tests**

Create:

```rust
use slugarch_type2_guest::{
    build_condition_plan, build_slot_layout, Mode, PassKind, MIB,
};

#[test]
fn timed_plan_has_20_conditions_and_exact_count_vocabulary() {
    let plan = build_condition_plan(3, PassKind::Timed);
    assert_eq!(plan.len(), 20);
    let full = plan
        .iter()
        .find(|condition| {
            condition.mode == Some(Mode::Full)
                && condition.request_record_count == Some(3_136)
        })
        .unwrap();
    assert_eq!(full.response_record_count, Some(3_136));
    assert_eq!(full.boundary_record_count, Some(6_272));
}

#[test]
fn work_window_has_21_non_overlapping_one_mib_slots() {
    let slots = build_slot_layout(80 * MIB);
    let expected_names = [
        "control",
        "calibration",
        "transfer-004096",
        "transfer-065536",
        "transfer-1048576",
        "slugarch-00049-baseline",
        "slugarch-00049-validation",
        "slugarch-00049-delta",
        "slugarch-00049-full",
        "slugarch-00196-baseline",
        "slugarch-00196-validation",
        "slugarch-00196-delta",
        "slugarch-00196-full",
        "slugarch-00784-baseline",
        "slugarch-00784-validation",
        "slugarch-00784-delta",
        "slugarch-00784-full",
        "slugarch-03136-baseline",
        "slugarch-03136-validation",
        "slugarch-03136-delta",
        "slugarch-03136-full",
    ];
    assert_eq!(slots.len(), 21);
    assert_eq!(
        slots.iter().map(|slot| slot.name).collect::<Vec<_>>(),
        expected_names,
    );
    assert_eq!(slots[0].start_dpa, 80 * MIB);
    assert_eq!(slots.last().unwrap().end_dpa, 101 * MIB);
    assert!(slots.iter().all(|slot| slot.end_dpa - slot.start_dpa == MIB));
    for pair in slots.windows(2) {
        assert_eq!(pair[0].end_dpa, pair[1].start_dpa);
    }
    assert!(slots.last().unwrap().end_dpa <= 112 * MIB);
}

#[test]
fn maximum_transfer_fits_subtraction_safely_inside_its_slot() {
    let slots = build_slot_layout(80 * MIB);
    let transfer_1m = &slots[4];
    assert!(transfer_1m.contains(transfer_1m.start_dpa, MIB));
    assert!(!transfer_1m.contains(transfer_1m.start_dpa, MIB + 1));
    assert!(!transfer_1m.contains(transfer_1m.end_dpa, 1));
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test -p slugarch-type2-guest --test offline
```

Expected: package or imported interfaces are missing.

- [ ] **Step 3: Add the guest crate manifest**

Create:

```toml
[package]
name = "slugarch-type2-guest"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
bincode.workspace = true
libc.workspace = true
serde.workspace = true
serde_json.workspace = true
sha2.workspace = true
slugarch-cxl-replay = { path = "../slugarch-cxl-replay" }
slugarch-cxl-wire = { path = "../slugarch-cxl-wire" }

[dev-dependencies]
slugarch-host = { path = "../slugarch-host" }
```

Add `"crates/slugarch-type2-guest"` to the root workspace member list in this
same step.

- [ ] **Step 4: Define exact guest output types**

In `schema.rs`, define serde structs with `schema_version: 1`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CountVocabulary {
    pub copy_count: u32,
    pub request_record_count: u32,
    pub response_record_count: u32,
    pub boundary_record_count: u32,
}

impl CountVocabulary {
    pub fn for_copies(copy_count: u32) -> Result<Self, String> {
        if !matches!(copy_count, 1 | 4 | 16 | 64) {
            return Err(format!("unsupported copy count {copy_count}"));
        }
        let request_record_count = 49 * copy_count;
        Ok(Self {
            copy_count,
            request_record_count,
            response_record_count: request_record_count,
            boundary_record_count: request_record_count * 2,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageTimings {
    pub encode_ns: u64,
    pub cxl_write_ns: u64,
    pub cxl_read_ns: u64,
    pub interpret_ns: u64,
    pub validate_ns: u64,
    pub end_to_end_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeclaredTraffic {
    pub reads: u64,
    pub read_bytes: u64,
    pub writes: u64,
    pub written_bytes: u64,
}
```

Define tagged `GuestEvent` variants for `Boot`, `Topology`, `Ready`, `Done`,
`ArmRequest`, `ConditionResult`, `CorruptionResult`, and `Fatal`. Every event
contains `campaign_id`, `attempt_id`, `guest_boot_uuid`, and a monotonically
increasing `guest_event_sequence`.

- [ ] **Step 5: Encode the fixed condition and mode orders**

Implement constants for transfer sizes `4_096`, `65_536`, and `1_048_576`;
copy counts `1`, `4`, `16`, and `64`; and modes `Baseline`, `Validation`,
`Delta`, and `Full`.

`build_condition_plan(replicate, pass)` returns this order:

1. calibration;
2. transfers in ascending size;
3. trace sizes in ascending request-record count;
4. modes in the approved replicate-specific rotation.

It returns 20 conditions in warmup and 20 in timed mode. The post-transport
negative test is separate and always uses the 49-request full-mode slot.

Allocate slots in this exact order:

```text
control
calibration
transfer-004096
transfer-065536
transfer-1048576
slugarch-00049-baseline
slugarch-00049-validation
slugarch-00049-delta
slugarch-00049-full
slugarch-00196-baseline
slugarch-00196-validation
slugarch-00196-delta
slugarch-00196-full
slugarch-00784-baseline
slugarch-00784-validation
slugarch-00784-delta
slugarch-00784-full
slugarch-03136-baseline
slugarch-03136-validation
slugarch-03136-delta
slugarch-03136-full
```

Every slot is exactly 1 MiB. Slot `i` covers
`[80 MiB + i * 1 MiB, 80 MiB + (i + 1) * 1 MiB)`, so the complete map is
`[80 MiB,101 MiB)` inside the frozen `[80 MiB,112 MiB)` work window.
`control` holds only sentinel/control bytes; calibration uses at most 256 KiB;
each transfer reuses its named slot sequentially for its logical write/read;
and each SlugArch slot stores that condition's request and response layouts
together. Baseline records are 64 bytes, but validation/delta/full use the
production replay serializer and therefore must not be sized as
`record_count * 64`. `Slot::contains(dpa, length)` must check `dpa >= start`,
`dpa <= end`, and `length <= end - dpa` without addition overflow. Task 6
derives and freezes every request/response layout from the actual serialized
buffers. The corruption check reuses the 49-request full slot only after its
timed condition has completed.

- [ ] **Step 6: Add a thin binary entry point**

`main.rs` calls `slugarch_type2_guest::run_from_env_and_args()` and exits with
code 1 after emitting a `Fatal` event when that function returns an error.
It must not print unstructured text to stdout.

- [ ] **Step 7: Run schema and plan tests**

Run:

```bash
cargo test -p slugarch-type2-guest --test offline \
  timed_plan_has_20_conditions_and_exact_count_vocabulary
cargo test -p slugarch-type2-guest --test offline \
  work_window_has_21_non_overlapping_one_mib_slots
```

Expected: both commands pass.

- [ ] **Step 8: Commit the guest schema**

Run:

```bash
git add crates/slugarch-type2-guest Cargo.toml Cargo.lock
git commit -m "feat: define Type-2 guest experiment schema"
```

## Task 4: Implement the Deterministic Guest GEMM Interpreter

**Files:**
- Create: `crates/slugarch-type2-guest/src/gemm.rs`
- Modify: `crates/slugarch-type2-guest/src/lib.rs`
- Modify: `crates/slugarch-type2-guest/tests/offline.rs`

- [ ] **Step 1: Write golden interpreter and copy-namespace tests**

Add tests that load a 49-request trace produced by
`slugarch_host::qemu_type2::export_requests` in the test process:

```rust
#[test]
fn interpreter_matches_the_identity_times_const_result() {
    let requests = fixture_requests();
    let responses = interpret_scaled_trace(&requests, 1).unwrap();
    assert_eq!(responses.len(), 49);
    assert_eq!(
        decode_copy_result(&responses).unwrap(),
        [
            [2, 3, 4, 5],
            [6, 7, 8, 9],
            [10, 11, 12, 13],
            [14, 15, 16, 17],
        ]
    );
}

#[test]
fn sixty_four_copies_use_unique_tags_and_reset_state() {
    let requests = fixture_requests();
    let responses = interpret_scaled_trace(&requests, 64).unwrap();
    assert_eq!(responses.len(), 3_136);
    let tags = responses.iter().map(CxlMsg::tag).collect::<BTreeSet<_>>();
    assert_eq!(tags.len(), 3_136);
    assert_eq!(responses.first().unwrap().tag(), 0);
    assert_eq!(responses.last().unwrap().tag(), 3_135);
    for copy in responses.chunks_exact(49) {
        assert_eq!(decode_copy_result(copy).unwrap()[3], [14, 15, 16, 17]);
    }
}
```

Use `slugarch-host` only as a dev-dependency for fixture generation. Production
guest code depends only on `slugarch-cxl-wire` and `slugarch-cxl-replay`.

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
cargo test -p slugarch-type2-guest --test offline interpreter_matches
```

Expected: unresolved `interpret_scaled_trace`.

- [ ] **Step 3: Port the interpreter behavior**

Implement `GemmInterpreter` with 16-by-16 `a`, `b`, and `c` arrays and the same
token decoding as `slugarch_type2_guest.c::handle_request`. Expose:

```rust
pub fn interpret_scaled_trace(
    base_requests: &[CxlMsg],
    copy_count: usize,
) -> Result<Vec<CxlMsg>, String>
```

For each copy:

1. instantiate a fresh `GemmInterpreter`;
2. clone and retag each request to `copy_index * 49 + local_index`;
3. interpret exactly 32 loads, one compute, and 16 reads;
4. produce exactly 49 matching responses;
5. reject any unsupported class, opcode, address, or malformed token.

Do not carry the `computed` flag or matrix contents across copies.

- [ ] **Step 4: Add request/response record construction**

Add:

```rust
pub fn record_scaled_trace(
    requests: &[CxlMsg],
    responses: &[CxlMsg],
    mode: CxlRecordMode,
) -> Result<CxlReplayArtifact, String>
```

Require equal nonzero lengths divisible by 49. Call
`record_gemm_msg_in_namespace(copy_index, local_index, direction, message)`
for each request/response pair, producing globally increasing sequences,
globally unique tags, and epoch `copy_index * 4 + local_epoch`. Compute a
deterministic final commitment from all decoded matrices, request count, and
response count.

- [ ] **Step 5: Run all guest interpreter tests**

Run:

```bash
cargo test -p slugarch-type2-guest --test offline
```

Expected: all current guest tests pass, including the 64-copy unique-tag test.

- [ ] **Step 6: Commit the guest interpreter**

Run:

```bash
git add crates/slugarch-type2-guest/src/gemm.rs \
  crates/slugarch-type2-guest/src/lib.rs \
  crates/slugarch-type2-guest/tests/offline.rs \
  crates/slugarch-type2-guest/Cargo.toml \
  Cargo.lock
git commit -m "feat: add deterministic Type-2 guest interpreter"
```

## Task 5: Implement Bounded devdax Access, Calibration, and Transfers

**Files:**
- Create: `crates/slugarch-type2-guest/src/dax.rs`
- Create: `crates/slugarch-type2-guest/src/measure.rs`
- Modify: `crates/slugarch-type2-guest/src/lib.rs`
- Modify: `crates/slugarch-type2-guest/tests/offline.rs`

The production path accepts only a character device whose canonical basename
matches `dax[0-9]+\\.[0-9]+`. A regular-file mapping is available only when
the explicit test flag `--allow-regular-file` is present; the host campaign
runner rejects that flag.

- [ ] **Step 1: Write bounds, permutation, and accounting tests**

Add:

```rust
#[test]
fn dax_window_rejects_access_past_its_mapped_slot() {
    let file = tempfile_backing_file(2 * MIB);
    let window = DaxWindow::map_for_test(&file, 0, MIB).unwrap();
    let error = window.write_bytes(MIB - 7, &[0u8; 8]).unwrap_err();
    assert!(error.contains("outside mapped DAX window"));
}

#[test]
fn seeded_pointer_cycle_visits_all_4096_lines_once() {
    let offsets = build_pointer_cycle(0x534c_5547, 4096, 64).unwrap();
    assert_eq!(offsets.len(), 4096);
    let unique = offsets.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(unique.len(), 4096);
    assert_eq!(follow_pointer_cycle(&offsets, 4096).unwrap(), 0);
}

#[test]
fn calibration_declares_exactly_4096_eight_byte_reads() {
    let declared = calibration_traffic();
    assert_eq!(declared.reads, 4_096);
    assert_eq!(declared.read_bytes, 32_768);
    assert_eq!(declared.writes, 0);
    assert_eq!(declared.written_bytes, 0);
}

#[test]
fn transfer_declares_one_logical_write_and_read() {
    for size in [4_096u64, 65_536, 1_048_576] {
        let declared = transfer_traffic(size);
        assert_eq!(declared.writes, 1);
        assert_eq!(declared.written_bytes, size);
        assert_eq!(declared.reads, 1);
        assert_eq!(declared.read_bytes, size);
        assert_eq!(expected_scalar_cxl_accesses(size, 8).unwrap(), size / 8);
    }
}

#[test]
fn four_kib_transfer_has_one_guest_operation_and_512_scalar_accesses() {
    let declared = transfer_traffic(4_096);
    assert_eq!((declared.writes, declared.reads), (1, 1));
    assert_eq!(expected_scalar_cxl_accesses(4_096, 8).unwrap(), 512);
}
```

Use a helper built from `std::fs::OpenOptions::set_len`; do not add the
`tempfile` crate.

- [ ] **Step 2: Run the measurement tests and verify RED**

Run:

```bash
cargo test -p slugarch-type2-guest --test offline seeded_pointer_cycle
```

Expected: unresolved `build_pointer_cycle`.

- [ ] **Step 3: Implement the mapped window**

`DaxWindow` stores the mapped pointer, mapped length, DAX resource start,
mapped file offset, and DPA start. Its constructor requires:

```rust
pub fn map_production(
    path: &Path,
    file_offset: u64,
    length: usize,
    dpa_start: u64,
) -> Result<Self, String>
```

Use `libc::mmap` with `PROT_READ | PROT_WRITE` and `MAP_SHARED`. Reject integer
overflow and any access outside the mapped length. Implement `Drop` with
`libc::munmap`.

Implement byte access with volatile 64-bit chunks and volatile trailing bytes.
Wrap measured ranges with `compiler_fence(Ordering::SeqCst)` and x86
`_mm_mfence`. Use `_mm_sfence` after stores. Use `clock_gettime` with
`CLOCK_MONOTONIC_RAW`; reject a clock that moves backward.

- [ ] **Step 4: Implement deterministic setup and cache eviction**

`build_pointer_cycle(0x534c5547, 4096, 64)` uses a fixed xorshift64 state and
Fisher-Yates permutation. It writes the DPA-relative next-line offset as a
little-endian `u64` into each line.

Pointer-cycle initialization and a pass that applies `_mm_clflush` to all
4,096 lines occur in a separately bracketed setup phase. The timed
calibration phase contains only 4,096 dependent volatile 64-bit reads plus
the outer fences and clock reads. Its final pointer must equal its initial
pointer.

- [ ] **Step 5: Implement calibration and transfer functions**

Expose:

```rust
pub fn run_calibration(window: &DaxWindow, slot_offset: usize)
    -> Result<CalibrationResult, String>;

pub fn run_transfer(
    window: &DaxWindow,
    slot_offset: usize,
    size: usize,
    seed: u64,
) -> Result<TransferResult, String>;
```

`CalibrationResult` includes total elapsed nanoseconds, nanoseconds per load
as `f64`, final pointer, checksum, and `calibration_traffic()`.

For transfers, generate bytes outside the timed range. The write timer covers
volatile stores, line flushes, and the persistence/order fence. The read timer
covers volatile readback and checksum calculation. Record write, read, and
round-trip nanoseconds plus integer bytes and checksums. Compute throughput
only during normalization so raw guest output retains exact integer
numerators and denominators.

- [ ] **Step 6: Run all DAX and measurement tests**

Run:

```bash
cargo test -p slugarch-type2-guest --test offline
```

Expected: bounds, pointer-cycle, calibration-accounting, and all three transfer
sizes pass against a regular-file test mapping.

- [ ] **Step 7: Commit DAX calibration and transfer support**

Run:

```bash
git add crates/slugarch-type2-guest/src/dax.rs \
  crates/slugarch-type2-guest/src/measure.rs \
  crates/slugarch-type2-guest/src/lib.rs \
  crates/slugarch-type2-guest/tests/offline.rs
git commit -m "feat: measure bounded Type-2 DAX access"
```

## Task 6: Implement SlugArch Modes and the Labeled Corruption Test

**Files:**
- Modify: `crates/slugarch-type2-guest/src/measure.rs`
- Modify: `crates/slugarch-type2-guest/src/schema.rs`
- Modify: `crates/slugarch-type2-guest/tests/offline.rs`

Each SlugArch condition performs this exact timed sequence:

1. encode the request stream as raw 64-byte records for baseline or replay
   records for validation/delta/full;
2. write and read back the request stream through its condition slot;
3. interpret only the readback requests in guest software;
4. encode the response stream under the same mode;
5. write and read back the response stream through the same slot at the next
   64-byte-aligned offset;
6. byte-compare raw streams for baseline or deserialize and validate the
   replay records for policy modes.

The end-to-end timer encloses all six actions. Request and response transport
times are accumulated into `cxl_write_ns` and `cxl_read_ns`. Input generation,
slot zeroing, SSH, and host validation are outside the timer.

- [ ] **Step 1: Write mode and corruption tests**

Add:

```rust
#[test]
fn all_modes_report_request_and_boundary_counts_separately() {
    let requests = fixture_requests();
    for mode in [Mode::Baseline, Mode::Validation, Mode::Delta, Mode::Full] {
        let result = run_slugarch_on_test_window(&requests, 4, mode).unwrap();
        assert_eq!(result.counts.request_record_count, 196);
        assert_eq!(result.counts.response_record_count, 196);
        assert_eq!(result.counts.boundary_record_count, 392);
        assert!(result.validation_passed);
        assert_eq!(result.input_sha256, result.staged_request_sha256);
        assert_eq!(result.output_sha256, result.staged_response_sha256);
    }
}

#[test]
fn maximum_full_mode_uses_the_production_serializer_and_fits() {
    let requests = fixture_requests_scaled(64);
    let responses = expected_fixture_responses(&requests);
    let request_bytes =
        encode_slugarch_stream(&requests, Mode::Full).unwrap();
    let response_bytes =
        encode_slugarch_stream(&responses, Mode::Full).unwrap();
    let layout = plan_serialized_stream_layout(
        MIB as usize,
        request_bytes.len(),
        response_bytes.len(),
    )
    .unwrap();

    assert_eq!(layout.request_offset, 0);
    assert_eq!(layout.request_len, request_bytes.len());
    assert_eq!(layout.response_offset, align_up_64(request_bytes.len()).unwrap());
    assert_eq!(layout.response_len, response_bytes.len());
    assert_eq!(
        layout.end_offset,
        layout.response_offset + response_bytes.len(),
    );
    assert!(layout.end_offset <= MIB as usize);
    assert!(plan_serialized_stream_layout(
        layout.end_offset - 1,
        request_bytes.len(),
        response_bytes.len(),
    )
    .is_err());
}

#[test]
fn post_transport_payload_flip_is_rejected_as_decoded_result_mismatch() {
    let requests = fixture_requests();
    let result = run_corruption_on_test_window(&requests).unwrap();
    assert_eq!(result.name, "post_transport_guest_payload_flip");
    assert!(result.transport_checksum_passed);
    assert!(result.rejected);
    assert_eq!(result.rejection_reason, "decoded_result_mismatch");
    assert!(!result.device_fault_injection);
}
```

- [ ] **Step 2: Run the corruption test and verify RED**

Run:

```bash
cargo test -p slugarch-type2-guest --test offline \
  post_transport_payload_flip_is_rejected_as_decoded_result_mismatch
```

Expected: unresolved corruption runner.

- [ ] **Step 3: Implement baseline and replay-policy serialization**

Baseline serializes every request and response with
`slugarch_cxl_wire::encode`, preserving 64 bytes per record. Policy modes use
`record_scaled_trace` and `slugarch_cxl_replay::serialize_records`.

Before writing, derive exact expected request and response byte counts from
the serialized buffers. Use `plan_serialized_stream_layout` for the
64-byte-aligned response offset and reject a layout unless its dynamically
derived end offset fits the assigned one-MiB slot subtraction-safely. The
production serializer used for execution and the fit test must be the same
function. After readback, require byte-for-byte checksums and lengths before
interpreting or validating. Store SHA-256 strings produced with
`sha2::{Digest,Sha256}`. Verify the helper with the standard `abc` digest:

```text
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
```

- [ ] **Step 4: Implement full-mode post-transport corruption**

After the timed pass:

1. use the 49-request full-mode condition slot;
2. produce, transport, and checksum a valid response stream;
3. deserialize it in guest DRAM;
4. locate the first `DeviceToHost` record with a full payload;
5. flip bit zero of payload byte zero without changing record framing;
6. decode the response matrix and compare it with the expected matrix;
7. require rejection reason `decoded_result_mismatch`.

The result row sets `device_fault_injection=false`,
`transport_corruption=false`, and `recovery_measured=false`.

- [ ] **Step 5: Run all mode tests**

Run:

```bash
cargo test -p slugarch-type2-guest --test offline
```

Expected: all modes validate, the labeled corruption is rejected, and no test
labels the event as device injection.

- [ ] **Step 6: Commit mode and corruption support**

Run:

```bash
git add crates/slugarch-type2-guest/src/measure.rs \
  crates/slugarch-type2-guest/src/schema.rs \
  crates/slugarch-type2-guest/tests/offline.rs
git commit -m "feat: measure SlugArch modes over DAX"
```

## Task 7: Implement the Guest Phase Protocol and Static Binary Gate

**Files:**
- Modify: `crates/slugarch-type2-guest/src/lib.rs`
- Modify: `crates/slugarch-type2-guest/src/main.rs`
- Modify: `crates/slugarch-type2-guest/src/schema.rs`
- Modify: `crates/slugarch-type2-guest/tests/offline.rs`

- [ ] **Step 1: Write a transcript-driven state-machine test**

Add a test that feeds:

```text
TOPOLOGY_ACK
SENTINEL_READ_GO
SENTINEL_WRITE_GO
ARM_ACK
GO setup-calibration
GO warmup-calibration
GO timed-calibration
GO post-transport-corruption
SHUTDOWN
```

Require the guest to reject a `GO` whose phase ID differs from its current
`READY`, a second `ARM_ACK`, a timed phase before durable arm, and any DAX
access outside a `GO`/`DONE` interval.

- [ ] **Step 2: Run the transcript test and verify RED**

Run:

```bash
cargo test -p slugarch-type2-guest --test offline phase_protocol
```

Expected: no phase-protocol implementation exists.

- [ ] **Step 3: Implement strict line protocol parsing**

Guest stdout contains only:

```text
EVENT {canonical_json}
READY {phase_id}
DONE {phase_id} {canonical_json}
ARM_REQUEST {canonical_json}
FATAL {canonical_json}
```

Host commands on stdin are:

```text
TOPOLOGY_ACK
SENTINEL_READ_GO
SENTINEL_WRITE_GO
ARM_ACK
GO {phase_id}
SHUTDOWN
```

Flush stdout after every line. Reject blank commands, extra tokens, duplicate
phase IDs, and EOF before `SHUTDOWN`. Emit every condition result as canonical
JSON with the exact identifiers and declared traffic.

- [ ] **Step 4: Add production preflight**

The guest:

- pins itself to guest CPU 1 and verifies the resulting affinity;
- records `CLOCK_MONOTONIC_RAW` resolution;
- reads `/sys/bus/pci/devices/*/{vendor,device}` and requires exactly one
  `8086:0d92`;
- records the selected memdev, region, devdax node, and DAX resource;
- computes `map_offset = work_hpa_start - dax_resource_start`;
- requires the 32 MiB `[80 MiB,112 MiB)` DPA work window to be covered;
- refuses `/sys/bus/pci/devices/*/resource4`;
- maps only the selected `/dev/daxX.Y`;
- reads and writes the sentinel only after the corresponding host command.

- [ ] **Step 5: Run all guest tests**

Run:

```bash
cargo test -p slugarch-type2-guest
cargo fmt --check --package slugarch-type2-guest
```

Expected: all tests pass and formatting is clean.

- [ ] **Step 6: Build and verify a static guest binary**

Run:

```bash
RUSTFLAGS="-C target-feature=+crt-static" \
  cargo build --release -p slugarch-type2-guest
file target/release/slugarch-type2-guest
ldd target/release/slugarch-type2-guest
```

Expected: `file` reports a statically linked x86-64 executable and `ldd`
reports `not a dynamic executable`. If static linking fails, stop before the
pilot; do not substitute a dynamically linked binary without proving its
loader and library versions inside the guest.

- [ ] **Step 7: Commit the guest phase protocol**

Run:

```bash
git add crates/slugarch-type2-guest/src \
  crates/slugarch-type2-guest/tests/offline.rs
git commit -m "feat: add armed Type-2 guest phase protocol"
```

## Task 8: Define the Python Campaign Contract and Frozen Defaults

**Files:**
- Create: `targets/qemu-type2/cxlmem_campaign/__init__.py`
- Create: `targets/qemu-type2/cxlmem_campaign/contract.py`
- Create: `targets/qemu-type2/cxlmem_campaign/provenance.py`
- Create: `targets/qemu-type2/cxlmem-campaign-defaults.json`
- Create: `targets/qemu-type2/tests/campaign_fixtures.py`
- Create: `targets/qemu-type2/tests/test_contract.py`
- Create: `targets/qemu-type2/tests/test_provenance.py`

- [ ] **Step 1: Write matrix and provenance tests**

Create `test_contract.py`:

```python
import unittest

from cxlmem_campaign.contract import (
    LATENCY_ORDERS,
    MODE_ORDERS,
    build_slots,
    count_vocabulary,
)


class ContractTests(unittest.TestCase):
    def test_matrix_has_twenty_unique_slots_in_block_order(self):
        slots = build_slots()
        self.assertEqual(len(slots), 20)
        self.assertEqual(len({(s.latency_ns, s.replicate) for s in slots}), 20)
        self.assertEqual(
            [s.latency_ns for s in slots[:4]],
            [80, 400, 2_000, 10_000],
        )
        self.assertEqual(
            [s.latency_ns for s in slots[4:8]],
            [400, 2_000, 10_000, 80],
        )

    def test_count_vocabulary_never_conflates_requests_and_boundaries(self):
        self.assertEqual(
            count_vocabulary(64),
            {
                "copy_count": 64,
                "request_record_count": 3_136,
                "response_record_count": 3_136,
                "boundary_record_count": 6_272,
            },
        )

    def test_all_fixed_orders_are_complete(self):
        self.assertEqual(len(LATENCY_ORDERS), 5)
        self.assertEqual(len(MODE_ORDERS), 5)
        for order in LATENCY_ORDERS:
            self.assertEqual(set(order), {80, 400, 2_000, 10_000})
        for order in MODE_ORDERS:
            self.assertEqual(
                set(order),
                {"baseline", "validation", "delta", "full"},
            )


if __name__ == "__main__":
    unittest.main()
```

Create `test_provenance.py` with a temporary two-file source tree. Require
deterministic hashes under reversed filesystem enumeration, rejection of a
missing required input, and distinct experiment-version hashes when one
validation-rule byte changes.

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest \
  targets/qemu-type2/tests/test_contract.py \
  targets/qemu-type2/tests/test_provenance.py -v
```

Expected: imports fail because `cxlmem_campaign` does not exist.

- [ ] **Step 3: Create the frozen defaults**

Create `targets/qemu-type2/cxlmem-campaign-defaults.json`:

```json
{
  "schema_version": 1,
  "artifact_root": "/root/Concordia/SlugArch/artifact/slugarch_type2_cxlmem",
  "runtime_root": "/tmp/slugarch-type2-cxlmem-runtime",
  "qemu_binary": "/tmp/slugarch-type2-cxlmem-build/qemu/qemu-system-x86_64",
  "server_binary": "/tmp/slugarch-type2-cxlmem-build/cxlmemsim_server",
  "kernel_image": "/tmp/slugarch-type2-cxlmem-build/bzImage",
  "base_disk_image": "/tmp/slugarch-type2-cxlmem-build/slugarch-type2.img",
  "guest_binary": "/tmp/slugarch-type2-cxlmem-build/slugarch-type2-guest",
  "job_json": "/tmp/slugarch-type2-campaign-src/targets/qemu-type2/identity_times_const.json",
  "design_spec": "/tmp/slugarch-type2-campaign-src/docs/superpowers/specs/2026-07-24-slugarch-type2-cxlmem-experiment-design.md",
  "plot_script": "/tmp/slugarch-paper-integration/scripts/plot_slugarch_type2_cxlmem.py",
  "qemu_cpus": "0-3",
  "server_cpus": "4-5",
  "guest_cpu": 1,
  "server_host": "127.0.0.1",
  "server_port": 10199,
  "ssh_host": "127.0.0.1",
  "ssh_port": 12022,
  "guest_user": "root",
  "guest_memory_bytes": 2147483648,
  "guest_vcpus": 2,
  "type2_capacity_bytes": 268435456,
  "type2_cache_bytes": 134217728,
  "work_dpa_start": 83886080,
  "work_dpa_end": 117440512,
  "protocol_magic": "SLT2",
  "protocol_version": 1,
  "seed": 1397511495
}
```

`1397511495` is hexadecimal `0x534c5547`.

- [ ] **Step 4: Implement the immutable matrix**

In `contract.py`, define tuples, frozen dataclasses, and no environment-based
fallbacks:

```python
from dataclasses import dataclass

LATENCIES_NS = (80, 400, 2_000, 10_000)
TRACE_COPY_COUNTS = (1, 4, 16, 64)
TRACE_REQUEST_COUNTS = (49, 196, 784, 3_136)
MODES = ("baseline", "validation", "delta", "full")
LATENCY_ORDERS = (
    (80, 400, 2_000, 10_000),
    (400, 2_000, 10_000, 80),
    (2_000, 10_000, 80, 400),
    (10_000, 80, 400, 2_000),
    (80, 10_000, 2_000, 400),
)
MODE_ORDERS = (
    ("baseline", "validation", "delta", "full"),
    ("validation", "delta", "full", "baseline"),
    ("delta", "full", "baseline", "validation"),
    ("full", "baseline", "validation", "delta"),
    ("baseline", "full", "delta", "validation"),
)


@dataclass(frozen=True)
class ReplicateSlot:
    block: int
    replicate: int
    latency_ns: int


def build_slots():
    return tuple(
        ReplicateSlot(block=block, replicate=block, latency_ns=latency)
        for block, order in enumerate(LATENCY_ORDERS, start=1)
        for latency in order
    )


def count_vocabulary(copy_count):
    if copy_count not in TRACE_COPY_COUNTS:
        raise ValueError(f"unsupported copy count {copy_count}")
    requests = 49 * copy_count
    return {
        "copy_count": copy_count,
        "request_record_count": requests,
        "response_record_count": requests,
        "boundary_record_count": requests * 2,
    }
```

Add helpers for canonical condition IDs, latency directory names, attempt
directory names, the exact 21-entry slot-name tuple above,
`DPA_SLOT_BYTES = 1_048_576`, a generated absolute DPA slot map rooted at
`work_dpa_start`, the dynamically derived request length, aligned response
offset, response length, and end offset for every SlugArch condition, and the
approved mode order. The Python contract obtains those lengths by invoking the
same production Rust exporter/serializer used to create `requests.bin` and
expected responses; it must not recreate replay-record sizing with a formula.
The Python map and derived layouts must byte-match the Rust guest values
serialized into `guest-config.json`.

- [ ] **Step 5: Implement canonical provenance**

`provenance.py` exposes:

```python
def sha256_file(path: pathlib.Path) -> str
def canonical_json_bytes(value: object) -> bytes
def git_snapshot(
    repo: pathlib.Path,
    in_scope_paths: collections.abc.Sequence[str],
) -> dict
def freeze_inputs(config: dict, destination: pathlib.Path) -> dict
def experiment_version_hash(frozen_manifest: dict) -> str
```

`freeze_inputs` copies the design spec, trace, validation rules, normalized
configuration, source patches, Git status, submodule status, and the plotting
script into `source/`. It hashes the QEMU, server, kernel, base disk, guest
binary, job, trace, validator package, and plotter. It opens each input once
for size and hash, then verifies the size and mtime did not change while
hashing. A changed input aborts registration.

The frozen manifest always describes the complete approved 20-slot campaign,
even when the caller is the one-boot pilot. Its `experiment_contract` contains
the full latency/block schedule, all condition orders, canonical typed server
and QEMU argument fields, CPU and port assignments, topology, seeds, arm and
retry rules, the complete 21-entry DPA slot map and per-slot capacity, hash
algorithms, every production-serialized per-condition layout length/offset,
and validation-rule hashes. Runtime identities
(`campaign_id`, `pilot_id`, attempt/boot/server UUIDs, UTC timestamps, runtime
directories, and registry ordinal) are excluded from
`experiment_version_hash` and are instead covered by each invocation manifest
and artifact seal. Thus the pilot and full campaign may reference the same
frozen-contract hash without pretending their invocations are identical.

Add tests proving that changing only a pilot/campaign ID preserves the
experiment-version hash, while changing one latency, condition order, DPA
slot offset/size, production-derived serialized layout length, binary hash,
launch argument, or validation rule changes it.

Use `json.dumps(value, sort_keys=True, separators=(",", ":"),
ensure_ascii=False)` plus a final newline for canonical JSON.

- [ ] **Step 6: Run contract and provenance tests**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest \
  targets/qemu-type2/tests/test_contract.py \
  targets/qemu-type2/tests/test_provenance.py -v
```

Expected: all tests pass.

- [ ] **Step 7: Commit the campaign contract**

Run:

```bash
git add targets/qemu-type2/cxlmem_campaign/__init__.py \
  targets/qemu-type2/cxlmem_campaign/contract.py \
  targets/qemu-type2/cxlmem_campaign/provenance.py \
  targets/qemu-type2/cxlmem-campaign-defaults.json \
  targets/qemu-type2/tests/campaign_fixtures.py \
  targets/qemu-type2/tests/test_contract.py \
  targets/qemu-type2/tests/test_provenance.py
git commit -m "feat: freeze Type-2 campaign contract"
```

## Task 9: Implement Atomic Arming and Artifact Seals

**Files:**
- Create: `targets/qemu-type2/cxlmem_campaign/artifacts.py`
- Create: `targets/qemu-type2/tests/test_artifacts.py`

- [ ] **Step 1: Write durable-arm and tamper tests**

Create tests for:

1. a successful arm leaves only `MEASUREMENT_ARMED.json`, with no temporary
   file;
2. a failure before `os.replace` leaves no arm marker and is retry-eligible;
3. a valid arm marker found after process death makes the attempt committed;
4. an existing attempt directory is rejected;
5. a sealed attempt verifies;
6. changing one byte after sealing makes verification fail;
7. `COMPLETE` and `FAILED.json` cannot coexist.

Use `unittest.mock.patch` to fail `os.replace` in the pre-arm test. Treat any
valid final marker found during recovery as committed, which is conservative
against outcome-based retry.

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_artifacts.py -v
```

Expected: import failure for `cxlmem_campaign.artifacts`.

- [ ] **Step 3: Implement atomic JSON creation**

Add:

```python
def fsync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def atomic_create_json(path, value):
    path = pathlib.Path(path)
    temporary = path.with_name(f".{path.name}.tmp-{uuid.uuid4()}")
    data = canonical_json_bytes(value)
    fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    try:
        with os.fdopen(fd, "wb", closefd=False) as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
    finally:
        os.close(fd)
    os.replace(temporary, path)
    fsync_directory(path.parent)
```

Reject an existing final path before creating the temporary file. On error,
unlink only the function’s own temporary path.

- [ ] **Step 4: Implement the irrevocable arm point**

Expose:

```python
def arm_attempt(attempt_dir, identifiers, preflight_hash):
    marker = pathlib.Path(attempt_dir) / "MEASUREMENT_ARMED.json"
    value = {
        "schema_version": 1,
        "campaign_id": identifiers["campaign_id"],
        "slot_id": identifiers["slot_id"],
        "attempt_id": identifiers["attempt_id"],
        "guest_boot_uuid": identifiers["guest_boot_uuid"],
        "server_instance_uuid": identifiers["server_instance_uuid"],
        "preflight_sha256": preflight_hash,
        "state": "measurement_armed",
    }
    atomic_create_json(marker, value)
    return value
```

The runner sends `ARM_ACK` only after this function returns.

- [ ] **Step 5: Implement canonical checksums and seals**

`write_checksum_manifest(root, exclusions)` recursively hashes sorted relative
paths and writes lines in `sha256sum` format. Attempt manifests exclude
`checksums.sha256`, `COMPLETE`, and `FAILED.json`. `seal_attempt`:

1. writes all terminal metadata except the seal;
2. writes and fsyncs `checksums.sha256`;
3. hashes that file;
4. writes either `COMPLETE` or `FAILED.json` containing the checksum-file hash;
5. fsyncs the attempt directory;
6. rejects all later writes through the harness API.

Campaign checksums exclude `campaign-checksums.sha256`,
`CAMPAIGN_COMPLETE`, and `CAMPAIGN_FAILED`. Export checksums use the same
rule with `EXPORT_COMPLETE`.

- [ ] **Step 6: Run artifact tests**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_artifacts.py -v
```

Expected: all atomic-arm, seal, exclusivity, and tamper tests pass.

- [ ] **Step 7: Commit artifact primitives**

Run:

```bash
git add targets/qemu-type2/cxlmem_campaign/artifacts.py \
  targets/qemu-type2/tests/test_artifacts.py
git commit -m "feat: seal Type-2 campaign artifacts"
```

## Task 10: Implement the Append-Only Campaign Registry

**Files:**
- Create: `targets/qemu-type2/cxlmem_campaign/registry.py`
- Create: `targets/qemu-type2/tests/test_registry.py`

- [ ] **Step 1: Write hash-chain, ordering, and eligibility tests**

Tests must cover:

- ordinals begin at 1 and increase by one;
- `previous_entry_hash` links exact canonical prior events;
- one-byte tampering rejects the complete registry;
- a partial final line rejects without truncation;
- terminal events must name an existing registration ordinal;
- the lowest registered checksum-valid complete campaign wins even when a
  later same-version campaign finishes first;
- a failed campaign remains disclosed;
- an `.inprogress` directory blocks a new registration until reconciliation.

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_registry.py -v
```

Expected: import failure for `cxlmem_campaign.registry`.

- [ ] **Step 3: Implement the held campaign lock**

Use:

```python
@contextlib.contextmanager
def campaign_lock(artifact_root):
    lock_path = pathlib.Path(artifact_root) / ".campaign.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o644)
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        yield descriptor
    finally:
        fcntl.flock(descriptor, fcntl.LOCK_UN)
        os.close(descriptor)
```

The top-level runner keeps this context open from reconciliation and
registration through terminal rename and terminal-event append. A resumed
process reacquires it before inspecting the registered campaign.

- [ ] **Step 4: Implement canonical registry events**

Every event includes:

```text
schema_version
ordinal
event
campaign_id
experiment_version_sha256
registration_ordinal
campaign_checksum_sha256
frozen_input_hashes
utc
previous_entry_hash
entry_hash
```

For `REGISTERED`, `registration_ordinal == ordinal` and
`campaign_checksum_sha256` is null. For `COMPLETE` or `FAILED`,
`registration_ordinal` identifies the registration and
`campaign_checksum_sha256` is non-null.

Compute `entry_hash` from canonical JSON with that field omitted. Append the
complete encoded line with one `os.write` to a descriptor opened with
`O_APPEND`, then `fsync` the descriptor and parent directory. Verify the full
registry again before returning.

- [ ] **Step 5: Implement reconciliation and selection**

`reconcile_registered_campaigns`:

- finalizes a checksum-valid terminal marker still under `.inprogress`;
- appends a missing terminal registry event for a valid finalized directory;
- refuses to infer success from files without a terminal marker;
- classifies a valid `MEASUREMENT_ARMED.json` as post-arm;
- refuses a new registration while unresolved state remains.

`select_eligible_campaign` groups by experiment-version hash and chooses the
lowest registration ordinal with a checksum-valid `CAMPAIGN_COMPLETE`.

- [ ] **Step 6: Run registry tests**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_registry.py -v
```

Expected: all chain, reconciliation, and selection tests pass.

- [ ] **Step 7: Commit the registry**

Run:

```bash
git add targets/qemu-type2/cxlmem_campaign/registry.py \
  targets/qemu-type2/tests/test_registry.py
git commit -m "feat: register Type-2 campaigns durably"
```

## Task 11: Implement the SLT2 v1 Oracle Client

**Files:**
- Create: `targets/qemu-type2/cxlmem_campaign/wire.py`
- Create: `targets/qemu-type2/tests/test_wire.py`

- [ ] **Step 1: Write CRC, golden-frame, short-I/O, and timeout tests**

Test CRC32C against:

```text
input: 123456789
CRC32C: e3069283
```

Use `socket.socketpair()` for:

- a HELLO frame delivered one byte at a time;
- an ACK read in partial chunks;
- a corrupted CRC;
- a mismatched request ID;
- an EOF at byte 39 of the 40-byte header;
- a response delayed beyond a 20 ms test deadline.

- [ ] **Step 2: Run wire tests and verify RED**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_wire.py -v
```

Expected: import failure for `cxlmem_campaign.wire`.

- [ ] **Step 3: Implement CRC32C exactly**

Add:

```python
def crc32c(data):
    crc = 0xFFFFFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            mask = -(crc & 1) & 0xFFFFFFFF
            crc = (crc >> 1) ^ (0x82F63B78 & mask)
    return crc ^ 0xFFFFFFFF
```

All integers are little-endian. Zero header bytes 32 through 35 before
calculating the complete-frame checksum.

- [ ] **Step 4: Implement frame encoding and decoding**

Use `struct.Struct("<4sHHIIQQII")` for the 40-byte header. Define all frame,
role, and status constants from the approved spec. Enforce exact frame lengths:

```text
HELLO 72
ACK 88
READ 128
WRITE 128
MEMORY_RESPONSE 128
COUNTER_SNAPSHOT 48
COUNTER_RESPONSE 112
ERROR 56
```

Reject nonzero reserved fields, version other than 1, maximum-frame other than
128, non-monotonic request IDs, unknown frame types, and unused nonzero payload
bytes.

- [ ] **Step 5: Implement exact-length deadline I/O**

Use an absolute `time.monotonic()` deadline. Before every socket operation,
set the remaining timeout. Retry interrupted calls, reject zero-byte
read/write before completion, and never reset the five-second deadline after
partial progress.

Expose the exact `OracleClient` interface:

```text
OracleClient(host: str, port: int, timeout_seconds: float = 5.0)
connect() -> Ack
write(dpa: int, data: bytes) -> MemoryResponse
read(dpa: int, length: int) -> MemoryResponse
counter_snapshot(qemu_client_id: int) -> CounterResponse
close() -> None
```

Implement every method with the frame and deadline helpers from the preceding
steps. `connect` generates a 16-byte nonce with `os.urandom`, sends oracle role
2, validates a successful ACK and nonzero client ID, and initializes request
ID 1. `write` and `read` require 1 through 64 bytes and validate matching
response IDs/status/lengths. `counter_snapshot` is available only after an
oracle-role handshake. `close` performs `shutdown(SHUT_RDWR)` when connected,
closes the socket, and is idempotent.

- [ ] **Step 6: Run wire tests**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_wire.py -v
```

Expected: all CRC, framing, short-I/O, mismatch, EOF, and deadline tests pass.

- [ ] **Step 7: Commit the oracle client**

Run:

```bash
git add targets/qemu-type2/cxlmem_campaign/wire.py \
  targets/qemu-type2/tests/test_wire.py
git commit -m "feat: add SlugArch Type-2 oracle client"
```

## Task 12: Implement Fresh Server/QEMU Launch and Guest Sessions

**Files:**
- Create: `targets/qemu-type2/cxlmem_campaign/launch.py`
- Create: `targets/qemu-type2/cxlmem_campaign/guest_session.py`
- Create: `targets/qemu-type2/tests/test_runner_state.py`

- [ ] **Step 1: Write exact command and ownership tests**

Test `build_server_command` and `build_qemu_command` as complete argument
lists. Require:

- server under `taskset -c 4-5`;
- QEMU under `taskset -c 0-3`;
- `-accel tcg`, no KVM, and no `icount`;
- `-m 2G` and `-smp 2`;
- one 256 MiB CFMWS and one Type-2 endpoint;
- `mem-size=256M`;
- synchronous SLT2 v1 enabled;
- private qcow2 overlay instead of the base image;
- a unique POSIX shared-memory name that did not exist before launch;
- unique attempt-local QEMU and server logs.

Also test that a pre-existing listener on port 10199 or 12022 aborts rather
than attaching to it, and that cleanup records process exit status.

- [ ] **Step 2: Run launch tests and verify RED**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_runner_state.py -v
```

Expected: import failure for launch/session modules.

- [ ] **Step 3: Implement private runtime preparation**

`prepare_attempt_runtime`:

1. creates `pathlib.Path(config["runtime_root"]) / attempt_id` with
   `os.mkdir`;
2. derives `/slugarch_type2_` followed by the attempt UUID without hyphens,
   rejects it if `/dev/shm/` already contains that name, and records the name
   in the manifest; the server creates and zeroes it with `O_CREAT|O_EXCL`;
3. executes:

```python
subprocess.run(
    [
        "qemu-img",
        "create",
        "-q",
        "-f",
        "qcow2",
        "-F",
        "raw",
        "-b",
        "/tmp/slugarch-type2-cxlmem-build/slugarch-type2.img",
        str(runtime_dir / "root.qcow2"),
    ],
    check=True,
)
```

4. records the overlay backing filename with `qemu-img info --output=json`;
5. rejects any overlay whose backing path differs from the frozen base image.

Runtime scratch is deleted only after QEMU/server exit and logs have been
copied. After the owned server exits, cleanup requires its owned
`/dev/shm/slugarch_type2_*` object to be absent; on abnormal termination the
harness unlinks only the exact recorded name and records that recovery action.
The manifest records preparation and cleanup status. The qcow2 overlay and
shared-memory bytes are not part of the scientific checksum tree.

- [ ] **Step 4: Build the exact server command**

The command builder returns this exact Python list:

```python
[
    "taskset", "-c", "4-5",
    "/tmp/slugarch-type2-cxlmem-build/cxlmemsim_server",
    "--comm-mode=tcp",
    "--slugarch-type2-protocol=true",
    "--host=127.0.0.1",
    "--port=10199",
    "--capacity=256",
    f"--default_latency={slot.latency_ns}",
    f"--slugarch-shm-name={shm_name}",
    f"--slugarch-event-log={attempt_dir / 'logs/server-events.jsonl.gz'}",
]
```

No shell expansion is used.

- [ ] **Step 5: Build the exact QEMU command**

The command builder returns a Python argument list equivalent to:

```text
taskset -c 0-3
/tmp/slugarch-type2-cxlmem-build/qemu/qemu-system-x86_64
-accel tcg
-cpu max
-M q35,cxl=on,cxl-fmw.0.targets.0=cxl.0,cxl-fmw.0.size=256M
-m 2G
-smp 2
-kernel /tmp/slugarch-type2-cxlmem-build/bzImage
-append root=/dev/vda rw console=ttyS0,115200 nokaslr systemd.mask=cxl-numa-setup.service
-drive file={runtime_dir}/root.qcow2,if=none,id=bootdisk,format=qcow2
-device virtio-blk-pci,drive=bootdisk,bus=pcie.0
-netdev user,id=net0,hostfwd=tcp:127.0.0.1:12022-:22
-device virtio-net-pci,netdev=net0,bus=pcie.0,mac=52:54:00:00:10:22
-qmp unix:{runtime_dir}/qmp.sock,server=on,wait=off
-device pxb-cxl,bus_nr=12,bus=pcie.0,id=cxl.0
-device cxl-rp,port=0,bus=cxl.0,id=type2_rp,chassis=0,slot=2
-device cxl-type2,bus=type2_rp,id=cxl-type2-slugarch,sn=200,gpu-mode=0,cache-size=128M,mem-size=256M,cxlmemsim-addr=127.0.0.1,cxlmemsim-port=10199,coherency-enabled=false,sync-type2-wire=on,type2-wire-version=1,slugarch-event-log={attempt_dir}/logs/qemu-events.jsonl
-nographic
```

Set `CXL_TRANSPORT_MODE=tcp`, `CXL_MEMSIM_HOST=127.0.0.1`, and
`CXL_MEMSIM_PORT=10199` in the recorded QEMU environment. The braces above
name values inserted by `build_qemu_command`; they are not shell syntax.

- [ ] **Step 6: Implement owned lifecycle and readiness**

Use `subprocess.Popen` with the complete argument list, recorded environment,
attempt log file handles, `start_new_session=True`, and `close_fds=True`. The
runner must:

- check both ports are unused before launch;
- launch a fresh server and retain its PID/start time;
- complete an oracle handshake before launching QEMU;
- launch QEMU and retain its PID/start time;
- connect only to the owned QMP socket, verify the QEMU greeting, and issue
  `qmp_capabilities`;
- wait up to 180 seconds for SSH while checking both owned PIDs;
- never use a process discovered by `pgrep`;
- send guest `sync; poweroff`, wait 30 seconds, then `SIGTERM` its owned
  process group, wait 10 seconds, and use `SIGKILL` only if still alive;
- stop the server after QEMU and record every signal and exit status.

- [ ] **Step 7: Implement the guest SSH session**

Copy these frozen files to `/root/slugarch-type2-campaign/`:

- `slugarch-type2-guest`
- `requests.bin`
- `expected.json`
- `guest-config.json`

`guest-config.json` contains the exact 21-entry DPA map plus each condition's
production-derived request length, 64-byte-aligned response offset, response
length, and end offset. The guest independently reserializes each condition
before its first DAX access and rejects any length mismatch or slot overflow.

Launch:

```bash
ssh -o BatchMode=yes \
  -o StrictHostKeyChecking=no \
  -o UserKnownHostsFile=/dev/null \
  -o ConnectTimeout=5 \
  -p 12022 \
  root@127.0.0.1 \
  taskset -c 1 /root/slugarch-type2-campaign/slugarch-type2-guest \
  --config /root/slugarch-type2-campaign/guest-config.json \
  --requests /root/slugarch-type2-campaign/requests.bin
```

Open stdin/stdout pipes, preserve every line in
`logs/guest-protocol.log`, and parse only the exact protocol from Task 7.

- [ ] **Step 8: Run launch/session unit tests**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_runner_state.py -v
```

Expected: exact commands, port ownership, cleanup, and transcript tests pass.
These tests use mocked processes and do not launch QEMU.

- [ ] **Step 9: Commit launch and guest-session support**

Run:

```bash
git add targets/qemu-type2/cxlmem_campaign/launch.py \
  targets/qemu-type2/cxlmem_campaign/guest_session.py \
  targets/qemu-type2/tests/test_runner_state.py
git commit -m "feat: launch fresh Type-2 campaign guests"
```

## Task 13: Implement Attempt Arming, Phase Barriers, and Block-Order Execution

**Files:**
- Create: `targets/qemu-type2/cxlmem_campaign/runner.py`
- Modify: `targets/qemu-type2/tests/test_runner_state.py`

- [ ] **Step 1: Write state-transition and retry-order tests**

Use fake launcher, oracle, guest, and artifact adapters. Test:

- four primary attempts in block 1 run before its first pre-arm retry;
- a pre-arm failure creates `FAILED.json`, remains retry-eligible, and receives
  attempt number 2;
- a post-arm failure immediately creates `CAMPAIGN_FAILED` and no replacement;
- an attempt after a committed attempt is rejected;
- every setup, warmup, timed, and corruption phase has start/end snapshots;
- start/end snapshots require `in_flight_requests == 0`;
- an unexpected phase ID fails the committed attempt;
- each committed boot performs one complete warmup list and one complete
  timed list.

Use this expected prefix:

```python
expected = [
    ("latency-00080ns/replicate-01", 1),
    ("latency-00400ns/replicate-01", 1),
    ("latency-02000ns/replicate-01", 1),
    ("latency-10000ns/replicate-01", 1),
    ("latency-00080ns/replicate-01", 2),
]
```

- [ ] **Step 2: Run runner tests and verify RED**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest \
  targets/qemu-type2/tests/test_runner_state.py -v
```

Expected: unresolved `CampaignRunner`.

- [ ] **Step 3: Implement one phase barrier**

Add:

```python
def run_phase(session, oracle, qmp, qemu_client_id, phase_id, snapshots):
    ready = session.expect_ready()
    if ready != phase_id:
        raise CampaignError(
            f"expected READY {phase_id}, received READY {ready}"
        )
    qmp.set_slugarch_phase_id(phase_id)
    server_before = oracle.counter_snapshot(qemu_client_id)
    qemu_before = qmp.slugarch_counter_snapshot()
    if server_before["in_flight_requests"] != 0:
        raise CampaignError(f"{phase_id} start snapshot has in-flight requests")
    snapshots.append({
        "phase_id": phase_id,
        "boundary": "start",
        "server": server_before,
        "qemu": qemu_before,
    })
    session.send_go(phase_id)
    done = session.expect_done()
    if done["phase_id"] != phase_id:
        raise CampaignError(
            f"expected DONE {phase_id}, received DONE {done['phase_id']}"
        )
    server_after = oracle.counter_snapshot(qemu_client_id)
    qemu_after = qmp.slugarch_counter_snapshot()
    if server_after["in_flight_requests"] != 0:
        raise CampaignError(f"{phase_id} end snapshot has in-flight requests")
    snapshots.append({
        "phase_id": phase_id,
        "boundary": "end",
        "server": server_after,
        "qemu": qemu_after,
    })
    qmp.set_slugarch_phase_id("idle")
    return done
```

The real function persists each QMP command/reply, counter snapshot, and guest
declaration before advancing. It verifies the `qom-set` value by reading it
back. A failure before clearing the phase ID invalidates the attempt. After the
sentinel, the validator rejects `phase_id="idle"` or any stale/mismatched
`phase_id` in a QEMU event and rejects every QEMU request outside these
intervals.

- [ ] **Step 4: Implement sentinel and durable arm sequence**

For every boot:

1. verify QEMU/server counters are zero;
2. set and read back QOM `slugarch-phase-id=preflight-sentinel`;
3. oracle-write deterministic 64-byte sentinel at DPA 80 MiB;
4. permit one guest devdax read and verify SHA-256;
5. permit one guest devdax inverse-sentinel write;
6. oracle-read it and verify SHA-256;
7. snapshot QEMU/server counters, restore and read back the QOM phase ID as
   `idle`, and
   persist matching IDs, offsets, operations, and hashes;
8. wait for `ARM_REQUEST`;
9. require zero in-flight requests;
10. hash the complete preflight directory;
11. call `arm_attempt`;
12. send `ARM_ACK`.

A failure through step 10 is pre-arm. A failure after step 11 is post-arm even
when the host dies before sending `ARM_ACK`.

- [ ] **Step 5: Implement attempt execution**

`run_attempt(slot, attempt_number)`:

- creates the exact attempt path with a new UUID;
- launches fresh server/QEMU/backing/overlay;
- records manifest and all commands before execution;
- performs topology and sentinel preflight;
- arms durably;
- runs all separately bracketed setup phases;
- runs the complete warmup plan;
- runs the complete timed plan;
- runs the one labeled corruption phase;
- copies raw guest outputs;
- stops owned processes;
- invokes attempt validation;
- seals `COMPLETE` only if validation passes;
- seals `FAILED.json` otherwise.

Never delete an attempt directory or reuse a guest/server UUID.

- [ ] **Step 6: Implement block scheduling**

`CampaignRunner.run()` iterates the five approved latency blocks. It completes
each block’s four attempt-1 launches before processing that block’s pre-arm
retry queue in original latency order. A retry that fails pre-arm returns to
the end of that block’s retry queue with its next attempt number. Operator
interruption leaves the same registered campaign resumable; it does not create
a new campaign.

The first attempt with a valid arm marker is committed. A post-arm failure
immediately seals and finalizes the campaign as failed.

- [ ] **Step 7: Run runner state tests**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest \
  targets/qemu-type2/tests/test_runner_state.py -v
```

Expected: all retry, arm, barrier, block-order, and failure-transition tests
pass.

- [ ] **Step 8: Commit the attempt runner**

Run:

```bash
git add targets/qemu-type2/cxlmem_campaign/runner.py \
  targets/qemu-type2/tests/test_runner_state.py
git commit -m "feat: orchestrate armed Type-2 attempts"
```

## Task 14: Implement Cross-Source Attempt and Campaign Validation

**Files:**
- Create: `targets/qemu-type2/cxlmem_campaign/validate.py`
- Create: `targets/qemu-type2/tests/test_validation.py`
- Modify: `targets/qemu-type2/tests/campaign_fixtures.py`

- [ ] **Step 1: Build a complete synthetic valid-attempt fixture**

`campaign_fixtures.py` generates, in a temporary directory:

- manifest and arm marker;
- topology and two-way sentinel evidence;
- guest condition JSONL;
- QEMU completion/delay/path JSONL;
- gzip server completion JSONL;
- start/end snapshots;
- raw scaled response streams;
- 20 warmup rows, 20 timed rows, and one corruption row;
- matching source and binary hashes.

Use small scalar counts in generic join fixtures, but use exact 4,096 reads
and 32,768 bytes in the calibration fixture.

- [ ] **Step 2: Write one positive and focused negative tests**

Create tests that independently mutate:

- one server request ID;
- one QEMU delay event;
- one QEMU completion status;
- one requested delay value;
- one returned modeled latency;
- one server configured base latency;
- one delay overshoot;
- one `delay_undershot` value;
- one phase’s byte total;
- one local-cache count;
- one request outside a phase;
- one baseline pair;
- one timed condition row;
- the corruption result to accepted;
- work-window start into the bulk bypass;
- one frozen input hash;
- a reused attempt ID in two different slots;
- a reused guest-boot UUID in two different slots; and
- a reused server-instance UUID in two different slots.

Every mutation must change the validation result to `valid=false` with one
specific error code.

- [ ] **Step 3: Run validation tests and verify RED**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_validation.py -v
```

Expected: import failure for `cxlmem_campaign.validate`.

- [ ] **Step 4: Implement strict readers**

Reject duplicate JSON keys with `object_pairs_hook`. Reject NaN, infinity,
negative durations, integers outside unsigned 64-bit range, malformed gzip,
non-canonical IDs, duplicate condition IDs, and trailing partial JSONL.

Parse guest, QEMU, and server events into dictionaries keyed by:

```text
(client_id, request_id, server_sequence)
```

Require a one-to-one set equality join. For every joined response:

- operation, DPA, length, payload SHA-256, and status match;
- the server's configured base latency equals the attempt slot and manifest
  latency;
- QEMU returned modeled latency equals server modeled latency and QEMU
  requested delay;
- exactly one QEMU delay application exists;
- `delay_undershot` is false;
- actual applied delay is at least requested delay; and
- `delay_overshoot_ns == applied_delay_ns - requested_delay_ns`.

- [ ] **Step 5: Implement phase and path validation**

For each phase, subtract bracketing snapshots and compare:

- byte totals across the guest declaration, QEMU direct-CFMWS counters, and
  server QEMU-client counters;
- guest logical-operation counts against the phase contract; and
- QEMU/server scalar-operation counts against each other and the exact
  access-width-derived phase contract.

Do not equate a guest logical transfer operation with scalar CXL.mem
transactions. In the transfer phase, one aligned logical read and one aligned
logical write of `size_bytes` must produce `size_bytes / 8` QEMU/server scalar
reads and writes because the frozen CFMWS maximum access size is eight bytes.
The positive fixture must include 4 KiB, where the guest declares one read and
one write while QEMU and the server each report 512 reads and 512 writes.

Require `bar4_overlay`, `local_shadow`, `local_cache`, `bulk_overlay`, and
`coherent_pool` completion counts to remain zero. Require
`bar4_overlay == bulk_overlay + coherent_pool` in every QOM snapshot and
path-counter event. Oracle traffic is retained but excluded by `client_role`.

Calibration requires exactly 4,096 reads, 32,768 read bytes, and no writes.
Transfers require exact size bytes in each direction. SlugArch conditions use
the exact serialized request/response sizes recorded before execution.

- [ ] **Step 6: Implement topology, matrix, and semantic validation**

Require:

- one `8086:0d92` endpoint;
- one Type-2 memdev and committed region;
- one devdax mapping covering the computed HPA offset;
- 256 MiB capacity and CFMWS;
- work DPA `[80 MiB,112 MiB)` above bulk end and below coherent-pool start;
- the frozen 21-entry one-MiB DPA map exactly matches both the host contract
  and guest configuration, has no gap or overlap, ends at 101 MiB, and every
  observed access is subtraction-safely contained in its assigned slot;
- the 1 MiB transfer fits exactly and every SlugArch request-plus-response
  layout equals its production-serialized manifest lengths and fits its
  one-MiB slot subtraction-safely, including 64-copy full mode;
- one complete warmup and one complete timed row for all 20 conditions;
- approved replicate mode order;
- `request_record_count` values 49/196/784/3,136;
- `boundary_record_count == request_record_count * 2`;
- uncorrupted validation success;
- one rejected `post_transport_guest_payload_flip`.

Invoke `slugarch validate-cxlmemsim-scaled` on every saved raw response stream
with its exact copy count. Treat any nonzero exit as attempt failure.

- [ ] **Step 7: Implement campaign completeness validation**

`validate_campaign` requires:

- a valid registry registration;
- one committed complete attempt in each of 20 slots;
- five complete boots per latency;
- no attempt after a committed attempt;
- all post-arm attempts complete;
- all identifiers and frozen hashes consistent;
- exactly 20 globally distinct committed attempt IDs, 20 globally distinct
  guest-boot UUIDs, and 20 globally distinct server-instance UUIDs;
- 20 of 20 corruption rejections;
- a valid attempt seal for every attempt.

Write `validation.json` before sealing. A failed validation preserves all
errors and makes the campaign terminally failed.

- [ ] **Step 8: Run validation tests**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_validation.py -v
```

Expected: the valid fixture passes and every single mutation fails with its
predeclared error code.

- [ ] **Step 9: Commit validation**

Run:

```bash
git add targets/qemu-type2/cxlmem_campaign/validate.py \
  targets/qemu-type2/tests/campaign_fixtures.py \
  targets/qemu-type2/tests/test_validation.py
git commit -m "feat: validate Type-2 campaign evidence"
```

## Task 15: Implement Eligible-Campaign Normalization and Paired Metrics

**Files:**
- Create: `targets/qemu-type2/cxlmem_campaign/normalize.py`
- Create: `targets/qemu-type2/tests/test_normalize.py`

- [ ] **Step 1: Write statistics and eligibility tests**

Cover:

- min/median/max over exactly five boot values;
- median of five paired ratios;
- a dataset where median ratios differ from ratio of medians;
- transfer bytes/s from exact byte/time integers;
- calibration least-squares slope over four medians;
- nondecreasing calibration gate;
- strict 10,000 ns greater-than-80 ns gate;
- separate read and write monotonic gates for every transfer size;
- rejection of `.inprogress`, `.failed`, unregistered, checksum-invalid, or
  later same-version campaigns;
- stable JSON/CSV hashes across two normalizations.

Use this anti-ratio-of-medians fixture:

```python
baseline = [1, 2, 100, 101, 102]
mode = [2, 4, 101, 202, 204]
paired = [2.0, 2.0, 1.01, 2.0, 2.0]
```

The paired median is `2.0`; the implementation must compute the five ratios
before summarizing.

- [ ] **Step 2: Run normalization tests and verify RED**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_normalize.py -v
```

Expected: import failure for `cxlmem_campaign.normalize`.

- [ ] **Step 3: Implement exact summary helpers**

Add:

```python
def summarize_five(values):
    ordered = sorted(values)
    if len(ordered) != 5:
        raise ValueError(f"expected five boot values, received {len(ordered)}")
    return {
        "values": list(values),
        "minimum": ordered[0],
        "median": ordered[2],
        "maximum": ordered[4],
    }


def paired_ratios(rows, mode):
    by_boot = {}
    for row in rows:
        key = (
            row["latency_ns"],
            row["replicate"],
            row["request_record_count"],
        )
        by_boot.setdefault(key, {})[row["mode"]] = row["end_to_end_ns"]
    ratios = []
    for key in sorted(by_boot):
        pair = by_boot[key]
        if "baseline" not in pair or mode not in pair:
            raise ValueError(f"missing same-boot pair {key} for {mode}")
        if pair["baseline"] <= 0:
            raise ValueError(f"nonpositive baseline for {key}")
        ratios.append(
            {
                "key": key,
                "numerator_ns": pair[mode],
                "denominator_ns": pair["baseline"],
                "ratio": pair[mode] / pair["baseline"],
            }
        )
    return ratios
```

Never compute a ratio from summarized medians.

- [ ] **Step 4: Produce observation-level data**

Write `observations.json` and `observations.csv` with one row per boot-level
timed observation. Include:

```text
campaign_id
experiment_version_sha256
campaign_checksum_sha256
registry_ordinal
attempt_id
guest_boot_uuid
server_instance_uuid
latency_ns
replicate
condition_kind
mode
copy_count
request_record_count
response_record_count
boundary_record_count
transfer_size_bytes
encode_ns
cxl_write_ns
cxl_read_ns
interpret_ns
validate_ns
end_to_end_ns
read_bytes
written_bytes
input_sha256
output_sha256
raw_artifact_path
```

Use empty JSON null/empty CSV fields for metrics that do not apply; never use
zero as “not applicable.”

- [ ] **Step 5: Produce the exact Figure 2 snapshot**

Write the one canonical paper schema also enforced by
`docs/superpowers/plans/2026-07-24-slugarch-paper-integration.md`:

```text
schema_version: 1
campaign:
  campaign_id
  experiment_version_sha256
  campaign_checksum_sha256
  registry_ordinal
  artifact_relative_path
  complete
  eligible
  committed_boots
  latencies_ns
  repeats_per_latency
  warmup_passes_per_boot
  timed_passes_per_boot
  corruption_rejections
panel_gates:
  protocol
  sentinel
  bypass
  artifact
  corruption
  calibration
  transfer
  paired_overhead
  record_scaling
validation:
  source_hashes_match
  protocol_counts_match
  byte_totals_match
  delay_event_join_complete
  zero_bypass_completions
  phase_barriers_valid
  no_failed_attempts_included
calibration: 20 boot-level rows
transfer: 60 boot/size rows
slugarch: 320 boot/request-record-count/mode rows
corruption: 20 boot-level rows
claim_limitations: nonempty string list
provenance: source hashes, registry history, raw relative paths, and exclusions
```

Every SlugArch row carries `copy_count`, `request_record_count`,
`response_record_count`, `boundary_record_count`, `mode`, `end_to_end_ns`,
the same boot's `baseline_end_to_end_ns`, and `paired_overhead`. The four
request counts are 49, 196, 784, and 3,136; response count equals request
count, and boundary count is twice request count. Calibration and transfer
rows retain their raw values; summaries are recomputed by the renderer.

Set `campaign.eligible=true` only when every validation flag and panel gate is
true. When a gate fails, retain the diagnostic raw summaries with
`campaign.eligible=false` and exact blocking gate codes in provenance, but do
not create the fixed paper handoff.

- [ ] **Step 6: Seal the external export and fixed paper handoff**

Acquire `.campaign.lock`, reverify registry eligibility and campaign
checksums, create a new export directory with `os.mkdir`, write the three data
files, write sorted `checksums.sha256`, then write `EXPORT_COMPLETE`. Refuse
an existing export path. Run normalization twice into two temporary
directories during tests and require identical file hashes.

Only when `campaign.eligible=true`, atomically create the fixed read-only
handoff directory `/tmp/slugarch-type2-cxlmem-paper-export/` from a sibling
temporary directory. It contains exactly:

```text
slugarch-type2-cxlmem.json
export-validation.json
export-checksums.sha256
```

`export-validation.json` records `status="pass"`,
`campaign_complete=true`, `campaign_checksum_tree_valid=true`,
`registry_hash_chain_valid=true`,
`selected_by_lowest_complete_ordinal=true`, `committed_slots=20`,
`committed_boots_per_latency=5`, `corruption_rejections=20`, and an empty
`failed_panel_gates` array. `export-checksums.sha256` covers the data JSON and
validation JSON. Refuse an existing final or temporary handoff directory; a
later run must never overwrite an earlier reviewed handoff.

- [ ] **Step 7: Run normalization tests**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_normalize.py -v
```

Expected: all statistical, monotonicity, eligibility, and deterministic-output
tests pass.

- [ ] **Step 8: Commit normalization**

Run:

```bash
git add targets/qemu-type2/cxlmem_campaign/normalize.py \
  targets/qemu-type2/tests/test_normalize.py
git commit -m "feat: normalize paired Type-2 campaign metrics"
```

## Task 16: Add the Campaign CLI, Documentation, and Raw-Artifact Ignore Rule

**Files:**
- Create: `targets/qemu-type2/cxlmem_campaign/cli.py`
- Create: `targets/qemu-type2/run_cxlmem_campaign.py`
- Modify: `targets/qemu-type2/README.md`
- Modify: `.gitignore`
- Modify: `targets/qemu-type2/tests/test_runner_state.py`

- [ ] **Step 1: Write CLI parsing and fail-closed tests**

Test that:

- `check`, `oracle-smoke`, `qemu-smoke`, `pilot`, `register-run`, `resume`,
  `validate`, `normalize`, and `inspect` appear in `--help`;
- every mutating command requires `--config`;
- `pilot` accepts only latency 400;
- `register-run` rejects a campaign ID that already exists;
- `normalize` rejects a pilot and failed campaign;
- no command accepts `--allow-regular-file`;
- command failures exit nonzero and preserve a machine-readable error.

- [ ] **Step 2: Run CLI tests and verify RED**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_runner_state.py -v
```

Expected: CLI imports or subcommands are missing.

- [ ] **Step 3: Implement the thin entry point**

Create `run_cxlmem_campaign.py`:

```python
#!/usr/bin/env python3
from cxlmem_campaign.cli import main


if __name__ == "__main__":
    raise SystemExit(main())
```

`cli.main(argv=None)` returns 0 only after the requested verification has
completed. It writes one canonical JSON status object to stdout and diagnostic
logs to stderr.

- [ ] **Step 4: Implement exact command behaviors**

- `check`: verify configuration, fixed matrix, all frozen paths, CPU-set
  disjointness, ports, static guest binary, and source hashes without launch.
- `oracle-smoke`: launch a fresh server, perform HELLO/ACK, write/read one
  64-byte sentinel, snapshot counters, stop server, and seal a non-paper smoke
  artifact.
- `qemu-smoke`: launch fresh server and QEMU with `-S`, verify QEMU’s SLT2
  handshake and legacy-layout rejection log, then stop both.
- `pilot`: run one complete unregistered 400 ns boot, validate it, and seal it
  under `pilots/`; it can never become paper data.
- `register-run`: reconcile, freeze inputs, register, run the 20 slots, seal,
  finalize, and append the terminal event while holding the lock.
- `resume`: continue the exact registered campaign after crash recovery; it
  cannot change config or experiment-version hash.
- `validate`: verify an existing finalized campaign without changing it.
- `normalize`: select the eligible complete campaign and create one immutable
  external export.
- `inspect`: acquire the same exclusive campaign lock, then report authoritative
  terminal or stopped-run status including current block/slot/attempt, arm
  state, owned PIDs, and the last guest/QEMU/server event. It must not bypass
  the lock while `register-run` or `resume` owns it. The running process emits
  non-authoritative progress objects to its own stdout for live monitoring.

- [ ] **Step 5: Separate raw artifacts from Git**

Append:

```gitignore
# Live Type-2 CXL.mem campaigns are immutable local evidence; reviewed
# normalized data is copied into the paper repository separately.
/artifact/slugarch_type2_cxlmem/
```

Do not ignore the existing `artifact/slugarch_cxlmemsim/` family.

- [ ] **Step 6: Document legacy and new paths separately**

Extend `targets/qemu-type2/README.md` with:

- “Legacy BAR2 evidence” using the existing scripts;
- “Type-2 CXL.mem campaign” using only `run_cxlmem_campaign.py`;
- exact four latency points and five independent boots;
- the request-record versus boundary-record definition;
- pilot/full campaign distinction;
- artifact immutability and failure rules;
- paper-safe simulator claim boundary.

- [ ] **Step 7: Run CLI tests and help smoke**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest targets/qemu-type2/tests/test_runner_state.py -v

PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py --help
```

Expected: tests pass and help lists all nine subcommands.

- [ ] **Step 8: Commit CLI and documentation**

Run:

```bash
git add .gitignore \
  targets/qemu-type2/cxlmem_campaign/cli.py \
  targets/qemu-type2/run_cxlmem_campaign.py \
  targets/qemu-type2/README.md \
  targets/qemu-type2/tests/test_runner_state.py
git commit -m "feat: expose the Type-2 campaign workflow"
```

## Task 17: Run the Complete Offline Verification Gate

**Files:** all implementation files from Tasks 1 through 16.

- [ ] **Step 1: Format Rust**

Run:

```bash
cargo fmt --check \
  --package slugarch-cxl-replay \
  --package slugarch-type2-guest \
  --package slugarch-host \
  --package slugarch-cli
```

Expected: exit 0 with no diff.

- [ ] **Step 2: Run replay and guest tests**

Run:

```bash
cargo test -p slugarch-cxl-replay
cargo test -p slugarch-type2-guest
```

Expected: all tests pass.

- [ ] **Step 3: Run host artifact regressions**

Run:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test qemu_type2_artifacts

env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test sim_feasible
```

Expected: all legacy BAR2 and simulator-feasible tests still pass.

- [ ] **Step 4: Run all campaign unit tests**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest discover -s targets/qemu-type2/tests -v
```

Expected: all contract, provenance, artifact, registry, wire, state, validation,
and normalization tests pass.

- [ ] **Step 5: Check scripts and JSON**

Run:

```bash
python3 -m py_compile \
  targets/qemu-type2/run_cxlmem_campaign.py \
  targets/qemu-type2/cxlmem_campaign/*.py

jq empty targets/qemu-type2/cxlmem-campaign-defaults.json
```

Expected: both commands exit 0.

- [ ] **Step 6: Build and copy the frozen static guest**

Run:

```bash
RUSTFLAGS="-C target-feature=+crt-static" \
  cargo build --release -p slugarch-type2-guest
test -f /tmp/slugarch-type2-cxlmem-build/TRANSPORT_SHA256SUMS
sha256sum -c /tmp/slugarch-type2-cxlmem-build/TRANSPORT_SHA256SUMS
test ! -e /tmp/slugarch-type2-cxlmem-build/slugarch-type2-guest
install -m 0555 target/release/slugarch-type2-guest \
  /tmp/slugarch-type2-cxlmem-build/slugarch-type2-guest
file /tmp/slugarch-type2-cxlmem-build/slugarch-type2-guest
ldd /tmp/slugarch-type2-cxlmem-build/slugarch-type2-guest
sha256sum /tmp/slugarch-type2-cxlmem-build/slugarch-type2-guest \
  > /tmp/slugarch-type2-cxlmem-build/GUEST_SHA256SUM
```

Expected: the copy is static, `ldd` reports `not a dynamic executable`, and
the frozen transport bundle still verifies before one separate guest SHA-256
line is written.

- [ ] **Step 7: Verify no campaign output was accidentally staged**

Run:

```bash
git status --short
git diff --check
git check-ignore \
  artifact/slugarch_type2_cxlmem/campaign-registry.jsonl
```

Expected: no raw campaign path is staged, `git diff --check` exits 0, and
`git check-ignore` names the Type-2 artifact ignore rule.

- [ ] **Step 8: Commit any test-only corrections**

If and only if the offline gate required code corrections, stage only the
files changed for those corrections and commit:

```bash
git commit -m "test: harden Type-2 campaign validation"
```

If no correction was required, do not create an empty commit.

## Task 18: Freeze an Isolated Source Snapshot and Pass Pre-Pilot Gates

**Files:**
- Runtime source: `/tmp/slugarch-type2-campaign-src`
- Runtime paper worktree: `/tmp/slugarch-paper-integration`
- Runtime smoke artifacts under:
  `/root/Concordia/SlugArch/artifact/slugarch_type2_cxlmem/pilots`

- [ ] **Step 1: Create a detached SlugArch worktree**

Run:

```bash
git worktree add --detach /tmp/slugarch-type2-campaign-src HEAD
git -C /tmp/slugarch-type2-campaign-src status --short
git -C /tmp/slugarch-type2-campaign-src rev-parse HEAD
```

Expected: the worktree is clean and prints the implementation commit.

- [ ] **Step 2: Verify all frozen inputs from the isolated worktree**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py check \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json
```

Expected JSON contains `"status":"pass"`, disjoint CPU sets, protocol version
1, 256 MiB capacity, the exact four latencies, and 20 slots.

- [ ] **Step 3: Run the server/oracle protocol smoke**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py oracle-smoke \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --smoke-id oracle-smoke-20260724
```

Expected: HELLO/ACK version 1, a successful 64-byte write/read round trip,
nonzero oracle counters, zero failed requests, and a sealed smoke artifact.

- [ ] **Step 4: Run the no-guest QEMU protocol smoke**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py qemu-smoke \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --smoke-id qemu-smoke-20260724
```

Expected: QEMU negotiates SLT2 v1, receives a nonzero client ID, exposes the
one-target 256 MiB Type-2 topology, and the smoke validator finds no
legacy-layout receive thread.

- [ ] **Step 5: Verify the empty-axis paper mockup gate**

Run:

```bash
cd /tmp/slugarch-paper-integration
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/build_slugarch_layout_mockup.py
pdfinfo /tmp/slugarch-paper-layout/paper/main.pdf
pdftotext -layout \
  /tmp/slugarch-paper-layout/paper/main.pdf \
  /tmp/slugarch-paper-integration-mockup.txt
```

Expected:

- `img/slugarch-results.pdf` and
  `img/slugarch-type2-cxlmem.pdf` are each 7.1 by 3.0 inches;
- all figure text is at least 7 points;
- the evaluation uses no more than three main-text pages;
- the conclusion ends on or before page 11;
- no LaTeX error, unresolved reference, or unresolved citation appears.

Do not run the pilot until all five steps pass.

## Task 19: Run and Validate the Unregistered 400 ns Pilot

**Files:**
- Generated pilot:
  `artifact/slugarch_type2_cxlmem/pilots/pilot-400ns-20260724`

- [ ] **Step 1: Launch exactly one complete pilot boot**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py pilot \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --pilot-id pilot-400ns-20260724 \
  --latency-ns 400 \
  --replicate 1
```

Expected: one fresh server and guest, successful topology/sentinel preflight,
durable arm, 20 warmup conditions, 20 timed conditions, one rejected
corruption, and a sealed pilot with `validation.json.valid=true`.

- [ ] **Step 2: Inspect the pilot**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py inspect \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --pilot-id pilot-400ns-20260724
```

Expected JSON reports:

- `state=complete`;
- one committed attempt;
- 4,096 calibration reads and 32,768 read bytes;
- exact transfer byte totals;
- complete baseline/mode pairs;
- `request_record_count=196` and `boundary_record_count=392` for the canonical
  across-latency condition;
- zero local/bypass completions;
- zero missing/duplicate/undershot delay events;
- one `decoded_result_mismatch` corruption rejection.

- [ ] **Step 3: Independently verify pilot seals**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py validate \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --pilot-id pilot-400ns-20260724
```

Expected: validation exits 0 and reports that the pilot is valid but
categorically ineligible for paper normalization.

- [ ] **Step 4: Stop on any pilot failure**

If any pilot gate fails, preserve the sealed pilot and return to the relevant
implementation task. Do not register the full campaign. A code, protocol, or
methodology correction changes the experiment-version hash.

## Task 20: Register and Run the Full 20-Slot Campaign

**Files:**
- Generated registry:
  `artifact/slugarch_type2_cxlmem/campaign-registry.jsonl`
- Generated campaign:
  `artifact/slugarch_type2_cxlmem/slugarch-type2-cxlmem-v1-20260724`

- [ ] **Step 1: Re-run the complete offline and preflight checks**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest discover -s targets/qemu-type2/tests -v
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py check \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json
```

Expected: all unit tests and the frozen-input check pass.

- [ ] **Step 2: Register before the first launch and run in block order**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py register-run \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --campaign-id slugarch-type2-cxlmem-v1-20260724 \
  --change-reason initial-approved-type2-cxlmem-campaign
```

Expected behavior:

- append and fsync one `REGISTERED` event before starting a server;
- hold `.campaign.lock` through the terminal event;
- run five balanced blocks in the approved order;
- use one new server, QEMU, memory backing, disk overlay, attempt ID,
  guest-boot UUID, and server-instance UUID per launch;
- never replace a post-arm result;
- finalize only with 20 committed complete boots or a terminal failed
  campaign.

- [ ] **Step 3: Resume only the same registered campaign after process loss**

If the runner process exits without a terminal event, run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py resume \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --campaign-id slugarch-type2-cxlmem-v1-20260724
```

Expected: registry and seals are reconciled; a valid arm marker is treated as
committed; only a pre-arm slot receives the next attempt number.

- [ ] **Step 4: Inspect without changing state after the runner exits**

After `register-run` exits, or after an interrupted runner has released the
campaign lock, run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py inspect \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --campaign-id slugarch-type2-cxlmem-v1-20260724
```

Expected: current block, latency, replicate, attempt, arm state, owned PIDs,
and last event are reported without modifying the campaign. While a runner
owns the lock, use only its stdout progress objects; do not launch
authoritative `inspect`.

- [ ] **Step 5: Verify terminal state**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py validate \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --campaign-id slugarch-type2-cxlmem-v1-20260724
```

Expected for success: 20 committed complete boots, five per latency, 20
corruption rejections, valid seals, valid registry terminal event, and no
failed general/panel gates.

If the campaign is terminally failed, stop. Preserve it and do not normalize
or silently register a same-version replacement.

## Task 21: Normalize, Verify Determinism, and Hand Off Figure 2 Data

**Files:**
- Generated immutable export:
  `artifact/slugarch_type2_cxlmem/exports/` followed by the campaign ID, a
  hyphen, and the verified campaign-checksum SHA-256

- [ ] **Step 1: Normalize the eligible campaign**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py normalize \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --campaign-id slugarch-type2-cxlmem-v1-20260724
```

Expected: one sealed export containing `observations.csv`,
`observations.json`, `slugarch-type2-cxlmem.json`, and
`campaign.eligible=true` with every `panel_gates` and `validation` value true.

- [ ] **Step 2: Resolve the exact export path without guessing the hash**

Run:

```bash
find /root/Concordia/SlugArch/artifact/slugarch_type2_cxlmem/exports \
  -mindepth 1 -maxdepth 1 -type d \
  -name 'slugarch-type2-cxlmem-v1-20260724-*' \
  -print
```

Expected: exactly one directory. More than one matching export is a failure.

- [ ] **Step 3: Re-run normalization as a deterministic verification**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py normalize \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --campaign-id slugarch-type2-cxlmem-v1-20260724 \
  --verify-existing
```

Expected: all regenerated bytes match the sealed export; no file is replaced.

- [ ] **Step 4: Inspect the normalized scientific gates**

Run:

```bash
jq '{
  campaign_eligible: .campaign.eligible,
  boot_count: .campaign.committed_boots,
  calibration_gate: .panel_gates.calibration,
  transfer_gate: .panel_gates.transfer,
  corruption_rejections: .campaign.corruption_rejections,
  canonical_request_records:
    ([.slugarch[]
      | select(.request_record_count == 196)]
      | .request_record_count] | unique | first),
  canonical_boundary_records:
    ([.slugarch[]
      | select(.request_record_count == 196)]
      | .boundary_record_count] | unique | first)
}' \
  /root/Concordia/SlugArch/artifact/slugarch_type2_cxlmem/exports/slugarch-type2-cxlmem-v1-20260724-*/slugarch-type2-cxlmem.json
```

Expected:

```json
{
  "campaign_eligible": true,
  "boot_count": 20,
  "calibration_gate": true,
  "transfer_gate": true,
  "corruption_rejections": 20,
  "canonical_request_records": 196,
  "canonical_boundary_records": 392
}
```

- [ ] **Step 5: Verify and hand off only the sealed reviewed snapshot**

Run:

```bash
cd /tmp/slugarch-type2-cxlmem-paper-export
sha256sum -c export-checksums.sha256
jq -e '
  .status == "pass" and
  .campaign_complete == true and
  .campaign_checksum_tree_valid == true and
  .registry_hash_chain_valid == true and
  .selected_by_lowest_complete_ordinal == true and
  .committed_slots == 20 and
  .committed_boots_per_latency == 5 and
  .corruption_rejections == 20 and
  (.failed_panel_gates | length) == 0
' export-validation.json
```

Expected: both handoff payload checksums print `OK`, and `jq` prints `true`.
The paper-integration plan then copies:

```text
/tmp/slugarch-type2-cxlmem-paper-export/slugarch-type2-cxlmem.json
```

byte-for-byte to
`/tmp/slugarch-paper-integration/data/slugarch-type2-cxlmem.json`. It must
verify the handoff checksum before and after copying. It must not copy raw
attempt logs, modify the finalized campaign, or use a pilot, failed, or later
same-version campaign.

- [ ] **Step 6: Record the execution boundary**

Update no source file from observed timing values. The execution handoff
records:

- implementation commit;
- experiment-version SHA-256;
- registry ordinal and chain hash;
- campaign checksum SHA-256;
- export checksum SHA-256;
- exact normalized data path;
- whether all Figure 2 gates passed;
- every failed campaign or version transition disclosed by the registry.

The manuscript may consume the numbers only after this handoff and the
separate plot/paper plan’s deterministic render and 11-page checks pass.

---

## Final Completion Checklist

- [ ] Legacy BAR2 helper and artifacts remain unchanged.
- [ ] The 49-request trace hash remains
  `f9f05b04d9352de8e0213c42e5efb46f56b05863e077d9cf1ce47a9ddef2b75c`.
- [ ] Every data row separates request, response, and boundary record counts.
- [ ] Guest binary is static and pinned to guest vCPU 1.
- [ ] Every attempt has a fresh server, QEMU, backing file, and disk overlay.
- [ ] The work window is devdax/CFMWS-backed and outside both bypasses.
- [ ] Sentinel proof succeeds in both directions.
- [ ] Durable arm precedes every warmup or timed access.
- [ ] Every DAX phase has zero-in-flight start/end snapshots.
- [ ] Every server completion joins exactly one QEMU delay event.
- [ ] All local/BAR/bypass completion counters are zero.
- [ ] The full campaign contains 20 committed complete boots.
- [ ] All 20 labeled corruptions are rejected.
- [ ] Paired ratios are computed per boot before median/min/max.
- [ ] Calibration and transfer monotonicity gates pass.
- [ ] Registry, attempt, campaign, and export checksums verify.
- [ ] The lowest eligible registration supplies the paper data.
- [ ] Figure 2 data contains no preview, estimated, interpolated, or reference
  manuscript numbers.
