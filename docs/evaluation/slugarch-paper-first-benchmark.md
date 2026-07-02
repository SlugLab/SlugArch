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
- Note: `qemu-type2-live-20260702-0028-summary.json` in this directory is an enriched evidence bundle; the raw `summary.json` hash above belongs to the original artifact directory.
- Source revisions: SlugArch `62fa01942d6cc0570e8195736e79332c3a9cda3d`, CXLMemSim `5475fa44d09ce27b645ed77caa6cc6b47a38a8d4`

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

## Pass-2 Addendum

The second benchmark pass adds two evidence bundles:

- `qemu-type2-repeatability-20260702`: five live QEMU Type-2 BAR2 guest runs in one booted guest. All five runs validated with 49 request FLITs, 49 response FLITs, zero tag mismatches, zero dispatch failures, and the expected 4x4 matrix. Guest elapsed times were 8, 6, 6, 6, and 6 ms.
- `qemu-type2-failstop-20260702`: offline malformed response-stream cases covering truncated bytes, bad tag, missing response, extra duplicate response, dispatch-failed opcode, wrong read data, and wrong response phase. These are validator/fault-stream tests, not live injected QEMU device faults.

The paper now cites both results. The boundary remains unchanged for broader
claims: this is still a BAR2 command/response path, not evidence for CXL.cache,
CXL.mem, DMA, ATS, migration, switch ordering, compression, runtime overhead,
provenance precision, portability, recovery, or FPGA resource cost.
