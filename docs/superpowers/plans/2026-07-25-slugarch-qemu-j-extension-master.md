# SlugArch QEMU J-Extension Master Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the server-authoritative Type-2 CFMWS path, add a fail-closed SlugArch J-extension with Rust, GPU, and FPGA-Verilator implementations, evaluate it with correctness-first five-boot experiments, and integrate only audited results into the 11-page paper.

**Architecture:** Execute four independently testable J-extension plans after completing the existing CFMWS prerequisite. A Rust policy core and stable C ABI define semantics; generated programmable RTL and its Verilated model implement the FPGA backend; QEMU dynamically loads the ABI and adds GPU compiler diagnostics; a predeclared campaign joins server, QEMU, Rust, RTL, GPU, and guest evidence before any paper number is rendered.

**Tech Stack:** Rust 2021, C11/C++17, QEMU Meson/Ninja/qtest, POSIX `dlopen`, CUDA Driver API, SystemVerilog, Verilator, optional Quartus Pro, Python 3, Matplotlib, LaTeX, SHA-256, JSONL, Git.

---

## Approved Design and Plan Suite

The implementation contract is:

`docs/superpowers/specs/2026-07-25-slugarch-qemu-j-extension-design.md`

Execute these plans in order:

1. `docs/superpowers/plans/2026-07-25-slugarch-jit-rust-ffi.md`
   owns the policy schema, canonicalization, verifier, interpreter, record
   semantics, digest, and stable C ABI.
2. `docs/superpowers/plans/2026-07-25-slugarch-jit-fpga-rtl.md`
   owns the runtime-loadable policy RTL, generated artifacts, Verilator HJ
   model, Rust FPGA backend, and semantic equivalence gates.
3. `docs/superpowers/plans/2026-07-25-slugarch-jit-qemu-gpu.md`
   owns the unfinished CFMWS prerequisite, QEMU dynamic loader, BAR2
   capability/register/command ABI, event hooks, fail-stop behavior, and CUDA
   `cuModuleLoadDataEx` diagnostics.
4. `docs/superpowers/plans/2026-07-25-slugarch-jit-evaluation-paper.md`
   owns the immutable campaign manifest, correctness corpus, five-boot timing,
   GPU and FPGA measurements, audited summaries, figure, and 11-page paper
   integration.

## Source and Worktree Boundaries

- SlugArch control tree: `/root/Concordia/SlugArch`
- SlugArch isolated feature worktree to create:
  `/tmp/slugarch-jext-src/SlugArch`
- Existing isolated CXLMemSim tree:
  `/tmp/slugarch-type2-transport-src/CXLMemSim`
- Existing isolated QEMU tree:
  `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu`
- Existing QEMU build:
  `/tmp/slugarch-type2-transport-build/qemu`
- Existing CXLMemSim server build:
  `/tmp/slugarch-type2-transport-build/server`
- Existing isolated kernel:
  `/tmp/slugarch-type2-transport-src/cxl`
- Paper source:
  `/root/Concordia/64fa450c44d0cdf46c7c3a7d`
- Paper isolation root to create:
  `/tmp/slugarch-jext-paper`

The QEMU worktree contains seven pre-existing user modifications. Preserve
them unstaged:

```text
hw/cxl/cxl_hetgpu.c
hw/cxl/cxl_type1.c
hw/cxl/cxl_type2.c
hw/cxl/cxl_type2_coherency.c
include/hw/cxl/cxl_type2.h
include/hw/cxl/cxl_type2_coherency.h
include/hw/cxl/cxl_type2_gpu_cmd.h
```

Intentional J-extension hunks that overlap those files must be staged by hunk
or cached patch and reviewed against:

`/tmp/slugarch-type2-transport-src/source-capture/status/qemu.status`

Never stage the existing untracked July result directories, the untracked July
15 paper plan, or `dvsec_hdm_devmem` from `/root/Concordia/SlugArch`.

## Existing Prerequisite State

The current QEMU implementation already contains:

- `d194be1951 cxl/type2: add SlugArch synchronous wire protocol`;
- `99e757f923 cxl/type2: make SlugArch memory server authoritative`; and
- `229b8b9c96 cxl/type2: advertise one active memory range`.

The unfinished CFMWS task currently has:

- a RED-then-GREEN pure shape validator test;
- a compiling route refactor in `hw/cxl/cxl-host.c`;
- uncommitted edits in `tests/unit/meson.build`;
- uncommitted `tests/unit/test-cxl-type2-route.c`; and
- no completed fake-server READ/WRITE qtest yet.

Resume that state. Do not recreate the transport clone or replay completed
Tasks 1 through 8.

## Global Stop Conditions

Stop timing and paper integration while preserving failed evidence if:

- direct CFMWS access does not reach the server at DPA 80 MiB;
- a write is acknowledged without server commit;
- QEMU, server, Rust, or RTL event counts disagree;
- the policy digest differs in any layer;
- the extension advertises ready after an ABI, policy, or backend failure;
- an explicit backend silently falls back;
- Rust and RTL records differ for a supported policy;
- a required record is dropped;
- malformed PTX produces no bounded diagnostic on a claimed GPU result;
- a five-boot cell has fewer than five valid independent boots;
- a Quartus report is incomplete or not comparable to its no-HJ baseline; or
- the paper requires fabricated, interpolated, borrowed, or untraceable data.

## Task 1: Freeze the Approved Contract and Current State

**Files:**
- Inspect: `docs/superpowers/specs/2026-07-25-slugarch-qemu-j-extension-design.md`
- Inspect: the four child plans listed above
- Inspect: `/tmp/slugarch-type2-transport-src/source-capture/`

- [ ] **Step 1: Record source identities**

Run:

```bash
git -C /root/Concordia/SlugArch rev-parse HEAD
git -C /root/Concordia/SlugArch status --short
git -C /tmp/slugarch-type2-transport-src/CXLMemSim rev-parse HEAD
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu rev-parse HEAD
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu status --short
```

Expected: SlugArch includes the committed design and plan suite; QEMU includes
the three prerequisite commits plus only the recorded user dirt and current
CFMWS task edits.

- [ ] **Step 2: Verify every plan header**

Run:

```bash
rg -n '^# .* Implementation Plan$' \
  docs/superpowers/plans/2026-07-25-slugarch-*.md
rg -n '^> \*\*For agentic workers:\*\*' \
  docs/superpowers/plans/2026-07-25-slugarch-*.md
```

Expected: five plan headers and five required-sub-skill headers.

## Task 2: Complete the Existing CFMWS Prerequisite

**Files:**
- Follow: `docs/superpowers/plans/2026-07-25-slugarch-jit-qemu-gpu.md`
- Modify in isolated QEMU: `hw/cxl/cxl-host.c`
- Modify in isolated QEMU: `include/hw/cxl/cxl_type2.h`
- Modify in isolated QEMU: `tests/unit/meson.build`
- Create in isolated QEMU: `tests/unit/test-cxl-type2-route.c`
- Modify in isolated QEMU: `tests/qtest/cxl-test.c`

- [ ] **Step 1: Finish the fake-server memory protocol**

Implement the qtest server request decoder and response encoder exactly as
specified in the QEMU/GPU child plan.

Expected: one read and one write remain on the same connection after
HELLO/ACK.

- [ ] **Step 2: Prove the direct route**

Run:

```bash
ninja -C /tmp/slugarch-type2-transport-build/qemu \
  tests/unit/test-cxl-type2-route tests/qtest/cxl-test
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 \
  /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test \
  -p /pci/cxl/type2_cfmws
```

Expected: the server observes READ and WRITE at DPA `83886080`, read bytes and
write bytes match, and QOM `slugarch-direct-cfmws` equals 2.

- [ ] **Step 3: Commit the prerequisite**

Stage only task hunks and commit:

```bash
git commit -m "cxl/type2: route one-target CFMWS accesses"
```

Expected: cached `git diff --check` passes and the seven pre-existing user
modifications remain unstaged.

## Task 3: Execute the Rust Policy and FFI Plan

**Files:**
- Follow: `docs/superpowers/plans/2026-07-25-slugarch-jit-rust-ffi.md`

- [ ] **Step 1: Complete all Rust RED/GREEN tasks**

Run:

```bash
cargo test -p slugarch-jit
cargo test -p slugarch-jit-ffi
cargo test -p slugarch-host
```

Expected: parser, verifier, interpreter, digest, ABI, panic containment, and
host migration tests pass.

- [ ] **Step 2: Prove the C ABI**

