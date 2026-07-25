# SlugArch JIT QEMU and GPU Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish direct Type-2 CFMWS routing, dynamically load the Rust J-extension in QEMU, expose a fail-closed vendor BAR2 capability, record CXL.mem and GPU boundary events, and capture real CUDA PTX compiler diagnostics.

**Architecture:** A focused `slugarch_jit.c` adapter owns `dlopen`, symbol/version negotiation, policy lifecycle, event conversion, counters, and diagnostics so `cxl_type2.c` remains a device model rather than an FFI implementation. The Type-2 device advertises the BAR2 extension only after strict initialization. Direct CFMWS request/completion pairs and GPU module/kernel boundaries synchronously enter the Rust engine; GPU module loading upgrades from `cuModuleLoadData` to `cuModuleLoadDataEx` with bounded logs while preserving the old behavior when the J-extension is off.

**Tech Stack:** QEMU C11, Meson/Ninja, qtest, QOM, POSIX `dlopen`, CUDA Driver API dynamic symbols, Rust `cdylib`, CRC32C, JSONL.

---

## File Structure

### QEMU adapter

- Create `include/hw/cxl/slugarch_jit.h`: adapter state, backend enum, event
  helpers, status and register access API.
- Create `hw/cxl/slugarch_jit.c`: dynamic library, policy, event, stats,
  diagnostics, and evidence.
- Modify `hw/cxl/meson.build`: build the adapter and link `dl`.
- Modify `include/hw/cxl/cxl_type2.h`: embed adapter state and QOM counters.
- Modify `include/hw/cxl/cxl_type2_gpu_cmd.h`: capability bit, register block,
  commands, backends, status, and errors.
- Modify `hw/cxl/cxl_type2.c`: lifecycle, BAR2 register/command handling,
  CFMWS hooks, module/kernel hooks, properties, and QOM.
- Modify `include/hw/cxl/cxl_hetgpu.h`: `cuModuleLoadDataEx` diagnostics API.
- Modify `hw/cxl/cxl_hetgpu.c`: dynamic symbol and bounded diagnostic call.
- Modify `hw/cxl/cxl-host.c`: completed direct Type-2 route only.

### Tests

- Create `tests/unit/slugarch-jit-fake.c`: ABI-compatible fake shared library.
- Create `tests/unit/test-cxl-slugarch-jit.c`: loader, policy, event, and
  failure tests.
- Modify `tests/unit/meson.build`: fake shared module and loader test.
- Modify `tests/qtest/cxl-test.c`: direct CFMWS, capability, commands,
  no-fallback, strict drop, GPU diagnostics, and regression cases.
- Modify `tests/qtest/meson.build` only if a test environment variable or
  generated module dependency must be registered.

### Guest shim

- Modify `qemu_integration/guest_libcuda/cxl_gpu_cmd.h`: mirror vendor ABI.
- Modify `qemu_integration/guest_libcuda/libcuda.c`: capability query and
  diagnostic retrieval without changing normal CUDA call signatures.
- Create `qemu_integration/guest_libcuda/jext_debug.c`: explicit debug CLI.
- Modify `qemu_integration/guest_libcuda/Makefile`: build the CLI.

## Task 1: Complete the One-Target Type-2 CFMWS Prerequisite

**Files:**
- Modify: `hw/cxl/cxl-host.c`
- Modify: `include/hw/cxl/cxl_type2.h`
- Modify: `tests/unit/meson.build`
- Create: `tests/unit/test-cxl-type2-route.c`
- Modify: `tests/qtest/cxl-test.c`

- [ ] **Step 1: Re-run the current unit gate**

```bash
ninja -C /tmp/slugarch-type2-transport-build/qemu \
  tests/unit/test-cxl-type2-route
/tmp/slugarch-type2-transport-build/qemu/tests/unit/test-cxl-type2-route
```

Expected: valid exact shape and all invalid/overflow shapes pass.

- [ ] **Step 2: Extend the fake server after ACK**

Add little-endian load helpers:

```c
static uint32_t cxl_t2_load_le32(const uint8_t *p);
static uint64_t cxl_t2_load_le64(const uint8_t *p);
```

For valid mode, keep the socket open and process exactly two 128-byte memory
requests. Require:

```text
magic SLT2
version 1
type READ(3) then WRITE(4)
length 128
client ID 1
DPA 83886080
operation length 8
valid CRC32C
```

The READ response returns bytes `11 22 33 44 55 66 77 88`, sequence 1, and
modeled latency 400. The WRITE response returns zero data, sequence 2, and
modeled latency 400. Save the observed write bytes and exact flags in
`CXLType2FakeServer`.

- [ ] **Step 3: Write the direct CFMWS qtest**

Start one command-line Type-2 endpoint with a 256 MiB one-target CFMWS. Enable
MMCFG, program root-port bus numbers, and obtain the CFMWS base from the CEDT
CFMWS entry or a read-only QOM base property added to `CXLFixedWindow`; do not
hardcode a host-physical address without asserting it against QEMU state.

Run:

```c
uint64_t value = qtest_readq(qts, cfmws_base + 80 * MiB);
g_assert_cmphex(value, ==, 0x8877665544332211ULL);
qtest_writeq(qts, cfmws_base + 80 * MiB, 0xfedcba9876543210ULL);
```

Assert server DPA/bytes, QOM completed reads/writes, and
`slugarch-direct-cfmws == 2`.

- [ ] **Step 4: Add invalid-topology qtests**

Test two targets, 512 MiB window, two global windows, sync disabled, and Type-2
behind a switch. Reads return zero plus `MEMTX_ERROR`; Type-2 writes return
`MEMTX_ERROR`. Preserve legacy silent invalid writes for unrelated missing or
invalid Type-3 routes.

- [ ] **Step 5: Run full CXL qtests**

```bash
ninja -C /tmp/slugarch-type2-transport-build/qemu tests/qtest/cxl-test
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 \
  /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test
```

Expected: all existing and new CXL cases pass.

- [ ] **Step 6: Stage only task hunks and commit**

```bash
git diff --check -- hw/cxl/cxl-host.c tests/unit tests/qtest/cxl-test.c
git diff --cached --check
git commit -m "cxl/type2: route one-target CFMWS accesses"
```

Expected: seven captured pre-existing user modifications remain unstaged.

## Task 2: Add and Test the Dynamic Rust Adapter

**Files:**
- Create: `include/hw/cxl/slugarch_jit.h`
- Create: `hw/cxl/slugarch_jit.c`
- Create: `tests/unit/slugarch-jit-fake.c`
- Create: `tests/unit/test-cxl-slugarch-jit.c`
- Modify: `hw/cxl/meson.build`
- Modify: `tests/unit/meson.build`

- [ ] **Step 1: Write the fake ABI module**

Export every required version-1 symbol. The fake supports policy strings:

```text
valid       -> load succeeds, deterministic digest, one record/event
reject      -> observe returns reject
drop        -> observe increments drop and returns strict error
panic       -> returns SLUG_JIT_ERR_PANIC
bad-digest  -> returns a digest different from policy info
```

It also exports an environment-selected incorrect ABI version for loader tests.

- [ ] **Step 2: Write failing loader tests**

Cover:

```c
g_assert_true(slugarch_jit_open(&state, valid_so, &error));
g_assert_cmpuint(state.abi_version, ==, 1);
g_assert_false(slugarch_jit_open(&state, missing_path, &error));
g_assert_nonnull(strstr(error_get_pretty(error), "dlopen"));
g_assert_false(slugarch_jit_open(&state, bad_abi_so, &error));
g_assert_nonnull(strstr(error_get_pretty(error), "ABI version"));
```

Also cover missing symbol, relative path, invalid policy, digest mismatch,
double close, null diagnostic buffer, strict drop, and event conversion.

- [ ] **Step 3: Confirm RED**

```bash
ninja -C /tmp/slugarch-type2-transport-build/qemu \
  tests/unit/test-cxl-slugarch-jit
```

Expected: missing adapter source/API.

- [ ] **Step 4: Implement the adapter**

`CXLType2JitState` owns:

```c
void *library;
void *handle;
char *library_path;
char *policy_path;
char *log_path;
uint32_t abi_version;
uint32_t requested_backend;
uint32_t selected_backend;
uint32_t status;
uint32_t last_error;
uint8_t policy_digest[32];
uint64_t next_event_id;
bool strict;
bool ready;
```

