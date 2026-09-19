# Concurrent SlugArch execution through Zettai

`scale256.py` runs 1, 2, 4, 8, 16, 32, 64, 128 and 256 independent Type-2
device objects through the QEMU fork's existing Zettai VCS implementation.
Each endpoint owns its memory, native gate, log, timer and failure state.

## Topology and forwarding

At 256 endpoints the topology has **four Zettai switches, four upstream ports,
32 downstream ports, and eight Type-2 PCI functions per downstream port**.
This is 256 endpoint instances, not 256 physical switch ports. Multifunction
enumeration keeps the topology inside one PCI domain; each function has a unique
BDF, serial number and pair of 64-bit BAR apertures.

The driver stages functions 1–7 with QMP `device_add`, then calls
`zettai-bind-vppb` to bind function 0 from each switch's physical port pool.
Function 0 is bound last to satisfy QEMU's multifunction hotplug ordering. The
driver powers on the downstream slot and configures all root, upstream and
downstream bridge windows. Functions 1–7 share the downstream bridge; they are
not independently bound physical Zettai ports.

Every process verifies PCI enumeration and BAR readback. It then disables memory
forwarding at **each** root, upstream and downstream bridge. Every descendant
must become inaccessible; attempted writes must leave its sentinel unchanged;
an unaffected branch must remain accessible when one exists. Restoring the
bridge must restore access and the original sentinel. QMP transcripts, bridge
configuration, topology and these negative controls are retained per process.

## Run and audit

Build the pinned backend using [README.md](README.md), then run from the SlugArch
root. `--source` identifies the CXLMemSim repository containing the qtest helper;
`--qemu` selects the patched executable.

```bash
python3 targets/splash-endpoint/scale256.py \
  --source /path/to/CXLMemSim \
  --qemu /path/to/CXLMemSim/lib/qemu/build-slugarch/qemu-system-x86_64 \
  --out artifact/slugarch_splash/NEW/zettai-256 \
  --summary /tmp/slugarch-zettai256.json --workers 4

python3 targets/splash-endpoint/scale256.py --validate-only \
  --out artifact/slugarch_splash/NEW/zettai-256 \
  --summary /tmp/slugarch-zettai256-verified.json
```

The driver refuses an existing output directory, preserves failures, snapshots
sources and hashes the executable, and seals all raw files after the declared
matrix completes. Keep several GiB of free storage. Fewer workers reduce host
resource use; worker count does not alter QEMU virtual-time results. Validation
uses the Python standard library and retained artifacts, without launching QEMU.
Do not run validators with `python -O`: assertions are part of the audit.

| Matrix component | Configurations | Fresh processes |
|---|---|---:|
| Scaling | 9 endpoint counts × strong/weak × 3 compute/link profiles × gate on/off × serial/concurrent × 3 repeats | 648 |
| Recording sensitivity | 256 endpoints × strong/weak × 3 profiles × 0/256 ns per record × 3 repeats | 36 |
| Correctness | 16/64/256 endpoints × 3 repeats | 9 |
| Total | Every declared cell; no retries or discarded runs | 693 |

Each timed process executes three dependent rounds. Strong scaling divides
4,096 words across endpoints; weak scaling uses 4,096 words per endpoint.
All output bytes, reduction dependencies, endpoint records and resource
timestamps are checked against independent oracles. Correctness processes test
pre-/post-commit faults, invalid descriptors, log capacity, frozen jobs, peer
isolation and epoch recovery. Ten offline corruptions at 256 endpoints must
also be rejected.

## Interpret the measurements

The clock is QEMU virtual time. Input is resident before admission. Independent
compute timers contend for a **single shared FIFO output-link model across all
switches**. Compute/link profiles are `(32 ns/word, 32 bytes/ns)`, `(8, 16)` and
`(1, 2)`. Each record costs 64 ns by default, and propagation adds 200 ns.
These costs are declared assumptions. Zettai supplies real bridge forwarding
and binding behavior, but its arbitration and physical link timing are not
modeled separately. Host wall time is retained only as execution context.

The [result report](../../docs/evaluation/slugarch-zettai256-20260919.md) provides
the measured curves, exact summary and provenance. The [figure script](../../scripts/plot_zettai_results.py)
rebuilds the public PNG/PDF using Matplotlib; that optional dependency is not
needed for the simulator or offline unit tests.
