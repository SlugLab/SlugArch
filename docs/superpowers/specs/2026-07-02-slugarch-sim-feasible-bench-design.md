# SlugArch Simulator-Feasible Benchmark Pass Design

## Goal

Measure the remaining Slug Architecture evaluation claims that can be exercised
with the current SlugArch and CXLMemSim simulator substrate, and turn every
unsupported claim into an explicit evidence-backed limitation instead of a
paper result.

## Approved Scope

This pass follows the simulator-feasible-first path. It extends the validated
QEMU Type-2 BAR2 evidence with measurements that do not require new hardware,
new CXL protocol support, or FPGA synthesis:

- CXL.mem/DAX reachability and streaming behavior if a Type-3 or DAX path is
  live in the current CXLMemSim setup.
- BAR2 command-path overhead for the already validated Type-2 SlugArch GEMM
  path.
- SlugArch replay metadata size, compression-mode accounting, replay
  validation latency, and provenance-label coverage using the committed GEMM
  trace and software replay recorder.
- A claim ledger for CXL.cache, DMA, ATS, migration, switch ordering, and FPGA
  resource cost that records whether each item is measured, partially
  measured, or blocked by missing substrate.

The pass must not imply that BAR2 command traffic proves CXL.cache coherence,
CXL.mem pooling, DMA, ATS, page migration, switch ordering, or FPGA feasibility.
Measured paper wording must stay tied to the path that actually produced the
data.

## Current Baseline

The branch starts from SlugArch commit `b478055`, which already contains:

- `targets/qemu-type2/run_existing_guest.sh`, the existing guest-visible Type-2
  BAR2 harness.
- `docs/evaluation/qemu-type2-repeatability-20260702.*`, five live Type-2 BAR2
  repetitions with 49 request FLITs, 49 response FLITs, zero tag mismatches,
  zero dispatch failures, and stable response bytes.
- `docs/evaluation/qemu-type2-failstop-20260702.*`, offline malformed-stream
  fail-stop validation cases.
- `crates/slugarch-host/src/replay.rs`, a software replay recorder with
  validation, delta, and full payload capture modes plus provenance labels for
  the 4x4 GEMM stream.
- `crates/slugcxl-gen/src/hj_overhead.rs` and
  `targets/agilex-vr2/generated/slugcxl_hj_overhead.json`, which provide a
  model-side Hardware-JIT metadata estimate but not a post-fit FPGA resource
  measurement.