Resolve the complete function table into typed pointers before calling any
symbol. Require absolute paths, regular files, ABI 1, nonzero capabilities,
matching requested backend, successful create/load, and a 32-byte digest.
Close/destroy in reverse order on every partial failure.

- [ ] **Step 5: Implement canonical event conversion**

Use explicit field assignments and zero initialization. Reject payloads above
64 bytes and nonzero unused payload. `slugarch_jit_observe_event()` writes one
joinable JSONL entry with event ID, policy digest, decision, counters, and
phase.

- [ ] **Step 6: Run unit tests repeatedly**

```bash
for i in 1 2 3 4 5; do
  /tmp/slugarch-type2-transport-build/qemu/tests/unit/test-cxl-slugarch-jit \
    || exit 1
done
```

Expected: five passes, no leaked file descriptor or stale handle.

- [ ] **Step 7: Commit**

```bash
git add include/hw/cxl/slugarch_jit.h hw/cxl/slugarch_jit.c \
  hw/cxl/meson.build tests/unit/slugarch-jit-fake.c \
  tests/unit/test-cxl-slugarch-jit.c tests/unit/meson.build
git commit -m "cxl/type2: load the SlugArch JIT runtime"
```

## Task 3: Expose the BAR2 J-Extension and QOM State

**Files:**
- Modify: `include/hw/cxl/cxl_type2_gpu_cmd.h`
- Modify: `include/hw/cxl/cxl_type2.h`
- Modify: `hw/cxl/cxl_type2.c`
- Modify: `tests/qtest/cxl-test.c`

- [ ] **Step 1: Write the failing capability qtest**

With extension off, assert capability bit 5 is clear and J magic/status are
zero. With a valid fake Rust backend, assert:

```text
CAP bit 5 set
J_MAGIC 0x4a474c53 ("SLGJ" little endian)
J_ABI_VERSION 1
J_STATUS READY
J_BACKEND RUST
J_POLICY_BYTES nonzero
J_POLICY_DIGEST matches fake policy info
```

- [ ] **Step 2: Confirm RED**

Run:

```bash
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 \
  /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test \
  -p /pci/cxl/type2_jext_capability
```

Expected: missing property or capability.

- [ ] **Step 3: Add exact vendor ABI constants**

Define capability bit 5, registers `0x0400..0x0468`, commands `0xc0..0xc4`,
backend IDs, capability bits, status values, and stable extension error codes.
Mirror these values later in the guest header.

- [ ] **Step 4: Add properties and lifecycle**

Properties:

```text
slugarch-j-ext = off|auto|rust|gpu|fpga-verilator
slugarch-jit-lib = absolute path
slugarch-jit-policy = absolute path
slugarch-jit-log = absolute path
slugarch-jit-strict = bool
```

Default is off. Realize opens and verifies before setting ready/capability.
Unrealize stops accesses, flushes evidence, destroys handle, and closes the
library.

`auto` has one deterministic priority order and no fallback:

1. select GPU only when the existing Type-2 GPU backend is real, the CUDA
   library exports `cuModuleLoadDataEx`, and the J library reports
   `SLUG_JIT_CAP_GPU_DIAGNOSTIC`;
2. otherwise select FPGA-Verilator only when the J library reports
   `SLUG_JIT_CAP_FPGA_RTL` and its model initialization probe succeeds; and
3. otherwise fail realization with `SLUG_JIT_ERR_UNSUPPORTED`.

When both are eligible, GPU wins by the approved ordering. Rust is selected
only by explicit `slugarch-j-ext=rust`. An explicit backend verifies its
matching capability and never tries the next backend after failure.

- [ ] **Step 5: Add register and command handling**

Reads return immutable negotiated fields and current counters. Policy load:

1. requires ready and strict mode;
2. validates `PARAM0` length in `1..CXL_GPU_DATA_SIZE`;
3. copies the BAR2 buffer;
4. loads into an inactive Rust/backend policy;
5. requires digest/backend success;
6. atomically updates policy fields and epoch; and
7. clears the command buffer prefix.

Diagnostic retrieval copies at most the requested capacity and reports
required/written sizes in result registers.

- [ ] **Step 6: Expose QOM observability**

Add read-only:

```text
slugarch-jit-status
slugarch-jit-backend
slugarch-jit-policy-digest
slugarch-jit-event-count
slugarch-jit-record-count
slugarch-jit-metadata-bytes
slugarch-jit-reject-count
slugarch-jit-drop-count
slugarch-jit-epoch
slugarch-jit-last-error
```

- [ ] **Step 7: Run tests and commit**

```bash
ninja -C /tmp/slugarch-type2-transport-build/qemu qemu-system-x86_64 \
  tests/qtest/cxl-test
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 \
  /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test \
  -p /pci/cxl/type2_jext
git commit -m "cxl/type2: expose the SlugArch J-extension"
```

## Task 4: Record Direct CFMWS Events Fail-Closed

**Files:**
- Modify: `hw/cxl/cxl_type2.c`
- Modify: `tests/qtest/cxl-test.c`

- [ ] **Step 1: Write failing request/completion tests**

For one read and one write, require four canonical events:

```text
1 CxlMemRead  HostToDevice DPA=80MiB tag=request_id
2 CxlMemData  DeviceToHost DPA=80MiB tag=request_id
3 CxlMemWrite HostToDevice DPA=80MiB tag=request_id
4 Completion  DeviceToHost DPA=80MiB tag=request_id
```

Require event IDs 1..4, the same phase/policy digest, exact payload prefix,
record count 4, and zero drops.

- [ ] **Step 2: Confirm RED**

Run the focused qtest. Expected: zero J-extension events.

- [ ] **Step 3: Add request and completion hooks**

In synchronous access:

1. build/observe request before server send;
2. reject before server access if policy rejects;
3. perform the authoritative server transaction;
4. build/observe completion using returned data/status;
5. fail the guest access on completion-record error;
6. emit join fields with server request ID/sequence; and
7. only then return `MEMTX_OK`.

Preserve the server's committed write if a post-commit record fails, but return
`MEMTX_ERROR`, mark the boot invalid, and log `external_commit=true`.

- [ ] **Step 4: Add strict-failure qtests**

Fake modes reject and drop must return `MEMTX_ERROR`; capability remains
present but status becomes ERROR, last error is exact, and no later memory
operation succeeds in that boot.

- [ ] **Step 5: Run and commit**

```bash
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 \
  /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test \
  -p /pci/cxl/type2_jext_cfmws
git commit -m "cxl/type2: record CFMWS events through SlugArch JIT"
```

## Task 5: Capture Real CUDA JIT Diagnostics

**Files:**
- Modify: `include/hw/cxl/cxl_hetgpu.h`
- Modify: `hw/cxl/cxl_hetgpu.c`
- Modify: `hw/cxl/cxl_type2.c`
- Modify: `tests/qtest/cxl-test.c`

- [ ] **Step 1: Add a fake-CUDA failing test**

Build or use a qtest-local fake CUDA shared library exporting:

```text
cuModuleLoadDataEx valid PTX -> success, info "compiled test module"
cuModuleLoadDataEx malformed PTX -> CUDA_ERROR_INVALID_PTX,
                                    error "line 3: unexpected token"
```

Assert QEMU returns success/module handle for valid input and invalid PTX plus
nonempty J diagnostic for malformed input.

- [ ] **Step 2: Confirm RED**

Expected: QEMU resolves only `cuModuleLoadData` and diagnostic length is zero.

- [ ] **Step 3: Resolve the extended driver API**

Add a typed `cuModuleLoadDataEx` pointer and the stable CUDA option values:

```c
CU_JIT_WALL_TIME = 2,
CU_JIT_INFO_LOG_BUFFER = 3,
CU_JIT_INFO_LOG_BUFFER_SIZE_BYTES = 4,
CU_JIT_ERROR_LOG_BUFFER = 5,
CU_JIT_ERROR_LOG_BUFFER_SIZE_BYTES = 6,
```

Use fixed zeroed 16 KiB info and error buffers. The size variables passed in
`optionValues` are `unsigned int`. Do not pass stack pointers beyond the call.

- [ ] **Step 4: Record module boundaries**

Before compile, observe `PtxModuleLoad` request using payload length/digest and
event ID. After compile, observe completion with CUDA status, wall time, log
lengths, and the same tag. Full PTX is recorded only under an explicit full
policy.

- [ ] **Step 5: Preserve off-mode behavior**

