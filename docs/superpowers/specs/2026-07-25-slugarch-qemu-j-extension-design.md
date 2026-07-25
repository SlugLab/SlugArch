# SlugArch QEMU J-Extension Design

## Status and Relationship to Existing Work

This document defines the approved dual GPU/FPGA SlugArch J-extension for the
CXLMemSim QEMU Type-2 device, its evaluation, and its paper integration.

It is additive to:

- `docs/superpowers/specs/2026-07-24-slugarch-type2-cxlmem-experiment-design.md`;
- `docs/superpowers/specs/2026-07-15-slugarch-paper-11-page-results-design.md`;
- the synchronous server-authoritative Type-2 wire implementation; and
- the one-target 256 MiB CFMWS routing work.

The existing CXL.mem implementation remains the first prerequisite. J-extension
timing is not admissible until the sentinel, server-counter, direct-path, and
five-independent-boot gates in the July 24 design pass.

This design does not reinterpret the July 4 BAR2 experiment as CXL.mem or
hardware-JIT evidence. It also does not equate NVIDIA PTX compilation with the
SlugArch replay-policy JIT. They are distinct capabilities joined by one
debugging interface.

## Objective

Add an explicit, vendor-specific SlugArch J-extension to QEMU's Type-2 device
that:

1. advertises the extension only when a usable implementation is present;
2. loads a verified, bounded replay policy through a stable Rust C ABI;
3. observes Type-2 boundary events and emits deterministic replay records;
4. selects a GPU or FPGA implementation without silent fallback;
5. exposes real GPU JIT diagnostics when the endpoint uses the CUDA driver;
6. executes the generated SlugArch Hardware-JIT RTL through Verilator for the
   FPGA debug path;
7. emits matching FPGA RTL for later synthesis without programming a board;
8. fails closed on unsupported policy, ABI, backend, or record behavior; and
9. produces carefully controlled results that may replace weaker material in
   the 11-page SlugArch paper.

## Verified Current State

The implementation starts from these repository facts:

- `crates/slugcxl-gen` is currently a Rust binary that emits a fixed
  `slugcxl_hj_pipeline.sv`, top wrappers, Quartus scaffolding, and a modeled
  overhead report.
- The generated Hardware-JIT pipeline is observational. It classifies the
  local 64-byte SlugCXL packet format, assigns GEMM epochs, hashes or captures
  payload metadata, and exposes counters.
- Its record mode, sample stride, FIFO depth, and record size are elaboration
  parameters, not runtime-installed bounded programs.
- `crates/slugarch-host/src/replay.rs` is a software boundary recorder for the
  same broad policy modes. It is not a stable FFI and does not currently expose
  a verifier, policy digest, or generic per-event API.
- `crates/slugarch-verilator-sys` builds `slugcxl_4x4_top`, not
  `slugcxl_4x4_hj_top`; the existing Rust wrapper therefore does not execute
  the generated Hardware-JIT block.
- The QEMU Type-2 backend dynamically resolves `cuModuleLoadData` and delegates
  PTX compilation to the host CUDA driver. It does not resolve
  `cuModuleLoadDataEx` and does not capture compiler information or error logs.
- `targets/qemu-type2/README.md` correctly describes the existing QEMU path as
  a simulator-backed replacement for FPGA Hardware-JIT evaluation.
- The paper describes runtime specialization into bounded endpoint-local
  programs as an architectural proposal. Its current evaluation text does not
  claim that the QEMU path executes those programs in hardware.

The implementation and revised paper must keep these facts visible.

## Terminology and Claim Boundaries

### SlugArch replay-policy JIT

The replay-policy JIT compiles a restricted policy into a verified controller
program. The program classifies boundary events, controls payload capture,
assigns epochs, and emits replay records. This is the mechanism described by
the SlugArch Hardware-JIT architecture.

### GPU native-code JIT

The GPU native-code JIT is the CUDA driver's PTX compiler reached through
`cuModuleLoadDataEx`. It compiles GPU kernels. It does not by itself install a
SlugArch replay policy at the CXL boundary.

The QEMU GPU path combines:

- the Rust replay-policy engine at the emulated Type-2 boundary; and
- CUDA JIT diagnostics for guest PTX module loads.

Results must report those two components separately.

### FPGA Hardware-JIT

