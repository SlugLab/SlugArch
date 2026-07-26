# SlugArch Replayable Small CXL Tiles Master Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the approved small-CXL-tile architecture, per-tile Hardware JIT, 125-boot evidence, two figures, references, and an 11-page SlugArch paper.

**Architecture:** Execute three independently testable child plans in dependency order. The model plan defines deterministic shared-line semantics and the six-fault oracle; the QEMU plan restores server-authoritative Type-2 CFMWS and gives every tile an isolated JIT instance; the evaluation/paper plan may arm timing only after both correctness gates pass and may render numeric results only from sealed normalized evidence.

**Tech Stack:** Rust 2021, C/C++20, QEMU, CXLMemSim, SystemVerilog, Verilator, optional Quartus, Python 3, Matplotlib, LaTeX, BibTeX, SHA-256, Git.

---

## Approved Contract and Child Plans

The implementation contract is:

`docs/superpowers/specs/2026-07-26-slugarch-small-cxl-tiles-paper-design.md`

Execute:

1. `docs/superpowers/plans/2026-07-26-slugarch-small-cxl-tiles-model.md`
2. `docs/superpowers/plans/2026-07-26-slugarch-small-cxl-tiles-qemu.md`
3. `docs/superpowers/plans/2026-07-26-slugarch-small-cxl-tiles-evaluation-paper.md`

The July 24 Type-2 and July 25 J-extension plans remain detailed dependency
contracts where the child plans explicitly reuse them. The July 26 design
supersedes their older 20/100-boot matrices, five-RQ paper structure, and
monolithic-accelerator narrative.

## Global Stop Conditions

Stop timing and paper-number integration if:

- direct DPA 80 MiB does not join server, QEMU, and J records;
- a write completes before server commit;
- tile or client identity is duplicated;
- an explicit backend falls back;
- a policy digest differs across participating tiles;
- Rust and Verilator differ on a supported record or first error;
- a required record drops;
- any of the six faults is missed or mislocalized;
- a cell lacks five valid fresh boots;
- the four off-backend latency medians are not strictly increasing;
- a normalized datum lacks five UUIDs and raw checksum provenance; or
- the 11-page/two-figure/no-quantitative-table contract fails.

## Task 1: Establish Isolated Execution Areas

**Files:**
- Follow the QEMU child plan Task 1.

- [ ] **Step 1: Use `superpowers:using-git-worktrees`**

Create isolated SlugArch, CXLMemSim, QEMU, and paper areas below `/tmp`.

- [ ] **Step 2: Capture all source identities and dirt**

Expected: original dirty repositories are untouched and reproducible patch
hashes are stored.

## Task 2: Implement and Gate the Event-Level Model

**Files:**
- Follow: `docs/superpowers/plans/2026-07-26-slugarch-small-cxl-tiles-model.md`

- [ ] **Step 1: Complete all RED/GREEN/commit tasks**

- [ ] **Step 2: Run the model acceptance gate**

Expected: four workloads pass at 1/2/4/8 tiles; all six faults return their
stable code and exact first tile/event; exports are deterministic.

## Task 3: Restore QEMU Transport and Hardware JIT

**Files:**
- Follow: `docs/superpowers/plans/2026-07-26-slugarch-small-cxl-tiles-qemu.md`

- [ ] **Step 1: Reconstruct direct CFMWS**

Expected: full CXL qtest passes and five fresh focused processes pass.

- [ ] **Step 2: Implement Rust and FPGA-Verilator backends**

Expected: supported cases are byte-exact with no fallback.

- [ ] **Step 3: Prove multi-tile isolation**

Expected: 1/2/4/8 devices enumerate with unique tile/client IDs and isolated
digest, epoch, counters, log, and first error.

## Task 4: Pass Correctness Before Timing

**Files:**
- Follow evaluation/paper child plan Tasks 1 through 4.

- [ ] **Step 1: Validate the exact 125-entry manifest**

- [ ] **Step 2: Pass all semantic, fault, transport, and evidence gates**

Expected: timing remains unarmed until all checks pass.

## Task 5: Run the 125 Independent Boots

**Files:**
- Follow evaluation/paper child plan Task 5.

- [ ] **Step 1: Arm once**

- [ ] **Step 2: Execute and validate each fresh boot**

- [ ] **Step 3: Apply monotonic calibration and seal**

Expected: 60 latency, 45 scale, and 20 record-mode boots are valid, with five
fresh boots per cell.

## Task 6: Generate Audited Results and Figures

**Files:**
- Follow evaluation/paper child plan Tasks 6 and 7.

- [ ] **Step 1: Aggregate only eligible evidence**

- [ ] **Step 2: Render architecture and four-panel results figures**

Expected: every number resolves to normalized JSON, five boot UUIDs, raw
files, and SHA-256 entries.

## Task 7: Rewrite and Validate the Paper

**Files:**
- Follow evaluation/paper child plan Tasks 8 and 9.

- [ ] **Step 1: Apply SlugArch-only small-tile narrative and LegoOS citation**

- [ ] **Step 2: Replace the old evaluation with four RQs**

- [ ] **Step 3: Build and pass the paper contract**

Expected: main text ends on or before page 11, exactly two figures remain, no
quantitative result table remains, and unsupported physical claims remain
blocked or not evaluated.
