# SlugArch Benchmark Pass 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the paper evidence from one QEMU Type-2 functional run to explicit validator fail-stop coverage and, if the guest can be relaunched, five live QEMU Type-2 repeatability runs.

**Architecture:** Keep fail-stop evidence offline and deterministic in SlugArch so it does not depend on a live guest. Treat live repeatability as a separate artifact gated by a reachable guest-visible BAR2 path. Update the paper only with measured outcomes and explicit limits.

**Tech Stack:** Rust integration tests, SlugArch CLI, CXLMemSim QEMU Type-2 guest helper, JSON/Markdown evidence, LaTeX.

---

### Task 1: Offline Validator Fail-Stop Coverage

**Files:**
- Modify: `crates/slugarch-host/tests/qemu_type2_artifacts.rs`
- Modify if needed: `crates/slugarch-host/src/qemu_type2.rs`
- Create: `docs/evaluation/qemu-type2-failstop-20260702.json`
- Create: `docs/evaluation/qemu-type2-failstop-20260702.md`

- [ ] **Step 1: Write failing tests for explicit fault cases**

Add tests that validate six malformed response streams:

```rust
#[test]
fn validate_fails_on_bad_tag_response() { /* mutate tag 10 to 99 */ }

#[test]
fn validate_fails_on_missing_response() { /* remove final FLIT */ }

#[test]
fn validate_fails_on_extra_duplicate_response() { /* append final FLIT */ }

#[test]
fn validate_fails_on_dispatch_failed_response() { /* replace one Cmp with DispatchFailed */ }

#[test]
fn validate_fails_on_wrong_read_data() { /* change one read payload byte */ }

#[test]
fn validate_fails_on_wrong_response_phase() { /* put Cmp in read-data phase */ }
```

Each test must call `validate_responses`, assert `summary.status == "fail"`, and assert the relevant counter or output mismatch.

- [ ] **Step 2: Run one new test to verify RED**

Run:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test qemu_type2_artifacts validate_fails_on_bad_tag_response
```

Expected before implementation: fail because the test does not compile or because the helper APIs do not exist.

- [ ] **Step 3: Add minimal test helpers or validator fields only if required**

Keep production changes minimal. If existing `QemuType2Summary` can already represent a case, change only tests. If a paper-safe evidence field is missing, add only the smallest field needed.

- [ ] **Step 4: Run the full offline validator test**

Run:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test qemu_type2_artifacts
```

Expected: all validator tests pass.

- [ ] **Step 5: Record fail-stop evidence**

Create `docs/evaluation/qemu-type2-failstop-20260702.json` with one row per fault case, listing `case`, `expected_status`, `observed_status`, `primary_signal`, and `live_qemu_injected=false`.

Create `docs/evaluation/qemu-type2-failstop-20260702.md` with a short claim boundary: these are offline validator/fault-stream cases, not live injected device faults.

### Task 2: Live QEMU Type-2 Repeatability

**Files:**
- Create if successful: `docs/evaluation/qemu-type2-repeatability-20260702.json`
- Create if successful: `docs/evaluation/qemu-type2-repeatability-20260702.md`
- Modify if needed: `targets/qemu-type2/README.md`

- [ ] **Step 1: Check live guest prerequisite**

Run:

```bash
pgrep -af "qemu|cxlmemsim|main_server|slugarch_type2"
ss -ltnp
ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=3 -p 12022 root@127.0.0.1 true
```

Expected for live repeatability: QEMU/CXLMemSim are running and SSH to port `12022` succeeds.

- [ ] **Step 2: Relaunch only if the full guest path is available**

Use the documented CXLMemSim full guest/QEMU Type-2 path, not the `-S` smoke-only path. If the guest cannot be relaunched without missing images or launch inputs, record the blocker and do not invent live results.

- [ ] **Step 3: Run five iterations**

For each run `i=1..5`, run:

```bash
RUN_DIR="artifact/slugarch_cxlmemsim/qemu-type2-repeat-20260702/run-${i}"
cargo run -p slugarch-cli -- export-cxlmemsim targets/qemu-type2/identity_times_const.json --out "$RUN_DIR"
CXLMEMSIM_GUEST_SSH_CMD="ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -p 12022 root@127.0.0.1" \
CXLMEMSIM_GUEST_SCP_CMD="scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -P 12022" \
CXLMEMSIM_GUEST_SCP_TARGET="root@127.0.0.1" \
CXLMEMSIM_ROOT=/tmp/CXLMemSim-slugarch-type2 \
  targets/qemu-type2/run_existing_guest.sh "$RUN_DIR"
cargo run -p slugarch-cli -- validate-cxlmemsim targets/qemu-type2/identity_times_const.json --responses "$RUN_DIR/responses.bin" --out "$RUN_DIR"
```

Expected if live path works: five `summary.json` files with `status=pass`, `request_count=49`, `response_count=49`, `tag_mismatches=0`, and `dispatch_failures=0`.

### Task 3: Paper Update

**Files:**
- Modify: `/root/Concordia/64fa450c44d0cdf46c7c3a7d/eval.tex`

- [ ] **Step 1: Add fail-stop table**

Add a compact table after the first QEMU Type-2 result. State that these are offline validator fault-stream cases and that live injected device faults remain future work.

- [ ] **Step 2: Add repeatability table only if live runs succeed**

If the five live runs complete, add a repeatability table. If relaunch is blocked, add no live-results table; keep the blocker in SlugArch evidence only.

- [ ] **Step 3: Build the paper**

Run from the paper repo:

```bash
latexmk -pdf -interaction=nonstopmode main.tex
```

Expected: PDF builds. Existing font/box warnings are acceptable; LaTeX errors are not.

### Task 4: Final Verification

**Files:** all touched files.

- [ ] **Step 1: Run Rust verification**

Run:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test qemu_type2_artifacts
cargo fmt --check --package slugarch-host --package slugarch-cli
```

- [ ] **Step 2: Check claim wording**

Run:

```bash
rg -n "throughput|speedup|overhead.*[0-9]|provenance precision|CXL.cache.*measured|CXL.mem.*measured" /root/Concordia/64fa450c44d0cdf46c7c3a7d/eval.tex
```

Expected: no unsupported measured claims.

