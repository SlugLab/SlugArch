# SlugArch JIT FPGA RTL and Verilator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the SlugArch Hardware-JIT pipeline runtime-programmable, compile its HJ top with Verilator, and prove that its records and failure behavior match the Rust policy oracle.

**Architecture:** Refactor `slugcxl-gen` into a reusable library that consumes `slugarch-jit::VerifiedPolicy` and emits one versioned 128-bit instruction encoding. Generated RTL owns two 32-word banks and atomically commits only a complete version-1 image. A dedicated Verilator shim loads programs, submits local 64-byte SlugCXL events, retrieves fixed semantic records, and exposes counters; the FFI's FPGA backend wraps that model while the Rust interpreter remains the oracle.

**Tech Stack:** Rust 2021, SystemVerilog, Verilator 5.x, C++17 shim, Cargo build scripts, optional Quartus Pro.

---

## File Structure

- Create `crates/slugcxl-gen/src/lib.rs`: public generator API.
- Create `crates/slugcxl-gen/src/emit_policy_image.rs`: 128-bit instruction
  image and JSON/hex exports.
- Modify `crates/slugcxl-gen/src/main.rs`: thin CLI over the library.
- Modify `crates/slugcxl-gen/src/config.rs`: consume shared policy types.
- Modify `crates/slugcxl-gen/src/emit_hj_pipeline.rs`: dual-bank loader and
  bounded interpreter RTL.
- Modify `crates/slugcxl-gen/src/emit_hj_top.rs`: policy and record ports.
- Modify `crates/slugcxl-gen/src/emit_fit_top.rs`: drive a valid default policy
  and preserve counters.
- Modify `crates/slugcxl-gen/src/emit_runtime.rs`: versions, instruction
  encoding, limits, and default-policy digest.
- Modify `crates/slugcxl-gen/Cargo.toml`: library and `slugarch-jit`.
- Regenerate `targets/agilex-vr2/generated/slugcxl_hj_pipeline.sv`.
- Regenerate `targets/agilex-vr2/generated/slugcxl_4x4_hj_top.sv`.
- Regenerate `targets/agilex-vr2/generated/slugcxl_hj_fit_top.sv`.
- Create `targets/agilex-vr2/generated/slugcxl_hj_policy.hex`.
- Modify `targets/agilex-vr2/generated/slugcxl_endpoint_runtime.json`.
- Create `targets/agilex-vr2/generated/slugcxl_hj_policy.json`.
- Modify `crates/slugarch-verilator-sys/build.rs`: HJ compile unit.
- Modify `crates/slugarch-verilator-sys/shim/ip_shim.h`: `SlugarchHj` ABI.
- Modify `crates/slugarch-verilator-sys/shim/ip_shim.cpp`: HJ drive/receive.
- Modify `crates/slugarch-verilator-sys/src/lib.rs`: generated bindings.
- Modify `crates/slugarch-verilator/src/lib.rs`: export `VerilatedHj`.
- Create `crates/slugarch-verilator/src/hj.rs`: safe HJ wrapper.
- Modify `crates/slugarch-jit-ffi/Cargo.toml`: optional FPGA feature.
- Create `crates/slugarch-jit-ffi/src/fpga.rs`: model backend.
- Create `crates/slugarch-jit-ffi/tests/rtl_equivalence.rs`.

## Normative Instruction Encoding

Each instruction is one little-endian 128-bit word:

| Bits | Field |
| ---: | --- |
| `7:0` | opcode |
| `15:8` | argument 0 |
| `23:16` | forward skip |
| `31:24` | flags/reserved |
| `63:32` | argument 1 |
| `127:64` | argument 2 |

Opcodes are:

```text
0x00 HALT
0x01 MATCH_CLASS
0x02 MATCH_DIRECTION
0x03 MATCH_STATUS
0x04 MATCH_RANGE
0x05 SAMPLE
0x06 CAPTURE
0x07 EMIT
0x08 EPOCH_INCREMENT
0x09 EPOCH_FROM_PHASE
0x0a REJECT
```

Reserved bits are zero. The Rust encoder and generated SystemVerilog package
must use these exact values.

The program image starts with a 64-byte header:

```text
0x00 magic "SJIT"
0x04 ABI version u32 = 1
0x08 event version u32 = 1
0x0c packet version u32 = 1
0x10 instruction count u32
0x14 range count u32
0x18 metadata budget u32
0x1c image bytes u32
0x20 policy digest [32]
```

