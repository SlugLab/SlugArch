# SlugArch Type-2 CXL.mem Evaluation Master Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and validate a server-authoritative QEMU Type-2 CXL.mem path, collect the predeclared four-latency/five-boot SlugArch campaign, and integrate two evidence-bounded figures into an IEEE paper whose main text ends by page 11.

**Architecture:** Execute three independently testable plans in dependency order. The transport plan repairs CXLMemSim and QEMU and ends with a live sentinel proof; the campaign plan builds the guest benchmark, immutable evidence pipeline, and validated normalized dataset; the paper plan can build its empty-axis layout and legacy Figure 1 early, but may populate Figure 2 or make new quantitative claims only from the first eligible checksum-valid campaign.

**Tech Stack:** C++20 and C for CXLMemSim/QEMU, QEMU Meson/qtests/QAPI, Rust 2021, standard-library Python 3, Linux CXL/devdax, QEMU TCG, Matplotlib, IEEEtran LaTeX, BibTeX, Git, SHA-256.

---

## Plan Suite and Ownership

The approved design is
`docs/superpowers/specs/2026-07-24-slugarch-type2-cxlmem-experiment-design.md`.
Implement it through these standalone plans:

1. `docs/superpowers/plans/2026-07-24-slugarch-type2-transport.md`
   owns the versioned wire protocol, server-authoritative memory, QEMU Type-2
   CFMWS dispatch, modeled delay, counters, QMP observability, and live
   transport proof.
2. `docs/superpowers/plans/2026-07-24-slugarch-type2-campaign.md`
   owns shared SlugArch replay semantics, the guest devdax benchmark,
   campaign registration and sealing, orchestration, validation,
   normalization, and the reviewed Figure 2 data snapshot.
3. `docs/superpowers/plans/2026-07-24-slugarch-paper-integration.md`
   owns the CFR-to-SlugArch rename, IEEE build repair, Figure 1 and Figure 2
   renderers, evidence-bounded evaluation rewrite, and 11-page gate.

The source checkouts remain distinct:

- control and SlugArch code: `/root/Concordia/SlugArch`;
- CXLMemSim and its QEMU submodule:
  `/home/victoryang00/CXLMemSim`;
- paper source: `/root/Concordia/64fa450c44d0cdf46c7c3a7d`;
- isolated implementation worktrees and paper clone: `/tmp`.

Never stage the existing July artifact directories or the untracked July 15
plan while executing this suite.

## Global Stop Conditions

Stop the quantitative campaign and retain failed evidence if any of these
conditions holds:

- a guest devdax access does not produce a matching server transaction;
- the DPA work window overlaps the BAR4 bulk or coherent-pool bypass;
- protocol, byte, operation, phase, or delay-event joins disagree;
- the durable arm/commit boundary cannot be reconstructed after a crash;
- any committed attempt fails after arming;
- the first eligible complete campaign fails a calibration or Figure 2 gate;
- a normalized export does not verify against its campaign and registry
  hashes; or
- the paper cannot fit the two 7.1-by-3.0-inch figures and main text by page
  11 without changing IEEE geometry or reducing figure text below 7 points.

Do not replace, estimate, interpolate, or borrow a number to clear a failed
gate.

### Task 1: Freeze Inputs and Establish Isolated Work Areas

**Files:**
- Inspect: `/root/Concordia/SlugArch`
- Inspect: `/home/victoryang00/CXLMemSim`
- Inspect: `/home/victoryang00/CXLMemSim/lib/qemu`
- Inspect: `/root/Concordia/64fa450c44d0cdf46c7c3a7d`
- Create during execution: `/tmp/slugarch-type2-transport-src`
- Create during execution: `/tmp/slugarch-type2-campaign-src`
- Create during execution: `/tmp/slugarch-paper-integration`

- [ ] **Step 1: Invoke the isolation skill**

Read and follow `superpowers:using-git-worktrees` before creating either
implementation area. The dirty CXLMemSim parent and QEMU submodule require
separate recorded source snapshots; the dirty paper checkout requires an
isolated local clone.

- [ ] **Step 2: Record all source identities and dirt**

Run:

```bash
git -C /root/Concordia/SlugArch status --short --branch
git -C /root/Concordia/SlugArch rev-parse HEAD
git -C /home/victoryang00/CXLMemSim status --short --branch
git -C /home/victoryang00/CXLMemSim rev-parse HEAD
git -C /home/victoryang00/CXLMemSim/lib/qemu status --short --branch
git -C /home/victoryang00/CXLMemSim/lib/qemu rev-parse HEAD
git -C /root/Concordia/64fa450c44d0cdf46c7c3a7d status --short --branch
git -C /root/Concordia/64fa450c44d0cdf46c7c3a7d rev-parse HEAD
sha256sum /root/Concordia/sigmetrics27summer-paper399.pdf
```

Expected: all commands succeed; the statuses and commit IDs are saved in the
campaign source snapshot rather than cleaned or reset. The reference PDF hash
is
`d6a90335b27188e2623f4cda2ee4da2639715cb15f0dc2f6a3da80c96ecd5e8f`.

- [ ] **Step 3: Verify the approved design and plan suite before coding**

Run:

```bash
git -C /root/Concordia/SlugArch diff --check
rg -n '^## Implementation and Verification Gates' \
  /root/Concordia/SlugArch/docs/superpowers/specs/2026-07-24-slugarch-type2-cxlmem-experiment-design.md
rg -n '^# .* Implementation Plan' \
  /root/Concordia/SlugArch/docs/superpowers/plans/2026-07-24-slugarch-type2-*.md \
  /root/Concordia/SlugArch/docs/superpowers/plans/2026-07-24-slugarch-paper-integration.md
```

Expected: no whitespace errors, one implementation-gate section, and four
plan headers including this master plan.

### Task 2: Complete the Type-2 Transport Plan

**Files:**
- Follow:
  `docs/superpowers/plans/2026-07-24-slugarch-type2-transport.md`
- Produce: isolated CXLMemSim and QEMU commits under
  `/tmp/slugarch-type2-transport-src`
- Produce: protocol and live-smoke artifacts under `/tmp`

- [ ] **Step 1: Execute the transport plan through unit and qtest gates**

Complete every checkbox through the targeted server and QEMU test tasks.

Expected minimum evidence:

```text
slugarch_type2_protocol tests: PASS
qemu test-cxl-type2-wire: PASS
qemu cxl-test Type-2 cases: PASS
```

- [ ] **Step 2: Execute the no-guest protocol smoke**

Use the exact oracle and QMP commands in the transport plan.

Expected: HELLO/ACK v1 succeeds, legacy-layout input is rejected, one write and
one read return matching bytes, the client counters are nonzero, and every
server response has exactly one joined QEMU delay event.

- [ ] **Step 3: Execute the guest enumeration and sentinel smoke**

Boot the recorded kernel and private disk overlay under TCG with one 256 MiB
Type-2 target.

Expected: the guest exposes `mem0`, a committed region, and devdax; the DPA
`[80 MiB,112 MiB)` work window is outside both bypasses; oracle-to-guest and
guest-to-oracle sentinels match; local/bypass completion counters remain zero.

- [ ] **Step 4: Freeze the transport source and binary hashes**

Run the transport plan's manifest command and save:

```text
CXLMemSim commit and in-scope patch hash
QEMU commit and in-scope patch hash
cxlmemsim_server SHA-256
qemu-system-x86_64 SHA-256
kernel SHA-256
base image SHA-256
```

Do not begin a timed pilot if any value is missing.

### Task 3: Complete Offline Campaign Software and Paper Mockups

**Files:**
- Follow:
  `docs/superpowers/plans/2026-07-24-slugarch-type2-campaign.md`
- Follow through mockup gate:
  `docs/superpowers/plans/2026-07-24-slugarch-paper-integration.md`

- [ ] **Step 1: Implement and test the shared replay core and guest binary**

Run:

```bash
cargo test -p slugarch-cxl-replay
cargo test -p slugarch-type2-guest
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test qemu_type2_artifacts
```

Expected: all tests pass, the 1x trace reports 49 request records and 98
boundary records, and every scaled copy has a unique tag namespace.

