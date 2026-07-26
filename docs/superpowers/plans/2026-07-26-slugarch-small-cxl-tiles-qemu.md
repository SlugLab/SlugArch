# SlugArch Small CXL Tiles QEMU Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reconstruct the server-authoritative Type-2 CFMWS prerequisite, add the Rust and FPGA-Verilator Hardware-JIT backends, and prove that 1, 2, 4, and 8 QEMU Type-2 tiles have unique, isolated SlugArch state.

**Architecture:** All implementation occurs in isolated worktrees because the live CXLMemSim, QEMU, SlugArch, and paper checkouts contain user state. The existing J-extension plans remain the detailed backend contract. This plan adds the missing durable reconstruction gate and a per-instance tile identity contract; only tile zero owns the direct 256 MiB CFMWS anchor, while shared-line behavior remains in the explicitly labeled event-level model.

**Tech Stack:** QEMU C11, Meson/Ninja/qtest/QOM, CXLMemSim C++20, Rust `cdylib`, SystemVerilog, Verilator, Python 3, Git.

---

## File Structure

- Reuse `docs/superpowers/plans/2026-07-24-slugarch-type2-transport.md` for
  synchronous wire protocol and server-authoritative CFMWS details.
- Reuse `docs/superpowers/plans/2026-07-25-slugarch-jit-rust-ffi.md` for the
  Rust policy core and stable ABI.
- Reuse `docs/superpowers/plans/2026-07-25-slugarch-jit-fpga-rtl.md` for the
  FPGA-Verilator policy backend.
- Reuse `docs/superpowers/plans/2026-07-25-slugarch-jit-qemu-gpu.md` for the
  QEMU adapter, BAR2 J capability, strict failure, and GPU diagnostics.
- Modify isolated QEMU `include/hw/cxl/cxl_type2.h`: tile/client identity.
- Modify isolated QEMU `hw/cxl/cxl_type2.c`: immutable properties and QOM.
- Modify isolated QEMU `tests/qtest/cxl-test.c`: 1/2/4/8 topology and state
  isolation.
- Create `targets/qemu-type2/tiles/topology.py`: deterministic command-line
  construction.
- Create `targets/qemu-type2/tiles/tests/test_topology.py`: exact topology
  tests.

## Task 1: Create Isolated Source Areas and Capture Dirt

**Files:**
- Inspect: `/root/Concordia/SlugArch`
- Inspect: `/home/victoryang00/CXLMemSim`
- Inspect: `/home/victoryang00/CXLMemSim/lib/qemu`
- Create: `/tmp/slugarch-small-tiles-src/`
- Create: `/tmp/slugarch-small-tiles-source-capture/`

- [ ] **Step 1: Invoke the worktree-isolation skill**

Read and follow `superpowers:using-git-worktrees` before changing any source
checkout.

- [ ] **Step 2: Capture source identities**

Save commit, branch, status, staged diff, unstaged diff, and submodule status
for all three repositories. The original QEMU currently has seven staged
Type-2/HetGPU files plus unrelated generated-header and ROM dirt; preserve
every byte.

- [ ] **Step 3: Prove the ephemeral commits are unavailable**

```bash
git -C /home/victoryang00/CXLMemSim/lib/qemu cat-file -e d194be1951^{commit}
git -C /home/victoryang00/CXLMemSim/lib/qemu cat-file -e 99e757f923^{commit}
git -C /home/victoryang00/CXLMemSim/lib/qemu cat-file -e 229b8b9c96^{commit}
```

Expected: all three commands fail. Reconstruct from the committed design and
transport plan; do not claim these vanished object IDs were cherry-picked.

- [ ] **Step 4: Create clean worktrees**

Create SlugArch, CXLMemSim, and QEMU feature worktrees below
`/tmp/slugarch-small-tiles-src`. Record each new branch and base commit in
`source-identities.json`.

## Task 2: Reconstruct and Prove Direct CFMWS

**Files:**
- Follow: `docs/superpowers/plans/2026-07-24-slugarch-type2-transport.md`
- Modify isolated CXLMemSim server protocol files named by that plan.
- Modify isolated QEMU `hw/cxl/cxl-host.c`
- Modify isolated QEMU `include/hw/cxl/cxl_type2.h`
- Modify isolated QEMU `hw/cxl/cxl_type2.c`
- Modify isolated QEMU `tests/unit/meson.build`
- Create isolated QEMU `tests/unit/test-cxl-type2-route.c`
- Modify isolated QEMU `tests/qtest/cxl-test.c`

