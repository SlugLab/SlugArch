# SlugArch Small CXL Tiles Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a deterministic host-home-agent model, per-tile evidence coordinator, four workloads, and six exact fault injections for the SlugArch small-CXL-tile evaluation.

**Architecture:** A new dependency-light Rust crate models one host coherence authority over identified Type-2 tiles. It consumes canonical events, maintains one explicit state record per 64-byte line, emits deterministic joined records, and fails the whole epoch at the first stable error. Workload and fault generators remain separate from the transition engine so normal and faulty executions use the same semantics.

**Tech Stack:** Rust 2021, Serde, SHA-256, `thiserror`, Cargo, proptest.

---

## File Structure

- Create `crates/slugarch-tile-model/Cargo.toml`: crate dependencies.
- Create `crates/slugarch-tile-model/src/lib.rs`: public exports and constants.
- Create `crates/slugarch-tile-model/src/types.rs`: identifiers, events, line
  state, records, counters, and stable errors.
- Create `crates/slugarch-tile-model/src/home_agent.rs`: deterministic state
  transitions.
- Create `crates/slugarch-tile-model/src/coordinator.rs`: digest checks,
  record joins, epoch sealing, and fail-stop state.
- Create `crates/slugarch-tile-model/src/workload.rs`: four deterministic
  workload generators.
- Create `crates/slugarch-tile-model/src/fault.rs`: six single-fault
  transformations.
- Create `crates/slugarch-tile-model/tests/home_agent.rs`: legal ordering and
  version tests.
- Create `crates/slugarch-tile-model/tests/faults.rs`: exact first-failure
  tests.
- Create `crates/slugarch-tile-model/tests/coordinator.rs`: per-tile identity,
  digest, counter, and epoch tests.
- Modify `Cargo.toml`: add the new workspace member.

## Task 1: Scaffold Stable Types and Errors

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/slugarch-tile-model/Cargo.toml`
- Create: `crates/slugarch-tile-model/src/lib.rs`
- Create: `crates/slugarch-tile-model/src/types.rs`
- Create: `crates/slugarch-tile-model/tests/home_agent.rs`

- [ ] **Step 1: Write the failing type contract**

Define a test that constructs every event kind and verifies stable numeric
error codes:

```rust
assert_eq!(FaultCode::CohInvalidatePending as u32, 0x1001);
assert_eq!(FaultCode::CohStaleVersion as u32, 0x1002);
assert_eq!(FaultCode::CohCompletionOrder as u32, 0x1003);
assert_eq!(FaultCode::CohFenceMissing as u32, 0x1004);
assert_eq!(FaultCode::PolicyDigest as u32, 0x2001);
assert_eq!(FaultCode::RecordDrop as u32, 0x2002);
assert_eq!(LINE_BYTES, 64);
```

- [ ] **Step 2: Run the test and confirm RED**

```bash
cargo test -p slugarch-tile-model --test home_agent
```

Expected: Cargo reports that `slugarch-tile-model` does not exist.

- [ ] **Step 3: Implement the exact public types**

Use this crate manifest:

```toml
[package]
name = "slugarch-tile-model"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
sha2 = "0.10"
thiserror.workspace = true

[dev-dependencies]
proptest.workspace = true
```

Use these discriminants and fields:

```rust
pub const LINE_BYTES: u64 = 64;

#[repr(u8)]
pub enum EventKind {
    ReadShared = 1,
    ReadExclusive = 2,
    Writeback = 3,
    Invalidate = 4,
    InvalidateAck = 5,
    Fence = 6,
    Completion = 7,
    EpochSeal = 8,
}

#[repr(u32)]
pub enum FaultCode {
    CohInvalidatePending = 0x1001,
    CohStaleVersion = 0x1002,
    CohCompletionOrder = 0x1003,
    CohFenceMissing = 0x1004,
    PolicyDigest = 0x2001,
    RecordDrop = 0x2002,
}

pub struct TileEvent {
    pub tile_id: u16,
    pub event_id: u64,
    pub request_id: u64,
    pub epoch: u64,
    pub line_address: u64,
    pub version: u64,
    pub kind: EventKind,
}

pub struct LineState {
    pub version: u64,
    pub owner_tile: Option<u16>,
    pub sharers: u64,
    pub last_writer_tile: Option<u16>,
    pub visible_epoch: u64,
    pub outstanding_invalidations: u64,
}

pub struct AppliedEvent {
    pub event: TileEvent,
    pub line_before: Option<LineState>,
    pub line_after: Option<LineState>,
}

pub struct TileCounters {
    pub event_count: u64,
    pub record_count: u64,
    pub metadata_bytes: u64,
    pub reject_count: u64,
    pub drop_count: u64,
}

pub struct FailureRecord {
    pub code: FaultCode,
    pub tile_id: u16,
    pub event_id: u64,
    pub epoch: u64,
}