The FPGA path executes the policy engine in generated RTL. During development,
QEMU reaches that RTL through a Verilated model. A generated or fitted design
is not evidence of execution on a physical FPGA.

The paper may call this:

- RTL-executed or Verilator-executed Hardware-JIT behavior, after equivalence
  tests pass; and
- synthesis or fit-harness cost, if a complete report is produced.

It must not call it measured FPGA runtime, link performance, board power, or
deployed CXL hardware without a separate live-board experiment.

## Architecture

### 1. One vendor-specific J-extension

The QEMU Type-2 BAR2 command interface gains a vendor capability named
`SLUGARCH_J_EXT`. It is not described as a standardized CXL capability.

Capability bit 5 in `CXL_GPU_REG_CAPS` is assigned to
`CXL_GPU_CAP_SLUGARCH_J_EXT`. The bit is set only after:

1. the Rust shared library loads;
2. ABI version negotiation succeeds;
3. the requested backend is available;
4. policy verification succeeds; and
5. the backend initializes.

The extension register block occupies the currently unused BAR2 range
`0x0400` through `0x04ff`:

| Offset | Width | Name | Meaning |
| ---: | ---: | --- | --- |
| `0x0400` | 4 | `J_MAGIC` | ASCII `SLGJ` |
| `0x0404` | 4 | `J_ABI_VERSION` | version 1 |
| `0x0408` | 4 | `J_CAPS` | policy, record, GPU-diagnostic, FPGA-RTL bits |
| `0x040c` | 4 | `J_STATUS` | disabled, loading, ready, error |
| `0x0410` | 4 | `J_BACKEND` | none, Rust reference, GPU, FPGA Verilator |
| `0x0414` | 4 | `J_POLICY_BYTES` | installed canonical policy length |
| `0x0418` | 4 | `J_LAST_ERROR` | stable extension error code |
| `0x0420` | 32 | `J_POLICY_DIGEST` | SHA-256 of canonical verified policy |
| `0x0440` | 8 | `J_RECORD_COUNT` | successfully emitted records |
| `0x0448` | 8 | `J_METADATA_BYTES` | record plus captured-payload bytes |
| `0x0450` | 8 | `J_EVENT_COUNT` | accepted boundary events |
| `0x0458` | 8 | `J_REJECT_COUNT` | rejected policies or events |
| `0x0460` | 8 | `J_DROP_COUNT` | records that could not be emitted |
| `0x0468` | 8 | `J_EPOCH` | current epoch identifier |

The existing 1 MiB BAR2 data buffer transports policy JSON and retrieves a
bounded diagnostic snapshot. New vendor commands use the unused `0xc0` range:

- `J_QUERY=0xc0`;
- `J_LOAD_POLICY=0xc1`;
- `J_RESET=0xc2`;
- `J_GET_STATS=0xc3`; and
- `J_GET_DIAGNOSTIC=0xc4`.

No guest-visible value may claim ready before backend initialization and policy
verification complete.

### 2. QEMU properties and backend selection

The Type-2 device gains:

- `slugarch-j-ext=off|auto|rust|gpu|fpga-verilator`, default `off`;
- `slugarch-jit-lib=<absolute path>`, required when enabled;
- `slugarch-jit-policy=<absolute path>`, required for boot-time loading;
- `slugarch-jit-log=<absolute path>`, required for evidence runs; and
- `slugarch-jit-strict=<bool>`, fixed to `true` in all paper runs.

`auto` uses explicit capabilities, not best-effort fallback:

1. a real CUDA backend with `cuModuleLoadDataEx` selects `gpu`;
2. an explicitly FPGA-marked endpoint with a compiled HJ model selects
   `fpga-verilator`;
3. a developer-only request may select `rust`; and
4. otherwise realization fails.

An explicitly requested backend that cannot initialize always fails device
realization. It never changes to simulation, Rust, or another device silently.

### 3. Rust core and stable C ABI

Add two crates:

- `slugarch-jit`: policy schema, canonicalization, verifier, interpreter,
  record encoder, policy digest, and backend-neutral event types;
- `slugarch-jit-ffi`: `cdylib`/`staticlib` C ABI and optional
  `fpga-verilator` feature.

Refactor `slugcxl-gen` into a library plus its existing binary. Both the
software interpreter and SystemVerilog emitter consume the same
`VerifiedPolicy` representation. Configuration constants must no longer be
duplicated across `slugarch-host`, `slugcxl-gen`, and generated RTL.

