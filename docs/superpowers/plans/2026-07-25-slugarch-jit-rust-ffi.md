# SlugArch JIT Rust Policy and FFI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the single source of truth for SlugArch J-extension policy verification and record semantics, then expose it to QEMU through a panic-safe versioned C ABI.

**Architecture:** `slugarch-jit` owns canonical event, policy, bytecode, verifier, interpreter, digest, record, and statistics types without QEMU dependencies. `slugarch-jit-ffi` wraps one serialized engine per opaque handle, copies all FFI inputs, catches panics, returns stable integer errors, and exports a checked-in C header. Existing host replay code consumes the same Rust core after equivalence tests pass.

**Tech Stack:** Rust 2021, Serde, `sha2`, `thiserror`, C11, Cargo, proptest, AddressSanitizer or Valgrind.

---

## File Structure

### `crates/slugarch-jit`

- `Cargo.toml`: core crate dependencies and optional FPGA feature boundary.
- `src/lib.rs`: public API and version constants only.
- `src/event.rs`: canonical fixed-size boundary event types.
- `src/policy.rs`: strict JSON policy schema and canonicalization.
- `src/program.rs`: bounded instruction representation.
- `src/verifier.rs`: compile and conservative safety/budget checks.
- `src/interpreter.rs`: allocation-free event execution and controller state.
- `src/record.rs`: semantic replay record and stable encoding.
- `src/error.rs`: stable verification/runtime errors.
- `tests/policy.rs`: valid/invalid parser and digest cases.
- `tests/verifier.rs`: every verifier rejection path.
- `tests/interpreter.rs`: event, epoch, capture, reject, and determinism cases.
- `tests/proptest.rs`: bounded arbitrary policies/events never panic.

### `crates/slugarch-jit-ffi`

- `Cargo.toml`: `cdylib`, `staticlib`, and `rlib` outputs.
- `src/lib.rs`: exported ABI and panic containment.
- `include/slugarch_jit.h`: checked-in ABI version 1 declarations.
- `tests/abi_layout.rs`: Rust-side sizes, offsets, and unknown-tail handling.
- `tests/panic_containment.rs`: no unwind crosses C.
- `tests/c_smoke.c`: compile-and-run consumer.
- `build.rs`: compile the C smoke against the checked-in header for tests.

### Existing files

- Modify workspace `Cargo.toml`: add both crates and `sha2`.
- Modify `Cargo.lock`: resolved dependency graph.
- Modify `crates/slugarch-host/src/replay.rs`: consume canonical types after
  equivalence is proven.
- Modify `crates/slugarch-host/Cargo.toml`: add `slugarch-jit`.

## Task 1: Create the Canonical Event and Policy Crate

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/slugarch-jit/Cargo.toml`
- Create: `crates/slugarch-jit/src/lib.rs`
- Create: `crates/slugarch-jit/src/event.rs`
- Create: `crates/slugarch-jit/src/policy.rs`
- Create: `crates/slugarch-jit/src/error.rs`
- Create: `crates/slugarch-jit/tests/policy.rs`

- [ ] **Step 1: Write the failing strict-policy test**

Create `crates/slugarch-jit/tests/policy.rs`:

```rust
use slugarch_jit::{Policy, RecordMode, SLUG_JIT_ABI_VERSION};

const POLICY: &str = r#"{
  "version":1,
  "name":"validation-cxlmem",
  "allowed_classes":["cxl_mem_read","cxl_mem_write","cxl_mem_data","completion"],
  "ranges":[{"base":83886080,"length":33554432}],
  "sample_stride":1,
  "record_mode":"validation",
  "metadata_budget":256,
  "epoch_policy":"phase",
  "rules":[
    {"op":"capture","mode":"validation"},
    {"op":"emit"},
    {"op":"epoch_from_phase"},
    {"op":"halt"}
  ]
}"#;

#[test]
fn strict_v1_policy_parses() {
    let policy = Policy::parse(POLICY.as_bytes()).unwrap();
    assert_eq!(policy.version, SLUG_JIT_ABI_VERSION);
    assert_eq!(policy.record_mode, RecordMode::Validation);
}

