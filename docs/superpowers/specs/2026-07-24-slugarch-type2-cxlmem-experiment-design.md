# SlugArch Type-2 CXL.mem Calibration and Evaluation Design

## Status and Relationship to the July 15 Design

This document is an additive experiment specification for the SlugArch paper.
It extends
`docs/superpowers/specs/2026-07-15-slugarch-paper-11-page-results-design.md`.

The July 15 design remains authoritative for:

- the global CFR-to-SlugArch rename;
- the corrected legacy BAR2 claim boundary;
- the July 4 software footprint and validator measurements;
- the paper build repair;
- the formal-semantics compression; and
- the requirement that the main text end no later than page 11.

This document supersedes only the July 15 clauses that say no new measurements
will be run, that CXL.mem/DAX must remain unmeasured, that evaluation has only
2.5 pages, that design and semantics each retain 2.5 pages, and that the result
presentation contains three plus one panels. The complete July 15 page-budget
table is replaced once by the table in this document; its earlier compression
savings are not counted a second time. Those clauses are replaced by the
experiment, evidence, figure, and page-budget rules below. Nothing in this
document upgrades the legacy BAR2 experiment into a CXL.mem result.

## Objective

Build and validate a reproducible QEMU Type-2 CXL.mem path backed by
CXLMemSim, then use that path to answer two bounded questions:

1. Does a configured CXLMemSim latency setting produce a measurable,
   monotonic response in guest-observed dependent-load time and transfer time?
2. On that same simulated substrate, what is the paired cost of SlugArch
   validation, delta, and full records relative to a same-boot common-path
   baseline?

If, and only if, the evidence gates in this specification pass, integrate the
calibration and SlugArch results into a second full-width four-panel figure in
the 11-page paper. The experiment is a simulator evaluation. It is not a
hardware-performance result and is not evidence for device-side SlugArch
execution.

## Fixed External Methodology Guidance

The only methodology imported from
`/root/Concordia/sigmetrics27summer-paper399.pdf` is:

- the latency grid of approximately 80 ns, 400 ns, 2 microseconds, and
  10 microseconds; and
- five repetitions summarized by the median with minimum and maximum.

The reference file has SHA-256
`d6a90335b27188e2623f4cda2ee4da2639715cb15f0dc2f6a3da80c96ecd5e8f`
and is titled *Quantifying Synchronization Costs Across CXL Memory Access
Modes*. It is an anonymous 21-page ACM-format manuscript with placeholder
publication metadata. The SlugArch paper must call it a reference manuscript,
not an accepted or published SIGMETRICS paper.

None of its absolute performance values, percentage differences, crossover
points, bandwidth claims, or platform claims may be overlaid with or used as a
quantitative baseline for SlugArch. Its access modes and workload are not the
SlugArch/CXLMemSim replay path, and its reported methodology contains internal
inconsistencies. The paper may state only that the latency points and
five-repeat reporting convention motivated this experiment grid.

## Current Evidence Boundary and Required Repairs

### Legacy BAR2 Path

The July 4 experiment remains a separate result. For each 64-byte request, the
guest helper writes the request to BAR2, reads it back, and issues a NOP. Guest
software then interprets the request and produces the response file for the
host validator. No response FLIT traverses BAR2. Those five back-to-back runs
occurred in one TCG guest boot, and the CXLMemSim server reported zero memory
reads and writes.

### Type-2 TCP Protocol Mismatch

The current QEMU Type-2 device writes `CXLType2Message` objects and starts a
receive thread that also expects `CXLType2Message`. The current CXLMemSim TCP
server instead reads packed `ServerRequest` objects and returns
`ServerResponse` objects. The layouts and request/response semantics differ.
Consequently, a successful socket connection is not proof of a valid memory
operation.

The existing Type-2 read and write handlers also update or consult QEMU-local
memory before sending an asynchronous notification. They do not make server
data authoritative, do not synchronously consume the server response, and do
not apply the returned `latency_ns` to the guest-visible access.

### CFMWS Routing Gap

The current QEMU CXL fixed-memory-window dispatcher accepts only
`TYPE_CXL_TYPE3` endpoints. A Type-2 device can enumerate, and the patched
guest kernel can register a Type-2 memdev and devdax region, without proving
that a load from the CXL fixed memory window reaches the Type-2 device-memory
handler. The implementation must add an explicit one-target Type-2 CFMWS
dispatch path and prove it with an end-to-end sentinel exchange. Merely mapping
PCI BAR4 is not sufficient for a CXL.mem claim.

### Local-RAM Bypass

The QEMU Type-2 model installs two priority-2 RAM subregions over its BAR4
device-memory handler:

- a bulk-staging region from offset 0 through 64 MiB; and
- a coherent-pool region at the top of device memory.

The July launch used `mem-size=64M`, so the complete device-memory aperture was
inside the bulk-staging bypass. An experiment using that configuration could
not exercise the slow Type-2 memory handler or its TCP path.

### Latency Semantics

The CXLMemSim server currently calculates and returns `latency_ns`; it does not
sleep for that duration. The experiment must distinguish:

- configured base latency;
- server-reported modeled latency;
- QEMU-applied delay; and
- guest-observed elapsed time.

The result is a calibrated simulator delay layered on the synchronous
QEMU-to-server RPC. It is not a measurement of physical CXL link latency.

## Architecture

### 1. Versioned Synchronous Type-2 Wire Protocol

Add a dedicated Type-2 wire-protocol mode without changing the default legacy
server protocol. The benchmark server is launched in this mode, and the QEMU
Type-2 device enables it through an explicit device property. Both defaults
remain off.

The protocol has these properties:

- fixed-width, little-endian fields;
- four-byte magic `SLT2` and protocol version 1 in every frame;
- an initial client hello and server acknowledgement;
- explicit frame type, frame length, request ID, client role, and client ID;
- READ and WRITE operations with a DPA offset and a length from 1 through
  64 bytes;
- an oracle-only `COUNTER_SNAPSHOT` control operation that never touches
  memory;
- a 64-byte payload area;
- response status, matching request ID, server sequence number,
  server-reported modeled latency, returned length, and read data;
- CRC32C over each complete wire frame and SHA-256 payload digests in the
  evidence log;