ABI version 1 exposes opaque handles and fixed-width structures:

```c
uint32_t slugarch_jit_abi_version(void);
uint64_t slugarch_jit_backend_caps(void);

int32_t slugarch_jit_create(const SlugJitCreateArgs *args,
                            SlugJitHandle **out);
int32_t slugarch_jit_load_policy(SlugJitHandle *handle,
                                 const uint8_t *json,
                                 uint32_t json_len,
                                 SlugJitPolicyInfo *out);
int32_t slugarch_jit_observe(SlugJitHandle *handle,
                             const SlugJitEvent *event,
                             SlugJitDecision *out);
int32_t slugarch_jit_stats(SlugJitHandle *handle,
                           SlugJitStats *out);
int32_t slugarch_jit_last_diagnostic(SlugJitHandle *handle,
                                     uint8_t *out,
                                     uint32_t capacity,
                                     uint32_t *written);
void slugarch_jit_destroy(SlugJitHandle *handle);
```

Rules:

- QEMU-owned pointers are valid only for the duration of one call.
- Rust retains no guest, QEMU, or CUDA pointer.
- All structures carry `struct_size` and `abi_version`.
- Unknown trailing fields are ignored only when the minimum known size is
  present.
- Every exported function catches Rust panics and returns a stable error.
- Error messages are bounded UTF-8 diagnostic data, not ownership-bearing
  strings.
- One handle belongs to one QEMU Type-2 device and is internally serialized.
- The ABI has C and Rust layout tests plus a C smoke executable.

The QEMU build does not directly compile Rust. It loads
`libslugarch_jit.so` with `dlopen`, resolves the complete version-1 symbol set,
and fails closed if any required symbol is absent. This keeps QEMU's normal
build usable when the extension is off.

### 4. Canonical event boundary

QEMU normalizes each observed operation into a fixed event:

- monotonically increasing event ID;
- client/device ID;
- direction;
- event class;
- opcode;
- CXL.mem DPA or command address;
- payload length from zero through 64 bytes;
- payload bytes;
- request/completion tag;
- phase ID;
- QEMU monotonic timestamp; and
- completion status.

Version 1 recognizes:

- CXL.mem read request and data response;
- CXL.mem write request and completion;
- PTX module-load request and completion;
- kernel-launch request and completion; and
- explicit phase/fence events.

The custom 64-byte SlugCXL packet used by the current RTL remains a local
encoding. The Rust layer performs the mapping from canonical events and records
the encoding version. The paper must not call it a CXL 2.0 or CXL 3.0 FLIT.

For a synchronous CFMWS operation, the order is:

1. create and accept the request event;
2. perform the server-authoritative SlugArch Type-2 transaction;
3. create and accept the completion event;
4. commit both records and counters; and
5. return the memory completion to the guest.

If strict recording fails at any step, QEMU returns `MEMTX_ERROR`, increments
the failure counter, and invalidates that boot. A write is never reported as
complete if its required record was dropped.

### 5. Restricted policy program

Policy JSON is canonicalized before hashing and compiled to a version-1
bounded program. Version 1 supports:

- event-class and opcode matches;
- one of four configured address ranges;
- direction and status matches;
- fixed sampling stride;
- validation, delta, or full payload capture;
- FNV-1a-64 payload commitment for RTL equivalence;
- record emission;
- epoch increment or assignment;
- fail-stop rejection; and
- termination.

Version 1 limits are:

- at most 32 instructions;
- at most four address ranges;
- no backward branch and therefore no loop;
- at most one record per input event;
- at most 64 captured payload bytes;
- at most 256 output metadata bytes;
- fixed 64-bit counters;
- no dynamic allocation in the event path; and
- no access outside the event, policy constants, and private controller state.

The verifier checks:

- opcode and branch validity;
- forward-only control flow and guaranteed termination;
- range-table bounds;
- capture and output bounds;
- metadata budget;
- allowed event classes;
- endpoint/backend compatibility; and
- canonical policy digest.

The accepted policy digest is SHA-256 over canonical JSON plus ABI version,
event-schema version, packet-encoding version, and backend contract version.

### 6. GPU backend

The GPU path has two deliberately separate layers.

