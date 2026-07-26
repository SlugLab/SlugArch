# SlugArch Small CXL Tiles Evaluation and Paper Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collect the approved 125 independent QEMU/CXLMemSim boots, validate four focused RQs, render two evidence-bounded figures, and rewrite the paper so main text ends by page 11.

**Architecture:** A standard-library Python harness predeclares every cell and UUID, launches fresh processes, validates each boot before aggregation, and seals raw evidence with SHA-256. Plotting consumes only the normalized checksum-valid snapshot. An isolated paper clone receives the small-tile story, LegoOS citation, SlugArch-only naming, and exactly two figures; a contract checker rejects extra quantitative tables, unsupported proof language, or a reference start after page 12.

**Tech Stack:** Python 3, JSON/JSONL, SHA-256, QEMU, CXLMemSim, Rust, Verilator, Matplotlib, LaTeX, BibTeX, `pdfinfo`, `pdftotext`.

---

## File Structure

- Create `targets/qemu-type2/tiles/campaign_schema.py`: frozen cell and
  manifest types.
- Create `targets/qemu-type2/tiles/create_manifest.py`: exact 125-boot
  expansion and arming.
- Create `targets/qemu-type2/tiles/run_campaign.py`: fresh-process state
  machine.
- Create `targets/qemu-type2/tiles/validate_campaign.py`: per-boot and global
  proof gates.
- Create `targets/qemu-type2/tiles/aggregate_campaign.py`: RQ metrics.
- Create `targets/qemu-type2/tiles/seal_campaign.py`: checksum inventory.
- Create `targets/qemu-type2/tiles/plot_results.py`: four-panel Figure 2.
- Create `targets/qemu-type2/tiles/tests/`: valid and one-error fixtures.
- Create `targets/qemu-type2/tiles/policies/{validation,delta,full}.json`.
- Produce `artifact/slugarch_small_tiles/20260726/`.
- Produce `docs/evaluation/slugarch-small-tiles-20260726.json`.
- Produce `docs/evaluation/slugarch-small-tiles-20260726.md`.
- Create in isolated paper `scripts/check_paper_contract.py`.
- Create in isolated paper `scripts/plot_tile_architecture.py`.
- Copy the reviewed normalized snapshot into isolated paper
  `data/slugarch-small-tiles-20260726.json`.
- Create isolated paper `img/slugarch-tile-architecture.pdf`.
- Create isolated paper `img/slugarch-tile-results.pdf`.
- Modify isolated paper `main.tex`, `intro.tex`, `background.tex`, `design.tex`,
  `semantics.tex`, `eval.tex`, `related.tex`, and `cite.bib`.

## Task 1: Encode and Test the Exact 125-Boot Matrix

**Files:**
- Create: `targets/qemu-type2/tiles/campaign_schema.py`
- Create: `targets/qemu-type2/tiles/create_manifest.py`
- Create: `targets/qemu-type2/tiles/tests/test_manifest.py`
- Create: `targets/qemu-type2/tiles/policies/validation.json`
- Create: `targets/qemu-type2/tiles/policies/delta.json`
- Create: `targets/qemu-type2/tiles/policies/full.json`

- [ ] **Step 1: Write failing matrix tests**

```python
cells = expand_cells()
self.assertEqual(len(cells), 125)
self.assertEqual(len({cell.boot_uuid for cell in cells}), 125)
self.assertEqual(sum(c.family == "latency" for c in cells), 60)
self.assertEqual(sum(c.family == "scale" for c in cells), 45)
self.assertEqual(sum(c.family == "record_mode" for c in cells), 20)
self.assertTrue(all(1 <= c.repetition <= 5 for c in cells))
```

Also require exact latency set `{80, 400, 2000, 10000}`, tile set
`{1, 2, 4, 8}`, backend set `{off, rust, fpga-verilator}`, and no duplicate
family entry for the same topology, latency, backend, mode, and repetition.
There are exactly 15 one-tile/400-ns validation boots and exactly 15
four-tile/400-ns validation boots.

- [ ] **Step 2: Confirm RED**

```bash
PYTHONPATH=targets/qemu-type2/tiles \
  python3 -m unittest discover -s targets/qemu-type2/tiles/tests \
  -p 'test_manifest.py' -v
```

Expected: missing schema.

- [ ] **Step 3: Implement frozen records**

Use frozen dataclasses for `Cell`, `InputIdentity`, and `Manifest`. UUIDv5 uses
campaign namespace `b9d3693d-5bad-5ee2-9198-83f668cb77df` plus canonical cell
identity. Shuffle uses seed `0x534c554754494c45`.