It is followed by 32 fixed instruction slots and four fixed
`{base u64, length u64}` range slots. Unused slots are zero.

## Task 1: Refactor `slugcxl-gen` into a Library

**Files:**
- Create: `crates/slugcxl-gen/src/lib.rs`
- Modify: `crates/slugcxl-gen/src/main.rs`
- Modify: `crates/slugcxl-gen/Cargo.toml`
- Create: `crates/slugcxl-gen/tests/library_api.rs`

- [ ] **Step 1: Write the failing library API test**

```rust
use slugcxl_gen::{generate, GenerateOptions};

#[test]
fn library_emits_hj_artifacts_without_process_exit() {
    let dir = tempfile_dir();
    let outputs = generate(&GenerateOptions {
        out: dir.clone(),
        hardware_jit: true,
        quartus_project: None,
        policy_path: None,
    }).unwrap();
    assert!(outputs.iter().any(|p| p.ends_with("slugcxl_hj_pipeline.sv")));
    assert!(dir.join("slugcxl_hj_policy.hex").is_file());
}
```

Use a test-local temporary-directory helper implemented with
`std::env::temp_dir`, PID, and an atomic counter; do not add an unnecessary
tempfile dependency.

- [ ] **Step 2: Run and confirm RED**

```bash
cargo test -p slugcxl-gen --test library_api
```

Expected: unresolved crate/module API.

- [ ] **Step 3: Move modules and writing into the library**

`src/lib.rs` exports:

```rust
#[derive(Debug, Clone)]
pub struct GenerateOptions {
    pub out: std::path::PathBuf,
    pub hardware_jit: bool,
    pub quartus_project: Option<std::path::PathBuf>,
    pub policy_path: Option<std::path::PathBuf>,
}

pub fn generate(options: &GenerateOptions) -> anyhow::Result<Vec<std::path::PathBuf>>;
```

`main.rs` parses CLI options, constructs `GenerateOptions`, calls `generate`,
and prints paths. It contains no emitter logic.

- [ ] **Step 4: Run tests and snapshots**

```bash
cargo test -p slugcxl-gen
```