#[test]
fn unknown_field_is_rejected() {
    let bad = POLICY.replace("\"version\":1,", "\"version\":1,\"surprise\":7,");
    assert!(Policy::parse(bad.as_bytes()).is_err());
}
```

- [ ] **Step 2: Run the test and confirm RED**

Run:

```bash
cargo test -p slugarch-jit --test policy
```

Expected: FAIL because the package does not exist.

- [ ] **Step 3: Add workspace and crate manifests**

Add workspace members `crates/slugarch-jit` and
`crates/slugarch-jit-ffi`, plus:

```toml
sha2 = "0.10"
```

Create `crates/slugarch-jit/Cargo.toml`:

```toml
[package]
name = "slugarch-jit"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
sha2.workspace = true
thiserror.workspace = true

[dev-dependencies]
proptest.workspace = true
```

- [ ] **Step 4: Implement the public types**

`src/lib.rs` must export:

```rust
pub mod error;
pub mod event;
pub mod interpreter;
pub mod policy;
pub mod program;
pub mod record;
pub mod verifier;

pub use error::{JitError, JitErrorCode};
pub use event::{Direction, Event, EventClass, MAX_EVENT_PAYLOAD};
pub use interpreter::{Decision, Engine, Stats};
pub use policy::{AddressRange, EpochPolicy, Policy, RecordMode};
pub use program::{Instruction, VerifiedPolicy, MAX_INSTRUCTIONS, MAX_RANGES};
pub use record::{PayloadCapture, ReplayRecord};

pub const SLUG_JIT_ABI_VERSION: u32 = 1;
pub const SLUG_JIT_EVENT_VERSION: u32 = 1;
pub const SLUG_JIT_PACKET_VERSION: u32 = 1;
pub const SLUG_JIT_BACKEND_CONTRACT_VERSION: u32 = 1;
```

`src/event.rs` defines:

```rust
pub const MAX_EVENT_PAYLOAD: usize = 64;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    HostToDevice = 0,
    DeviceToHost = 1,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventClass {
    CxlMemRead = 1,
    CxlMemWrite = 2,
    CxlMemData = 3,
    Completion = 4,
    PtxModuleLoad = 5,
    KernelLaunch = 6,
    Phase = 7,
    Fence = 8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub event_id: u64,
    pub client_id: u64,
    pub direction: Direction,
    pub class: EventClass,
    pub opcode: u16,
    pub address: u64,
    pub payload_len: u8,
    pub payload: [u8; MAX_EVENT_PAYLOAD],
    pub tag: u64,
    pub phase_id: u64,
    pub monotonic_ns: u64,
    pub status: u32,
}
```

`Event::validate()` rejects `payload_len > 64` and nonzero bytes after the
declared payload.

`src/policy.rs` defines these exact strict structures:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddressRange {
    pub base: u64,
    pub length: u64,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordMode {
    Validation = 0,
    Delta = 1,
    Full = 2,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpochPolicy {
    Phase = 0,
    Increment = 1,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rule {
    MatchClass { class: EventClass, skip: u8 },
    MatchDirection { direction: Direction, skip: u8 },
    MatchOpcode { opcode: u16, skip: u8 },
    MatchStatus { status: u32, skip: u8 },
    MatchRange { range: u8, skip: u8 },
    Sample { stride: u32, skip: u8 },
    Capture { mode: RecordMode },
    Emit,
    EpochIncrement,
    EpochFromPhase,
    Reject { code: u16 },
    Halt,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub version: u32,
    pub name: String,
    pub allowed_classes: Vec<EventClass>,
    pub ranges: Vec<AddressRange>,
    pub sample_stride: u32,
    pub record_mode: RecordMode,
    pub metadata_budget: u32,
    pub epoch_policy: EpochPolicy,
    pub rules: Vec<Rule>,
}
```

