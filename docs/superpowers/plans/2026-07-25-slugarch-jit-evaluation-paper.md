# SlugArch JIT Evaluation and Paper Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collect correctness-first, five-independent-boot evidence for the QEMU SlugArch J-extension, audit every cross-layer join, and replace weaker paper material with one critical four-panel figure while preserving the 11-page main-text limit.

**Architecture:** A checked-in campaign schema and immutable armed manifest predeclare all cells, ordering, metrics, exclusions, hashes, and stop conditions. Each boot has a fresh server, QEMU process, event files, UUID, and counter brackets; validation joins guest, QEMU, Rust/RTL, GPU, and server evidence before aggregation. Plotting accepts only a checksum-valid normalized snapshot, and the paper contract rejects unsupported proof language or an extra figure/table/page.

**Tech Stack:** Python 3 standard library, Bash, Rust/C test binaries, QEMU, CXLMemSim, JSON/JSONL, SHA-256, Matplotlib, LaTeX, BibTeX, `pdfinfo`, `pdftotext`.

---

## File Structure

### SlugArch campaign control

- Create `targets/qemu-type2/jext/campaign_schema.py`: typed manifest and
  result validation without third-party packages.
- Create `targets/qemu-type2/jext/create_manifest.py`: predeclare exact cells,
  deterministic order, hashes, and arm state.
- Create `targets/qemu-type2/jext/run_campaign.py`: one fresh
  server/QEMU/guest boot per cell repetition.
- Create `targets/qemu-type2/jext/run_gpu_jit.py`: fresh-context valid/invalid
  PTX campaign.
- Create `targets/qemu-type2/jext/validate_campaign.py`: per-boot and global
  joins, eligibility, and first-failure reasons.
- Create `targets/qemu-type2/jext/aggregate_campaign.py`: median/min/max and
  derived metrics from validated boots only.
- Create `targets/qemu-type2/jext/seal_campaign.py`: immutable SHA-256
  inventory and outcome.
- Create `targets/qemu-type2/jext/tests/`: synthetic valid and corrupt cases.
- Create `targets/qemu-type2/jext/policies/validation.json`.
- Create `targets/qemu-type2/jext/policies/delta.json`.
- Create `targets/qemu-type2/jext/policies/full.json`.
- Create `targets/qemu-type2/jext/ptx/minimal-valid.ptx`.
- Reuse `qemu_integration/guest_libcuda/gpu_benchmark.c` PTX fixtures through
  extracted, hashed test inputs.
- Create `targets/qemu-type2/jext/ptx/malformed.ptx`.

### Evidence outputs

- Raw root: `artifact/slugarch_jext/<campaign-uuid>/`
- Create after validation:
  `docs/evaluation/slugarch-jext-<date>.json`
- Create after validation:
  `docs/evaluation/slugarch-jext-<date>.md`
- Create:
  `docs/evaluation/slugarch-jext-claim-ledger-<date>.json`

### Paper repository

- Create or modify `/tmp/slugarch-jext-paper/scripts/check_paper_contract.py`.
- Create `/tmp/slugarch-jext-paper/scripts/plot_slugarch_jext.py`.
- Create `/tmp/slugarch-jext-paper/scripts/test_plot_slugarch_jext.py`.
- Modify `/tmp/slugarch-jext-paper/eval.tex`.
- Modify `/tmp/slugarch-jext-paper/design.tex`.
- Modify `/tmp/slugarch-jext-paper/intro.tex` only to align implemented versus
  proposed wording.
- Modify `/tmp/slugarch-jext-paper/related.tex` only if the staged
  implementation paragraph is stale.
- Modify `/tmp/slugarch-jext-paper/main.tex` only for global SlugArch naming
  and macro consistency.
- Create `/tmp/slugarch-jext-paper/img/slugarch-jext.pdf`.
- Modify bibliography only for already-approved references; do not invent a
  new citation for the implementation itself.

## Predeclared Measurement Matrix

### CXL.mem timing

Validation mode uses:

```text
latency_ns = [80, 400, 2000, 10000]
backend = [off, rust, fpga-verilator]
repetitions = 5 independent boots
```

This is 60 boots. Every boot measures both:

- dependent 8-byte load; and
- bounded 64-byte transfer implemented as eight ordered 8-byte accesses.

Delta/full payload modes use:

```text
latency_ns = [400, 2000]
backend = [rust, fpga-verilator]
record_mode = [delta, full]
repetitions = 5 independent boots
```

This is 40 additional boots. The complete timed matrix is exactly 100 boots.
No off cell is repeated for delta/full because off has no record mode; the
validation off cell at the same latency is the declared common baseline.

Deterministic shuffle seed:

```text
0x534c55474a455854
```

The manifest stores the fully expanded order before arming.

### GPU JIT

Each of four PTX inputs runs in five fresh CUDA contexts:

```text
minimal valid
vector add
GEMM
malformed
```

Then the three valid inputs run five times in one retained context to separate
cold-context from same-context behavior. These are GPU context repetitions,
not CXL.mem boots, and are reported separately.

### Fixed phase sizes

Per timed boot:

```text
warmup iterations = 100 (unmeasured)
dependent-load iterations = 10,000
64-byte transfer iterations = 2,000
counter snapshots = before and after each measured phase
```

The operation counts may change only before arming and only if a recorded pilot
shows a phase exceeds 120 seconds. A pilot uses a different campaign UUID and
never enters the paper dataset.

## Task 1: Build the Manifest and Synthetic Validation Framework

**Files:**
- Create all `campaign_schema.py`, `create_manifest.py`,
  `validate_campaign.py`, and test files listed above.

- [ ] **Step 1: Write failing schema tests**

Tests require:

```python
self.assertEqual(len(expand_cxlmem_cells()), 100)
self.assertEqual(len({c.boot_uuid for c in cells}), 100)
self.assertEqual(sorted({c.latency_ns for c in cells}), [80, 400, 2000, 10000])
self.assertEqual(CAMPAIGN_SEED, 0x534C55474A455854)
```

Also reject duplicate UUID, missing hash, nonabsolute binary path, wrong
repetition count, undeclared cell, post-arm mutation, and a manifest whose
ordered-cell hash differs.

- [ ] **Step 2: Run and confirm RED**

```bash
PYTHONPATH=targets/qemu-type2/jext \
  python3 -m unittest discover -s targets/qemu-type2/jext/tests -v
```

Expected: import or missing-function failures.

- [ ] **Step 3: Implement immutable typed records**

Use frozen dataclasses for `Cell`, `InputIdentity`, and `Manifest`. Serialize
with:

```python
json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
```

Hash the canonical bytes with SHA-256. `armed_at` changes the manifest from
draft to immutable; no run command accepts a draft or a modified armed file.

- [ ] **Step 4: Implement exact matrix expansion**

Validation creates 60 cells and delta/full creates 40. UUIDs use UUIDv5 over
campaign UUID plus canonical cell identity; shuffle uses
`random.Random(CAMPAIGN_SEED)`.

- [ ] **Step 5: Add synthetic boot fixtures**

One valid fixture contains joined:

```text
server handshake and four memory completions
QEMU request/completion and delay events
J-extension request/completion records
guest phase and checksum
before/after server counters
before/after J counters
process exits and binary hashes
```

Corrupt fixtures change exactly one field each.

- [ ] **Step 6: Implement exact first-failure validation**

Order:

```text
manifest/hash
process/boot identity
topology/path
server operation
QEMU/server join
J-extension/QEMU join
policy digest
payload/checksum
counter delta
drop/reject/error
timing completeness
```

Return the first stable code, such as
`E_JOIN_SERVER_SEQUENCE`, not a generic invalid result.

- [ ] **Step 7: Test and commit**

```bash
PYTHONPATH=targets/qemu-type2/jext \
  python3 -m unittest discover -s targets/qemu-type2/jext/tests -v
git add targets/qemu-type2/jext
git commit -m "eval: define the SlugArch J-extension campaign"
```