```python
@dataclass(frozen=True)
class Cell:
    family: str
    tiles: int
    latency_ns: int
    backend: str
    record_mode: str
    repetition: int
    boot_uuid: str
    order_seed: int

@dataclass(frozen=True)
class InputIdentity:
    name: str
    absolute_path: str
    sha256: str

@dataclass(frozen=True)
class Manifest:
    campaign_uuid: str
    cells: tuple[Cell, ...]
    inputs: tuple[InputIdentity, ...]
    order_seed: int
    ordered_cells_sha256: str
    armed_at: str | None
```

- [ ] **Step 4: Implement arm-once behavior**

Canonical JSON uses sorted keys and compact separators. Arming stores the
manifest SHA-256; every run command recomputes it and rejects any mutation.

- [ ] **Step 5: Run tests and commit**

```bash
PYTHONPATH=targets/qemu-type2/tiles \
  python3 -m unittest discover -s targets/qemu-type2/tiles/tests -v
git add targets/qemu-type2/tiles
git commit -m "eval: predeclare the 125-boot tile campaign"
```

## Task 2: Implement the Fresh-Boot Runner

**Files:**
- Create: `targets/qemu-type2/tiles/run_campaign.py`
- Create: `targets/qemu-type2/tiles/seal_campaign.py`
- Create: `targets/qemu-type2/tiles/tests/test_runner.py`

- [ ] **Step 1: Write failing state-machine tests**

Require:

```text
CREATED
SERVER_READY
QEMU_READY
PREFLIGHT_PASSED
WARMED
MEASURED
VALIDATED
SEALED
FAILED
```

Each transition atomically replaces and fsyncs `run_state.json`. A retry gets a
new UUID and immutable `retry_of`; no directory is overwritten.

- [ ] **Step 2: Confirm RED**

Run `test_runner.py`. Expected: missing runner.

- [ ] **Step 3: Implement preflight**

Require fresh server/QEMU PIDs, unique tile/client IDs, exact backend without
fallback, policy digest agreement, zero counters, direct DPA 80 MiB read/write,
nonzero server sequence, QEMU direct counter two, and no local/bypass
completion.

- [ ] **Step 4: Implement measured phases**

Every boot runs 100 warmup iterations then:

```text
private partitions: 10,000 measured events per active tile
read-shared fanout: 10,000 measured events per active tile
producer/consumer: 10,000 measured events per active tile
hot-line ping-pong: 10,000 measured events per active tile
dependent 8-byte loads: 10,000 iterations for one-tile calibration
64-byte transfers: 2,000 iterations for one-tile calibration
```

Snapshot server, QEMU, model, Rust/RTL, and coordinator counters before and
after every phase.

- [ ] **Step 5: Implement process cleanup and sealing**

Gracefully terminate guest/QEMU/server, wait with bounded deadlines, record
exit status, and preserve logs. `SHA256SUMS` covers every regular raw file
except itself.

- [ ] **Step 6: Dry-run and commit**

```bash
python3 targets/qemu-type2/tiles/run_campaign.py \
  --manifest /tmp/slugarch-small-tiles-manifest.json --cell-index 0 --dry-run
```

Expected: absolute commands and output paths, no process launch.

## Task 3: Validate Correctness and Fault Localization

**Files:**
- Create: `targets/qemu-type2/tiles/validate_campaign.py`
- Create: `targets/qemu-type2/tiles/tests/test_validation.py`
- Use: `crates/slugarch-tile-model`

- [ ] **Step 1: Create one valid and twelve corrupt fixtures**

Corrupt exactly one field for process identity, binary hash, tile identity,
server sequence, event join, request join, policy digest, payload commitment,
counter delta, record drop, checksum, and seal hash.

- [ ] **Step 2: Implement stable first-failure order**

```text
manifest
process identity
binary identity
topology
direct CFMWS
server/QEMU join
tile/model join
policy digest
payload/checksum
counters
drop/reject/error
timing
seal
```

- [ ] **Step 3: Add the six-fault oracle**

Require five fresh executions per eligible topology/backend pair. Every fault
must return its declared code and exact `(tile_id, event_id)` with zero false
positives. Host-only suspect set equals all tiles that could write or complete
the affected object.

- [ ] **Step 4: Run synthetic validation**

```bash
PYTHONPATH=targets/qemu-type2/tiles \
  python3 -m unittest discover -s targets/qemu-type2/tiles/tests -v
```