`Policy::parse(&[u8])` rejects unknown fields, zero-length ranges, and trailing
non-whitespace bytes. The verifier checks that `record_mode` and
`epoch_policy` agree with the corresponding program rules; it does not
silently synthesize missing rules.

`src/error.rs` fixes the externally visible error discriminants:

```rust
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitErrorCode {
    Null = 1,
    StructSize = 2,
    AbiVersion = 3,
    Parse = 4,
    PolicyVersion = 5,
    TooManyInstructions = 6,
    TooManyRanges = 7,
    InvalidRange = 8,
    InvalidStride = 9,
    BudgetExceeded = 10,
    InvalidControlFlow = 11,
    Unsupported = 12,
    DigestMismatch = 13,
    Rejected = 14,
    Drop = 15,
    Timeout = 16,
    Backend = 17,
    Io = 18,
    Poisoned = 19,
    Panic = 20,
}
```

Success is integer zero and is not an enum member. C, Rust, QEMU, RTL, logs,
and guest headers use these values verbatim.

- [ ] **Step 5: Run the focused tests**

Run:

```bash
cargo test -p slugarch-jit --test policy
cargo fmt --all -- --check
```

Expected: both policy tests pass and formatting is clean.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/slugarch-jit
git commit -m "jit: add canonical event and policy schema"
```

## Task 2: Implement Canonicalization, Bytecode, and Verification

**Files:**
- Create: `crates/slugarch-jit/src/program.rs`
- Create: `crates/slugarch-jit/src/verifier.rs`
- Create: `crates/slugarch-jit/tests/verifier.rs`
- Modify: `crates/slugarch-jit/src/policy.rs`
- Modify: `crates/slugarch-jit/src/error.rs`

- [ ] **Step 1: Write verifier rejection tests**

Create tests that assert exact `JitErrorCode` values:

```rust
#[test]
fn rejects_oversized_program() {
    let mut p = valid_policy();
    p.rules = (0..33).map(|_| emit_rule()).collect();
    assert_eq!(p.verify().unwrap_err().code(), JitErrorCode::TooManyInstructions);
}

#[test]
fn rejects_wrapping_range() {
    let mut p = valid_policy();
    p.ranges[0] = AddressRange { base: u64::MAX - 3, length: 8 };
    assert_eq!(p.verify().unwrap_err().code(), JitErrorCode::InvalidRange);
}

#[test]
fn rejects_zero_stride_and_excess_budget() {
    let mut p = valid_policy();
    p.sample_stride = 0;
    assert_eq!(p.verify().unwrap_err().code(), JitErrorCode::InvalidStride);
    p.sample_stride = 1;
    p.metadata_budget = 257;
    assert_eq!(p.verify().unwrap_err().code(), JitErrorCode::BudgetExceeded);
}
```

Also cover version mismatch, more than four ranges, unsupported class,
backward branch, missing halt, more than one emit, capture over 64 bytes, and
empty allowed-class set.

- [ ] **Step 2: Run and confirm RED**

Run:

```bash
cargo test -p slugarch-jit --test verifier
```

Expected: FAIL on missing `verify`, `Instruction`, or error variants.

- [ ] **Step 3: Implement the fixed instruction set**

`src/program.rs` defines:

```rust
pub const MAX_INSTRUCTIONS: usize = 32;
pub const MAX_RANGES: usize = 4;
pub const MAX_METADATA_BYTES: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    MatchClass { class: EventClass, skip: u8 },
    MatchDirection { direction: Direction, skip: u8 },
    MatchOpcode { opcode: u16, skip: u8 },
    MatchStatus { status: u32, skip: u8 },
    MatchRange { range: u8, skip: u8 },
    Sample { stride: u32, skip: u8 },
    Capture { mode: RecordMode },
    Emit,
    EpochIncrement,
    EpochFromPhase,
    Reject { code: u16 },
    Halt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPolicy {
    pub canonical_json: Vec<u8>,
    pub digest: [u8; 32],
    pub instructions: Vec<Instruction>,
    pub ranges: Vec<AddressRange>,
    pub metadata_budget: u32,
}
```

Every `skip` is forward-only, nonzero, and lands within the instruction vector.

- [ ] **Step 4: Implement canonical digest and verifier**

Canonicalization serializes the strict typed policy, which contains vectors and
no unordered map. Prefix the digest input with four little-endian version
words:

```rust
let mut hasher = sha2::Sha256::new();
hasher.update(SLUG_JIT_ABI_VERSION.to_le_bytes());
hasher.update(SLUG_JIT_EVENT_VERSION.to_le_bytes());
hasher.update(SLUG_JIT_PACKET_VERSION.to_le_bytes());
hasher.update(SLUG_JIT_BACKEND_CONTRACT_VERSION.to_le_bytes());
hasher.update(&canonical_json);
let digest: [u8; 32] = hasher.finalize().into();
```

The verifier performs subtraction-safe range checks and explores all
forward-only control-flow paths to prove one terminating `Halt`, at most one
`Emit`, and no capture above the declared metadata budget.

- [ ] **Step 5: Run focused and property tests**

Run:

```bash
cargo test -p slugarch-jit --test verifier
cargo test -p slugarch-jit --test policy
```

Expected: all exact rejection codes and stable digest tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/slugarch-jit
git commit -m "jit: verify bounded replay policies"
```

