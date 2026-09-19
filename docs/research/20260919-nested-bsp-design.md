# Nested BSP replay experiment (declared before execution)

Reference: Heng Liao, *Nested Parallel von Neumann Architecture and Nested BSP*,
arXiv:2609.16787v1, 2026-09-15, Huawei Technologies Co., Ltd.
https://arxiv.org/html/2609.16787v1

Liao motivates matching a nested parallel software structure to a hierarchy of
peer-connected stored-program hardware. Classic BSP and Valiant's Multi-BSP
already establish synchronization and hierarchical models. We do not claim
hierarchical BSP itself as our contribution, reproduce Huawei performance
claims, or equate CXL host-mediated devices with UnifiedBus peers.

Our proposed contribution: attach replay completeness to a superstep release.
A barrier requires both a known output and its evidence, not merely arrivals.
A child certificate binds epoch, membership, policy, event count and output
commitment. Parent certificates bind exact children. Failed children cannot
be silently omitted. All lower-level evidence remains retained.

## Three falsifiable questions

1. **Release safety:** Does a completion-only barrier advance when output is
   correct but required evidence is missing? Does a validated barrier refuse
   release for missing records, stale epochs, wrong device identity, or a bad
   digest? A post-write missing-record injection must retain actual readback
   proving that data committed before rejecting the barrier.
2. **Hierarchical fan-in:** With the same work and retained local evidence,
   compare per-operation receipts, per-tile BSP seals, and two-/four-tile group
   seals. Measure root-facing receipt reads/bytes separately from total receipt
   IO and wall time. A hierarchy may lower root fan-in while increasing total
   work; do not claim a speedup from fewer root requests alone.
3. **Recovery before publication:** If one group fails before the root barrier
   publishes a reduction, preserve verified sibling outputs and rerun only the
   failed group. Compare with a flat retry of all devices, check the next
   superstep against a serial oracle, and count actual rewritten output blocks.
   This is valid only for deterministic, idempotent writes within an unexposed
   superstep. It is not rollback of a globally published effect.

## Substrate and workload

Use the same current Splash/CXLMemSim QEMU binary and local BAR2/BAR4 path as
`targets/splash-type2`. Actual application arithmetic and logical coordinators
execute in the host driver. All device output and certificates go through
QEMU. Logical group representatives rotate per epoch, but QEMU access remains
serialized and host-driven; no peer hardware or parallel performance claim.

Each of four supersteps computes 1 or 16 blocks of eight unsigned 64-bit
integers per device from the previous global reduction, epoch, device, and
block. All blocks cross the Type-2 path and are checked exactly. Leaf/group
certificates reside in separate device-memory ranges and are read through the
device consume command. The root releases the next superstep only after all
required certificates validate. The global reduction uses returned bytes.

Matrix: 1/2/4/8 devices; event, tile, nested2, nested4 policies; 1/16 blocks;
five fresh QEMU processes each = 160 successful runs. Failure matrix:
2/8 devices, four policies, missing record / stale epoch / wrong tile / corrupt
digest, five fresh processes each = 160 injection/recovery runs. Inject on the
last tile in superstep 1. No replacement of failed runs. Randomized fixed-seed
configuration order, all raw records and commands retained with hashes.

Root traffic and recovery savings are counts from actual device IO. Wall time
is whole serialized simulation and Python work. No fabricated BSP `g` or `L`,
no extrapolation to Huawei scale, no assumed link speedup, and no loss of
local records hidden as a compression result.