- [ ] **Step 1: Rebuild the wire protocol with RED tests**

Require HELLO/ACK followed by 128-byte READ and WRITE messages with CRC32C,
client ID, request ID, DPA, server sequence, modeled latency, and exact
response bytes.

- [ ] **Step 2: Restore server-authoritative memory**

No direct-CFMWS read may return local RAM bytes. A write counts complete only
after the server acknowledges the committed bytes.

- [ ] **Step 3: Restore the one-target route**

Accept exactly one 256 MiB CFMWS, one target, one interleave way, DPA base zero,
sync protocol enabled, and an eligible Type-2 device. Reject ambiguous shapes
with `MEMTX_ERROR`.

- [ ] **Step 4: Run the focused qtest five times**

```bash
for run in 1 2 3 4 5; do
  env QTEST_QEMU_BINARY=/tmp/slugarch-small-tiles-build/qemu/qemu-system-x86_64 \
    /tmp/slugarch-small-tiles-build/qemu/tests/qtest/cxl-test \
    -p /pci/cxl/type2_cfmws
done
```

Expected every run: DPA `83886080`, one 8-byte read, one 8-byte write,
nonzero server sequences, modeled latency applied, and
`slugarch-direct-cfmws == 2`.

- [ ] **Step 5: Run the complete CXL regression set**

```bash
env QTEST_QEMU_BINARY=/tmp/slugarch-small-tiles-build/qemu/qemu-system-x86_64 \
  /tmp/slugarch-small-tiles-build/qemu/tests/qtest/cxl-test
```

Expected: all CXL qtests pass.

- [ ] **Step 6: Commit only isolated worktree changes**

Commit server and QEMU changes separately. Confirm the original dirty
checkouts have byte-identical status and diffs to the source capture.

## Task 3: Implement the Rust and FPGA-Verilator JIT Backends

**Files:**
- Follow: `docs/superpowers/plans/2026-07-25-slugarch-jit-rust-ffi.md`
- Follow: `docs/superpowers/plans/2026-07-25-slugarch-jit-fpga-rtl.md`

- [ ] **Step 1: Execute the Rust plan through its ABI gate**

```bash
cargo test -p slugarch-jit
cargo test -p slugarch-jit-ffi
```

Expected: verified policy parsing, digest, interpreter, records, panic
containment, and C ABI smoke pass.

- [ ] **Step 2: Execute the RTL plan through equivalence**

```bash
cargo test -p slugcxl-gen
cargo test -p slugarch-verilator-sys
cargo test -p slugarch-verilator
cargo test -p slugarch-jit --test rtl_equivalence --features fpga-verilator
```

Expected: validation, delta, and full records are byte-exact between Rust and
Verilator for supported events, including identical first error codes.

- [ ] **Step 3: Reject backend fallback**

Run explicit Rust-missing and FPGA-Verilator-init-failure tests. Expected:
realization fails with the selected backend's stable error; no alternate
backend becomes ready.

## Task 4: Integrate the J-Extension into QEMU

**Files:**
- Follow: `docs/superpowers/plans/2026-07-25-slugarch-jit-qemu-gpu.md`

- [ ] **Step 1: Implement and unit-test the dynamic adapter**

Build `slugarch_jit.c`, its versioned symbol table, fake ABI module, and
loader tests. Run the loader test five times and check file-descriptor count
does not grow.

- [ ] **Step 2: Expose BAR2 and QOM state**

Require capability magic `0x4a474c53`, ABI 1, exact backend, policy digest,
event/record/metadata/reject/drop counters, epoch, and last error.

- [ ] **Step 3: Hook direct CFMWS events fail-closed**

Each sentinel read and write produces request and completion events. Required
record failure causes `MEMTX_ERROR`, increments the exact counter, and prevents
a successful completion.

- [ ] **Step 4: Keep native GPU PTX JIT diagnostic-only**

Use `cuModuleLoadDataEx` only when a real CUDA backend exists. Store bounded
compiler information/error logs and label them `native_gpu_ptx_jit`; never
count them as SlugArch policy-backend results.

- [ ] **Step 5: Run focused and full QEMU tests**

```bash
ninja -C /tmp/slugarch-small-tiles-build/qemu qemu-system-x86_64 \
  tests/unit/test-cxl-slugarch-jit tests/qtest/cxl-test
env QTEST_QEMU_BINARY=/tmp/slugarch-small-tiles-build/qemu/qemu-system-x86_64 \
  /tmp/slugarch-small-tiles-build/qemu/tests/qtest/cxl-test \
  -p /pci/cxl/type2_jext
```