## Task 3: Implement the Deterministic Interpreter and Records

**Files:**
- Create: `crates/slugarch-jit/src/record.rs`
- Create: `crates/slugarch-jit/src/interpreter.rs`
- Create: `crates/slugarch-jit/tests/interpreter.rs`
- Create: `crates/slugarch-jit/tests/proptest.rs`

- [ ] **Step 1: Write semantic tests**

Use a fixed event whose payload starts `01 00 02`:

```rust
#[test]
fn validation_record_is_deterministic() {
    let policy = validation_policy().verify().unwrap();
    let mut left = Engine::new(policy.clone());
    let mut right = Engine::new(policy);
    let event = cxl_write_event();
    assert_eq!(left.observe(&event).unwrap(), right.observe(&event).unwrap());
    assert_eq!(left.stats().record_count, 1);
    assert_eq!(left.stats().event_count, 1);
}

#[test]
fn strict_reject_does_not_emit() {
    let mut engine = Engine::new(rejecting_policy().verify().unwrap());
    let decision = engine.observe(&cxl_write_event()).unwrap();
    assert!(matches!(decision, Decision::Reject { code: 7 }));
    assert_eq!(engine.stats().record_count, 0);
    assert_eq!(engine.stats().reject_count, 1);
}
```

Add exact tests for validation hash, sparse delta `(index,value)` pairs, full
payload, phase epoch assignment, increment, sampling, range edge, unsupported
event, repeated determinism, and no mutation after a runtime error.

- [ ] **Step 2: Confirm RED**

Run:

```bash
cargo test -p slugarch-jit --test interpreter
```

Expected: FAIL because `Engine` and `ReplayRecord` are missing.

- [ ] **Step 3: Implement record and engine types**

`ReplayRecord` contains:

```rust
pub struct ReplayRecord {
    pub sequence: u64,
    pub event_id: u64,
    pub policy_digest: [u8; 32],
    pub epoch: u64,
    pub direction: Direction,
    pub class: EventClass,
    pub opcode: u16,
    pub address: u64,
    pub tag: u64,
    pub status: u32,
    pub payload: PayloadCapture,
}
```

Use the same FNV-1a-64 constants as current generated RTL:

```rust
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x00000100000001b3;
```

`Engine` stores the verified policy, instruction counter, epoch, sequence, and
`Stats`. The event path allocates no new policy state; record payload storage is
bounded at 64 bytes.

- [ ] **Step 4: Add panic-resistance property tests**

Generate valid fixed-size events and arbitrary byte slices for policy parsing.
Assert:

```rust
proptest! {
    #[test]
    fn arbitrary_policy_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = Policy::parse(&bytes);
    }
}
```

Also generate valid verified programs and events, asserting stats never
decrease and output never exceeds one record.

- [ ] **Step 5: Run all core tests**

Run:

```bash
cargo test -p slugarch-jit
```

Expected: parser, verifier, semantic, determinism, and property tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/slugarch-jit
git commit -m "jit: execute deterministic replay policies"
```

## Task 4: Add the Stable Panic-Safe C ABI

**Files:**
- Create: `crates/slugarch-jit-ffi/Cargo.toml`
- Create: `crates/slugarch-jit-ffi/src/lib.rs`
- Create: `crates/slugarch-jit-ffi/include/slugarch_jit.h`
- Create: `crates/slugarch-jit-ffi/tests/abi_layout.rs`
- Create: `crates/slugarch-jit-ffi/tests/panic_containment.rs`

- [ ] **Step 1: Write ABI layout tests**

Tests must assert ABI 1, fixed payload/digest widths, null rejection, undersized
structure rejection, oversized known-prefix acceptance, and idempotent
diagnostic reads.

Run:

```bash
cargo test -p slugarch-jit-ffi
```

Expected: FAIL because the crate is not implemented.

- [ ] **Step 2: Create the manifest**

```toml
[package]
name = "slugarch-jit-ffi"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib", "staticlib", "rlib"]

[dependencies]
slugarch-jit = { path = "../slugarch-jit" }
serde_json.workspace = true

[build-dependencies]
cc.workspace = true
```

- [ ] **Step 3: Define the checked-in header**

`include/slugarch_jit.h` declares:

```c
#include <stdint.h>

#define SLUG_JIT_ABI_VERSION 1u
#define SLUG_JIT_PAYLOAD_BYTES 64u
#define SLUG_JIT_DIGEST_BYTES 32u

typedef struct SlugJitHandle SlugJitHandle;

enum {
    SLUG_JIT_BACKEND_NONE = 0,
    SLUG_JIT_BACKEND_RUST = 1,
    SLUG_JIT_BACKEND_GPU = 2,
    SLUG_JIT_BACKEND_FPGA_VERILATOR = 3,
};

enum {
    SLUG_JIT_CAP_POLICY = UINT64_C(1) << 0,
    SLUG_JIT_CAP_RECORD = UINT64_C(1) << 1,
    SLUG_JIT_CAP_GPU_DIAGNOSTIC = UINT64_C(1) << 2,
    SLUG_JIT_CAP_FPGA_RTL = UINT64_C(1) << 3,
};

enum {
    SLUG_JIT_OK = 0,
    SLUG_JIT_ERR_NULL = 1,
    SLUG_JIT_ERR_STRUCT_SIZE = 2,
    SLUG_JIT_ERR_ABI_VERSION = 3,
    SLUG_JIT_ERR_PARSE = 4,
    SLUG_JIT_ERR_POLICY_VERSION = 5,
    SLUG_JIT_ERR_TOO_MANY_INSTRUCTIONS = 6,
    SLUG_JIT_ERR_TOO_MANY_RANGES = 7,
    SLUG_JIT_ERR_INVALID_RANGE = 8,
    SLUG_JIT_ERR_INVALID_STRIDE = 9,
    SLUG_JIT_ERR_BUDGET_EXCEEDED = 10,
    SLUG_JIT_ERR_INVALID_CONTROL_FLOW = 11,
    SLUG_JIT_ERR_UNSUPPORTED = 12,
    SLUG_JIT_ERR_DIGEST_MISMATCH = 13,
    SLUG_JIT_ERR_REJECTED = 14,
    SLUG_JIT_ERR_DROP = 15,
    SLUG_JIT_ERR_TIMEOUT = 16,
    SLUG_JIT_ERR_BACKEND = 17,
    SLUG_JIT_ERR_IO = 18,
    SLUG_JIT_ERR_POISONED = 19,
    SLUG_JIT_ERR_PANIC = 20,
};

typedef struct SlugJitCreateArgs {
    uint32_t struct_size;
    uint32_t abi_version;
    uint32_t backend;
    uint32_t strict;
    uint32_t diagnostic_capacity;
    uint32_t reserved;
} SlugJitCreateArgs;

