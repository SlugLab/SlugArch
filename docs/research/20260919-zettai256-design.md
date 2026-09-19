# SlugArch scaling to 256 endpoints through Zettai

The user requested 256 endpoints, explicitly selected Zettai bridge, and asked
for the SlugArch repository and README to be pushed. The paper remains 12 pages
including references, centered on SlugArch.

## Topology and identity

Use the existing `zettai` VCS switch objects and their registered CXL upstream /
downstream PCI bridges, not a renamed direct-root topology. Each switch has one
USP and up to eight DSPs. Up to eight independent Type-2 functions share each
DSP; four switches host 256 separate QEMU device objects. Unique BDFs, serials,
64-bit BAR apertures, local memories, logs, timers and errors are verified.
This is a simulation arrangement of PCI functions, not 256 separate physical
ports or a modeled 256-link fabric. Function zero is instantiated by the actual
`zettai-bind-vppb` QMP command after staging other functions, following QEMU's
multifunction hotplug requirement. The host then enables slot power and assigns
bridge windows. Every run retains QMP requests/responses and query-pci output.

For every root/USP/DSP, disable memory forwarding, check all affected endpoints
are inaccessible, attempt blocked writes, check an unaffected peer, restore the
window, and verify every affected endpoint's original sentinel. This control
proves that the address path uses the bridges and retains isolation.

## Declared experiment

Use the unchanged native endpoint binary from the <=8-device campaign. The timing
model remains one shared output-link FIFO plus independent endpoint compute;
Zettai arbitration/switch-specific timing and input loading are not newly modeled.
Report virtual times, not wall-clock parallel speedup. All inputs are resident.

- N = 1,2,4,8,16,32,64,128,256.
- 4,096 total words vs 4,096 words per endpoint; three dependent rounds.
- Three compute/link settings; gate on/off; serialized/concurrent; three fresh
  process repetitions: 648 main runs.
- At N=256, two additional per-record costs, three settings, two allocations,
  three repetitions: 36 sensitivity runs.
- N=16/64/256, three repetitions of the five endpoint failure cases plus recovery,
  ordering, frozen input/descriptor, and read-only log checks: nine fault runs.
- Total: 693 fresh processes; 2,052 workload rounds; 45 live fault injections.
- Four independent QEMU processes may run at once. This reduces campaign wall time;
  virtual clocks are independent and process wall times are not compared.

The manifest fixes every cell and order before execution. Failed attempts are
retained, never replaced. Full output bytes, records, resource timestamps, bridge
proofs, command lines, source snapshots, executable identity and a complete seal
are retained locally. Commit source and compact result summaries to Git; exclude
large machine-specific raw artifacts and the separate Overleaf repository.