The SlugArch layer executes the verified replay policy through the Rust engine
on the QEMU boundary events. It emits the same canonical records used by other
backends.

The native GPU layer:

- resolves `cuModuleLoadDataEx`;
- supplies `CU_JIT_INFO_LOG_BUFFER`, `CU_JIT_ERROR_LOG_BUFFER`, and their
  bounded sizes;
- records compile result, wall-clock duration, driver error name, information
  log, and error log;
- associates the diagnostic with the module-load event ID and policy digest;
- never logs the complete PTX payload unless the explicit full-capture policy
  permits it; and
- preserves the existing CUDA module handle behavior on success.

A malformed PTX module must yield a nonempty bounded error diagnostic and a
failed module-load completion. A valid PTX module must produce a usable module
and successful completion. GPU compile latency is reported as native PTX JIT
latency, not SlugArch policy-install latency.

### 7. FPGA Verilator and RTL backend

Update `slugcxl_hj_pipeline.sv` from elaboration-only policy constants to a
small runtime-loadable policy store and interpreter matching the version-1
instruction set.

The generated module adds:

- policy-load valid/ready/data signals;
- explicit load begin, commit, and abort controls;
- policy digest input;
- verifier-approved instruction count;
- active/inactive program banks for atomic installation;
- compatibility/version registers;
- a sticky policy or event error code; and
- the existing metadata, epoch, and accounting outputs.

Only a Rust-verified program can be committed. The RTL still checks the program
header, version, length, and digest framing before bank swap. A partial load,
bad digest, unsupported instruction, or reset keeps the prior program inactive
and reports error.

`crates/slugarch-verilator-sys` gains a separate
`slugcxl_4x4_hj_top` compile unit and C shim. The Rust FPGA backend:

1. resets the model;
2. streams the verified program into the inactive bank;
3. commits it;
4. converts canonical events to the local 64-byte encoding;
5. ticks until acceptance and any record/completion, subject to a fixed cycle
   deadline;
6. retrieves record, epoch, and counter state; and
7. returns a fail-stop error on timeout or mismatch.

The Rust reference interpreter remains an oracle. Every supported policy and
event stream used in evaluation must produce equivalent semantic records in
the interpreter and Verilated RTL. A difference blocks timing experiments and
paper integration.

`slugcxl-gen --hj` continues to emit:

- programmable HJ pipeline RTL;
- endpoint/HJ top;
- fit harness;
- runtime contract JSON;
- default verified policy image; and
- modeled accounting metadata.

Generating RTL or a Quartus report does not authorize programming an FPGA.

### 8. QEMU lifecycle and locking

QEMU creates the Rust handle during Type-2 realization and destroys it during
unrealize after all memory transactions stop.

Lock order is:

1. QEMU Type-2 transaction lock;
2. Rust handle lock;
3. backend-local Verilator or CUDA lock.

No callback may acquire these locks in reverse order. The Rust library does not
call back into QEMU.

Policy replacement is atomic:

- state changes from ready to loading;
- new events either continue under the old policy or wait at an explicit
  phase boundary;
- verification and backend installation occur in an inactive slot;
- commit changes the digest and epoch together;
- a failed load preserves the old policy and emits an error event.

Paper runs load one policy at boot and do not replace it during a measured
phase.

## Evidence and Logging Contract

Each boot produces append-only JSONL with:

- run and boot UUIDs;
- QEMU, CXLMemSim, SlugArch, policy, and shared-library hashes;
- requested and selected backend;
- ABI and event-schema versions;
- capability advertisement state;
- policy digest and verification result;
- every normalized event and its semantic record digest;
- backend record, epoch, drop, reject, and metadata counters;
- GPU compiler diagnostics where applicable;
- Verilator cycle counts where applicable;
- server-authoritative CXL.mem request IDs and sequence numbers;
- phase IDs and monotonic timestamps; and
- final pass, fail, or blocked reason.

Large payloads are represented by length and digest unless full capture was
explicitly selected. Evidence logging failure is fatal in strict mode.

## Experimental Design

### Research questions

The J-extension evaluation answers:

1. **J1, semantic correctness:** Do Rust and Verilated FPGA implementations
   emit equivalent records, epochs, payload commitments, and failure outcomes
   for the same supported policies and event streams?