typedef struct SlugJitEvent {
    uint32_t struct_size;
    uint32_t abi_version;
    uint64_t event_id;
    uint64_t client_id;
    uint32_t direction;
    uint32_t event_class;
    uint32_t opcode;
    uint32_t payload_len;
    uint64_t address;
    uint64_t tag;
    uint64_t phase_id;
    uint64_t monotonic_ns;
    uint32_t status;
    uint32_t reserved;
    uint8_t payload[SLUG_JIT_PAYLOAD_BYTES];
} SlugJitEvent;

typedef struct SlugJitPolicyInfo {
    uint32_t struct_size;
    uint32_t abi_version;
    uint32_t backend;
    uint32_t canonical_bytes;
    uint8_t digest[SLUG_JIT_DIGEST_BYTES];
    uint32_t instruction_count;
    uint32_t range_count;
    uint32_t metadata_budget;
    uint32_t reserved;
} SlugJitPolicyInfo;

typedef struct SlugJitDecision {
    uint32_t struct_size;
    uint32_t abi_version;
    uint32_t accepted;
    uint32_t emitted;
    uint32_t error_code;
    uint32_t record_bytes;
    uint32_t payload_bytes;
    uint32_t reserved;
    uint64_t epoch;
    uint64_t record_id;
} SlugJitDecision;

typedef struct SlugJitStats {
    uint32_t struct_size;
    uint32_t abi_version;
    uint64_t event_count;
    uint64_t record_count;
    uint64_t metadata_bytes;
    uint64_t reject_count;
    uint64_t drop_count;
    uint64_t epoch;
} SlugJitStats;

uint32_t slugarch_jit_abi_version(void);
uint64_t slugarch_jit_backend_caps(void);
int32_t slugarch_jit_create(const SlugJitCreateArgs *args,
                            SlugJitHandle **out);
int32_t slugarch_jit_load_policy(SlugJitHandle *handle,
                                 const uint8_t *json, uint32_t json_len,
                                 SlugJitPolicyInfo *out);
int32_t slugarch_jit_observe(SlugJitHandle *handle,
                             const SlugJitEvent *event,
                             SlugJitDecision *out);
int32_t slugarch_jit_stats(SlugJitHandle *handle, SlugJitStats *out);
int32_t slugarch_jit_last_diagnostic(SlugJitHandle *handle, uint8_t *out,
                                     uint32_t capacity, uint32_t *written);
void slugarch_jit_destroy(SlugJitHandle *handle);

_Static_assert(sizeof(((SlugJitEvent *)0)->payload) == SLUG_JIT_PAYLOAD_BYTES,
               "event payload width changed");
_Static_assert(sizeof(((SlugJitPolicyInfo *)0)->digest) ==
                   SLUG_JIT_DIGEST_BYTES,
               "policy digest width changed");