- exact-length reads and writes that handle short I/O and interruption;
- a five-second per-request timeout; and
- a clean disconnect on magic, version, length, opcode, request-ID, checksum,
  or status mismatch.

The normative 40-byte header is:

| Offset | Width | Field | Rule |
| ---: | ---: | --- | --- |
| 0 | 4 | magic | ASCII `SLT2` |
| 4 | 2 | version | unsigned little-endian value 1 |
| 6 | 2 | frame type | `HELLO=1`, `ACK=2`, `READ=3`, `WRITE=4`, `MEMORY_RESPONSE=5`, `COUNTER_SNAPSHOT=6`, `COUNTER_RESPONSE=7`, `ERROR=255` |
| 8 | 4 | frame length | total header plus body bytes; maximum 128 |
| 12 | 4 | flags | zero in version 1 |
| 16 | 8 | request ID | starts at 1 and increases monotonically per connection without reuse |
| 24 | 8 | client ID | zero in `HELLO`; server-assigned nonzero value thereafter |
| 32 | 4 | CRC32C | CRC-32C/iSCSI over the complete frame with these four bytes zeroed |
| 36 | 4 | reserved | zero |

CRC-32C uses polynomial `0x1EDC6F41` (reflected implementation
`0x82F63B78`), initial value `0xffffffff`, reflected input/output, and final
XOR `0xffffffff`. Multi-byte body fields are also unsigned little-endian.
Unused payload and reserved bytes are zero and are covered by the CRC.

The fixed bodies are:

| Frame | Body layout | Total bytes |
| --- | --- | ---: |
| `HELLO` | role `u32` (`QEMU=1`, `ORACLE=2`), capabilities `u32` (zero), 16-byte client nonce, maximum-frame `u32`, reserved `u32` | 72 |
| `ACK` | status `u32`, capabilities `u32` (zero), 16-byte server-instance UUID, capacity `u64`, configured base latency `u64`, maximum-frame `u32`, reserved `u32` | 88 |
| `READ` or `WRITE` | length `u32`, reserved `u32`, DPA `u64`, client monotonic timestamp `u64`, 64-byte data | 128 |
| `MEMORY_RESPONSE` | status `u32`, returned length `u32`, server sequence `u64`, modeled latency `u64`, 64-byte data | 128 |
| `COUNTER_SNAPSHOT` | target client ID `u64` | 48 |
| `COUNTER_RESPONSE` | status `u32`, reserved `u32`, target client ID `u64`, completed reads `u64`, completed writes `u64`, read bytes `u64`, written bytes `u64`, failed requests `u64`, in-flight requests `u64`, modeled-latency sum `u64` | 112 |
| `ERROR` | status `u32`, reason code `u32`, related request ID `u64` | 56 |

A `READ` carries zero data. A `WRITE` carries valid bytes only in
`data[0:length]`. A successful memory response returns the same length; a read
returns valid data in that prefix, while a write returns a zero data area.
Status values are `SUCCESS=0`, `PROTOCOL=1`, `RANGE=2`, `CHECKSUM=3`,
`BACKEND=4`, and `UNSUPPORTED=5`. Server sequence numbers start at 1 and
increase for every completed memory request in one server instance. Both peers
require maximum-frame 128 in the handshake.
`COUNTER_SNAPSHOT` is accepted only from an oracle connection. The QEMU client
ID needed for a snapshot is taken from QEMU's validated `ACK` log. All receive
and send loops use an absolute `CLOCK_MONOTONIC` deadline five seconds after
the operation starts; any incomplete frame at that deadline fails the
connection and the boot.

The QEMU connection uses its existing mutex to serialize the complete
request/reply transaction. The asynchronous receive thread is not started in
this mode, because it would race the synchronous response reader. Unsolicited
snoop and invalidation messages are outside this protocol version and outside
the experiment.

The server records one gzip-compressed JSONL completion event for each
completed request/response pair and a separate error event for every rejected
or failed request. Each event includes server instance ID, client role and ID,
request ID, server sequence number, operation, DPA offset, length, data
checksum, status, configured base latency, modeled latency, and monotonic
timestamps. It also maintains per-client completed-read, completed-write,
failed-request, and modeled-latency counters. Counter snapshots are
non-destructive and monotonically increasing; phase counts are computed by
subtracting the snapshots bracketing that phase.

### 2. Server-Authoritative Type-2 Memory

When synchronous mode is enabled:

- every measured Type-2 CXL.mem READ is sent to the server;
- returned server bytes are the value supplied to the guest;
- every WRITE completes on the server before QEMU reports success;
- QEMU updates an optional local shadow only after a successful server
  response;
- a failed or timed-out request returns a memory transaction error and marks
  the boot invalid; and
- local QEMU cache hits cannot satisfy a measured access without a server RPC.

Legacy asynchronous cache/coherency behavior remains available only when the
new mode is disabled. This experiment does not evaluate that legacy behavior.

### 3. Type-2 CFMWS Dispatch

Extend the CXL fixed-memory-window dispatcher to recognize the benchmark's
single Type-2 endpoint and call exported Type-2 memory read/write entry points.
The supported experiment topology is exactly:

- one CXL fixed memory window;
- one host bridge target;
- one root port;
- one Type-2 endpoint;
- one-way interleave; and
- a zero-based 256 MiB DPA range.

Correct the Type-2 Device DVSEC so its cache-size capability is not also
advertised or counted as a memory range. The benchmark endpoint exposes
exactly one active, zero-based 256 MiB memory range.

The dispatcher passes the CFMWS-relative offset as the Type-2 DPA offset and
invokes the synchronous, server-authoritative Type-2 handler directly. It
never dispatches a CFMWS access through PCI BAR4 or QEMU's priority-resolved
BAR4 address space. If implementation code is shared with the BAR4 slow
handler, a runtime path counter must still prove that the direct synchronous
entry point was used and that no RAM overlay or local cache completed the
access.

The dispatcher must reject multiple targets, unsupported interleave, a nonzero
DPA base, out-of-range access, overflow, and a device whose advertised range
is not 256 MiB. The paper must describe this as a one-endpoint QEMU Type-2
CXL.mem model, not general Type-2 switch or interleave support.

