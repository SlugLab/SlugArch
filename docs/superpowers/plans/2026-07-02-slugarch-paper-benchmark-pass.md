# SlugArch Paper Benchmark Pass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Put a truthful first QEMU Type-2 benchmark result into the Slug Architecture paper while making remaining paper claims explicit benchmark slots or limitations.

**Architecture:** The branch stores a durable evidence bundle under `docs/evaluation/` and updates the external paper's `eval.tex` with one measured-results subsection. The measured result is scoped to the guest-visible CXLMemSim QEMU Type-2 BAR2 command path; broader CXL.cache, CXL.mem, DMA, ATS, migration, switch-ordering, overhead, provenance, portability, recovery, and FPGA claims remain unmeasured in this pass.

**Tech Stack:** Rust/Cargo tests for SlugArch artifact validation, LaTeX for manuscript edits, JSON/CSV/Markdown for durable evidence.

---

### Task 1: Add Durable Evidence Bundle

**Files:**
- Create: `docs/evaluation/qemu-type2-live-20260702-0028-summary.json`
- Create: `docs/evaluation/qemu-type2-live-20260702-0028-guest-summary.json`
- Create: `docs/evaluation/slugarch-paper-first-benchmark.md`

- [ ] **Step 1: Write the evidence files**

Create `docs/evaluation/qemu-type2-live-20260702-0028-summary.json` with:

```json
{
  "status": "pass",
  "workload": "slugcxl_gemm_4x4",
  "flit_bytes": 64,
  "request_count": 49,
  "response_count": 49,
  "request_bytes": 3136,
  "response_bytes": 3136,
  "tag_mismatches": 0,
  "dispatch_failures": 0,
  "result_c": [[2, 3, 4, 5], [6, 7, 8, 9], [10, 11, 12, 13], [14, 15, 16, 17]],
  "expected_c": [[2, 3, 4, 5], [6, 7, 8, 9], [10, 11, 12, 13], [14, 15, 16, 17]],
  "artifact_path": "/tmp/slugarch-cxlmemsim-type2/artifact/slugarch_cxlmemsim/qemu-type2-live-20260702-0028-summary",
  "raw_artifact_sha256": {
    "requests.bin": "f9f05b04d9352de8e0213c42e5efb46f56b05863e077d9cf1ce47a9ddef2b75c",
    "responses.bin": "0562b4dda7e4ec3076b407936b3179eb36fdbc755b92985911b547fb27a7e85c",
    "summary.json": "049c5cc19f7bdb4c450a0fe503ea0271a1d6b67d73b71085bebeefb4985ba9e8"
  },
  "note": "This checked-in file is an enriched evidence bundle. raw_artifact_sha256.summary.json is the hash of the original summary.json at artifact_path, not this enriched file.",
  "slugarch_head": "62fa01942d6cc0570e8195736e79332c3a9cda3d",
  "cxlmemsim_head": "5475fa44d09ce27b645ed77caa6cc6b47a38a8d4"
}
```

Create `docs/evaluation/qemu-type2-live-20260702-0028-guest-summary.json` with:

```json
{
  "status": "pass",
  "device": "0000:0d:00.0",
  "requests": 49,
  "responses": 49,
  "slug_submitted": 49,
  "slug_completed": 49,
  "slug_failed": 0,
  "elapsed_ms": 9,
  "command_failures": 0,
  "slug_loads": 32,
  "slug_computes": 1,
  "slug_reads": 16,
  "slug_bad_tags": 0
}
```

Create `docs/evaluation/slugarch-paper-first-benchmark.md` with:

```markdown
# SlugArch Paper First Benchmark Pass

## Measured Claim

The validated artifact supports the claim that the SlugArch host and CXLMemSim QEMU Type-2 BAR bridge can execute a complete guest-visible 4x4 GEMM command stream with one validated response per request.

## Evidence

- Artifact: `/tmp/slugarch-cxlmemsim-type2/artifact/slugarch_cxlmemsim/qemu-type2-live-20260702-0028-summary`
- Host validation: `status=pass`, workload `slugcxl_gemm_4x4`, 49 request FLITs, 49 response FLITs, `tag_mismatches=0`, `dispatch_failures=0`
- FLIT stream size: 64 bytes per FLIT, 3136 request bytes, 3136 response bytes
- Guest summary: device `0000:0d:00.0`, `slug_submitted=49`, `slug_completed=49`, `slug_failed=0`, `command_failures=0`, `elapsed_ms=9`
- Operation mix: `slug_loads=32`, `slug_computes=1`, `slug_reads=16`
- Result matrix equals expected matrix: `[[2,3,4,5],[6,7,8,9],[10,11,12,13],[14,15,16,17]]`
- Raw artifact SHA-256: `requests.bin=f9f05b04d9352de8e0213c42e5efb46f56b05863e077d9cf1ce47a9ddef2b75c`, `responses.bin=0562b4dda7e4ec3076b407936b3179eb36fdbc755b92985911b547fb27a7e85c`, `summary.json=049c5cc19f7bdb4c450a0fe503ea0271a1d6b67d73b71085bebeefb4985ba9e8`

## Claim Ledger

| Paper claim | First-pass status | Wording boundary |
| --- | --- | --- |
| Guest-visible Type-2 endpoint boundary can carry SlugArch command and response records | Measured | Claim only for the CXLMemSim QEMU Type-2 BAR prototype and 4x4 GEMM workload. |
| Replay/fail-stop validation detects malformed response streams | Unit-tested | The Rust artifact tests validate known-good and truncated response streams; the live run did not inject a fault. |
| CXL.cache, CXL.mem, DMA, ATS, migration, and switch-ordering replay | Unmeasured | The first result exercises BAR2 command/response traffic only. |
| Continuous overhead is low | Unmeasured | Keep as an evaluation question, not a result. |
| Log compression and policy modes improve cost/fidelity tradeoffs | Unmeasured | Keep as an evaluation question, not a result. |
| Fabric logs provide endpoint/protection-domain provenance | Unmeasured | Keep as an evaluation question, not a result. |
| Boundary contract is portable across CPUs, GPUs, DMA engines, memory devices, and switches | Unmeasured | Keep as an evaluation question, not a result. |
| FPGA or hardware JIT feasibility | Replaced in this pass | State that this pass evaluates the simulator-backed QEMU Type-2 path instead of FPGA hardware JIT. |
```