pub struct ModelError {
    pub code: u32,
    pub tile_id: u16,
    pub event_id: u64,
    pub epoch: u64,
    pub detail: String,
}
```

Reject tile IDs above 63 and non-64-byte-aligned line addresses.

- [ ] **Step 4: Run the focused test**

```bash
cargo test -p slugarch-tile-model --test home_agent
```

Expected: the type and error-code contract passes.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/slugarch-tile-model
git commit -m "model: define small-tile coherence events"
```

## Task 2: Implement Legal Home-Agent Transitions

**Files:**
- Create: `crates/slugarch-tile-model/src/home_agent.rs`
- Modify: `crates/slugarch-tile-model/src/lib.rs`
- Modify: `crates/slugarch-tile-model/tests/home_agent.rs`

- [ ] **Step 1: Write legal-sequence tests**

Cover private write, read-shared fanout, producer/fence/consumer, and alternating
exclusive ownership. Each test asserts the final `LineState`, ordered records,
and monotonically increasing visible version.

- [ ] **Step 2: Confirm RED**

```bash
cargo test -p slugarch-tile-model --test home_agent
```

Expected: unresolved `HomeAgent`.

- [ ] **Step 3: Implement one transition API**

```rust
pub struct HomeAgent {
    lines: BTreeMap<u64, LineState>,
    records: Vec<AppliedEvent>,
}

impl HomeAgent {
    pub fn apply(&mut self, event: TileEvent) -> Result<&AppliedEvent, ModelError>;
    pub fn line(&self, address: u64) -> Option<&LineState>;
    pub fn records(&self) -> &[AppliedEvent];
}
```

`ReadExclusive` sets the requester as owner and creates an invalidation bit for
every other sharer. `InvalidateAck` clears exactly the acknowledging tile bit.
`Writeback` advances the line version but does not make it visible until a
matching `Fence`. `Completion` rejects pending invalidations and unpublished
producer data. `EpochSeal` is legal only when no line has pending
invalidations.

- [ ] **Step 4: Prove determinism**

Run the same generated legal trace twice and assert byte-identical serialized
records:

```bash
cargo test -p slugarch-tile-model --test home_agent determinism
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/slugarch-tile-model
git commit -m "model: enforce host-home-agent ordering"
```

## Task 3: Add the Four Workload Generators

**Files:**
- Create: `crates/slugarch-tile-model/src/workload.rs`
- Modify: `crates/slugarch-tile-model/src/lib.rs`
- Create: `crates/slugarch-tile-model/tests/workloads.rs`

- [ ] **Step 1: Write exact-count tests**

For tile counts 1, 2, 4, and 8, require 100 warmup iterations and 10,000
measured events per active tile. Assert deterministic `(tile_id, event_id,
request_id)` assignment and a fixed seed of `0x534c554754494c45`.

- [ ] **Step 2: Confirm RED**

```bash
cargo test -p slugarch-tile-model --test workloads
```

Expected: missing workload generator.

- [ ] **Step 3: Implement generators**

Expose:

```rust
pub enum WorkloadKind {
    PrivatePartitions,
    ReadSharedFanout,
    ProducerConsumer,
    HotLinePingPong,
}

pub struct WorkloadTrace {
    pub warmup: Vec<TileEvent>,
    pub measured: Vec<TileEvent>,
    pub seed: u64,
}

pub fn generate_workload(
    kind: WorkloadKind,
    tiles: u16,
    warmup_per_tile: u64,
    measured_per_tile: u64,
    seed: u64,
) -> Result<WorkloadTrace, ModelError>;
```

The producer/consumer generator emits `Writeback`, `Fence`, signal
`Completion`, consumer `ReadShared`, and consumer `Completion` in that order.
The ping-pong generator alternates writers and emits all required
`InvalidateAck` events before each completion.

- [ ] **Step 4: Test all topology sizes**

```bash
cargo test -p slugarch-tile-model --test workloads
```

Expected: all four workloads pass for 1, 2, 4, and 8 tiles.

- [ ] **Step 5: Commit**

```bash
git add crates/slugarch-tile-model
git commit -m "model: generate deterministic tile workloads"
```

## Task 4: Implement the Six Single-Fault Transformations

**Files:**
- Create: `crates/slugarch-tile-model/src/fault.rs`
- Modify: `crates/slugarch-tile-model/src/lib.rs`
- Create: `crates/slugarch-tile-model/tests/faults.rs`

- [ ] **Step 1: Write six failing first-error tests**

Each case starts from one legal trace, changes exactly one event, and asserts:

```rust
assert_eq!(observed.code, expected_code);
assert_eq!(observed.tile_id, injected_tile);
assert_eq!(observed.event_id, injected_event);
assert_eq!(observed.epoch, injected_epoch);
```

- [ ] **Step 2: Confirm RED**

```bash
cargo test -p slugarch-tile-model --test faults
```

Expected: missing injector.

- [ ] **Step 3: Implement deterministic injection**

Expose:

```rust
pub enum FaultKind {
    MissingInvalidateAck,
    StaleLineVersion,
    ReorderedCompletion,
    FenceOmission,
    PolicyDigestMismatch,
    RequiredRecordDrop,
}

pub struct FaultedTrace {
    pub trace: WorkloadTrace,
    pub kind: FaultKind,
    pub injected_tile_id: u16,
    pub injected_event_id: u64,
    pub original_event_sha256: [u8; 32],
    pub transformed_event_sha256: [u8; 32],
}

pub fn inject_one(
    trace: &WorkloadTrace,
    kind: FaultKind,
    tile_id: u16,
    event_id: u64,
) -> Result<FaultedTrace, ModelError>;
```

The function rejects an injection point that cannot express the selected
fault. It records the original and transformed event hashes and never injects
more than one semantic change.

- [ ] **Step 4: Run five repetitions**

```bash
for run in 1 2 3 4 5; do
  cargo test -p slugarch-tile-model --test faults
done
```

Expected: all six cases return the same code and first divergent event in all
five runs.

- [ ] **Step 5: Commit**

```bash
git add crates/slugarch-tile-model
git commit -m "model: add exact tile fault corpus"
```

## Task 5: Add the Fail-Stop Epoch Coordinator

**Files:**
- Create: `crates/slugarch-tile-model/src/coordinator.rs`
- Modify: `crates/slugarch-tile-model/src/lib.rs`
- Create: `crates/slugarch-tile-model/tests/coordinator.rs`

- [ ] **Step 1: Write failing coordinator tests**

Test unique tile IDs, one digest across participants, strictly increasing
event IDs per tile, exact event/record counts, zero drops, one first failure,
and rejection of a successful partial epoch.

- [ ] **Step 2: Confirm RED**

```bash
cargo test -p slugarch-tile-model --test coordinator
```

Expected: missing coordinator.

- [ ] **Step 3: Implement coordinator state**

```rust
pub enum EpochStatus {
    Open,
    Complete,
    Failed(FailureRecord),
}

pub struct EpochCoordinator {
    pub epoch: u64,
    pub policy_digest: [u8; 32],
    pub participants: BTreeMap<u16, TileCounters>,
    pub status: EpochStatus,
}
```

`observe_tile_record` checks digest, identity, sequence, and counters before
accepting a record. `fail` is idempotent and preserves the first error.
`seal_success` requires every declared tile, zero drops, exact event/record
joins, and a legally sealed home-agent model. `begin_recovery` returns a new
coordinator with a different epoch and zero counters.

- [ ] **Step 4: Run all crate tests**

```bash
cargo test -p slugarch-tile-model
```

Expected: legal epochs seal; all six fault epochs fail without a successful
partial result.

- [ ] **Step 5: Commit**

```bash
git add crates/slugarch-tile-model
git commit -m "model: join tile records into fail-stop epochs"
```

## Task 6: Add Machine-Readable Corpus Export

**Files:**
- Create: `crates/slugarch-tile-model/examples/export_corpus.rs`
- Create: `crates/slugarch-tile-model/tests/export.rs`

- [ ] **Step 1: Write a canonical-JSON test**

Assert sorted keys, one JSON object per case, stable numeric enums, no pointer
or wall-clock fields, and identical SHA-256 on two runs.

- [ ] **Step 2: Confirm RED**

```bash
cargo test -p slugarch-tile-model --test export
```

Expected: missing exporter.

- [ ] **Step 3: Implement the exporter**

The executable accepts exact output path, tile count, mode, seed, and optional
fault. It writes through a temporary file in the same directory, calls
`sync_all`, renames once, and prints the final SHA-256.

- [ ] **Step 4: Export and compare twice**

```bash
cargo run -p slugarch-tile-model --example export_corpus -- \
  --output /tmp/slugarch-tile-corpus-a.json --tiles 4 --mode validation \
  --seed 6002266168223157317
cargo run -p slugarch-tile-model --example export_corpus -- \
  --output /tmp/slugarch-tile-corpus-b.json --tiles 4 --mode validation \
  --seed 6002266168223157317
sha256sum /tmp/slugarch-tile-corpus-a.json /tmp/slugarch-tile-corpus-b.json
```

Expected: identical hashes.

- [ ] **Step 5: Commit**

```bash
git add crates/slugarch-tile-model
git commit -m "model: export canonical tile evidence"
```

## Task 7: Run the Model Acceptance Gate

**Files:**
- Verify: `crates/slugarch-tile-model`

- [ ] **Step 1: Run formatting, lint, and tests**

```bash
cargo fmt --all -- --check
cargo clippy -p slugarch-tile-model --all-targets -- -D warnings
cargo test -p slugarch-tile-model
```

Expected: all commands pass.

- [ ] **Step 2: Check the six declared codes**

```bash
rg -n "0x1001|0x1002|0x1003|0x1004|0x2001|0x2002" \
  crates/slugarch-tile-model
```

Expected: each code is defined once and asserted in tests.

- [ ] **Step 3: Record the proof boundary**

The generated summary must identify these results as
`qemu_event_level_home_agent_model` and must not contain
`physical_cxl_cache=true`.