### 4. Non-Bypass Memory Layout

Launch the Type-2 endpoint and CXLMemSim server with 256 MiB of memory. Use
DPA offsets 80 MiB through 112 MiB as the 32 MiB experiment work window.

With the current device layout, 256 MiB produces:

- bulk-staging bypass: `[0 MiB, 64 MiB)`;
- measured slow window: `[80 MiB, 112 MiB)`;
- remaining slow-handler space through 192 MiB; and
- coherent-pool bypass: `[192 MiB, 256 MiB)`.

The harness must read the realized bulk-staging size, coherent-pool base and
size, device-memory size, CFMWS base and size, and DAX resource range from
runtime logs/sysfs. It then computes the DPA interval rather than trusting the
values above. Although CFMWS dispatch is direct, these checks defend against a
future shared-handler regression. The run aborts unless the entire 32 MiB work
window is:

- inside the realized Type-2 DPA capacity;
- reachable through the CFMWS and devdax mapping;
- above the realized bulk-staging end; and
- below the realized coherent-pool base.

No benchmark result may be collected through PCI `resource4` or a direct BAR4
mapping. Per-phase counters for BAR4 RAM-overlay completions, local-shadow
completions, and local-cache completions must all remain zero.

### 5. Applying and Observing Modeled Latency

After a successful synchronous response, QEMU applies the response's
`latency_ns` by spinning against `CLOCK_MONOTONIC_RAW` in the device access
path. The routine rejects a requested delay above one millisecond, records the
requested delay, actual wall-clock delay, undershoot count, and overshoot. It
never subtracts TCP or host scheduling time; therefore guest-observed time
includes RPC time plus the modeled device delay.

The 80 ns point is the lowest configured model point, not a claim that the
guest observes an 80 ns absolute load. Sub-microsecond host timing may be
dominated by RPC, scheduling, and delay-loop resolution. Calibration reports
that behavior rather than silently treating configured and observed latency as
equivalent.

The primary calibration plot shows guest-observed time per dependent load at
the four configured points. The artifact also retains server-reported and
QEMU-applied delay distributions. The paper does not claim the reference
manuscript's reported three-percent delay accuracy. The configured value is
the server's base setting; if the server model returns different read and write
latencies or adds congestion terms, the paper reports those realized values
instead of treating the configured base as the latency of every operation.

For every successful memory response, QEMU emits exactly one delay event keyed
by client ID, request ID, and server sequence number. Validation performs a
one-to-one join with the server completion event and requires the QEMU
requested delay to equal the returned modeled latency, with zero missing,
duplicate, or undershot delay applications. Overshoot is retained
descriptively. A stale, omitted, duplicated, or altered delay event invalidates
the boot even if aggregate guest timing appears monotonic.

### 6. Guest and Host Components

The guest benchmark binary performs:

- topology and DAX preflight;
- a server-authoritative sentinel exchange;
- dependent-load calibration;
- sequential transfer measurements;
- SlugArch baseline and policy measurements;
- post-transport guest-software corruption; and
- machine-readable JSONL output with stage timings and checksums.

The guest uses `CLOCK_MONOTONIC_RAW` for elapsed time and executes a full
compiler and CPU fence around timed ranges. It maps the selected devdax device
with a shared DAX mapping and accesses only the validated work-window offset.

The host harness:

- launches one fresh server and one fresh guest per repetition;
- copies the benchmark into the guest;
- controls the single untimed warmup and timed pass;
- collects all raw artifacts;
- validates counters, pairings, hashes, and topology; and
- creates immutable per-boot and campaign summaries.

A separate host oracle client uses the same versioned protocol with role
`oracle`. Its traffic is counted separately and is never included in benchmark
metrics.

## Mandatory End-to-End Preflight

Every boot must pass all of these checks before its warmup:

1. The expected QEMU and server binaries, guest image, kernel image, benchmark
   binary, and trace file exist, and their SHA-256 hashes are recorded.
2. The server acknowledges the exact wire-protocol version and reports a
   256 MiB capacity and the requested base latency.
3. QEMU reports the expected one-target CFMWS, the Type-2 endpoint, synchronous
   mode, the non-bypass work window, and a successful server handshake.
4. The guest reports the expected `8086:0d92` endpoint, `mem0`-class Type-2
   memdev, committed region, devdax device, and a mapping that covers the
   computed work-window offset.
5. The server's QEMU-client memory counters are zero before the sentinel
   exchange.
6. The oracle writes a deterministic 64-byte sentinel to the first cache line
   of the work window.
7. The guest reads that sentinel through devdax and verifies its checksum.
8. The guest writes a distinct deterministic inverse sentinel through devdax.
9. The oracle reads the inverse sentinel and verifies its checksum.
10. QEMU and server logs contain matching request IDs, server sequence numbers,
    operation types, DPA offsets, lengths, and payload checksums for both guest
    operations.
11. The QEMU successful-RPC counts equal the server's completed QEMU-client
    counts, with no failed, timed-out, partial, or mismatched transaction.

After the exchange, the oracle obtains a non-destructive counter snapshot for
the QEMU client. The harness obtains another snapshot before and after every
later DAX-touching setup, warmup, timed, and negative-test phase; differences
define the phase counts. Oracle traffic remains available in the raw log but is
excluded from all reported counts. A boot that fails any check is retained as
a failed artifact and contributes no measurement.

Snapshot boundaries use this race-free host/guest barrier:

1. The guest completes initialization, executes a full fence, stops touching
   DAX, and emits `READY <phase-id>`.
2. The host waits until the server reports zero in-flight QEMU requests, takes
   the start snapshot, and sends `GO <phase-id>`.
3. The guest runs only that phase, executes a full fence, stops touching DAX,
   and emits `DONE <phase-id>` with its declared operation and byte totals.
4. The host again waits for zero in-flight QEMU requests and takes the end
   snapshot before acknowledging completion.

After the sentinel exchange, a QEMU request outside a declared `GO` through
`DONE` interval, an unexpected phase ID, or a snapshot taken with a nonzero
in-flight count invalidates the boot.

## Experiment Matrix

### Fixed Execution Platform