Expected: existing snapshots plus the library API pass unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/slugcxl-gen
git commit -m "generator: expose the SlugCXL emitter as a library"
```

## Task 2: Encode Verified Policies Once

**Files:**
- Create: `crates/slugcxl-gen/src/emit_policy_image.rs`
- Modify: `crates/slugcxl-gen/src/lib.rs`
- Modify: `crates/slugcxl-gen/src/emit_runtime.rs`
- Create: `crates/slugcxl-gen/tests/policy_image.rs`

- [ ] **Step 1: Write exact-byte tests**

The test compiles one policy containing `MATCH_CLASS`, `CAPTURE`, `EMIT`,
`EPOCH_FROM_PHASE`, and `HALT`, then asserts the first instruction word:

```rust
assert_eq!(image[64], 0x01);
assert_eq!(image[65], EventClass::CxlMemWrite as u8);
assert_eq!(image[66], 1);
assert!(image[67..80].iter().all(|byte| *byte == 0));
assert_eq!(&image[0..4], b"SJIT");
assert_eq!(&image[0x20..0x40], &verified.digest);
```

Also test zero-filled unused slots, 64-byte header, exact total size, and
round-trip decode.

- [ ] **Step 2: Confirm RED**

```bash
cargo test -p slugcxl-gen --test policy_image
```

Expected: missing encoder.

- [ ] **Step 3: Implement the image encoder/decoder**

Use explicit little-endian stores. Never transmute Rust enums or structures.
Reject images whose reserved bytes are nonzero, versions differ, instruction
count exceeds 32, range count exceeds four, or digest differs from the
caller-provided verified policy.

- [ ] **Step 4: Emit runtime artifacts**

For the default validation policy, emit:

```text
slugcxl_hj_policy.hex
slugcxl_hj_policy.json
```

The JSON contains versions, digest, instruction words, ranges, and source
canonical policy. Runtime JSON points to both files by relative path.

- [ ] **Step 5: Test and commit**

```bash
cargo test -p slugcxl-gen --test policy_image
git add crates/slugcxl-gen
git commit -m "generator: encode verified Hardware-JIT policies"
```

## Task 3: Add a Runtime-Loadable Dual-Bank RTL Policy Store

**Files:**
- Modify: `crates/slugcxl-gen/src/emit_hj_pipeline.rs`
- Modify: `crates/slugcxl-gen/src/emit_hj_top.rs`
- Modify: `crates/slugcxl-gen/src/emit_fit_top.rs`
- Modify: snapshots under `crates/slugcxl-gen/src/snapshots/`
- Create: `crates/slugcxl-gen/tests/hj_contract.rs`

- [ ] **Step 1: Write generated-contract tests**

Assert the emitted RTL contains:

```rust
for token in [
    "policy_load_begin",
    "policy_load_valid",
    "policy_load_ready",
    "policy_load_word",
    "policy_load_commit",
    "policy_load_abort",
    "active_bank",
    "policy_digest",
    "record_valid",
    "record_ready",
    "record_data",
] {
    assert!(rtl.contains(token), "missing {token}");
}
assert!(!rtl.contains("parameter integer RECORD_MODE"));
```

- [ ] **Step 2: Confirm RED**

```bash
cargo test -p slugcxl-gen --test hj_contract
```

Expected: missing runtime loader signals and old elaboration parameter found.

- [ ] **Step 3: Emit the dual-bank store**

The generated module has:

```systemverilog
input  wire         policy_load_begin,
input  wire         policy_load_valid,
output wire         policy_load_ready,
input  wire [15:0]  policy_load_index,
input  wire [127:0] policy_load_word,
input  wire         policy_load_commit,
input  wire         policy_load_abort,
input  wire [255:0] policy_load_digest,
input  wire [31:0]  policy_load_instruction_count,
input  wire [31:0]  policy_load_range_count,
output reg          policy_ready,
output reg  [31:0]  policy_error,
output reg  [255:0] policy_digest,
```

Two arrays hold 32 instruction words and four range pairs per bank. Begin
clears only the inactive bank and load counters. Commit succeeds only after
exact declared words arrive with no duplicate/out-of-range index. Abort or
reset leaves no new active policy. Commit swaps the bank and increments epoch
atomically.

- [ ] **Step 4: Emit a fixed semantic record stream**

Add:

```systemverilog
output reg          record_valid,
input  wire         record_ready,
output reg  [15:0]  record_length,
output reg  [1023:0] record_data,
```

The fixed little-endian record contains versions, sequence, event ID, digest,
epoch, direction, class, opcode, address, tag, status, capture kind/length,
hash, delta pairs, or full payload. Bytes above `record_length` are zero.

If `record_valid && !record_ready`, hold all record fields stable and count
stall cycles. A second required record while one is pending increments drop
count and sets sticky error; strict QEMU treats that as fatal.

- [ ] **Step 5: Update snapshots and run tests**

```bash
INSTA_UPDATE=always cargo test -p slugcxl-gen
git diff -- crates/slugcxl-gen/src/snapshots
```

Expected: only intentional runtime-loader/record changes.

- [ ] **Step 6: Commit**

```bash
git add crates/slugcxl-gen
git commit -m "rtl: add runtime-loadable Hardware-JIT policies"
```

## Task 4: Implement the Bounded RTL Interpreter

**Files:**
- Modify: `crates/slugcxl-gen/src/emit_hj_pipeline.rs`
- Create: `crates/slugcxl-gen/tests/hj_opcode_contract.rs`
- Create: `crates/slugarch-verilator-sys/tests/hj_policy_smoke.rs`

- [ ] **Step 1: Write opcode and timeout tests**

Test every opcode value and a 32-instruction worst-case program. Include
unsupported opcode, forward skip out of range, no halt, record backpressure,
abort, partial load, and 33-word load cases with exact sticky error codes.

- [ ] **Step 2: Confirm RED**

```bash
cargo test -p slugcxl-gen --test hj_opcode_contract
```

Expected: generated RTL lacks matching interpreter cases.

- [ ] **Step 3: Emit the interpreter FSM**

States are:

```text
IDLE
FETCH
DECODE
MATCH
CAPTURE
EMIT_WAIT
REJECT
DONE
ERROR
```

One event is latched only when `policy_ready`. Program counter starts at zero,
increments or applies a verified forward skip, and has a 32-step watchdog.
`HALT` returns to IDLE. Unsupported op, bad range index, or watchdog expiry
enters ERROR without emitting a success record.

Hash exactly payload bytes `0..payload_len`; delta emits only nonzero
`(index,value)` pairs; full capture copies exactly the declared prefix.

- [ ] **Step 4: Lint generated RTL**

```bash
verilator --lint-only --Wall -Wno-fatal \
  targets/agilex-vr2/generated/slugcxl_hj_pipeline.sv
