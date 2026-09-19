# Multi-device Splash Type-2 experiments

Run 1, 2, 4, or 8 actual QEMU Type-2 objects in one simulator process, using
Splash's current CXLMemSim/QEMU backend. The driver and results live on the
current SlugArch branch; neither the Splash nor CXLMemSim branch is modified.

The path is BAR2 commands / BAR4 local simulated memory. It uses no guest OS,
external memory service, physical accelerator, or SlugArch endpoint JIT.
Arithmetic, the recorder, and validation run on the host. Timings describe the
serialized simulator path and JSONL recorder, not hardware latency.

From the SlugArch repository root:

```bash
python3 -m unittest discover -s targets/splash-type2 -p 'test_*.py' -v
python3 targets/splash-type2/campaign.py \
  --out artifact/slugarch_splash/NEW/campaign
python3 targets/splash-type2/validate.py \
  artifact/slugarch_splash/NEW/campaign \
  --output docs/evaluation/splash-NEW.json
python3 targets/splash-type2/replay.py \
  artifact/slugarch_splash/NEW/campaign \
  --out artifact/slugarch_splash/NEW/replay
```

Use a new output directory for each run; the tools refuse to overwrite runs.
`--source`, `--splash`, and `--qemu` override the default local repositories and
executable. Python 3.11+ is required (`hashlib.file_digest`); the paper plotters
also require Matplotlib. QEMU must support the Type-2 commands used by the
current Splash build, including statistics and timing counters.

Defaults: device counts 1/2/4/8; payloads 64/4096 bytes; off/validation/full
recording; five fresh processes per cell (120 processes). Each process runs
correctness assertions and fixed-total (256), fixed-per-device (64/device),
and host-relay (16 rounds) workloads. Eight warmup transfers per device precede
each phase. The command, enumeration, per-device counters, individual timing
samples, trace, and output commitments are retained. A manifest fixes the
matrix and order before execution; a final checksum list seals every file.

The validator requires all cells and traces. It compares payloads to an
independent deterministic generator, checks request counts, and rejects 10
trace corruptions in validation mode and 11 in full mode. Full-trace replay
launches a fresh simulator for each full-capture run and sends the recorded
payloads through the devices again.

The retained campaign is `artifact/slugarch_splash/20260919/campaign`; its
validated summary is `docs/evaluation/slugarch-splash-multidevice-20260919.json`.
Raw evidence occupies about 216 MiB. Keep its `SHA256SUMS` with the data.
The later nested-BSP extension is in `../splash-bsp/`.
