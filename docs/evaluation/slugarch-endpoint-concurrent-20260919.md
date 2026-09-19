# SlugArch native endpoint and concurrent Splash results — 2026-09-19

The new native QEMU endpoint gate passed the complete declared campaign: **336
fresh processes, 972 dependent execution rounds, 1,157 listed correctness/regression
checks, and 45 live fault cases**. An additional independent audit verified all
nine fault-run evidence files and rejected ten offline evidence mutations.
Together with the earlier campaigns, this stage accounts for 816 new QEMU processes.
The subsequent [Zettai campaign](slugarch-zettai256-20260919.md) extends the study
to 256 endpoints and brings the total to 1,509 processes.

The gate executes inside the device model. It records requests before executing
its vector job and completes the evidence before exposing success. Faults before
execution leave output untouched; a completion-record failure after output commit
preserves that distinction and withholds success. Busy descriptors are immutable,
input is snapshotted, and logs are read-only through the experimental MMIO region.
Peer endpoints remain independent and later acknowledged epochs recover.

## Concurrent scaling

All timings below are **QEMU virtual microseconds for three dependent rounds**,
with the gate enabled and a modeled 64-ns cost per record. Strong scaling fixes
4,096 total words; weak scaling fixes 4,096 words per endpoint. Each cell has three
fresh repetitions; their virtual times agree exactly. The independent scheduling
oracle agrees with every exported stage timestamp.

| Compute/link setting | Strong N=1 (µs) | Strong N=8 (µs) | N=1 / N=8 speedup | Serial / concurrent at N=8 | Weak N=8 (µs) |
|---|---:|---:|---:|---:|---:|
| compute | 397.272 | 53.208 | 7.47× | 7.60× | 418.776 |
| balanced | 105.432 | 19.416 | 5.43× | 5.78× | 148.440 |
| link | 62.424 | 51.672 | 1.21× | 1.34× | 406.488 |

Settings are `(compute ns/word, link bytes/ns)`: compute `(32,32)`, balanced
`(8,16)`, and link `(1,2)`. Propagation adds 200 ns and can overlap a later transfer.
Independent device compute overlaps; a single shared output-link FIFO limits
communication throughput. Gate on/off and serialized/concurrent controls use the
same endpoint kernel and work allocation. An additional 36 runs vary record cost
between 0 and 256 ns at N=8, alongside the 64-ns main matrix.

These figures describe an **uncalibrated resource model**. They establish that
multiple native endpoint jobs execute concurrently in simulated time and reveal
the model's bottlenecks; they are not physical CXL performance measurements,
CPU-thread speedup, or end-to-end application results. Input is already resident.
The current gate covers only the new native job interface, not legacy commands
or a programmable Hardware JIT.

## Artifact and manuscript

- Driver/build instructions: `targets/splash-endpoint/README.md`.
- Reviewable QEMU changes: `targets/splash-endpoint/qemu/` and the current external
  QEMU working tree. Pre-existing changes are preserved.
- Full sealed evidence: `artifact/slugarch_splash/20260919/endpoint-concurrent/`.
- Validated summary: `docs/evaluation/slugarch-endpoint-concurrent-20260919.json`.
- New binary SHA-256: `63e190067b62903828dbfbdda5945ee2d07c620afc02bbfe4c1d73f33728b097`.
- Manifest SHA-256: `a4a65455a0739703269410f02cc3d0d2eb86901fcad8092125cadbdafe6654b9`.
- Seal SHA-256: `3db0573390a31b23c5c65ba61cf19279c94fc569e54226397352ea41b3aacb0d`.
- The earlier `build-perf` executable remains byte-for-byte unchanged.

The revised paper is **12 pages total: 11 main-text pages and one reference page**.
SlugArch is the main narrative. BSP/Multi-BSP and Liao remain briefly cited in
background and related work; their hierarchical organization is not claimed as
SlugArch's invention. The new figure separates compute overlap and link limits.