## Task 2: Add One-Boot Orchestration

**Files:**
- Create: `targets/qemu-type2/jext/run_campaign.py`
- Create: `targets/qemu-type2/jext/seal_campaign.py`
- Create: `targets/qemu-type2/jext/tests/test_runner.py`

- [ ] **Step 1: Write command-construction tests**

Given one cell, assert exact server and QEMU arguments include:

```text
256 MiB server capacity
selected modeled latency
one 256 MiB CFMWS target
sync-type2-wire=true
strict J-extension mode/backend/policy/log
fresh absolute event paths
phase and boot UUID
```

Off mode must omit J library/policy and advertise no J capability.

- [ ] **Step 2: Confirm RED**

Run the focused runner tests. Expected: missing runner.

- [ ] **Step 3: Implement one-cell state machine**

States:

```text
CREATED
SERVER_READY
QEMU_READY
GUEST_READY
PREFLIGHT_PASSED
WARMED
MEASURED
VALIDATED
SEALED
FAILED
```

Each transition writes and fsyncs `run_state.json`. Any process exit, timeout,
counter mismatch, or validation error goes to FAILED and preserves logs.

- [ ] **Step 4: Implement preflight**

Require:

- command-line Type-2 endpoint and one CFMWS;
- server handshake capacity 256 MiB;
- direct DPA 80 MiB sentinel read/write;
- J capability absent for off and ready/digest/backend exact otherwise;
- QOM and BAR2 J views agree;
- zero preflight drop/reject/error;
- guest clock and phase helper available; and
- server/QEMU/J counters start from a bracketed snapshot.

- [ ] **Step 5: Implement measured phases**

Guest emits one JSON record per phase with count, bytes, elapsed ns, checksum,
and exit. The runner snapshots counters before/after and validates exact
expected deltas before advancing.

- [ ] **Step 6: Implement sealing**

`SHA256SUMS` covers every regular file except itself and mutable lock files.
`seal.json` records manifest hash, ordered-cell hash, validator version/hash,
outcome, and immutable timestamp.

- [ ] **Step 7: Test dry-run and commit**

```bash
python3 targets/qemu-type2/jext/run_campaign.py \
  --manifest /tmp/jext-test-manifest.json --cell 0 --dry-run
git commit -m "eval: orchestrate isolated J-extension boots"
```

Expected: dry-run prints absolute commands and changes no external process.

## Task 3: Run Correctness and Fault-Injection Gates

**Files:**
- Use: Rust/RTL/QEMU tests from preceding plans.
- Produce: `artifact/slugarch_jext/<uuid>/correctness/`.

- [ ] **Step 1: Record all binary/source identities**

Hash:

```text
SlugArch commit and in-scope patch
CXLMemSim commit and binary
QEMU commit, in-scope patch, and binary
Rust JIT shared library
policy files
generated RTL and policy image
kernel and guest image
guest benchmark/debug binaries
CUDA driver and GPU identity
Verilator and optional Quartus versions
```

- [ ] **Step 2: Run the deterministic semantic corpus**

Run Rust and RTL equivalence for validation/delta/full, including all injected
faults. Archive one semantic JSON record per case.

Expected: supported nonfault cases have zero mismatch; every fault has its
declared first-failure code.

- [ ] **Step 3: Run QEMU capability/fail-stop matrix**

Expected: no silent fallback, exact error states, and complete close/unrealize.

- [ ] **Step 4: Run the live CFMWS preflight**

Expected: DPA 80 MiB read/write joins server, QEMU, and J records with direct
counter 2 for the two operations and zero bypass/local completions.

- [ ] **Step 5: Seal correctness outcome**

If any gate fails, do not arm timing. Preserve and report the failed
correctness campaign.

## Task 4: Arm and Run the 100-Boot Timing Campaign

**Files:**
- Create armed manifest under the raw campaign root.
- Produce one directory per boot UUID.

- [ ] **Step 1: Create draft manifest**