- [ ] **Step 2: Verify evidence files parse**

Run:

```bash
python3 -m json.tool docs/evaluation/qemu-type2-live-20260702-0028-summary.json >/tmp/slugarch-summary-check.json
python3 -m json.tool docs/evaluation/qemu-type2-live-20260702-0028-guest-summary.json >/tmp/slugarch-guest-summary-check.json
```

Expected: both commands exit with status 0.

### Task 2: Update Paper Evaluation Text

**Files:**
- Modify: `/root/Concordia/64fa450c44d0cdf46c7c3a7d/eval.tex`

- [ ] **Step 1: Replace methodology-only opening with result-aware wording**

Change the opening paragraph so it says the section contains the complete evaluation plan plus one first prototype result, and that the result is not evidence for unrun overhead, provenance, portability, recovery, or FPGA claims.

- [ ] **Step 2: Add first-results subsection**

Append this subsection after the ablations paragraph:

```latex
\subsection{First prototype result: QEMU Type-2 BAR path}

The first measured artifact exercises the simulator-backed path requested for
this work: SlugArch host code exports CXL-like request flits, a guest program
submits them through a CXLMemSim QEMU Type-2 BAR, and the host validates the
response stream against the expected 4x4 GEMM result. This path replaces the
earlier FPGA hardware-JIT evaluation target for the current artifact pass; it
tests whether the replay boundary can be driven through a full guest/QEMU
Type-2 device interface rather than through an in-process model.

\begin{table}[t]
\caption{First measured QEMU Type-2 BAR result.}
\label{tab:qemu-type2-first-result}
\footnotesize
\setlength{\tabcolsep}{4pt}
\begin{tabular}{@{}ll@{}}
\toprule
Metric & Result \\
\midrule
Prototype path & CXLMemSim QEMU Type-2 BAR \\
Guest-visible device & 0000:0d:00.0 \\
Workload & 4x4 GEMM, identity-times-constant matrix \\
Request flits & 49 \\
Response flits & 49 \\
Guest submitted/completed/failed & 49 / 49 / 0 \\
Operation mix & 32 loads, 1 compute, 16 reads \\
Tag mismatches & 0 \\
Dispatch failures & 0 \\
Command failures & 0 \\
Validated output & matches expected matrix \\
\bottomrule
\end{tabular}
\end{table}

The artifact therefore supports a narrow boundary result: a complete SlugArch
command stream can cross the guest-visible QEMU Type-2 BAR path and return
validated responses without tag or dispatch failures. It does not yet measure
runtime overhead, log bandwidth, compression ratio, replay latency, provenance
precision, cross-device portability, recovery behavior, or FPGA resource cost.
Those quantities remain the benchmark slots in Table~\ref{tab:evaluation-map}
and the measurement protocol above.
```

- [ ] **Step 3: Check for overclaiming**

Run:

```bash
rg -n "speedup|overhead.*[0-9]|compression ratio|provenance precision|FPGA.*measured|recovery.*measured" /root/Concordia/64fa450c44d0cdf46c7c3a7d/eval.tex
```

Expected: no measured numeric claims for unrun categories.

### Task 3: Verify Repo-Side Artifact Tests

**Files:**
- No edits.

- [ ] **Step 1: Run the focused Rust artifact tests**

Run:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test qemu_type2_artifacts
```

Expected: 3 tests pass.

- [ ] **Step 2: Run Rust formatting check for changed Rust packages**

Run:

```bash
cargo fmt --check --package slugarch-host --package slugarch-cli
```

Expected: exits with status 0.

### Task 4: Verify Paper Build or Report Toolchain Boundary

**Files:**
- No edits unless LaTeX emits a real syntax error in changed text.

- [ ] **Step 1: Check available LaTeX commands**

Run:

```bash
command -v latexmk
command -v pdflatex
```

Expected: at least one command path if the local LaTeX toolchain is installed.

- [ ] **Step 2: Build the paper when a tool is available**

Run from `/root/Concordia/64fa450c44d0cdf46c7c3a7d`:

```bash
latexmk -pdf -interaction=nonstopmode main.tex
```

If `latexmk` is unavailable but `pdflatex` exists, run:

```bash
pdflatex -interaction=nonstopmode main.tex
bibtex main
pdflatex -interaction=nonstopmode main.tex
pdflatex -interaction=nonstopmode main.tex
```

Expected: `main.pdf` is produced or the missing-tool boundary is reported.

### Task 5: Final Diff Review

**Files:**
- Review all changed files.

- [ ] **Step 1: Inspect changed files**

Run:

```bash
git status --short
git diff -- docs/evaluation docs/superpowers/specs/2026-07-02-slugarch-paper-benchmark-pass-design.md docs/superpowers/plans/2026-07-02-slugarch-paper-benchmark-pass.md
git -C /root/Concordia/64fa450c44d0cdf46c7c3a7d diff -- eval.tex
```

Expected: only the intended evidence, plan/spec, and paper evaluation edits are present.