2. **J2, capability and fail-stop behavior:** Does QEMU advertise the extension
   only for an initialized backend and reject bad ABI, policy, backend, digest,
   PTX, and RTL timeout cases?
3. **J3, debug utility:** Does the GPU path return actionable native compiler
   diagnostics for invalid PTX while preserving successful valid-PTX loading?
4. **J4, simulator overhead:** What host and guest-visible overhead does the
   J-extension add to the validated Type-2 CXL.mem path at 80 ns, 400 ns,
   2 microseconds, and 10 microseconds?
5. **J5, implementation cost:** What Verilator cycle cost, metadata volume, and
   optionally post-synthesis or fit-harness resource cost does the FPGA
   policy engine add?

### Correctness gates before timing

Timing is blocked unless all of these pass:

- Rust policy parser, canonicalizer, verifier, interpreter, and FFI tests;
- C ABI layout and panic-containment tests;
- generated RTL snapshot and lint tests;
- Rust-versus-Verilator equivalence for validation, delta, and full modes;
- valid and invalid policy tests;
- valid and malformed PTX tests on a real CUDA backend for GPU results;
- QEMU capability present/absent and no-fallback qtests;
- direct 80 MiB Type-2 CFMWS sentinel read/write with exact server counters;
- zero record drops and zero unclassified required events;
- exact policy digest agreement in QEMU, Rust, RTL, and artifacts; and
- all pre-existing CXL Type-3 and Type-2 qtests remain green.

### Functional corpus

The deterministic corpus contains:

- the existing 98-event 4x4 GEMM request/response trace;
- direct CFMWS read and write events at DPA 80 MiB;
- zero, sparse, and full 64-byte payloads;
- address-range boundary events;
- explicit phase and fence events;
- one reordered completion;
- one tag mismatch;
- one payload-bit corruption;
- one unsupported event class;
- truncated, oversized, cyclic, out-of-range, and over-budget policies; and
- valid and deliberately malformed PTX modules.

Every injected fault has one expected first-failure code. A generic later
failure is not a substitute.

### Timing matrix

The CXL.mem timing matrix uses:

- configured latency: 80 ns, 400 ns, 2 microseconds, 10 microseconds;
- J-extension: off, Rust reference, FPGA Verilator;
- record mode: validation, delta, full;
- workload: dependent 8-byte load and bounded 64-byte transfer phase;
- five independent QEMU boots per cell;
- one fresh server instance and event log per boot; and
- median with minimum and maximum.

To keep the matrix tractable, the full cross-product is used for validation
mode. Delta and full modes are measured at 400 ns and 2 microseconds, where
payload work is visible without the extremes dominating. The fixed raw
manifest records every included and excluded cell before execution.

Within each boot:

1. verify capability, policy digest, and backend;
2. run one unmeasured correctness warm-up;
3. bracket each measured phase with server and J-extension counter snapshots;
4. run a fixed operation count;
5. validate the returned sentinel and exact event counts;
6. fsync and close evidence; and
7. mark the boot invalid on any mismatch, timeout, drop, or diagnostic error.

Boot order is deterministically shuffled from a recorded seed. CPU affinity,
QEMU accelerator, host governor, CUDA driver/device identity, Rust/QEMU
binaries, and server hashes are recorded. Invalid boots are reported and
rerun only under a new boot UUID; they are never silently replaced.

### GPU JIT experiment

On a real CUDA backend, five fresh QEMU/device contexts compile:

- one minimal valid PTX module;
- the existing vector-add PTX module;
- the existing GEMM PTX module; and
- one malformed PTX module.

Report:

- success/failure;
- compile duration;
- information-log bytes;
- error-log bytes;
- stable error code/name;
- module-load event and completion records; and
- whether the diagnostic identifies the injected syntax failure.

Cold-context and same-context repeat measurements are separate. They are not
mixed into one distribution.

### FPGA experiment

The FPGA debug result reports:

- Rust-versus-Verilator semantic equivalence;
- cycles from event valid to acceptance;
- cycles from acceptance to metadata valid;
- policy-load and atomic-commit cycles;
- record count, metadata bytes, epoch count, and drop count; and
- behavior under backpressure and timeout injection.

If Quartus can synthesize or fit the generated harness with a complete,
identified target and no fatal constraint warnings, archive its reports and
report ALMs/LUTs, registers, memory bits/blocks, achieved Fmax, and build
version. Otherwise the resource result remains explicitly blocked. A fit
harness is reported as a fit harness, not a board-ready CXL endpoint.

