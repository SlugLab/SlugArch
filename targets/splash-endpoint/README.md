# SlugArch endpoint enforcement and concurrent Splash execution

This extension adds a **native, fixed-policy endpoint gate** to the current QEMU
Type-2 implementation used by Splash. The gate and vector arithmetic execute
inside QEMU's device model. Each endpoint has one asynchronous job; virtual compute
intervals overlap and output publication contends for one shared FIFO link.
The host driver admits jobs and verifies results; it does not compute device outputs.

The retained September 19 campaign contains 336 fresh QEMU processes, 972 dependent
execution rounds, 1,157 listed correctness/regression checks, and 45 deliberate
endpoint failures. All declared outcomes pass. It augments the earlier host-only
campaign rather than changing its evidence or executable.

## Reproduce the original 1–8-endpoint campaign

```bash
python3 -m unittest discover -s targets/splash-endpoint -p 'test_*.py' -v
python3 targets/splash-endpoint/experiment.py \
  --out artifact/slugarch_splash/NEW/endpoint-concurrent \
  --summary docs/evaluation/endpoint-concurrent-NEW.json
python3 targets/splash-endpoint/experiment.py --validate-only \
  --out artifact/slugarch_splash/NEW/endpoint-concurrent \
  --summary /tmp/endpoint-concurrent-verified.json
python3 targets/splash-endpoint/audit.py \
  artifact/slugarch_splash/NEW/endpoint-concurrent
```

The driver defaults to `/root/CXLMemSim/lib/qemu/build-slugarch/qemu-system-x86_64`.
Use `--source` and `--qemu` for other locations. The shared transport imports
`qemu_integration/qtest_switch_offload.py` from the CXLMemSim source repository.
The full matrix and fixed ordering are declared in `experiment.py::matrix`.
No process is retried or discarded. Existing output directories are refused.

The independent audit rechecks the complete checksum seal, matrix, values,
resource schedule, raw endpoint records, and fault evidence. Ten corruptions of
payloads, times, dependencies, identities, and completion records must be rejected.
These are offline validator tests, separate from the 45 live endpoint failures.

## QEMU integration and build

Use the [CXLMemSim source](https://github.com/SlugLab/CXLMemSim) at
`10271685893ca74ba2e5244d342991edd7919ba3` for the qtest helper, and its
[QEMU fork](https://github.com/CXLMemUring/qemu) at
`2bdb60c32bfbc9e26c20b9442e8895002204dff6`. The experiment snapshots both the
helper and all modified QEMU source. Linux build dependencies include a C/C++
toolchain, Python, Ninja, pkg-config, GLib and pixman development headers; QEMU's
configure step identifies any additional requirements.

From the SlugArch checkout, apply the following to a **fresh** checkout of that
QEMU revision. Choose your own absolute `QEMU_SRC` path:

```bash
SLUGARCH_SRC="$PWD"
QEMU_SRC=/path/to/CXLMemSim/lib/qemu
cd "$QEMU_SRC"
git apply --check "$SLUGARCH_SRC/targets/splash-endpoint/qemu/coherency-prerequisite.patch" \
  "$SLUGARCH_SRC/targets/splash-endpoint/qemu/integration.patch"
git apply "$SLUGARCH_SRC/targets/splash-endpoint/qemu/coherency-prerequisite.patch" \
  "$SLUGARCH_SRC/targets/splash-endpoint/qemu/integration.patch"
cp "$SLUGARCH_SRC/targets/splash-endpoint/qemu/cxl_slugarch.c" hw/cxl/
cp "$SLUGARCH_SRC/targets/splash-endpoint/qemu/cxl_slugarch.h" include/hw/cxl/
mkdir build-slugarch
cd build-slugarch
../configure --target-list=x86_64-softmmu --enable-kvm \
  --enable-trace-backends=nop --disable-debug-info --disable-werror
ninja -j 24 qemu-system-x86_64
```

`coherency-prerequisite.patch` preserves the coherency lock fix that was already
present when the experiments began. It is required for the exercised memory
path. `integration.patch` adds the SlugArch gate; the C module/header are supplied
separately. The retained campaign's `sources/qemu-working-tree.patch` combines
both tracked patches: use that file **instead of** these two patches when
reconstructing from raw artifacts. Do not apply patches to an already modified
checkout. The original `build-perf` binary was preserved during these experiments.

For the new 1–256-endpoint campaign through Zettai bridges, continue with
[ZETTAI.md](ZETTAI.md).

## Interface and evidence

Enable with `slugarch=on,gpu-mode=0`; all other users default to the old interface.
Use local memory backing, no DCD/GFAM or direct shared mapping, and a BAR2 region
at least `0x221000` bytes. The experiment reserves an unlistened TCP port so no
external memory server can be attached. Host configuration and memory ownership
are trusted; no concurrent client may change a job's output region.

The new MMIO region starts at BAR2 + `0x200000`. `driver.py::REGS` gives the
64-bit aligned register map. Read-only 128-byte records start at region + `0x1000`;
a separate reserved 128-byte failure diagnostic is at + `0x800`. The gate
reserves capacity for a request/completion pair before starting work. Epoch
advance acknowledges terminal state and drains the log, so export records first.
Fault status is sticky until that explicit advance. Descriptor writes and duplicate
submissions while busy are rejected. A failed completion record can leave output
committed, but never reports job success.

Input and output reside in each endpoint's BAR4 RAM. The native unsigned-64 kernel
is `y[i] = 3*x[i] + scalar + i`. The driver checks every output byte against an
independent Python oracle and uses the verified global reduction in the next round.
It tests missing request/completion records, capacity pressure, stale epochs,
invalid ranges, peer isolation, recovery, immutable descriptors, frozen input,
read-only logs, and visibility immediately before/after virtual deadlines.

## Timing scope

QEMU timers **delay state transitions** on `QEMU_CLOCK_VIRTUAL`. Unlike the older
additive timing counters, they control output commit and success visibility.
Compute takes `words * compute_ns` independently per endpoint. The shared output
link occupies `ceil(bytes / bandwidth)` ns; propagation adds 200 ns but does not
block the next transmission. Enabled recording adds one local record cost before
compute and another before success. Gate-off retains descriptor validation and
the same kernel/scheduler but omits records and their modeled cost.

Compute/link settings `(ns/word, bytes/ns)` are `(32,32)`, `(8,16)`, `(1,2)`;
record cost is 64 ns, with 0/256 ns sensitivity runs. These are **uncalibrated
model assumptions**. Input is resident; host admission, input loading, switches,
input-link contention, crash durability and arbitrary application scheduling are
not modeled. Repeated virtual times are deterministic, not hardware samples.
This interface is not a JIT and does not gate legacy BAR/CUDA/DMA operations.

See the [measured timer results](../../docs/evaluation/slugarch-endpoint-concurrent-20260919.md)
and [experiment design](../../docs/research/20260919-endpoint-concurrency-design.md).
QEMU clock behavior follows its [qtest documentation](https://www.qemu.org/docs/master/devel/testing/qtest.html).