```bash
python3 targets/qemu-type2/jext/create_manifest.py \
  --out artifact/slugarch_jext/<uuid>/manifest.draft.json \
  --server /tmp/slugarch-type2-transport-build/server/cxlmemsim_server \
  --qemu /tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 \
  --jit-lib /tmp/slugarch-jext-src/SlugArch/target/release/libslugarch_jit_ffi.so \
  --guest-image <private-image> \
  --kernel <kernel>
```

Expected: exactly 100 ordered cells and all hashes.

- [ ] **Step 2: Audit then arm**

```bash
python3 targets/qemu-type2/jext/create_manifest.py \
  --arm artifact/slugarch_jext/<uuid>/manifest.draft.json \
  --out artifact/slugarch_jext/<uuid>/manifest.json
```

Expected: armed canonical hash and no mutable sentinel values.

- [ ] **Step 3: Execute in predeclared order**

```bash
python3 targets/qemu-type2/jext/run_campaign.py \
  --manifest artifact/slugarch_jext/<uuid>/manifest.json
```

Send a progress update after every completed cell and at least every 60
seconds. Do not use a blocking wait longer than 60 seconds.

- [ ] **Step 4: Validate every boot immediately**

Invalid boots remain invalid under their UUID. Environmental retry receives a
new UUID linked by `retry_of`; it does not overwrite the original. A cell is
complete only with five valid independent boots.

- [ ] **Step 5: Seal the raw campaign**

```bash
python3 targets/qemu-type2/jext/seal_campaign.py \
  artifact/slugarch_jext/<uuid>
```

Expected: complete checksum inventory and immutable complete or failed outcome.

## Task 5: Run the GPU JIT Experiment

**Files:**
- Create: `targets/qemu-type2/jext/run_gpu_jit.py`
- Produce: `artifact/slugarch_jext/<uuid>/gpu-jit/`.

- [ ] **Step 1: Verify eligibility**

Require real NVIDIA device, driver, `cuModuleLoadDataEx`, and non-simulation
QEMU backend. Record `nvidia-smi -q`, driver version, device UUID, compute
capability, and PTX hashes.

If any requirement fails, create `blocked.json` and stop the GPU measurement.

- [ ] **Step 2: Run five fresh-context repetitions**

For each of four inputs, create a new context, compile once, retrieve
diagnostics, unload successful modules, destroy context, and fsync evidence.

- [ ] **Step 3: Run same-context repetitions**

For each valid input, compile five times in one retained context. Do not mix
these values with fresh-context results.

- [ ] **Step 4: Validate diagnostic utility**

Valid inputs require success and usable module handles. Malformed input
requires invalid-PTX status, nonempty error log, and a diagnostic containing
the injected source line or token location. Otherwise J3 fails.

- [ ] **Step 5: Seal the GPU evidence**

Include PTX payload hashes but not full proprietary input unless explicitly
approved.

## Task 6: Collect FPGA Debug and Optional Resource Evidence

**Files:**
- Produce: `artifact/slugarch_jext/<uuid>/fpga/`.

- [ ] **Step 1: Export Verilator metrics**

For each corpus case record policy-load cycles, commit cycles, event-accept
cycles, record-valid cycles, stalls, records, metadata bytes, epochs, drops,
and semantic comparison.

- [ ] **Step 2: Verify repeatability**

Run the complete equivalence corpus five times from reset. Require identical
cycle counts and records or explain deterministic build/runtime variation
before proceeding.

- [ ] **Step 3: Attempt matched Quartus evidence only if eligible**

HJ and no-HJ builds must use the same tool, device, top-level boundary,
constraints, seeds, and non-HJ endpoint sources. Archive reports and compare
ALMs/LUTs, registers, memory, and Fmax.

If unmatched, incomplete, unconstrained, or license-blocked, emit
`resource_status: blocked` and no numeric delta.

## Task 7: Validate and Aggregate Without Fabrication

**Files:**
- Create: `targets/qemu-type2/jext/aggregate_campaign.py`
- Create: `targets/qemu-type2/jext/tests/test_aggregate.py`
- Create eligible normalized JSON/Markdown under `docs/evaluation/`.