- [ ] **Step 2: Implement and test the host evidence pipeline**

Run:

```bash
PYTHONPATH=targets/qemu-type2 \
  python3 -m unittest discover -s targets/qemu-type2/tests -v
```

Expected: the valid synthetic 20-slot campaign passes; tampered registry,
checksum, pairing, byte-count, delay-join, bypass, and outcome-selection
fixtures fail with their exact declared reason.

- [ ] **Step 3: Build the static guest executable and prove portability**

Run:

```bash
RUSTFLAGS="-C target-feature=+crt-static" \
  cargo build --release -p slugarch-type2-guest
file target/release/slugarch-type2-guest
ldd target/release/slugarch-type2-guest
```

Expected: `file` reports an x86-64 statically linked executable and `ldd`
reports that it is not dynamically linked.

- [ ] **Step 4: Build the empty-axis two-figure paper mockup**

Follow the paper plan through the layout task using only labeled mock data
under `/tmp`; never create a paper data JSON containing mock values.

Expected: Figure 1 and the empty-axis Figure 2 are each 7.1 by 3.0 inches,
their printed text is at least 7 points, captions are at most 90 words, the
evaluation occupies at most three pages, and the planned main-text allocation
sums to exactly 11.0 pages.

### Task 4: Run and Seal the 400 ns Pilot

**Files:**
- Create through the campaign harness:
  `artifact/slugarch_type2_cxlmem/pilots/pilot-400ns-20260724/`
- Never edit a sealed directory.

- [ ] **Step 1: Reverify frozen transport and guest hashes**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py check \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json
```

Expected: the command records the pre-pilot frozen full-campaign contract hash,
and no source or binary hash changed after Task 2. Pilot ID, campaign ID,
runtime UUIDs, and timestamps are invocation metadata outside that contract
hash.

- [ ] **Step 2: Run one complete pilot boot**

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

Expected: one fresh server, disk overlay, QEMU process, and guest boot; durable
arming precedes the only warmup; the only timed pass contains calibration,
three transfer sizes, sixteen SlugArch conditions, and one post-transport
negative test.

- [ ] **Step 3: Validate every pilot join**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py validate \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --pilot-id pilot-400ns-20260724
```

Expected: topology, sentinel, phase, operation, byte, checksum, delay, pairing,
bypass-zero, and corruption gates all pass. The validator reports that this
pilot is valid but categorically ineligible for paper normalization.

- [ ] **Step 4: Freeze the full-campaign experiment version**

Create a new registered campaign only after the pilot passes and without
changing source, binaries, commands, matrix, ordering, or validation rules.

Expected: both the successful pilot manifest and the full campaign
registration reference the same byte-for-byte frozen full-campaign contract
hash; their separate invocation/artifact hashes remain different.

### Task 5: Run the Predeclared Twenty-Slot Campaign

**Files:**
- Produce:
  `artifact/slugarch_type2_cxlmem/slugarch-type2-cxlmem-v1-20260724/`
- Produce after validation:
  `artifact/slugarch_type2_cxlmem/exports/` followed by
  `slugarch-type2-cxlmem-v1-20260724-` and the verified campaign checksum.

- [ ] **Step 1: Register and run the full campaign**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py register-run \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --campaign-id slugarch-type2-cxlmem-v1-20260724 \
  --change-reason initial-approved-type2-cxlmem-campaign
```

Expected: five blocks follow the approved latency rotation, with primary
attempts completed before any permitted pre-arm retry. The first durable arm
commits a slot; any later failure seals the campaign as failed.

- [ ] **Step 2: Reconcile and inspect terminal state**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py inspect \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --campaign-id slugarch-type2-cxlmem-v1-20260724
```

Expected: the registry hash chain and ordinals verify, no unresolved
`.inprogress` campaign remains, and the fixed campaign is reported as the
lowest registration ordinal with a checksum-valid complete directory without
using result values. `register-run` performs reconciliation while holding the
campaign lock; there is no separate mutating reconciliation command.

