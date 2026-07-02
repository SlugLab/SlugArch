# SlugArch Simulator-Feasible Benchmark Pass

- Workload: `slugcxl_gemm_4x4`
- Replay modes measured: `3`
- BAR2 pass runs: `5/5`
- BAR2 evidence source: `artifact/slugarch_cxlmemsim/qemu-type2-repeatability-20260702-0627`
- DAX probe: `Blocked`; No /dev/dax* device was visible under /dev

## Replay Metadata

| Mode | Records | Payload bytes | Compression vs full | Equivalent ns | Mismatch ns |
| --- | ---: | ---: | ---: | ---: | ---: |
| validation | 98 | 392 | 4.00 | 24569 | 16800 |
| delta | 98 | 552 | 2.84 | 18962 | 8737 |
| full | 98 | 1568 | 1.00 | 17396 | 8513 |

## Claim Ledger

| Claim | Status | Evidence artifact | Paper-safe wording | Limitation |
| --- | --- | --- | --- | --- |
| QEMU Type-2 BAR2 command/replay boundary | Measured | artifact/slugarch_cxlmemsim/qemu-type2-repeatability-20260702-0627 | CXLMemSim QEMU Type-2 BAR2 carried the SlugArch command stream when the supplied artifact has all runs passing. | This is Type-2 BAR2 command-path evidence, not CXL link latency or hardware endpoint latency |
| CXL.mem/DAX simulator traffic | Blocked | /dev | CXL.mem/DAX simulator traffic remains blocked unless a real streaming workload artifact is present. | No /dev/dax* device was visible under /dev |
| CXL.cache coherence | Blocked | none | CXL.cache coherence is not measured by BAR2 command traffic or software replay metadata. | No live CXLMemSim GPU coherency statistic or CXL.cache transaction artifact is produced by this pass. |
| DMA | Blocked | none | DMA replay remains a benchmark slot. | No real DMA path is exercised or logged by this pass. |
| ATS | Blocked | none | ATS replay remains a benchmark slot. | No simulator or kernel ATS event path is exposed by this pass. |
| Page migration | Blocked | none | Page migration replay remains a benchmark slot. | No migration event source is exercised or logged by this pass. |
| Switch ordering | Blocked | none | Switch ordering replay remains a benchmark slot. | No two-host or switch-lock workload is run by this pass. |
| Runtime overhead | PartiallyMeasured | artifact/slugarch_cxlmemsim/qemu-type2-repeatability-20260702-0627 | Runtime overhead is partially measured for the QEMU Type-2 BAR2 command path only. | This is not a CXL link, hardware endpoint, or production continuous-overhead measurement. |
| Compression | PartiallyMeasured | software replay metadata report | Compression is measured for software replay artifact modes only. | Validation, delta, and full payload accounting do not prove a hardware compression engine. |
| Replay latency | PartiallyMeasured | software replay metadata report | Replay latency is measured as software replay validation only. | The pass times validation of replay artifacts, not hardware replay execution. |
| Provenance | PartiallyMeasured | software replay metadata report | Provenance is measured for software labels on the GEMM trace only. | The pass does not prove fabric-wide endpoint or protection-domain provenance. |
| FPGA resource cost | Blocked | targets/agilex-vr2/generated/slugcxl_hj_overhead.json | FPGA resource cost is blocked for post-fit resources; model-side metadata estimates may be cited separately. | No Quartus synthesis, fit, timing, or resource report is produced by this simulator pass. |
