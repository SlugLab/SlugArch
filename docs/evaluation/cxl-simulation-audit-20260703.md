# SlugArch CXL Simulation Audit, 2026-07-03

## Scope

This pass reran the simulator-feasible CXL lanes available in the SlugArch
checkout and validated the saved CXLMemSim QEMU Type-2 BAR2 artifacts. It did
not launch a new live QEMU guest because the current shell has no
`CXLMEMSIM_*` guest variables, no visible `/dev/dax*` or `/dev/cxl*` devices,
and the expected CXLMemSim guest helper
`/home/victoryang00/CXLMemSim/qemu_integration/slugarch_type2_guest.c` is not
present.

Raw generated outputs for this pass are under:

- `/tmp/slugarch-cxl-sim-20260703-023237`

## Passed Simulation Lanes

| Lane | Result | Evidence |
| --- | --- | --- |
| CXL FLIT wire encoding/decoding | Passed | `cargo test -p slugarch-cxl-wire`: 12 unit tests and 1 proptest passed. |
| Verilated `slugcxl_4x4` endpoint smoke | Passed | `cargo test -p slugarch-verilator --test slugcxl_smoke`: endpoint reset, bogus FLIT dispatch failure response, 1 test passed. |
| Host GEMM CXL e2e | Passed | `cargo test -p slugarch-host --test gemm_cxl_e2e`: expected `I * B = B`, 49 request FLITs, 49 response FLITs, 1 test passed. |
| CLI CXL runtime | Passed | `slugarch run-cxl targets/qemu-type2/identity_times_const.json`: 212 cycles, 49 sent FLITs, 49 received FLITs, expected 4x4 result. |
| QEMU Type-2 request export | Passed | `slugarch export-cxlmemsim`: 49 request FLITs, 64 bytes per FLIT. |
| Saved QEMU Type-2 BAR2 response validation | Passed | Five saved runs validated: 5/5 pass, 245 requests, 245 responses, 0 tag mismatches, 0 dispatch failures. |
| QEMU Type-2 artifact regression tests | Passed | `cargo test -p slugarch-host --test qemu_type2_artifacts`: 9 tests passed, including malformed-stream negative cases. |
| Simulator-feasible claim ledger tests | Passed | `cargo test -p slugarch-host --test sim_feasible`: 9 tests passed. |
| SlugCXL generator snapshots | Passed | `cargo test -p slugcxl-gen`: 11 tests passed. |

## Replay Metadata Measurements

All modes replayed equivalently with zero record mismatches. Measurements are
for software replay metadata over the 4x4 GEMM trace, not for CXL link latency
or hardware replay execution.

| Mode | Records | Epochs | App FLIT bytes | Replay bytes | Payload bytes | Compression vs full | Equivalent validation ns | Mismatch validation ns |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| validation | 98 | 4 | 6272 | 9732 | 392 | 4.00 | 11995 | 7754 |
| delta | 98 | 4 | 6272 | 10284 | 552 | 2.84 | 15922 | 9051 |
| full | 98 | 4 | 6272 | 11300 | 1568 | 1.00 | 12816 | 8169 |

Provenance labels covered all records in each replay mode:
`gemm.load_a=32`, `gemm.load_b=32`, `gemm.compute=2`, and `gemm.readback=32`;
`uncovered_records=0`.

## QEMU Type-2 BAR2 Evidence

The saved repeatability artifact at
`artifact/slugarch_cxlmemsim/qemu-type2-repeatability-20260702-0627` remains
the strongest simulator-backed CXL evidence in this checkout.

| Metric | Value |
| --- | ---: |
| Runs | 5 |
| Passing runs | 5 |
| Total requests | 245 |
| Total responses | 245 |
| Tag mismatches | 0 |
| Dispatch failures | 0 |
| Guest elapsed ms | 8, 6, 6, 6, 6 |

Boundary: this is Type-2 BAR2 command-path evidence. It is not CXL.mem
bandwidth, CXL.cache coherence, DMA, ATS, page migration, switch ordering, or
hardware endpoint latency evidence.

## Blocked CXL Claims

| Claim | Status | Blocking condition |
| --- | --- | --- |
| CXL.mem/DAX simulator traffic | Blocked | No `/dev/dax*` device was visible under `/dev`; no streaming CXL.mem workload artifact was produced in this pass. |
| CXL.cache coherence | Blocked | The BAR2 command stream and software replay metadata do not expose CXL.cache transaction evidence. |
| DMA | Blocked | No real DMA path is exercised or logged by this pass. |
| ATS | Blocked | No simulator or kernel ATS event path is exposed by this pass. |
| Page migration | Blocked | No migration event source is exercised or logged by this pass. |
| Switch ordering | Blocked | No two-host or switch-lock workload is run by this pass. |
| FPGA resource cost | Blocked | No Quartus synthesis, fit, timing, or resource report is produced by this simulator pass. |

## Paper-Safe Wording

The current results support this wording:

> In the simulator-feasible evaluation, SlugArch executes and validates a
> 4x4 GEMM command stream through the CXLMemSim QEMU Type-2 BAR2 path across
> five saved guest runs, with 245/245 responses validated, zero tag
> mismatches, and zero dispatch failures. The same workload also validates
> through the local Verilated endpoint and software replay modes.

The current results do not support claims that SlugArch has measured
CXL.cache, CXL.mem streaming, DMA, ATS, migration, switch ordering, hardware
runtime overhead, or FPGA post-fit resource cost.