```

Expected: exit 0 and no width, latch, incomplete-case, or multiple-driver
warning in authored HJ logic.

- [ ] **Step 5: Commit**

```bash
git add crates/slugcxl-gen targets/agilex-vr2/generated
git commit -m "rtl: execute bounded SlugArch JIT programs"
```

## Task 5: Compile and Drive the HJ Top with Verilator

**Files:**
- Modify: `crates/slugarch-verilator-sys/build.rs`
- Modify: `crates/slugarch-verilator-sys/shim/ip_shim.h`
- Modify: `crates/slugarch-verilator-sys/shim/ip_shim.cpp`
- Modify: `crates/slugarch-verilator/src/lib.rs`
- Create: `crates/slugarch-verilator/src/hj.rs`
- Create: `crates/slugarch-verilator/tests/hj_smoke.rs`

- [ ] **Step 1: Write the failing safe-wrapper smoke**

```rust
#[test]
fn load_policy_and_observe_one_write() {
    let verified = validation_policy().verify().unwrap();
    let mut hj = VerilatedHj::new();
    hj.reset();
    hj.load_policy(&verified).unwrap();
    let result = hj.observe(&cxl_write_event()).unwrap();
    assert_eq!(result.record.event_id, 1);
    assert_eq!(result.stats.record_count, 1);
    assert_eq!(result.stats.drop_count, 0);
}
```

- [ ] **Step 2: Confirm RED**

```bash
cargo test -p slugarch-verilator --test hj_smoke
```

Expected: missing `VerilatedHj`.

- [ ] **Step 3: Add a separate HJ compile unit**

`build.rs` compiles `slugcxl_4x4_hj_top` with:

```text
generated/slugcxl/slugcxl_endpoint.sv
generated/slugcxl/slugcxl_4x4_top.sv
generated/slugcxl/slugcxl_hj_pipeline.sv
generated/slugcxl/slugcxl_4x4_hj_top.sv
```

Keep the original non-HJ `slugcxl_4x4_top` unit for baseline comparisons.

- [ ] **Step 4: Add the C++ shim**

Define opaque `SlugarchHj` and C functions:

```c
SlugarchHj *slugarch_hj_new(void);
void slugarch_hj_free(SlugarchHj *);
void slugarch_hj_reset(SlugarchHj *);
int slugarch_hj_load_policy(SlugarchHj *, const uint8_t *, uint32_t);
int slugarch_hj_observe(SlugarchHj *, const uint8_t event[64],
                        uint8_t record[128], uint32_t *record_len,
                        uint64_t *cycles);
int slugarch_hj_stats(const SlugarchHj *, SlugarchHjStats *);
```

All loops use fixed cycle deadlines and return distinct timeout, RTL error,
buffer, and protocol codes.

- [ ] **Step 5: Implement `VerilatedHj`**

The safe wrapper owns one raw pointer, is `Send + !Sync`, validates input sizes,
converts C errors to Rust errors, and frees on drop.

- [ ] **Step 6: Run smoke and commit**

```bash
cargo test -p slugarch-verilator-sys
cargo test -p slugarch-verilator --test hj_smoke
git add crates/slugarch-verilator-sys crates/slugarch-verilator
git commit -m "verilator: expose the SlugArch Hardware-JIT model"
```

## Task 6: Add the FPGA Backend to the FFI

**Files:**
- Modify: `crates/slugarch-jit-ffi/Cargo.toml`
- Modify: `crates/slugarch-jit-ffi/src/lib.rs`
- Create: `crates/slugarch-jit-ffi/src/fpga.rs`
- Create: `crates/slugarch-jit-ffi/tests/rtl_equivalence.rs`

- [ ] **Step 1: Add the optional dependency**

```toml
[features]
default = []
fpga-verilator = ["dep:slugarch-verilator"]