Expected: valid fixture passes and every corrupt fixture fails at its expected
first code.

- [ ] **Step 5: Commit**

```bash
git add targets/qemu-type2/tiles
git commit -m "eval: validate tile evidence and fault localization"
```

## Task 4: Run Correctness Gates Before Timing

**Files:**
- Produce: `artifact/slugarch_small_tiles/20260726/correctness/`

- [ ] **Step 1: Freeze all source and binary identities**

Record SlugArch, CXLMemSim, QEMU, Rust JIT, generated RTL, Verilator, policy,
kernel, image, and guest helper hashes.

- [ ] **Step 2: Run supported Rust/RTL equivalence**

Expected: zero byte, digest, epoch, counter, or error-code mismatches for
validation, delta, and full modes.

- [ ] **Step 3: Run all six injected faults**

Expected: every declared fault is detected with exact tile/event attribution
and no successful partial epoch.

- [ ] **Step 4: Run five direct-CFMWS sentinel processes**

Expected: all five pass with joined server/QEMU/J records.

- [ ] **Step 5: Seal the correctness directory**

Any failure blocks timing and Figure 2 population while retaining the failed
artifact.

## Task 5: Run and Seal the 125 Boots

**Files:**
- Produce: `artifact/slugarch_small_tiles/20260726/timed/`

- [ ] **Step 1: Create and audit the draft manifest**

The manifest must contain 125 cells, all absolute binary paths, all hashes,
fixed phase sizes, and the deterministic order.

- [ ] **Step 2: Arm the manifest**

After arming, `git diff`, source hashes, policy hashes, and binary hashes must
remain unchanged through the campaign.

- [ ] **Step 3: Execute in declared order**

Run one cell at a time. Report progress after each completed cell and at least
once per 60 seconds. An invalid attempt remains preserved; a replacement uses
`retry_of`.

- [ ] **Step 4: Validate immediately**

A matrix cell is eligible only with five valid fresh boots. No within-boot
sample may replace a missing boot.

- [ ] **Step 5: Apply the calibration gate**

One-tile off-backend dependent-load medians must satisfy:

```text
median(80 ns) < median(400 ns) < median(2000 ns) < median(10000 ns)
```

Failure blocks timing claims but not already valid correctness claims.

- [ ] **Step 6: Seal the timed campaign**

Write complete `SHA256SUMS`, outcome, validator hash, manifest hash, and
normalized-source hash.

## Task 6: Aggregate the Four RQs

**Files:**
- Create: `targets/qemu-type2/tiles/aggregate_campaign.py`
- Create: `targets/qemu-type2/tiles/tests/test_aggregate.py`
- Produce: `docs/evaluation/slugarch-small-tiles-20260726.json`
- Produce: `docs/evaluation/slugarch-small-tiles-20260726.md`

- [ ] **Step 1: Write aggregation tests**

Require median/min/max across exactly five boots, within-boot p95/p99 only from
measured events, metadata bytes per operation, extension-added time against
matching off cell, detection/localization rates, suspect-set size, and
Rust/RTL mismatch count.

- [ ] **Step 2: Implement eligibility-first aggregation**

The aggregator accepts only a sealed campaign whose checksums and validator
version match. It never silently drops a boot or substitutes another cell.

- [ ] **Step 3: Generate the normalized snapshot**

Every plotted datum includes five boot UUIDs and raw JSON pointers. Claims use
the statuses `qemu_artifact_backed`, `event_level_model`,
`software_artifact_backed`, `verilator_rtl`, `blocked`, or `not_evaluated`.

- [ ] **Step 4: Validate and commit**

```bash
python3 targets/qemu-type2/tiles/aggregate_campaign.py \
  --campaign artifact/slugarch_small_tiles/20260726/timed \
  --output docs/evaluation/slugarch-small-tiles-20260726.json \
  --report docs/evaluation/slugarch-small-tiles-20260726.md
git add targets/qemu-type2/tiles \
  docs/evaluation/slugarch-small-tiles-20260726.json \
  docs/evaluation/slugarch-small-tiles-20260726.md
git commit -m "eval: summarize small CXL tile evidence"
```

Expected: snapshot checksum verifies and all numeric fields trace to five
boots.

## Task 7: Render the Two Figures

**Files:**
- Create in isolated paper: `scripts/plot_tile_architecture.py`
- Create: `targets/qemu-type2/tiles/plot_results.py`
- Create: `targets/qemu-type2/tiles/tests/test_plot.py`
- Create in isolated paper: `img/slugarch-tile-architecture.pdf`
- Create in isolated paper: `img/slugarch-tile-results.pdf`