Run the C smoke command from the child plan.

Expected: ABI 1, policy load, one record, stats, diagnostics, and destroy all
succeed under AddressSanitizer or Valgrind with no invalid access.

## Task 4: Execute the FPGA RTL and Verilator Plan

**Files:**
- Follow: `docs/superpowers/plans/2026-07-25-slugarch-jit-fpga-rtl.md`

- [ ] **Step 1: Generate and compile the programmable HJ top**

Run:

```bash
cargo test -p slugcxl-gen
cargo test -p slugarch-verilator-sys
cargo test -p slugarch-verilator
```

Expected: snapshots, runtime-policy load, atomic commit, timeout, and counter
tests pass.

- [ ] **Step 2: Prove semantic equivalence**

Run:

```bash
cargo test -p slugarch-jit --test rtl_equivalence --features fpga-verilator
```

Expected: validation, delta, and full modes produce zero semantic record,
epoch, digest, or failure mismatches across the full deterministic corpus.

## Task 5: Execute the QEMU and GPU Plan

**Files:**
- Follow: `docs/superpowers/plans/2026-07-25-slugarch-jit-qemu-gpu.md`

- [ ] **Step 1: Build QEMU and run focused tests**

Run:

```bash
ninja -C /tmp/slugarch-type2-transport-build/qemu \
  qemu-system-x86_64 tests/unit/test-cxl-slugarch-jit \
  tests/qtest/cxl-test
```

Expected: the loader, ABI, registers, commands, strict event path, and backend
selection compile.

- [ ] **Step 2: Run the J-extension qtest matrix**

Run the child plan's `QTEST_QEMU_BINARY` command.

Expected: absent, Rust, GPU, FPGA, ABI-failure, policy-failure, no-fallback,
drop, and CFMWS record cases all have their exact declared outcome.

- [ ] **Step 3: Run GPU diagnostic tests only when real CUDA is present**

Run:

```bash
nvidia-smi
/tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test \
  -p /pci/cxl/type2_jext_gpu
```

Expected when eligible: valid PTX loads; malformed PTX returns a nonempty
bounded compiler error. Otherwise record a blocked GPU-hardware result without
converting simulation success into GPU evidence.

## Task 6: Execute the Evaluation and Paper Plan

**Files:**
- Follow: `docs/superpowers/plans/2026-07-25-slugarch-jit-evaluation-paper.md`

- [ ] **Step 1: Seal the manifest before measurements**

Expected: all matrix cells, order seed, repetitions, hashes, exclusions,
metrics, and stop conditions exist before the first measured boot.

- [ ] **Step 2: Complete correctness and timing gates**

Expected: the complete eligible campaign has five valid independent boots per
cell, exact cross-layer joins, no drops, and immutable raw artifacts.

- [ ] **Step 3: Generate audited summaries and the four-panel figure**

Expected: every plotted number resolves to a JSON key derived from raw data;
blocked physical fields remain visibly blocked.

- [ ] **Step 4: Integrate and build the paper**

Run:

```bash
python3 scripts/check_paper_contract.py --stage final
latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
pdfinfo main.pdf
```

Expected: contract passes, LaTeX builds, main text ends no later than page 11,
and references begin after the main-text boundary.

## Final Verification

- [ ] **Step 1: Run the complete software regression**

Run:

```bash
cargo test --workspace
ctest --test-dir /tmp/slugarch-type2-transport-build/server --output-on-failure
ninja -C /tmp/slugarch-type2-transport-build/qemu test
```

Expected: all in-scope tests pass; unrelated recorded baseline failures, if
any, remain separately identified.

- [ ] **Step 2: Audit claims against artifacts**

Verify:

```text
paper number -> normalized JSON key -> raw boot UUID -> checksums
Rust record -> QEMU event ID -> server request/sequence
RTL record -> policy digest -> generated RTL hash
GPU diagnostic -> module event ID -> driver/device metadata
```

Expected: every arrow resolves without inference.

- [ ] **Step 3: Commit the final paper and evidence index**

Commit only after all gates pass:

```bash
git commit -m "paper: evaluate the SlugArch QEMU J-extension"
```

Expected: the commit includes the audited paper sources, figure source, data
index, and claim ledger, but not mutable build directories or unreviewed raw
payloads.
