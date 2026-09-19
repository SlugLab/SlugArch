# SlugArch endpoint gate and concurrent virtual-time execution

This extends the September 19 host-only campaigns. The user selected concurrent
Splash simulation and a 12-page paper including references, centered on SlugArch.

## Declared implementation and hypotheses

An opt-in native bounded gate lives in the QEMU Type-2 device. Its new vector-job
MMIO interface snapshots input and freezes the descriptor at admission. It owns
a bounded, read-only request/completion log and a separately reserved failure
record. It rejects stale epochs, invalid ranges and insufficient log capacity
before effects; a completion-record failure after output commit withholds success.
This is endpoint enforcement for this new interface, not a JIT or coverage of all
legacy BAR/CUDA/DMA operations. Host configuration and MMIO setup are trusted.

Each device accepts one outstanding vector-map job. Independent compute intervals
can overlap on QEMU_CLOCK_VIRTUAL. Output publication uses a single FIFO shared-link
resource across enabled devices. Timer callbacks, not a host-computed makespan,
perform the output writes and publish terminal status. qtest controls time as
specified by https://www.qemu.org/docs/master/devel/testing/qtest.html.

For w 64-bit words, input is initially resident in endpoint RAM; the native kernel
computes y[i] = 3*x[i] + scalar + i modulo 2^64. Per-job compute duration is w*c;
shared-link occupancy for output is ceil(8*w/b), followed by a propagation delay L.
The next transmission may start after serialization, before propagation completes.
Gate-on adds r before compute and r before success for two local 128-byte records.
SHA-256 work is included in the abstract per-record r, not separately calibrated.
The per-word compute c, bytes/ns b, delay L, and per-record r are assumptions, not
measured hardware parameters. Their sensitivity is part of the experiment.

## Checks and controls

- 1/2/4/8 endpoint objects, equal total work and equal work per endpoint.
- Baseline gate off and gate on, otherwise identical native kernel and scheduler.
- Concurrent vs deliberately serialized submission; fixed-seed submission shuffles.
- Independent Python value oracle and independent resource-scheduling oracle.
- Before-timer output stays unchanged; request log exists before output; output
  commits before success; post-commit injected fault exposes committed diagnostic.
- Request failure, completion failure, capacity pressure, stale epoch, invalid range,
  descriptor mutation while busy, input snapshot, duplicate doorbell, cross-device
  isolation, subsequent valid epochs, and log read-only behavior.
- Three dependent rounds exercise map/reduce iteration using the preceding verified
  reduction as the next scalar; release requires every endpoint success and evidence.
- Preserve source, binary identity, exact commands, raw endpoint logs and statuses,
  complete matrix, failures, and checksum seals. Never overwrite the earlier binary
  or claim this experiment reproduces the archived endpoint JIT implementation.

Virtual-time speedups characterize this explicit compute/link model. They are not
host wall-clock parallelism, a physical CXL measurement, or a cycle-accurate fabric.