- [ ] **Step 1: Draw Figure 1**

Show host/home agent, host memory, CXL switch, four representative Type-2
tiles, one local HJ per tile, optional Type-3 memory, coordinator, and log
join. Use a legend for proposed physical, implemented QEMU, event-level model,
and blocked paths.

- [ ] **Step 2: Draw Figure 2 from normalized JSON**

Panels are latency calibration, recorder scaling, metadata by record mode, and
fault localization/backend equivalence. Use median with min/max whiskers and
separate axes for unlike units.

- [ ] **Step 3: Enforce plotting provenance**

The plotter verifies snapshot SHA-256, campaign SHA-256, five UUIDs per cell,
zero eligible drops, and monotonic calibration before importing Matplotlib.

- [ ] **Step 4: Check dimensions and fonts**

```bash
pdfinfo img/slugarch-tile-architecture.pdf
pdfinfo img/slugarch-tile-results.pdf
pdffonts img/slugarch-tile-architecture.pdf
pdffonts img/slugarch-tile-results.pdf
```

Expected: both fit double-column width and all printed text is at least 7 pt.

## Task 8: Rewrite and Shrink the Paper

**Files:**
- Modify isolated paper: `main.tex`
- Modify isolated paper: `intro.tex`
- Modify isolated paper: `background.tex`
- Modify isolated paper: `design.tex`
- Modify isolated paper: `semantics.tex`
- Modify isolated paper: `eval.tex`
- Modify isolated paper: `related.tex`
- Modify isolated paper: `cite.bib`
- Create isolated paper: `scripts/check_paper_contract.py`

- [ ] **Step 1: Create an isolated paper clone**

Capture original status and commit. Copy only source-controlled inputs and the
two verified figure PDFs into the isolated clone.

- [ ] **Step 2: Apply title and naming**

Use `SlugArch: Replayable Small CXL Tiles for Composable Accelerators`. Remove
the `\cfr` macro and all uses of CFR as a system name.

- [ ] **Step 3: Rewrite the narrative**

Tell the five-step opaque-endpoint, composable-tile, distributed-causality,
per-tile-HJ, evidence-backed evaluation story. State board-level endpoints and
host-home-agent coherence explicitly.

- [ ] **Step 4: Add LegoOS accurately**

Add the exact approved BibTeX entry and cite LegoOS only for independently
managed resource components. Do not state that LegoOS used CXL or replay.

- [ ] **Step 5: Replace evaluation content**

Use exactly RQ1 correctness, RQ2 scaling, RQ3 localization, and RQ4 backend
portability. Include Figure 2 and no quantitative result table.

- [ ] **Step 6: Compress semantics and limitations**

Keep one replay property and compact proof sketch. Explicitly mark physical
CXL.cache, peer coherence, switch ordering, matched FPGA fit, small-tile
power/area/yield/performance, and NVIDIA performance as blocked or not
evaluated.

- [ ] **Step 7: Enforce the paper contract**

The checker rejects:

```text
\cfr or standalone CFR system naming
more or fewer than two figure environments
any quantitative table environment in eval.tex
five-RQ wording
physical CXL.cache claims
NVIDIA performance comparisons
reference section beginning after page 12
main text longer than 11 pages
```

- [ ] **Step 8: Build and inspect**

```bash
latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
python3 scripts/check_paper_contract.py main.pdf
pdfinfo main.pdf
pdftotext -layout main.pdf /tmp/slugarch-small-tiles-paper.txt
```

Expected: clean build, main text ends by page 11, references begin after a
forced page boundary, exactly two figures, no quantitative result table, and
all plotted values match the normalized snapshot.

- [ ] **Step 9: Commit the isolated paper**

Commit source, bibliography, checker, data snapshot, and figures. Do not add
LaTeX auxiliary files.

## Task 9: Final Claim Audit

**Files:**
- Verify all source and evidence outputs.

- [ ] **Step 1: Trace every number**

For every paper number, record:

```text
paper sentence or figure datum
normalized JSON key
campaign cell
five boot UUIDs
raw files
SHA256SUMS entry
```

- [ ] **Step 2: Re-run all checks from clean builds**

Run Rust, Verilator, QEMU, Python, plotting, and LaTeX gates without using
stale build products.

- [ ] **Step 3: Report blocked boundaries honestly**

Do not convert missing physical FPGA, GPU, CXL.cache, switch, or peer-coherence
evidence into a positive result.