Use QEMU TCG with 2 GiB of guest DRAM and two guest vCPUs. KVM, hardware
virtualization, and QEMU `icount` are disabled. The benchmark process is pinned
to guest vCPU 1, leaving guest vCPU 0 for the OS and SSH activity. The host
selects and records fixed, non-overlapping allowed CPU sets for the QEMU and
server processes before the pilot, and the same sets are used for all campaign
boots.

Boot the recorded `/home/victoryang00/cxl/arch/x86/boot/bzImage` with the
Type-2 memdev path enabled and the recorded
`/home/victoryang00/CXLMemSim/build/qemu1.img`. These paths identify current
inputs only; the campaign manifest freezes their content hashes, and a changed
hash creates a new campaign rather than silently updating the existing one.

### Independent Repetition Unit

The independent repetition is a complete guest boot. For each latency point,
run five boots. Each boot uses:

- a newly started CXLMemSim server;
- a newly created or zeroed private backing object;
- a newly started QEMU guest;
- a globally unique attempt ID, guest-boot UUID, and server instance UUID; and
- the same frozen binaries, trace, topology, CPU count, memory sizes, and
  benchmark order rule.

There are 20 required replicate slots: four latency points times replicate
numbers 01 through 05. Every launch is a new complete guest boot with its own
attempt ID and guest-boot UUID. Each slot starts with attempt 01. An attempt
is retry-eligible with the next monotonically increasing attempt number only
if it terminates before the host durably creates `MEASUREMENT_ARMED.json` as
specified below. The first attempt with that durable marker is irrevocably
committed to its slot. It is required to perform exactly one warmup pass and
one timed pass; if it later fails a protocol, semantic, timing-integrity,
corruption, or artifact gate, the campaign is incomplete and that slot is not
replaced. This prevents outcome-based retry selection. Failed attempts never
replace or overwrite artifacts, and attempts after a committed attempt are
forbidden. The summary records every failed, committed, and excluded attempt.

After all preflight and sentinel checks pass, and before any post-preflight
DAX-touching setup, warmup, or timed access, the guest emits `ARM_REQUEST` and
blocks without touching DAX. The host rechecks zero in-flight requests, writes
a temporary `MEASUREMENT_ARMED.json` containing the campaign, slot, attempt,
guest-boot, and server IDs plus the preflight hash, fsyncs the file, atomically
renames it into the attempt directory, and fsyncs the parent directory. This
durable rename is the irrevocable commitment point. Only then does the host
send `ARM_ACK`; the guest may begin the single warmup pass only after receiving
that acknowledgment. A crash before the durable rename is pre-arm and
retry-eligible. A crash after it is post-arm and makes the campaign incomplete.

Run five campaign blocks, where block number is also the replicate number.
Each block contains one slot at every latency using this fixed rotation:

| Block | Latency order |
| ---: | --- |
| 01 | 80 ns, 400 ns, 2,000 ns, 10,000 ns |
| 02 | 400 ns, 2,000 ns, 10,000 ns, 80 ns |
| 03 | 2,000 ns, 10,000 ns, 80 ns, 400 ns |
| 04 | 10,000 ns, 80 ns, 400 ns, 2,000 ns |
| 05 | 80 ns, 10,000 ns, 2,000 ns, 400 ns |

Finish the four primary attempts in a block before retrying a pre-arm failure.
Eligible retries then follow the failed slots' original order within that
block and remain in the same block until every slot has one committed attempt
or the campaign is declared incomplete. A post-arm failure ends the campaign
without a replacement. This prevents both outcome-based selection and all
samples at one latency from being collected in one contiguous time period.

### Latency Points

The fixed configured base-latency values are:

| Label | Configured base latency |
| --- | ---: |
| local-like | 80 ns |
| fabric-like | 400 ns |
| elevated | 2,000 ns |
| stress | 10,000 ns |

The labels are descriptive simulator settings, not physical topology claims.

### Warmup

Each boot executes exactly one untimed warmup pass over the complete ordered
condition list. It then executes exactly one timed pass over the same list.
Individual iterations inside a condition contribute to one aggregate
boot-level observation; they are not treated as independent samples.

Within both passes, conditions are ordered as calibration, transfer sizes in
ascending order, and SlugArch trace sizes in ascending order. The mode order
within every trace size is fixed by replicate number:

| Replicate | Mode order |
| ---: | --- |
| 01 | baseline, validation, delta, full |
| 02 | validation, delta, full, baseline |
| 03 | delta, full, baseline, validation |
| 04 | full, baseline, validation, delta |
| 05 | baseline, full, delta, validation |

Use seed `0x534c5547` for the pointer permutation, generated data, and repeated
trace namespaces. The order and seed are identical at all latency points.

### Calibration

Construct a fixed, seeded permutation of 4,096 cache lines in a 256 KiB region
inside the work window. Each line contains the DPA offset of the next line.
One pass performs 4,096 dependent 64-bit loads and returns to the start.

For each boot, retain:

- total elapsed nanoseconds;
- observed nanoseconds per dependent load;
- final pointer and checksum;
- server completed-read count;
- server modeled-latency sum and distribution; and
- QEMU requested/applied-delay sum, undershoot count, and overshoot
  distribution.

The timed pass is one boot-level observation. The warmup pass is not included.
The calibration phase must declare exactly 4,096 reads of eight bytes and no
writes; the QEMU synchronous-handler and server deltas must match exactly
4,096 completed reads and 32,768 read bytes.

### Transfer-Size Sensitivity

Measure sequential devdax write and readback for exactly:

- 4 KiB;
- 64 KiB; and
- 1 MiB.

For each size, the timed observation includes one write, a persistence/order
fence, one readback, and checksum verification. Report write time, read time,
round-trip time, and guest-effective write/read throughput from that single
aggregate. Do not call this physical CXL link bandwidth, and do not count the
internal scalar CXL transactions as statistical repetitions.

All three sizes are collected at all four latency points. The figure may use
compact read/write facets; the complete validated table remains in the
artifact and paper data snapshot.

For each size, guest-declared write/read bytes, QEMU synchronous-handler bytes,
and server-completed bytes must each equal the requested size in the timed
phase. The corresponding operation counts may reflect QEMU's scalar splitting,
but QEMU and server counts must match exactly.