[dependencies.slugarch-verilator]
path = "../slugarch-verilator"
optional = true
```

- [ ] **Step 2: Write equivalence tests**

For validation, delta, and full policies, compare:

```rust
assert_eq!(rust_decision.kind, rtl_decision.kind);
assert_eq!(rust_record.semantic_tuple(), rtl_record.semantic_tuple());
assert_eq!(rust_stats.record_count, rtl_stats.record_count);
assert_eq!(rust_stats.metadata_bytes, rtl_stats.metadata_bytes);
assert_eq!(rust_stats.epoch, rtl_stats.epoch);
```

Use zero, sparse, and full payloads, range endpoints, phase/fence, tag mismatch,
payload corruption, unsupported class, backpressure, and timeout injection.

- [ ] **Step 3: Confirm RED**

```bash
cargo test -p slugarch-jit-ffi --test rtl_equivalence --features fpga-verilator
```

Expected: missing backend adapter or semantic mismatch.

- [ ] **Step 4: Implement the adapter**

On policy load, encode the verified image and require read-back digest equality.
On observe, convert the canonical event to the local packet format, execute the
model, decode its fixed record, and compare sticky counters. Return an error on
timeout, drop, unsupported opcode, or malformed record.

- [ ] **Step 5: Run equivalence repeatedly**

```bash
for i in 1 2 3 4 5 6 7 8 9 10; do
  cargo test -q -p slugarch-jit-ffi --test rtl_equivalence \
    --features fpga-verilator || exit 1
done
```

Expected: ten deterministic passes and zero mismatches.

- [ ] **Step 6: Commit**

```bash
git add crates/slugarch-jit-ffi Cargo.lock
git commit -m "jit: add the FPGA Verilator backend"
```

## Task 7: Regenerate and Audit Target RTL

**Files:**
- Regenerate: `targets/agilex-vr2/generated/*`
- Regenerate mirror used by Verilator:
  `vendor/gemma-generated/generated/slugcxl/*`
- Create: `targets/agilex-vr2/generated/SHA256SUMS`

- [ ] **Step 1: Generate into a clean temporary directory**

```bash
rm -rf /tmp/slugcxl-jext-generated
cargo run -p slugcxl-gen -- \
  --out /tmp/slugcxl-jext-generated \
  --hj \
  --quartus-project /tmp/slugcxl-jext-generated
```

Expected: endpoint, HJ pipeline/top/fit top, runtime JSON, policy JSON/hex,
overhead report, and Quartus scaffolding.

- [ ] **Step 2: Compare before copying**

```bash
diff -ru targets/agilex-vr2/generated /tmp/slugcxl-jext-generated
```

Expected: only files governed by the generator differ. Review every difference.

- [ ] **Step 3: Regenerate through the project-supported command**

Use the generator/library, not hand edits, to update both checked-in locations.
Create deterministic sorted `SHA256SUMS`.

- [ ] **Step 4: Run generated-artifact tests**

```bash
cargo test -p slugcxl-gen
cargo test -p slugarch-verilator --test hj_smoke
git diff --check -- targets/agilex-vr2/generated \
  vendor/gemma-generated/generated/slugcxl
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add targets/agilex-vr2/generated \
  vendor/gemma-generated/generated/slugcxl
git commit -m "rtl: regenerate the programmable SlugCXL J-extension"
```

## Optional Quartus Evidence Gate

- [ ] **Step 1: Identify tool, device, constraints, and baseline**

Before building, record Quartus version, exact device, QSF/SDC hashes, HJ and
no-HJ revision names, and whether the fit top is a harness or board top.

- [ ] **Step 2: Compile only if the comparison is valid**

Run the generated build script for both matched revisions.

Expected for eligible evidence: synthesis, fitter, assembler, and timing
reports complete without fatal or unconstrained-clock conditions.

- [ ] **Step 3: Archive reports without upgrading the proof boundary**

Report ALMs/LUTs, registers, memory blocks/bits, achieved Fmax, and delta. If
either revision is incomplete or incomparable, mark the result blocked and do
not put a numeric FPGA delta in the paper.

## Final Verification

- [ ] **Step 1: Run generator, RTL, Verilator, FFI, and equivalence gates**

```bash
cargo fmt --all -- --check
cargo test -p slugcxl-gen
cargo test -p slugarch-verilator-sys
cargo test -p slugarch-verilator
cargo test -p slugarch-jit-ffi --features fpga-verilator
```

Expected: all pass.

- [ ] **Step 2: Record proof-level metadata**

The final manifest states separately:

```text
Rust policy verification: tested
Generated RTL: snapshot/lint tested
Verilated RTL execution: tested
Quartus synthesis/fit: measured or blocked
Physical FPGA execution: blocked unless separately run
```