```

Rust layout tests assert `sizeof`, `alignof`, and every `offsetof` against the
checked-in C header on both the normal build and the C smoke build.

- [ ] **Step 4: Implement FFI ownership and panic containment**

Every exported function enters one helper:

```rust
fn ffi_guard<F>(f: F) -> i32
where
    F: FnOnce() -> Result<(), JitError>,
{
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => error.code() as i32,
        Err(_) => JitErrorCode::Panic as i32,
    }
}
```

The opaque handle owns `std::sync::Mutex<EngineState>` and a bounded diagnostic
buffer. Null, poison, and lock errors return stable codes. `destroy(NULL)` is a
no-op; no other null handle is accepted.

- [ ] **Step 5: Run ABI tests**

Run:

```bash
cargo test -p slugarch-jit-ffi
cargo build -p slugarch-jit-ffi --release
nm -D target/release/libslugarch_jit_ffi.so | rg 'slugarch_jit_(abi_version|create|load_policy|observe|stats|last_diagnostic|destroy)'
```

Expected: tests pass and all seven required symbols are exported.

- [ ] **Step 6: Commit**

```bash
git add crates/slugarch-jit-ffi Cargo.lock
git commit -m "jit: expose a stable panic-safe C ABI"
```

## Task 5: Prove the ABI from C

**Files:**
- Create: `crates/slugarch-jit-ffi/tests/c_smoke.c`
- Create: `crates/slugarch-jit-ffi/build.rs`
- Modify: `crates/slugarch-jit-ffi/Cargo.toml`

- [ ] **Step 1: Write the C smoke consumer**

The executable must:

1. require ABI 1;
2. create a strict Rust backend;
3. load the fixed validation policy;
4. submit one 8-byte CXL.mem write event at 80 MiB;
5. require `decision.emitted == 1`;
6. require stats `event_count == 1`, `record_count == 1`, `drop_count == 0`;
7. retrieve the diagnostic length twice;
8. destroy the handle; and
9. verify an undersized event returns `SLUG_JIT_ERR_STRUCT_SIZE`.

Return a unique nonzero exit code for each failed check.

- [ ] **Step 2: Compile and run against the release library**

Run:

```bash
cc -std=c11 -Wall -Wextra -Werror \
  -Icrates/slugarch-jit-ffi/include \
  crates/slugarch-jit-ffi/tests/c_smoke.c \
  -Ltarget/release -lslugarch_jit_ffi \
  -Wl,-rpath,"$PWD/target/release" \
  -o /tmp/slugarch-jit-c-smoke
/tmp/slugarch-jit-c-smoke
```

Expected: exit 0 and one line `SLUG_JIT_C_ABI_PASS`.

- [ ] **Step 3: Run a memory checker**

Run one available command:

```bash
valgrind --error-exitcode=99 --leak-check=full /tmp/slugarch-jit-c-smoke
```

or rebuild/run with AddressSanitizer.

Expected: no invalid read/write, use-after-free, or definitely lost allocation.

- [ ] **Step 4: Commit**

```bash
git add crates/slugarch-jit-ffi
git commit -m "test: prove the SlugArch JIT C ABI"
```

## Task 6: Migrate Host Replay Semantics to the Shared Core

**Files:**
- Modify: `crates/slugarch-host/Cargo.toml`
- Modify: `crates/slugarch-host/src/replay.rs`
- Create: `crates/slugarch-host/tests/jit_replay_compat.rs`

- [ ] **Step 1: Freeze old-versus-new output in a failing compatibility test**

For the existing 98-event GEMM trace, assert:

```rust
assert_eq!(old.summary.record_count, new.summary.record_count);
assert_eq!(old.summary.epoch_count, new.summary.epoch_count);
assert_eq!(old.summary.payload_capture_bytes, new.summary.payload_capture_bytes);
assert_eq!(old_records_semantic(&old), new_records_semantic(&new));
```

Run:

```bash
cargo test -p slugarch-host --test jit_replay_compat
```

Expected: RED until the adapter exists.

- [ ] **Step 2: Add a narrow adapter**

Map existing `CxlMsg` values into canonical `Event` values in one function.
Do not duplicate hashing, capture, epoch, or record accounting in
`slugarch-host`.

- [ ] **Step 3: Run host and workspace tests**

Run:

```bash
cargo test -p slugarch-host
cargo test --workspace
```

Expected: all existing host semantics and the new compatibility test pass.

- [ ] **Step 4: Commit**

```bash
git add crates/slugarch-host
git commit -m "host: share SlugArch JIT record semantics"
```

## Final Verification

- [ ] **Step 1: Run formatting, lint, tests, and ABI smoke**

```bash
cargo fmt --all -- --check
cargo clippy -p slugarch-jit -p slugarch-jit-ffi -p slugarch-host --all-targets -- -D warnings
cargo test -p slugarch-jit -p slugarch-jit-ffi -p slugarch-host
/tmp/slugarch-jit-c-smoke
```

Expected: all commands pass.

- [ ] **Step 2: Record library identity**

```bash
sha256sum target/release/libslugarch_jit_ffi.so
readelf -d target/release/libslugarch_jit_ffi.so
```

Expected: a recorded SHA-256 and no unexpected absolute runtime dependency on
the source tree.