### SlugArch Modes

The four fixed modes are:

| Mode | Timed behavior |
| --- | --- |
| baseline | Run the same guest software interpreter, stage raw 64-byte request and response records through CXL.mem, read them back, and perform bytewise equality checking; do not create or validate SlugArch metadata. |
| validation | Encode and validate the existing SlugArch validation policy while staging request and guest-software response records through CXL.mem. |
| delta | Encode and validate the existing SlugArch delta policy while staging request and guest-software response records through CXL.mem. |
| full | Encode and validate the existing SlugArch full-record policy while staging request and guest-software response records through CXL.mem. |

All response records are produced by guest software. The simulated Type-2
device transports memory bytes; it does not execute the SlugArch interpreter.
The end-to-end timer starts immediately before baseline copying or SlugArch
encoding and stops immediately after bytewise equality checking or SlugArch
validation. The stage timers are nested inside that interval. SSH transfer,
host orchestration, input initialization, output export, and the host's
independent post-run validation are outside the timed interval.

Each timed observation records:

- encode time;
- CXL.mem write time;
- CXL.mem read time;
- guest-software interpretation time;
- validation or equality-check time;
- end-to-end total time;
- bytes written and read;
- request and response record counts;
- input, staged, readback, and output checksums;
- server and QEMU read/write/RPC counts; and
- pass/fail status.

For every SlugArch condition, the frozen condition manifest derives expected
write and read byte totals from the exact serialized records before execution.
Guest-declared totals, QEMU synchronous-handler totals, and server-completed
totals must equal those expectations. QEMU's local-shadow, local-cache, BAR4
bulk-overlay, and coherent-pool completion deltas must all be zero.

### Trace Sizes

Use the frozen 49-record, 64-byte GEMM trace at these scales:

| Scale | Records | Construction |
| ---: | ---: | --- |
| 1x | 49 | Original trace |
| 4x | 196 | Four ordered copies |
| 16x | 784 | Sixteen ordered copies |
| 64x | 3,136 | Sixty-four ordered copies |

Repeated copies receive deterministic sequence/tag namespaces so accidental
cross-copy matches cannot pass validation. The semantic request content
otherwise remains unchanged.

Collect all four modes and all four trace sizes in every boot. For the
across-latency headline, use 196 records as the predeclared canonical size.
For the scaling headline, use the 400 ns setting as the predeclared canonical
latency.

The 4x through 64x conditions are deterministic repetitions of one 49-record
GEMM trace. They measure byte-count and record-count sensitivity only; they are
not workload diversity, independent workload replication, or evidence that
unseen applications scale the same way.

Reserve the first 1 MiB of the work window for the sentinel and control
patterns. Allocate a distinct, one-MiB-aligned subregion after it for
calibration, each transfer size, and each mode/trace-size condition. These 21
one-MiB slots fit within the 32 MiB work window. Initialization and zeroing
occur outside timed ranges. The warmup and timed pass reuse the same subregion
for a condition, while different conditions never alias.

### Post-Transport Corruption

Each boot runs one untimed, labeled negative test after the timed pass:

1. Produce a valid full-mode response stream.
2. Write and read it back through the live CXL.mem work window.
3. Verify the transport checksum.
4. In guest DRAM, after readback, flip the low bit of the first response
   payload byte while leaving framing intact.
5. Submit the corrupted stream to the normal validator.
6. Require rejection as a decoded-result mismatch.

This yields 20 expected rejections across four latencies and five boots. It is
called `post_transport_guest_payload_flip`. It demonstrates software
fail-stop validation after live transport, not device fault injection,
transport corruption detection, or recovery.

## Metrics and Statistical Contract

### Reported Summaries

For every latency-setting cell, report the five boot-level observations as:

- median;
- minimum; and
- maximum.

Figures show the median with min/max whiskers and, where legible, the five
individual boot points. Do not report a confidence interval, standard error,
or standard deviation as the primary uncertainty summary. Do not promote
within-boot iterations, records, scalar loads, or RPCs to independent samples.

### Paired SlugArch Overhead

For each boot, latency, and trace size, compute:

`paired_overhead(mode) = mode_end_to_end_ns / baseline_end_to_end_ns`

for validation, delta, and full. Summarize the five per-boot ratios by median,
minimum, and maximum. Never divide a median mode time by a median baseline
time. A mode cell without its same-boot baseline is not analyzed as a ratio
and causes that committed attempt to fail validation, which makes the campaign
incomplete.

The raw stage times and transferred bytes remain available so the paper can
distinguish metadata/validation work from the extra memory traffic caused by
larger records.

### Calibration Interpretation

The primary calibration value is guest-observed nanoseconds per dependent
load. Also compute, but do not substitute for the raw values:

- the median difference from the 80 ns setting;
- the least-squares slope of observed versus configured latency over the four
  medians; and
- the rank order of the four medians.

No run is discarded because its timing is surprising. If the dependent-load
medians are not monotonic nondecreasing, or if the 10 microsecond median is not
greater than the 80 ns median, the latency-sensitive SlugArch comparisons fail
calibration and are not promoted into paper claims. The raw failed calibration
is retained and described as a simulator limitation.

Apply the same predeclared check separately to median transfer write time and
median transfer read time at each of 4 KiB, 64 KiB, and 1 MiB: the four
latency-point medians must be nondecreasing, and the 10 microsecond median must
be strictly greater than the 80 ns median. A failure is reported as observed,
never repaired by discarding a committed boot or changing the grid. It blocks
the new four-panel Figure 2 and latency-dependent RQ2 claim.

### Data-Quality Checks

An attempt fails validation if any of these occurs:

- missing or mismatched binary, source, trace, kernel, image, or command hash;
- protocol negotiation, framing, checksum, status, timeout, or partial-I/O
  failure;
- topology or DAX preflight failure;
- work-window overlap with either RAM bypass;
- missing sentinel round trip;
- zero measured QEMU-client server reads or writes;
- QEMU/server completed-operation count mismatch;
- guest/QEMU/server byte-total mismatch or a nonzero local-cache,
  local-shadow, bulk-overlay, or coherent-pool completion count;
