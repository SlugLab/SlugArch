# SlugArch / Splash evaluation — 2026-09-19

All 480 new QEMU processes passed their declared outcome checks: 120 base
campaign runs, 40 full-trace replays, and 320 nested-BSP runs. Of the BSP runs,
160 intentionally inject an invalid certificate and must reject the first
barrier attempt before recovering. They are not fault-free successes.

## Main findings

- 1, 2, 4, and 8 independently mapped Type-2 endpoints execute in one QEMU process.
- Base campaign: 10,800 correctness assertions, 66,720 exact measured payload checks,
  88,960 host records, and 840 rejected offline mutations.
- Fresh functional replay: 40 processes reproduce 22,240 recorded payloads.
- BSP campaign: 1,280 checked supersteps; 160/160 injected faults block release;
  all corrected supersteps and following reductions match the serial oracle.

At eight devices with sixteen 64-byte output blocks per device:

| Policy | Root receipt reads / step | All receipt reads / step | Retry output blocks | Median four-step simulator time (ms) |
|---|---:|---:|---:|---:|
| Per-operation | 128 | 128 | 128 | 201.47 |
| Tile BSP | 8 | 8 | 128 | 99.97 |
| Groups of two | 4 | 12 | 32 | 104.12 |
| Groups of four | 2 | 10 | 64 | 103.58 |

Nesting lowers root fan-in but increases total certificate IO relative to
per-tile seals. The flat per-tile baseline is faster in the serialized driver.
Recovery savings apply to output rewrites before publication, with deterministic,
idempotent work and reread checks on retained siblings. They are not general
rollback or exactly-once execution of external side effects.

For 4-KiB base transfers at eight devices (64 transfers per device), medians are
362.31 µs with recording off, 391.11 µs with validation, and 460.47 µs with full
payload recording: 7.9% and 27.1% increases over the off median. The corresponding
JSONL sizes are 716.89 and 17,134.89 bytes per transaction. Full mode stores both
request and completion payloads as hex; the ratio is representation-specific.

## Substrate and provenance

The new work uses local QEMU BAR2/BAR4 device memory, a serialized qtest driver,
host arithmetic, host recorders, and host-emulated logical BSP coordinators.
No guest OS, endpoint SlugArch JIT, physical accelerator, UnifiedBus, peer DMA,
physical CXL latency, or concurrent shared-link contention is evaluated. Modeled
device counters do not stall execution and are not added to measured wall time.

- QEMU SHA-256: `3f1d1ad9c10645de5dc2afcd9eddb9022ee376db365719fb0a78fc488eca0951`
- SlugArch base commit: `6001be12f0b60663ad1f1ec970364efa71cde367` on `main`.
- Splash base commit: `69dc8f0bde8556f469d2db98603c4542bf79968c` on `main`.
- CXLMemSim base commit: `10271685893ca74ba2e5244d342991edd7919ba3` on `codex/cxl-switch-damer-runtime`.
- QEMU source commit: `2bdb60c32bfbc9e26c20b9442e8895002204dff6`.
- Host: `: Intel(R) Xeon(R) 6710E`.
- Platform: `Linux-7.0.0-31-generic-x86_64-with-glibc2.43`; Python 3.13.9.
- Pre-existing external working-tree changes are recorded in the manifests; they were not overwritten.
- Each campaign has exact commands, raw evidence, copied source files, executable identity,
  and `SHA256SUMS`. The base campaign and BSP campaign occupy about 262 MiB together.

## Files and reproduction

- Base experiment: `targets/splash-type2/README.md`.
- BSP extension: `targets/splash-bsp/README.md`.
- Source-grounded BSP design: `docs/research/20260919-nested-bsp-design.md`.
- Base raw evidence: `artifact/slugarch_splash/20260919/campaign/`.
- Replay evidence: `artifact/slugarch_splash/20260919/replay/`.
- BSP evidence: `artifact/slugarch_splash/20260919/nested-bsp/`.
- Summaries: [base](slugarch-splash-multidevice-20260919.json),
  [replay](slugarch-splash-replay-20260919.json), and
  [certificates](slugarch-nested-bsp-20260919.json).

## Paper status

The manuscript is maintained separately. Its prose, citations, figures, and
evidence checks have been updated. The retained July endpoint JIT results are explicitly historical:
neither their JIT hooks nor complete original raw campaigns are present in the
current checkout. No new result claims to reproduce them.

The candidate contribution is a SlugArch release condition requiring complete
replay evidence, combined with hierarchical certificates and scoped pre-publication
recovery. Hierarchical parallel organization is prior work. This initial campaign
did not implement the native endpoint gate or concurrent resource contention;
the subsequent experiments below address those two simulation boundaries.

## Subsequent endpoint extension

A later campaign adds native endpoint gating and concurrent virtual-time jobs,
with an explicit shared-link model. See
[the endpoint report](slugarch-endpoint-concurrent-20260919.md) and the subsequent
[256-endpoint Zettai report](slugarch-zettai256-20260919.md). The earlier campaigns and their
executable remain unchanged; the new extension has its own sealed evidence.