- [ ] **Step 1: Write aggregation tests**

Synthetic five-value input `[11, 9, 10, 12, 8]` yields:

```python
{"median": 10, "min": 8, "max": 12, "n": 5}
```

Reject `n != 5`, duplicate boot UUID, mixed cell identity, unsealed campaign,
invalid boot, and any missing join.

- [ ] **Step 2: Implement primary metrics**

For each eligible cell compute median/min/max for:

- guest dependent-load ns/op;
- guest 64-byte transfer ns/op;
- QEMU/Rust or RTL event time;
- applied delay;
- metadata bytes;
- record/epoch/drop/reject counts; and
- extension-added median relative to the declared off baseline.

GPU fresh-context and same-context compile durations are separate summaries.
FPGA cycles and resource status are separate summaries.

- [ ] **Step 3: Enforce calibration**

For off-mode dependent loads, the four medians must be strictly increasing in
configured-latency order. Report configured, server-returned, QEMU-applied, and
guest-observed values separately. A failed monotonic gate blocks quantitative
timing claims but preserves correctness evidence.

- [ ] **Step 4: Enforce semantic and drop gates**

Require:

```text
Rust/RTL semantic mismatches = 0
required record drops = 0
unclassified required events = 0
policy digest mismatches = 0
cross-layer join mismatches = 0
```

- [ ] **Step 5: Write normalized snapshot and claim ledger**

Every numeric field carries source campaign UUID, raw relative paths, and
metric definition. The ledger marks claims measured, simulator-only, RTL-only,
fit-harness-only, blocked, or not evaluated.

- [ ] **Step 6: Test and commit**

```bash
PYTHONPATH=targets/qemu-type2/jext \
  python3 -m unittest discover -s targets/qemu-type2/jext/tests -v
git add targets/qemu-type2/jext docs/evaluation/slugarch-jext-*.json \
  docs/evaluation/slugarch-jext-*.md
git commit -m "eval: validate the SlugArch J-extension results"
```

## Task 8: Render the Critical Four-Panel Figure

**Files:**
- Create: `/tmp/slugarch-jext-paper/scripts/plot_slugarch_jext.py`
- Create: `/tmp/slugarch-jext-paper/scripts/test_plot_slugarch_jext.py`
- Create: `/tmp/slugarch-jext-paper/img/slugarch-jext.pdf`

- [ ] **Step 1: Write failing data and geometry tests**

Require:

```text
Figure size 7.1 x 3.0 inches
exactly four labeled panels
font size >= 7 pt
all timing whiskers use min/max
blocked resource field renders "blocked", not zero
no input outside the normalized audited JSON
deterministic PDF metadata/output hash after normalization
```

- [ ] **Step 2: Confirm RED**

```bash
cd /tmp/slugarch-jext-paper
python3 -m unittest scripts/test_plot_slugarch_jext.py -v
```

Expected: missing renderer.

- [ ] **Step 3: Implement panels**

Panel A: off-mode guest dependent-load and 64-byte-transfer medians across
80/400/2000/10000 ns with min/max whiskers.

Panel B: extension-added time for Rust and FPGA-Verilator validation mode at
the same latency points, plus delta/full markers at 400/2000 ns.

Panel C: validation/delta/full metadata bytes and a compact correctness/fault
annotation showing zero semantic mismatch and detected injected faults.

Panel D: two aligned insets:

- GPU fresh-context valid/malformed compile time and diagnostic outcome; and
- FPGA policy/event/record cycles plus resource delta or a visible blocked
  label.

Do not put unlike units on one unlabeled axis.

- [ ] **Step 4: Render only from eligible data**

```bash
python3 scripts/plot_slugarch_jext.py \
  --input /root/Concordia/SlugArch/docs/evaluation/slugarch-jext-<date>.json \
  --output img/slugarch-jext.pdf
```

Expected: renderer refuses ineligible timing or checksum mismatch.

- [ ] **Step 5: Test and commit in the isolated paper tree**

