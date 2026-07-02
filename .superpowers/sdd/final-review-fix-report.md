# Final Review Fix Report

Date: 2026-07-02
Branch: `slugarch-sim-feasible-bench`
Reviewer findings source: `.superpowers/sdd/final-review-findings.md`

## Scope

Addressed the four final findings in the owned sim-feasible benchmark surface:

- `crates/slugarch-host/src/sim_feasible.rs`
- `crates/slugarch-host/tests/sim_feasible.rs`
- `docs/evaluation/sim-feasible-bench-20260702.json`
- `docs/evaluation/sim-feasible-bench-20260702.md`
- removed duplicated compact summaries under `artifact/slugarch_cxlmemsim/sim-feasible-20260702-1747/`

## TDD Record

### Added failing tests first

Added coverage for:

1. runtime overhead blocked without BAR2 evidence
2. dynamic BAR2 `run-*` discovery catching an extra failing `run-6`
3. Markdown summary and claim ledger including the BAR2 evidence path

### Focused red run

Command:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include cargo test -p slugarch-host --test sim_feasible
```

Observed expected failures before code changes:

- `runtime_overhead_is_blocked_without_bar2_evidence`
  - actual `PartiallyMeasured`
  - expected `Blocked`
- `bar2_repeatability_discovers_all_run_directories_and_blocks_on_extra_failure`
  - actual `Measured`
  - expected `Blocked`
- `markdown_includes_bar2_evidence_source_and_claim_artifact_column`
  - missing `BAR2 evidence source` line and claim-ledger artifact column

### Minimal fixes applied

- BAR2 repeatability now enumerates every `run-*` subdirectory containing `summary.json` and sorts them by numeric suffix before aggregation.
- Runtime-overhead claim status now follows BAR2 evidence:
  - `PartiallyMeasured` only when BAR2 is `Measured`
  - `Blocked` when BAR2 is `Blocked`
- Markdown summary now emits `BAR2 evidence source`.
- Markdown claim ledger now includes an `Evidence artifact` column.
- Regenerated `docs/evaluation/sim-feasible-bench-20260702.{json,md}` from the existing repeatability artifact:
  - job: `targets/qemu-type2/identity_times_const.json`
  - BAR2 artifact: `artifact/slugarch_cxlmemsim/qemu-type2-repeatability-20260702-0627`
- Removed duplicate compact summaries from `artifact/slugarch_cxlmemsim/sim-feasible-20260702-1747/`.

### Focused green run

Re-ran:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include cargo test -p slugarch-host --test sim_feasible
```

Result: `9 passed; 0 failed`

## Generated docs state

- `docs/evaluation/sim-feasible-bench-20260702.md` now includes:
  - `BAR2 evidence source: artifact/slugarch_cxlmemsim/qemu-type2-repeatability-20260702-0627`
  - claim-ledger `Evidence artifact` column
- `docs/evaluation/sim-feasible-bench-20260702.json` preserves the BAR2 `source_dir` and claim `evidence_artifact` fields with the regenerated content.

## Verification

Executed after fixes:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include cargo test -p slugarch-host --test sim_feasible
cargo fmt --check --package slugarch-host --package slugarch-cli --package slugcxl-gen
jq empty docs/evaluation/sim-feasible-bench-20260702.json
```

Notes:

- `cargo fmt --check` initially reported one formatting diff in `crates/slugarch-host/tests/sim_feasible.rs`; ran `cargo fmt --package slugarch-host --package slugarch-cli --package slugcxl-gen` and re-ran the required `--check` command successfully.

## Result

All four findings are addressed, the compact summaries live only under `docs/evaluation/`, and the required verification commands pass on the branch.