When J-extension is off, prefer `cuModuleLoadDataEx` only for internal error
quality if available, but do not advertise J capability or write J evidence.
If the extended symbol is unavailable and J-extension is off, retain plain
`cuModuleLoadData`. If GPU J-extension is explicitly requested, missing
extended symbol fails realize.

- [ ] **Step 6: Run fake and real GPU tests**

```bash
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 \
  /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test \
  -p /pci/cxl/type2_jext_gpu_fake
nvidia-smi
```

If a real CUDA backend is available, run valid and malformed PTX under a fresh
context. Otherwise record real-GPU result as blocked.

- [ ] **Step 7: Commit**

```bash
git commit -m "cxl/type2: capture CUDA JIT diagnostics"
```

## Task 6: Add Guest Debug Access

**Files:**
- Modify: `qemu_integration/guest_libcuda/cxl_gpu_cmd.h`
- Modify: `qemu_integration/guest_libcuda/libcuda.c`
- Create: `qemu_integration/guest_libcuda/jext_debug.c`
- Modify: `qemu_integration/guest_libcuda/Makefile`

- [ ] **Step 1: Mirror the ABI with static assertions**

Add all J constants and assert no register exceeds `CXL_GPU_DATA_OFFSET`.

- [ ] **Step 2: Implement the CLI**

Commands:

```text
jext_debug query
jext_debug stats
jext_debug diagnostic
jext_debug load-policy FILE
```

`query` prints one JSON object with capability, ABI, state, backend, digest,
and counters. Exit 2 when the capability is absent; exit 3 on extension error.

- [ ] **Step 3: Build and run a no-device parser test**

```bash
make -C qemu_integration/guest_libcuda jext_debug
qemu_integration/guest_libcuda/jext_debug --self-test
```

Expected: `SLUGARCH_JEXT_GUEST_SELFTEST_PASS`.

- [ ] **Step 4: Commit**

```bash
git add qemu_integration/guest_libcuda
git commit -m "guest: expose SlugArch J-extension diagnostics"
```

## Task 7: Complete the QEMU Regression Matrix

**Files:**
- Modify: `tests/qtest/cxl-test.c`
- Modify: `tests/unit/test-cxl-slugarch-jit.c`

- [ ] **Step 1: Add the complete matrix**

Cases:

```text
extension off/absent
valid Rust backend
valid FPGA fake backend
valid GPU fake backend
auto GPU
auto FPGA-marked
auto unsupported
explicit backend missing
relative library/policy/log path
ABI mismatch
missing symbol
policy reject
digest mismatch
strict record drop
CFMWS four-event join
valid PTX diagnostics
invalid PTX diagnostics
device unrealize/close
Type-3 regressions
```

- [ ] **Step 2: Run focused tests 20 times**

```bash
for i in $(seq 1 20); do
  /tmp/slugarch-type2-transport-build/qemu/tests/unit/test-cxl-slugarch-jit \
    || exit 1
done
```

Expected: 20 passes.

- [ ] **Step 3: Run full CXL qtests**

```bash
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 \
  /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test
```

Expected: all cases pass.

- [ ] **Step 4: Run style checks on task patches**

```bash
scripts/checkpatch.pl --no-tree --terse /tmp/slugarch-jext-qemu.patch
git diff --check -- \
  hw/cxl/cxl-host.c hw/cxl/slugarch_jit.c \
  include/hw/cxl/slugarch_jit.h tests/unit tests/qtest/cxl-test.c
```

Expected: zero authored warnings and whitespace errors.

- [ ] **Step 5: Commit**

```bash
git commit -m "test: cover the SlugArch QEMU J-extension"
```

## Final Verification

- [ ] **Step 1: Build QEMU**

```bash
ninja -C /tmp/slugarch-type2-transport-build/qemu qemu-system-x86_64
```

Expected: success under the recorded GCC 15 workaround.

- [ ] **Step 2: Run unit and qtests**

```bash
/tmp/slugarch-type2-transport-build/qemu/tests/unit/test-cxl-type2-route
/tmp/slugarch-type2-transport-build/qemu/tests/unit/test-cxl-slugarch-jit
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 \
  /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test
```

Expected: all pass.

- [ ] **Step 3: Verify worktree ownership**

```bash
git status --short
git diff --cached --check
```

Expected: task commits are clean and only the seven recorded pre-existing user
files remain modified outside committed task hunks.