### Analysis

Primary derived quantities are:

- Rust/RTL record mismatch count;
- record and epoch coverage;
- extension-added median latency relative to same-configuration off;
- metadata bytes per application GiB;
- policy-load time;
- native GPU compile time;
- Verilator event and record cycles; and
- resource delta relative to the same generated endpoint without HJ, if both
  comparable reports exist.

With five boots, the paper shows medians and min/max whiskers. It does not claim
normality, confidence intervals, or statistical significance. Raw per-boot
values remain machine-readable.

## Paper Integration

Paper changes occur only after the correctness gates and artifact audit pass.

The main paper remains no longer than 11 pages before references. The
J-extension result replaces weaker simulator-only material; it does not add an
unbudgeted page.

The preferred result is one full-width four-panel figure:

1. CXL.mem latency response with J-extension off and validation mode;
2. extension-added overhead for Rust and FPGA Verilator across the latency
   grid;
3. policy/record equivalence and injected-fault detection; and
4. GPU native JIT diagnostics plus FPGA cycle/resource cost, with blocked
   hardware fields visibly marked when unavailable.

The text must state:

- which values are guest-observed, host-observed, server-modeled, or
  Verilator-cycle results;
- that GPU PTX JIT is not the SlugArch replay-policy JIT;
- that the FPGA result is Verilator or fit-harness evidence unless a board run
  occurs;
- that the local 64-byte SlugCXL encoding is not standards-compliant CXL FLIT
  evidence;
- that the legacy BAR2 experiment remains a separate software boundary result;
  and
- which broader CXL.cache, DMA, ATS, migration, switch-ordering, recovery,
  physical-FPGA, power, and production-security claims remain blocked.

No number enters LaTeX manually. Plot and table inputs come from audited JSON
summaries derived from raw artifacts. The paper checker verifies that every
printed value has a source key and that the conclusion remains on or before
page 11.

## Implementation Sequence

1. Finish and commit the one-target Type-2 CFMWS routing proof.
2. Add the Rust policy core with verifier/interpreter tests.
3. Add the stable FFI and C ABI tests.
4. Refactor `slugcxl-gen` to consume the verified policy representation.
5. Make the HJ RTL runtime-programmable and prove Rust/RTL equivalence.
6. Add the Verilated HJ top and Rust FPGA backend.
7. Add QEMU dynamic loading, lifecycle, capability, registers, commands, and
   fail-closed qtests.
8. Add GPU `cuModuleLoadDataEx` diagnostics and tests.
9. Run the functional and fault-injection gates.
10. Run the predeclared five-boot timing campaign.
11. Audit artifacts, generate figures, update the claim ledger, and revise the
    paper within the 11-page budget.

Each step gets a focused commit. Existing user modifications in the nested
QEMU checkout remain unstaged unless the step intentionally overlaps a
required hunk; overlapping changes use hunk-level staging and a cached-diff
review.

## Acceptance Criteria

The work is complete only when:

- the J-extension capability is absent when disabled or unavailable;
- explicit GPU and FPGA backend requests never fall back silently;
- the same verified policy digest is visible in Rust, QEMU, RTL, and evidence;
- supported Rust and FPGA-Verilator executions are semantically equivalent;
- valid GPU PTX loads and malformed PTX produces a useful compiler diagnostic;
- strict-mode record failures stop the affected CXL.mem or command completion;
- the original CXL.mem sentinel and exact server-counter proof remains green;
- five independent boots complete for every predeclared timing cell;
- all raw records, manifests, summaries, hashes, and plotting inputs are
  archived;
- any unavailable physical-hardware evidence remains marked blocked;
- the paper uses only audited results and separates all proof levels; and
- the main paper ends no later than page 11 before references.

## Non-Goals

This pass does not:

- standardize a CXL capability;
- claim the CUDA PTX compiler is itself the SlugArch policy engine;
- implement transparent interception inside a physical GPU;
- make the local SlugCXL packet encoding standards compliant;
- program an FPGA board;
- claim physical CXL latency, bandwidth, power, or FPGA runtime;
- evaluate CXL.cache coherence, DMA, ATS, migration, or switch ordering; or
- weaken the fail-stop or evidence requirements to obtain a complete plot.