The baseline verification for this worktree is:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test qemu_type2_artifacts
cargo test -p slugcxl-gen
```

The first command passed in the sandbox. The second command passed when rerun
with filesystem access to `/root/.cargo`, because the sandbox could not unpack
the `console` crate into Cargo's registry cache.

## Measurement Lanes

### Lane 1: CXL.mem / DAX Feasibility

Use existing CXLMemSim Type-3/DAX tools instead of inventing a new memory
benchmark. The first usable candidates are:

- `/home/victoryang00/CXLMemSim/qemu_integration/dax_stream_bench.c`
- `/home/victoryang00/CXLMemSim/qemu_integration/ssd_stream_two_qemu_bench.sh`
- `/home/victoryang00/CXLMemSim/qemu_integration/setup_cxl_numa.sh`
- `/home/victoryang00/CXLMemSim/qemu_integration/test_cxl_mem.c`

The benchmark runner should first record whether the live system exposes a DAX
or CXL memory path. Evidence includes guest `/dev/dax*` listing, `cxl list` if
available, `dmesg` excerpts relevant to CXL memory, and the exact CXLMemSim
server/QEMU command lines. If the path is live, run a bounded write/verify
streaming workload and record bytes, elapsed time, bandwidth, checksum/errors,
device path, access mode, and server counters/log excerpts. If the path is not
live, emit a blocked-result JSON with the checked commands and observed failure
strings.

This lane can support a narrow CXL.mem simulator claim only when the workload
actually touches the Type-3/DAX path. A BAR2-only run must remain a negative
boundary for CXL.mem.

### Lane 2: BAR2 Command-Path Overhead

Reuse the already working Type-2 BAR2 path and collect timing evidence without
changing its claim boundary. The runner should repeat the SlugArch GEMM request
stream through the existing guest helper, record guest elapsed time, host
wall-clock time around export, guest execution, copy-back, and validation, and
compare those with the in-process Verilator host path where available.

The output should report:

- request and response FLIT counts;
- request and response byte hashes;
- guest elapsed milliseconds;
- host wall-clock milliseconds for export, guest run, validation, and total;
- validation status, tag mismatches, dispatch failures, and decoded result;
- QEMU/CXLMemSim process and log evidence used for the run.

This lane measures the prototype command path overhead. It does not measure CXL
link latency, hardware endpoint latency, or continuous production overhead.

### Lane 3: Replay Metadata, Compression, and Replay Latency

Use SlugArch's software replay recorder on the committed 4x4 GEMM workload in
all three record modes:

- validation mode: payload commitments only;
- delta mode: changed-byte accounting relative to a zero baseline;
- full mode: complete payload capture.

For each mode, record:

- record count and epoch count;
- application FLIT bytes;
- serialized replay record bytes;
- payload capture bytes;
- metadata bytes per application GiB;
- hash/delta/full payload record counts;
- compression ratio versus full mode;
- replay validation latency for equivalent artifacts;
- replay validation failure latency for one malformed artifact;
- provenance label count and uncovered record count.

This lane supports software replay metadata and provenance accounting for the
current SlugArch boundary-record model. It does not prove a hardware compression
engine, hardware replay engine, or fabric-wide provenance plane.

### Lane 4: Claim Ledger and Paper Boundary

Produce a machine-readable ledger under `docs/evaluation/` that classifies each
paper claim as `measured`, `partially_measured`, or `blocked`. Every entry must
include:

- claim name;
- status;
- evidence artifact path;
- measured substrate;
- paper-safe wording;
- limitation text;
- commands or files checked;
- missing substrate if blocked.

Expected initial classifications:

| Claim | Expected status for this pass |
| --- | --- |
| QEMU Type-2 BAR2 command/replay boundary | measured |
| CXL.mem/DAX simulator traffic | measured if DAX is live, otherwise blocked |
| CXL.cache coherence | blocked unless CXLMemSim GPU coherency stats are exercised live |
| DMA | blocked unless a real DMA path is exercised and logged |
| ATS | blocked unless a simulator or kernel ATS event path is exposed |
| Page migration | blocked unless a migration event source is exercised and logged |
| Switch ordering | blocked unless two-host or switch-lock workload runs live |
| Runtime overhead | partially measured for BAR2 command path only |
| Compression | measured for software replay artifact modes only |
| Replay latency | measured for software replay validation only |
| Provenance | measured for software labels on the GEMM trace only |
| FPGA resource cost | blocked for post-fit resources; estimator output may be cited only as a model-side estimate |

## Artifacts

Create one top-level artifact directory for this pass:

```text
artifact/slugarch_cxlmemsim/sim-feasible-20260702-<time>/
  manifest.json
  bar2-overhead/
  cxlmem-dax/
  replay-metadata/
  blocked-claims/
```

Summaries copied into `docs/evaluation/` should be small and stable enough to
review in git:

- `docs/evaluation/sim-feasible-bench-20260702.json`
- `docs/evaluation/sim-feasible-bench-20260702.md`
- optional narrow per-lane JSON summaries if a lane produces enough detail to
  deserve its own file.

Raw logs and binary artifacts should stay under `artifact/` unless the paper
needs a compact, human-readable summary.

## Error Handling and Boundaries

The runner must prefer explicit blocked artifacts over silent skips. A blocked
artifact is successful evidence if it proves that the current substrate cannot
exercise a claim. It must include the command attempted, exit code, relevant
stderr/stdout excerpt, and the missing file/device/process.

For live QEMU/CXLMemSim commands, the runner must not leave background QEMU or
server processes running. If it starts a process, it records the PID, log path,
and cleanup status. If it attaches to an existing guest, it records that fact
and does not claim ownership of the VM lifecycle.

## Paper Update Rules

The paper should gain a short simulator-feasible pass subsection or table only
after the artifacts exist. That update must:

- promote measured BAR2, replay-metadata, and live CXL.mem/DAX results only
  when the artifacts support them;
- label software-only replay metadata, compression, provenance, and replay
  latency as software-boundary results;
- keep blocked claims visible as limitations, not hidden omissions;
- preserve the earlier statement that the pass replaces FPGA hardware-JIT
  evaluation with the QEMU/CXLMemSim simulator path for now.

## Verification

Before claiming this pass complete, run:

```bash
cargo fmt --check --package slugarch-host --package slugarch-cli --package slugcxl-gen
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test qemu_type2_artifacts
cargo test -p slugcxl-gen
jq empty docs/evaluation/sim-feasible-bench-20260702.json
```

If paper text is edited, also run `latexmk -pdf -interaction=nonstopmode
main.tex` in `/root/Concordia/64fa450c44d0cdf46c7c3a7d` and check the rendered
PDF text for the new measurement subsection.
