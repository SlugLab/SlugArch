# Replay-checked nested BSP on Splash

This experiment connects SlugArch's evidence contract to BSP supersteps,
motivated by Heng Liao's *Nested Parallel von Neumann Architecture and Nested
BSP*, arXiv:2609.16787v1, and by Valiant's BSP and Multi-BSP models.

It is a host-driven protocol prototype: arithmetic and logical coordinators
execute in Python. Actual outputs and 64-byte certificates traverse the
multi-device QEMU Type-2 BAR path. It is not UnifiedBus, a hardware barrier,
endpoint JIT, or parallel performance evaluation.

Three experiments:

1. Refuse superstep release when work completed but a required certificate has
   a missing-record count, stale epoch, wrong device identity, or bad digest.
2. Compare per-operation receipts, tile seals, and groups of two/four. Count
   root-facing IO separately from total certificate IO; retain all leaf logs.
3. Retry the failed group before root publication and retain verified siblings.
   Check rewritten blocks, preserved sibling readback, every reduction, and the
   next superstep against a serial oracle. This relies on deterministic,
   idempotent overwrites and does not recover already-published effects.

Run from the SlugArch root:

```bash
python3 -m unittest discover -s targets/splash-bsp -p 'test_*.py' -v
python3 targets/splash-bsp/experiment.py \
  --out artifact/slugarch_splash/NEW/nested-bsp \
  --summary docs/evaluation/nested-bsp-NEW.json
python3 targets/splash-bsp/experiment.py --validate-only \
  --out artifact/slugarch_splash/NEW/nested-bsp \
  --summary docs/evaluation/nested-bsp-NEW-verified.json
```

Defaults: 160 healthy processes (1/2/4/8 devices × 1/16 blocks × four policies ×
five repeats) plus 160 fault/recovery processes (2/8 devices × four policies ×
four faults × five repeats). Four supersteps per process. Faults occur in
superstep 1 on the last device. No run is replaced or silently discarded.
Every run retains raw device receipt reads/writes, output records, diagnostics,
releases, retry writes, topology, command, and outcome. Sources, executable
identity, and a complete checksum seal accompany the campaign.

`--source` and `--qemu` select the current Splash/CXLMemSim backend. The shared
transport is `../splash-type2/transport.py`; Python 3.11+ is required. The
retained campaign is `artifact/slugarch_splash/20260919/nested-bsp` (about 46 MiB).
The design and scope were written in `docs/research/20260919-nested-bsp-design.md`
before the full matrix was run.