Expected: all focused cases pass before the complete CXL qtest is run.

## Task 5: Add Immutable Tile and Client Identity

**Files:**
- Modify isolated QEMU `include/hw/cxl/cxl_type2.h`
- Modify isolated QEMU `hw/cxl/cxl_type2.c`
- Modify isolated QEMU `tests/qtest/cxl-test.c`

- [ ] **Step 1: Write failing identity qtests**

Instantiate two devices with equal tile ID, two devices with equal client ID,
and one device with a tile ID above 63. Require realize failure. Instantiate
eight devices with IDs 0 through 7 and client IDs 1 through 8; require success.

- [ ] **Step 2: Add exact state**

Add:

```c
uint16_t slugarch_tile_id;
uint32_t slugarch_client_id;
bool slugarch_identity_locked;
```

Properties are `slugarch-tile-id` and `slugarch-client-id`. They are mandatory
when the J-extension is enabled, immutable after realize, and exposed read-only
through QOM.

- [ ] **Step 3: Validate machine-wide uniqueness**

At realize, walk Type-2 siblings under the same machine and reject duplicate
tile/client IDs before opening the JIT library or server socket.

- [ ] **Step 4: Prove per-instance isolation**

Load a different test policy into tile 3, inject a strict drop on tile 5, and
assert all other tiles retain their digest, epoch, record, drop, and error
counters. The global test result fails because tile 5 failed.

- [ ] **Step 5: Run topology qtests five times**

```bash
for run in 1 2 3 4 5; do
  env QTEST_QEMU_BINARY=/tmp/slugarch-small-tiles-build/qemu/qemu-system-x86_64 \
    /tmp/slugarch-small-tiles-build/qemu/tests/qtest/cxl-test \
    -p /pci/cxl/type2_slugarch_tiles
done
```

Expected: 1, 2, 4, and 8 tile cases pass every run.

- [ ] **Step 6: Commit**

Commit the identity and isolation change separately from the J-extension
commit.

## Task 6: Add a Deterministic Topology Builder

**Files:**
- Create: `targets/qemu-type2/tiles/topology.py`
- Create: `targets/qemu-type2/tiles/tests/test_topology.py`

- [ ] **Step 1: Write failing command tests**

Assert topology sizes 1, 2, 4, and 8 produce unique device IDs, tile IDs,
client IDs, log paths, and JIT handles. Assert only tile zero appears in the
one-target CFMWS.

- [ ] **Step 2: Confirm RED**

```bash
PYTHONPATH=targets/qemu-type2/tiles \
  python3 -m unittest discover -s targets/qemu-type2/tiles/tests \
  -p 'test_topology.py' -v
```

Expected: missing module.

- [ ] **Step 3: Implement pure command construction**

Expose:

```python
def build_tile_args(tile_count, backend, policy, log_root, server_socket):
    if tile_count not in (1, 2, 4, 8):
        raise ValueError("tile_count must be one of 1,2,4,8")
```

The function returns an immutable tuple of arguments and performs no process
or file mutation.

- [ ] **Step 4: Run tests and dry-run**

```bash
PYTHONPATH=targets/qemu-type2/tiles \
  python3 -m unittest discover -s targets/qemu-type2/tiles/tests -v
python3 targets/qemu-type2/tiles/topology.py --tiles 8 --backend rust \
  --policy /tmp/validation.json --log-root /tmp/slugarch-eight-tile \
  --server-socket /tmp/cxlmemsim.sock
```

Expected: one shell-escaped argument per line, eight unique tile objects, one
CFMWS target.

- [ ] **Step 5: Commit**

```bash
git add targets/qemu-type2/tiles
git commit -m "qemu: construct isolated SlugArch tile topologies"
```

## Task 7: Run the QEMU Acceptance Gate

**Files:**
- Verify isolated SlugArch, CXLMemSim, and QEMU worktrees.

- [ ] **Step 1: Run unit, semantic, and full QEMU suites**

Run all commands from Tasks 2 through 6 from clean build outputs.

- [ ] **Step 2: Verify source boundaries**

The original QEMU and paper checkouts must have the same pre-existing status
and patch hashes captured in Task 1.

- [ ] **Step 3: Freeze identities**

Record commit IDs and SHA-256 hashes for server, QEMU, Rust shared library,
policy files, generated RTL, and Verilator model. Timing is blocked if any
identity is absent.