```bash
python3 -m unittest scripts/test_plot_slugarch_jext.py -v
git add scripts/plot_slugarch_jext.py scripts/test_plot_slugarch_jext.py \
  img/slugarch-jext.pdf
git commit -m "paper: add the SlugArch J-extension results figure"
```

## Task 9: Revise the Paper Within 11 Pages

**Files:**
- Modify paper sources listed in File Structure.
- Modify/create `scripts/check_paper_contract.py`.

- [ ] **Step 1: Add source-contract failures first**

The checker requires:

```text
global system name SlugArch, no stale CFR system naming
exactly two evaluation figure* environments
no quantitative result table
one legacy evidence figure
one slugarch-jext figure
GPU PTX JIT explicitly distinguished from replay-policy JIT
Verilator/fit/physical proof levels explicitly distinguished
legacy BAR2 boundary statement retained
CXL.mem claim requires eligible normalized J-extension snapshot
main text ends on or before page 11
```

- [ ] **Step 2: Run and confirm RED**

```bash
cd /tmp/slugarch-jext-paper
python3 scripts/check_paper_contract.py --stage final
```

Expected: fail on old evaluation/figure/claim structure.

- [ ] **Step 3: Update design wording**

Describe the implemented version-1 restricted program precisely:

- 32 forward-only instructions;
- four address ranges;
- one record/event;
- fixed metadata/payload bounds;
- SHA-256 policy digest;
- Rust verifier/interpreter;
- programmable FPGA RTL/Verilator backend; and
- QEMU vendor capability.

Keep broader cross-vendor, physical-device, CXL.cache, migration, and recovery
mechanisms as future architecture.

- [ ] **Step 4: Rewrite evaluation around critical results**

Use the four-panel figure as the quantitative center. State exact methodology:
100 independent QEMU boots, fixed latency grid, five repetitions,
median/min/max, separate GPU context experiment, semantic equivalence gate, and
optional FPGA resource gate.

Every number is inserted from the normalized snapshot, preferably through
generated LaTeX macros or a checked script, not manual transcription.

- [ ] **Step 5: Preserve proof boundaries**

Required statements:

```text
GPU native PTX compilation is not the SlugArch replay-policy JIT.
The FPGA execution result is Verilator RTL unless a live board run is present.
The 64-byte SlugCXL encoding is local and not a standards-compliant CXL FLIT.
The earlier BAR2 result remains guest-software boundary evidence.
Physical CXL, CXL.cache, DMA, ATS, migration, switch ordering, recovery,
power, and production security remain blocked or not evaluated.
```

- [ ] **Step 6: Build and enforce the page gate**

```bash
latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
pdfinfo main.pdf
pdftotext -layout main.pdf /tmp/slugarch-jext-paper.txt
python3 scripts/check_paper_contract.py --stage final
```

Expected: build and contract pass; conclusion ends no later than page 11
before references. Do not change geometry, margins, or use sub-7-point figure
text to pass.

- [ ] **Step 7: Commit**

```bash
git add intro.tex design.tex eval.tex related.tex main.tex \
  scripts/check_paper_contract.py img/slugarch-jext.pdf
git commit -m "paper: evaluate the SlugArch QEMU J-extension"
```

## Final Artifact Audit

- [ ] **Step 1: Resolve every paper number**

Produce an audit table:

```text
LaTeX macro/text -> normalized JSON key -> campaign cell ->
five boot UUIDs -> raw files -> SHA256SUMS
```

Expected: no missing edge.

- [ ] **Step 2: Verify citations and page count**

```bash
rg -n 'undefined citations|undefined references|LaTeX Warning' main.log
pdfinfo main.pdf | rg '^Pages:'
```

Expected: no undefined citation/reference and compliant page structure.

- [ ] **Step 3: Verify deliverables**

Final handoff includes:

```text
source commits
binary and policy hashes
correctness campaign outcome
100-boot campaign outcome
GPU eligible result or blocked reason
FPGA Verilator result and optional resource status
normalized JSON and claim ledger
figure source and PDF
paper PDF and page boundary
remaining blocked claims
```