- a missing, duplicate, undershot, or value-mismatched QEMU delay event for
  any successful server memory response;
- a phase-barrier, phase-ID, in-flight, or out-of-phase-access violation;
- missing result row, baseline pair, checksum, or stage timing;
- request/response record-count mismatch;
- a validator failure on an uncorrupted stream;
- acceptance of the labeled corrupted stream;
- QEMU, server, guest, or harness crash;
- a negative or overflowed duration;
- reuse or overwrite of an existing attempt ID or guest-boot UUID; or
- a changed executable or source snapshot inside one campaign.

A failed attempt is never silently replaced in place. Only a pre-arm failure
is retry-eligible under the independent-repetition rule above; any post-arm
failure makes the campaign incomplete. The campaign summary lists the failure
reason and any permitted pre-arm retry.

## Immutable Artifact Contract

### Directory Layout

Use a new result family that is never mixed with the July 2 or July 4 BAR2
artifacts:

```text
artifact/slugarch_type2_cxlmem/
  .campaign.lock
  campaign-registry.jsonl
  .<campaign-id>.inprogress/
    campaign-manifest.json
    campaign-summary.json
    exclusions.jsonl
    source/
    latency-00080ns/replicate-01/attempt-01-<guest-boot-uuid>/
    latency-00080ns/replicate-02/attempt-01-<guest-boot-uuid>/
    ...
    latency-10000ns/replicate-05/attempt-01-<guest-boot-uuid>/
```

Every boot directory contains:

```text
manifest.json
commands/
preflight/
logs/
raw/
checksums.sha256
validation.json
```

A committed attempt also contains `MEASUREMENT_ARMED.json`. `COMPLETE` is
created only after all hashes and validation rules pass. A failed attempt
contains `FAILED.json` instead. Existing campaign and attempt directories are
immutable; the harness fails closed if a target path already exists.

An attempt is writable only until `COMPLETE` or `FAILED.json` is created. Its
seal contains the SHA-256 of a canonical, sorted `checksums.sha256` covering
every other regular file in that attempt. After all 20 replicate slots are
represented by one committed, complete attempt, the harness writes a canonical
campaign-level `campaign-checksums.sha256` covering all source snapshots,
attempt seals, summaries, and exclusions, then writes `CAMPAIGN_COMPLETE`
containing the hash of that checksum file. The seal and checksum file exclude
themselves from their own coverage.

If the campaign terminates without 20 committed, complete attempts, including
after any post-arm failure, the harness first seals every attempt collected so
far. It then writes the terminal campaign summary and exclusions, creates the
same canonical `campaign-checksums.sha256`, and writes `CAMPAIGN_FAILED`
containing the failed slot, failure reason, and checksum-file hash.
`CAMPAIGN_COMPLETE` and `CAMPAIGN_FAILED` are mutually exclusive and are not
included in their own checksum file.

Only after successful validation does the harness atomically rename
`.<campaign-id>.inprogress` to `<campaign-id>`. A terminal failed campaign is
atomically renamed to `<campaign-id>.failed`. In either case, the final
campaign directory is read-only to the harness. Recollection, repair, summary
regeneration, or changed source creates a new campaign ID; it never modifies a
final complete or failed directory.

### Campaign Registration and Selection

The harness holds an exclusive lock on `.campaign.lock` from pre-launch
registration through terminal registry append. A later harness or exporter
must acquire the same lock before inspecting or changing campaign state. After
a crash releases the lock, the next harness first reconciles every registered
campaign: it completes the prescribed atomic rename for any checksum-valid
terminal marker still under `.inprogress`, and appends any missing terminal
registry event for a checksum-valid finalized directory. A registered
`.inprogress` campaign with no terminal marker must be resolved under the
pre-arm/post-arm rules before another campaign can register. No launch or
export may proceed while reconciliation is incomplete.

Before the first attempt of a campaign launches, compute an experiment-version
SHA-256 over the approved design spec, in-scope source snapshots and patches,
binaries, traces, topology, commands, matrix, order, seeds, and validation
rules. Append a `REGISTERED` event to
`artifact/slugarch_type2_cxlmem/campaign-registry.jsonl` containing a monotonic
ordinal, campaign ID, experiment-version hash, frozen-input hashes, UTC time,
and previous-entry hash. Open the registry in append-only mode, fsync the new
registry state and parent directory, and refuse to launch if its existing hash
chain or ordinal sequence does not verify. The harness never truncates,
rewrites, or replaces an existing registry entry. Registry-tail verification,
ordinal allocation, append, fsync, and post-append verification occur within
one exclusive-lock transaction.

After the atomic final rename, append and fsync a terminal `COMPLETE` or
`FAILED` event that names the registration ordinal and final campaign-checksum
hash. The registry is copied into every normalized provenance snapshot.

For one experiment-version hash, the lowest registration ordinal whose
finalized directory contains a checksum-valid `CAMPAIGN_COMPLETE` is the sole
campaign eligible to supply the paper dataset; terminal-event append order is
irrelevant. All preceding `CAMPAIGN_FAILED` entries and their reasons remain
linked and disclosed. A complete campaign cannot be superseded because of
timing values, calibration outcome, or any other scientific result. A later
campaign becomes eligible only under a new experiment-version hash with a
pre-arm change record that identifies the necessary protocol, implementation,
or methodology change and its technical reason. Observed performance alone is
not a valid reason. The paper provenance discloses the version transition and
all earlier registered campaigns.

### Manifest Contents

The campaign and boot manifests record:

- UTC start/end timestamps, campaign ID, replicate slot, attempt ID,
  experiment-version hash, registry ordinal and hashes, guest-boot UUID, server
  instance UUID, attempt state, and commit status;
- host kernel, CPU model, online CPU set, memory, governor, load snapshot, and
  QEMU accelerator;
- exact SlugArch, CXLMemSim, QEMU submodule, and guest-kernel commits;
- `git status --porcelain=v2`, submodule status, staged and unstaged diff
  hashes, and archived in-scope patches for every dirty source tree;
- SHA-256 hashes and sizes of QEMU, server, guest image, kernel, guest
  benchmark, trace, validator, topology, and plotting scripts;