- [ ] **Step 3: Validate the complete campaign**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py validate \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --campaign-id slugarch-type2-cxlmem-v1-20260724
```

Expected: exactly 20 committed complete attempts, exactly five boots per
latency, 20 corruption rejections, complete same-boot pairs, zero bypass
completions, and no missing or duplicate delay application.

- [ ] **Step 4: Normalize only the eligible complete campaign**

Run:

```bash
cd /tmp/slugarch-type2-campaign-src
PYTHONPATH=targets/qemu-type2 \
  python3 targets/qemu-type2/run_cxlmem_campaign.py normalize \
  --config targets/qemu-type2/cxlmem-campaign-defaults.json \
  --campaign-id slugarch-type2-cxlmem-v1-20260724
```

Expected: the export contains raw five-boot values, median/minimum/maximum,
same-boot ratios, calibration gates, request/response/boundary record counts,
limitations, registry history, raw relative paths, and a checksum manifest.

If validation or calibration fails, stop here and preserve the blocked result.

### Task 6: Populate Figures and Rewrite the Paper

**Files:**
- Follow:
  `docs/superpowers/plans/2026-07-24-slugarch-paper-integration.md`
- Produce in the isolated paper clone:
  `data/slugarch-results-20260704.json`
- Produce in the isolated paper clone:
  `data/slugarch-type2-cxlmem.json`
- Produce in the isolated paper clone:
  `img/slugarch-results.pdf`
- Produce in the isolated paper clone:
  `img/slugarch-type2-cxlmem.pdf`

- [ ] **Step 1: Copy the reviewed normalized snapshot by allowlist**

Use the exact source export and destination paths emitted by the campaign
normalizer.

Expected: the copied JSON hash equals the export-manifest hash, and the paper
contract verifies that it came from the eligible complete campaign.

- [ ] **Step 2: Render and visually inspect both figures**

Run:

```bash
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 -m unittest scripts/test_plot_slugarch_results.py \
  scripts/test_plot_slugarch_type2_cxlmem.py -v
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/plot_slugarch_results.py
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/plot_slugarch_type2_cxlmem.py
```

Expected: tests pass; each PDF is one 7.1-by-3.0-inch vector page; no mock,
reference-paper, estimated, or interpolated value appears.

- [ ] **Step 3: Perform the evidence-bounded manuscript rewrite**

Follow the paper plan's exact source edits and page allocation.

Expected: CFR is absent, Figure 1 states the legacy BAR2/software boundary,
Figure 2 states the QEMU/CXLMemSim CXL.mem boundary, CXL.cache remains
unmeasured, and all hardware-only claims remain blocked.

- [ ] **Step 4: Build and enforce the page gate**

Run:

```bash
latexmk -C main.tex
latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
python3 scripts/check_paper_contract.py
```

Expected: the build and contract checker pass, references are resolved, and
the `sec:maintext-end` label is on or before page 11.

### Task 7: Independent Review and Scoped Handoff

**Files:**
- Review all commits produced by the three subplans.
- Sync only the allowlists declared in those plans.

- [ ] **Step 1: Invoke the code-review skill**

Read and follow `superpowers:requesting-code-review`. The reviewer must inspect
the transport proof boundary, campaign selection state machine, normalized
numbers, figure provenance, paper claims, and page gate.

- [ ] **Step 2: Run the complete verification suite fresh**

Run every final verification command in all three subplans in one clean
session. Do not rely on earlier output.

Expected: zero unit, integration, campaign, plot, build, naming, provenance,
or page-gate failures.

- [ ] **Step 3: Confirm source destinations have not drifted**

Compare the current destination commit and tracked-file hashes with the
identities captured in Task 1.

Expected: either they match or the sync is stopped for explicit review; never
overwrite newly changed destination files.

- [ ] **Step 4: Sync only reviewed files and reverify**

Copy the transport patches, SlugArch commits, and paper allowlist using the
subplans' explicit commands. Re-run the targeted build/test suites in their
destination checkouts.

Expected: destination verification matches the isolated evidence, unrelated
dirty and untracked files are untouched, and the final handoff lists commit
IDs, artifact hashes, campaign ID, experiment-version hash, figure hashes,
paper PDF hash, and main-text end page.
