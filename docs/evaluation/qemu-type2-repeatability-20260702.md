# QEMU Type-2 BAR2 Repeatability Pass

## Measured Claim

The CXLMemSim QEMU Type-2 BAR2 path repeatedly carries the SlugArch 4x4 GEMM
command stream and returns a validator-clean response stream across five
back-to-back guest runs.

## Setup

- Artifact directory: `artifact/slugarch_cxlmemsim/qemu-type2-repeatability-20260702-0627`
- CXLMemSim checkout: `5475fa44d09ce27b645ed77caa6cc6b47a38a8d4`
- SlugArch base revision: `3ce1a69541708f87247e2ea39794bbff16a29309`
- QEMU binary: `/tmp/CXLMemSim-slugarch-type2/lib/qemu/build-slug2/qemu-system-x86_64`
- Kernel: `Linux node1 6.18.0-rc5+ #8 SMP PREEMPT_DYNAMIC Tue May 19 04:14:31 UTC 2026 x86_64 GNU/Linux`
- Guest PCI device: `0d:00.0 CXL [0502]: Intel Corporation Device [8086:0d92] (rev 01)`
- Launch mode: TCG, 2 GiB guest memory, 2 vCPUs, one simulated Type-2 device, `gpu-mode=0`, `cache-size=16M`, `mem-size=64M`, CXLMemSim TCP port `10199`, guest SSH port `12022`.

## Result

| Run | Host status | Requests | Responses | Tag mismatches | Dispatch failures | Guest elapsed ms | Guest failures |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | pass | 49 | 49 | 0 | 0 | 8 | 0 |
| 2 | pass | 49 | 49 | 0 | 0 | 6 | 0 |
| 3 | pass | 49 | 49 | 0 | 0 | 6 | 0 |
| 4 | pass | 49 | 49 | 0 | 0 | 6 | 0 |
| 5 | pass | 49 | 49 | 0 | 0 | 6 | 0 |

All five runs produced identical request and response byte streams:

- `requests.bin` SHA-256: `f9f05b04d9352de8e0213c42e5efb46f56b05863e077d9cf1ce47a9ddef2b75c`
- `responses.bin` SHA-256: `0562b4dda7e4ec3076b407936b3179eb36fdbc755b92985911b547fb27a7e85c`
- Response size: 3136 bytes per run, 49 64-byte FLITs.
- Decoded matrix in every run: `[[2,3,4,5],[6,7,8,9],[10,11,12,13],[14,15,16,17]]`.

## Boundary

This is repeatability evidence for the simulator-backed Type-2 BAR2
command/response path. It is not a CXL.mem bandwidth test, not a CXL.cache
coherence test, not a DMA or ATS test, and not an end-to-end runtime-overhead
measurement. The CXLMemSim server shutdown reported zero Type3 memory
read/write operations, which is consistent with this BAR2-only path.