- complete server, QEMU, guest-kernel-command-line, and benchmark commands;
- all explicit environment variables that affect the run;
- configured latency and the realized server/model settings;
- wire-protocol magic/version and client/server instance IDs;
- CFMWS/HPA/DPA, memdev, region, devdax, bulk-bypass, coherent-pool, and work
  window ranges;
- deterministic seeds and condition order;
- warmup/timed phase boundaries;
- server/QEMU counter snapshots;
- all raw-data file hashes; and
- validation and exclusion results.

Because the current CXLMemSim checkout is dirty, a commit ID alone is
insufficient. The implementation runs from an isolated, recorded source
snapshot and never edits or builds over unrelated user changes in place.

### Machine-Readable Data

The guest emits one JSON object per condition. The host preserves it verbatim.
Normalization for inclusion, plotting, or paper export accepts only a final
`<campaign-id>` directory whose `CAMPAIGN_COMPLETE` and complete checksum tree
verify and whose registration ordinal is the lowest among checksum-valid
complete directories for that experiment version. An `.inprogress`, `.failed`,
unregistered, or superseding same-version campaign is categorically
ineligible; diagnostic tools may summarize it only as failed or non-paper
evidence. For an eligible campaign, the host produces a normalized CSV/JSON
summary with one row per boot-level observation. Summary generation verifies:

- exactly one committed, complete attempt in each of the 20 replicate slots;
- exactly five committed, complete boots per latency;
- exactly one timed row for every required condition per committed boot;
- complete same-boot pairing;
- matching identifiers and hashes across guest, QEMU, server, and manifest;
- exact guest/QEMU/server operation and byte accounting for every phase, with
  zero local/bypass completions;
- the predeclared block, latency, trace, and mode order;
- 20 of 20 labeled corruption rejections; and
- no included row from a failed or incomplete attempt.

The plotting input copied into the paper repository includes the normalized
values, raw artifact relative paths, campaign hash, experiment-version hash,
registry chain and prior-campaign disclosures, source hashes, exclusions, and
the complete claim-limitations text.

The paper worktree uses these exact new files:

- `data/slugarch-type2-cxlmem.json` for the reviewed normalized snapshot and
  provenance;
- `scripts/plot_slugarch_type2_cxlmem.py` for the deterministic renderer; and
- `img/slugarch-type2-cxlmem.pdf` for Figure 2.

The July 4 snapshot and its Figure 1 renderer remain separate. The renderer
fixes PDF metadata and is run twice so an unchanged data/source input produces
an identical output hash.

Paper integration occurs in a clean isolated paper worktree at a recorded base
commit. Before copying, the harness records any existing destination hashes
and fails if they differ from the reviewed base. It copies only the three-file
allowlist above through temporary names, verifies source and destination
hashes, and atomically renames them. Manuscript edits use a separate explicit
allowlist. Only those reviewed paths are staged, and one paper-only commit is
created before any later cherry-pick or synchronization. Unrelated dirty files
in the user's paper checkout are never staged or overwritten.

## Figure Design

### Figure 1: Existing SlugArch Prototype Evidence

Consolidate the July 15 result design into one full-width, 7.1-by-3.0-inch
vector figure with a 2-by-2 panel layout:

1. five-run legacy BAR2 helper repeatability;
2. software log bytes for validation, delta, and full;
3. software validator means with the explicit note that dispersion was not
   retained; and
4. the seven-case offline fail-stop matrix.

The BAR2 panel keeps the one-boot and guest-software boundary explicit. The
offline mutation panel remains labeled offline and must not be combined with
the new post-transport corruption count.

### Figure 2: Type-2 CXL.mem Calibration and SlugArch Evaluation

Create one full-width, 7.1-by-3.0-inch vector figure with a 2-by-2 panel
layout:

1. **Delay calibration.** Configured latency on the x-axis and guest-observed
   nanoseconds per dependent load on the y-axis. Show five boot points plus
   median/min-max. Do not draw a physical-CXL ideal line or claim three-percent
   accuracy.
2. **Transfer-size sensitivity.** Guest-effective read and write throughput for
   4 KiB, 64 KiB, and 1 MiB at all four configured points. Use compact
   read/write facets, consistent latency colors, direct units, and
   median/min-max.
3. **Paired SlugArch overhead.** At 196 records, show per-boot ratios for
   validation, delta, and full relative to the same boot's baseline across all
   four latency settings. Include a quiet 1.0 reference line.
4. **Record-count scaling.** At 400 ns, show end-to-end time versus 49, 196,
   784, and 3,136 records for baseline, validation, delta, and full, with
   median/min-max.

Use a white background, dark text, quiet guides, one consistent latency color
family, and distinct marker/line/hatch encodings that remain legible in
grayscale. Axis labels state simulator configuration and guest-observed units.
Captions define `n=5 independent guest boots`, `median with min-max`, and the
single warmup-pass rule. Printed figure text is at least 7 points, and each
caption is at most 90 words.

No plot may contain preview, estimated, interpolated, copied-from-reference, or
fabricated numeric values.

Before the pilot, create a paper-layout mockup using labeled empty axes and
legend shapes without numeric data. The mockup fixes both figure dimensions,
2-by-2 layouts, font sizes, caption allocation, and the three-page evaluation
flow. A clean paper build must show that the two figures remain legible and the
11-page main-text budget is feasible. If it does not, reduce duplicated prose
or caption wording before the campaign; do not shrink figure text or change the
experiment matrix.

Figure 2 is all-or-nothing. Its panel gates are:

| Panel | Additional gate beyond the complete 20-boot dataset |
| --- | --- |
| calibration | exact 4,096-read accounting and the dependent-load monotonicity/separation rule |
| transfer | exact byte accounting and every size's read/write monotonicity/separation rule |
| paired overhead | all 196-record same-boot baseline/mode pairs and hashes |
| record scaling | all four modes and record counts at 400 ns, with the repeated-trace limitation |

If any general protocol, sentinel, bypass, artifact, corruption, or panel gate
fails, do not place a reduced Figure 2 in the paper and do not promote the new
RQ2 result. Retain the validated raw evidence and keep the claim blocked.

## Paper Integration

### Evaluation Sequence

The revised evaluation proceeds in this order:

1. setup, artifact policy, and the boundary between legacy BAR2 and new
   CXL.mem evidence;
2. legacy BAR2 repeatability and software results in Figure 1;
3. Type-2 CXL.mem protocol, sentinel proof, latency calibration, and transfer
   sensitivity;
4. paired SlugArch overhead and record-count scaling in Figure 2;
5. offline mutation coverage, 20 post-transport guest-software corruptions,
   and fail-stop interpretation; and
6. measured boundary, exclusions, and blocked hardware claims.

### Research-Question Gates

Only after all required evidence passes:

- RQ1 may add that request and guest-software response records were written to
  and read back from a mapped QEMU Type-2 CXL.mem region whose operations
  reached CXLMemSim.
- RQ2 may report paired simulator overhead for validation, delta, and full
  relative to the same-substrate, same-boot common-path baseline that retains
  the interpreter, raw-record CXL.mem staging/readback, and equality check but
  omits SlugArch metadata and validation.
- RQ3 keeps the July 4 software footprint and validator timing, augmented by
  live CXL.mem stage breakdowns.
- RQ4 remains scoped to existing GEMM label-coverage instrumentation.
- RQ5 remains blocked beyond this one-endpoint QEMU Type-2 integration.

CXL.cache remains unmeasured. The paper may not infer that a successful
CXL.mem experiment validates CXL.cache, BI/snoop behavior, or a combined
CXL.cache+CXL.mem replay path.

### Page Budget

The fixed main-text budget is:

| Material | Main-text budget |
| --- | ---: |
| Abstract, introduction, and background | 2.0 pages |
| Design, including coherency-debugging recipe | 2.25 pages |
| Operational semantics and proof sketch | 2.25 pages |
| Evaluation and both result figures | 3.0 pages |
| Related work, deployment/limitations, conclusion | 1.5 pages |
| **Total** | **11.0 pages** |

Recover 0.25 page each from repeated design prose and individually headed
semantic obligations. Do not reduce font size, margins, line spacing, figure
text below readable size, or IEEE class geometry. The conclusion must finish
on or before page 11; references may start afterward.

## Paper-Safe Claims

After successful execution and validation, the manuscript may claim:

- the one-target QEMU Type-2 CXL.mem path completed synchronous reads and
  writes against a server-authoritative CXLMemSim backing store;
- the sentinel exchange and matching QEMU/server IDs, counts, offsets, and
  checksums rule out the prior zero-traffic and local-RAM-only paths;
- the four configured simulator points were evaluated over five independent
  boots and are reported as median with minimum and maximum;
- guest-observed calibration and transfer sensitivity have the values present
  in the validated campaign, without relabeling them as physical link
  latency/bandwidth;
- SlugArch modes have the paired same-boot simulator overheads present in the
  validated campaign;
- SlugArch request and guest-software response records were staged and read
  back through the live mapped CXL.mem region before host validation; and
- all 20 labeled post-transport guest-software payload flips were rejected, if
  the required 20 of 20 result is observed.

The manuscript must separately retain the corrected legacy BAR2 claim: BAR2
carried request write/readback and NOP, while guest software generated the
response.

## Claims Explicitly Out of Scope

This experiment does not establish:

- FPGA or other physical-hardware performance;
- absolute comparability with the reference manuscript;
- 80 ns guest loads or physical CXL latency accuracy;
- production overhead or native-speed performance;
- device-side SlugArch execution;
- response FLIT transport through legacy BAR2;
- CXL.cache, BI, snoop, cache-coherence, or Type-2 accelerator-cache behavior;
- DMA, ATS, PASID, migration, peer-to-peer traffic, or accelerator execution;
- switch ordering, multi-switch routing, multi-host behavior, or multi-way
  interleave;
- hardware record generation, compression, replay, provenance recovery, or
  post-error recovery;
- hardware fault injection;
- performance equivalence between QEMU TCG and KVM or hardware; or
- FPGA resource, timing, power, or area cost.

## Implementation and Verification Gates

Implementation must proceed through these gates in order:

1. Unit-test wire encoding/decoding, endian handling, length limits, request-ID
   matching, checksums, short I/O, timeout, and malformed-frame rejection.
2. Build the isolated CXLMemSim server and QEMU snapshot and record their
   hashes.
3. Run a protocol smoke test with oracle READ/WRITE round trips and nonzero
   per-client counters.
4. Run a QEMU no-guest smoke test that proves version negotiation and rejects
   the legacy-layout mismatch.
5. Boot the guest and prove Type-2 enumeration, committed CXL region, devdax
   availability, and a non-bypass work-window mapping.
6. Pass the two-way sentinel proof through devdax and the server-authoritative
   backing store.
7. Build the two-figure empty-axis paper mockup and verify dimensions,
   minimum font size, caption budget, three-page evaluation flow, and the
   11-page main-text layout strategy.
8. Run one complete pilot boot at 400 ns and validate every required matrix
   row, pair, checksum, count, and negative test.
9. Freeze binaries, source snapshot, seeds, commands, and campaign ID; compute
   the experiment-version hash and durably register the campaign before launch.
10. Run the 20-slot campaign in the predeclared block order without editing
    code or replacing artifacts.
11. Validate the complete dataset and exclusions before computing paper
    summaries.
12. Render both figures twice and require stable data and PDF hashes.
13. Inspect the figures directly for clipping, density, small text, and
    grayscale legibility.
14. Copy only the reviewed normalized snapshot, plotting source, and final
    vector figures into the paper worktree.
15. Update claims, RQ status, limitations, abstract, conclusion, and captions
    from the validated snapshot.
16. Build the paper from clean auxiliary state, confirm resolved references
    and citations, confirm no stale CFR naming, and verify that main text ends
    no later than page 11.
17. Review source and PDF text to ensure every CXL.mem statement names the
    QEMU/CXLMemSim simulator boundary and every excluded hardware claim remains
    excluded.

If the host cannot expose a devdax mapping that reaches the repaired Type-2
CFMWS dispatcher, if protocol/data authority cannot be proven, or if any of
the 20 replicate slots lacks one committed, complete boot, the new
quantitative CXL.mem result is blocked. The implementation retains the failed
evidence and does not fill the paper with partial or estimated numbers.
