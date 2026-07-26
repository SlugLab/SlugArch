# SlugArch Small CXL Tiles and Evaluation Design

**Status:** Approved design
**Date:** 2026-07-26
**Supersedes:** The monolithic-accelerator narrative and the five broad,
roadmap-heavy research questions in the current paper draft
**Extends:** `2026-07-25-slugarch-qemu-j-extension-design.md`

## 1. Objective

Reframe SlugArch around a board-level fabric of small, independently managed
CXL Type-2 compute tiles instead of one large opaque GPU-class endpoint. Each
tile owns a bounded Hardware-JIT controller that observes its external memory
and offload boundary. The host remains the coherence authority and joins the
per-tile records into one fail-stop epoch.

The paper thesis is:

> Replacing one opaque accelerator with composable CXL tiles improves the
> architectural unit of allocation and failure, but distributes causality
> across more boundaries. SlugArch restores explainability by placing the same
> verified, attributable replay controller at every tile boundary.

The evaluation must answer meaningful questions about correctness, recorder
scaling, fault localization, and implementation equivalence. It must not claim
that QEMU proves physical CXL.cache behavior or that small tiles outperform an
NVIDIA GPU.

## 2. Design Decisions

The approved decisions are:

1. **Physical scale:** board-level CXL endpoints, not literal bare dies linked
   by on-package CXL.
2. **Coherence authority:** the host home agent, matching the standard CXL
   host/device model.
3. **Controller placement:** one Hardware JIT and recorder per Type-2 tile.
4. **Thesis:** composability creates a new observability and debugging problem;
   SlugArch addresses that problem.
5. **Evaluation:** four focused research questions and 125 timed boots, with
   five fresh boots per matrix cell.
6. **Paper shape:** at most 11 pages of main text before references, exactly
   two evaluation figures, and no quantitative result table.
7. **Naming:** SlugArch is the only system name. The standalone CFR name and
   `\cfr` macro are removed.

## 3. Relation to LegoOS and CXL

LegoOS separates processor, memory, and storage hardware into independently
managed components and distributes OS functions across those components. The
SlugArch story borrows the component-independence motivation, but differs in
scale, interconnect, and goal:

- LegoOS studies server-scale hardware resource disaggregation over a network.
- SlugArch studies board-level Type-2 compute tiles and memory devices under a
  host-mediated CXL coherence domain.
- LegoOS focuses on OS resource management, utilization, and failure
  isolation.
- SlugArch focuses on boundary observability, replay, and first-fault
  explanation.

The paper must not imply that LegoOS used CXL or implemented SlugArch's replay
contract.

CXL.cache lets a device cache host memory, and CXL.mem lets the host access
device-attached memory. For peer-device memory, coherence remains resolved by
the host home agent. SlugArch uses that host-mediated model. Direct
tile-to-tile coherence is not part of version 1.

Primary references:

- Yizhou Shan, Yutong Huang, Yilun Chen, and Yiying Zhang. “LegoOS: A
  Disseminated, Distributed OS for Hardware Resource Disaggregation.” OSDI
  2018. <https://www.usenix.org/conference/osdi18/presentation/shan>
- Compute Express Link Consortium specifications and coherence material.
  <https://computeexpresslink.org/specifications/>
- CXL Consortium, “Questions from the CXL Exploring Coherent Memory and
  Innovative Use Cases Webinar.”
  <https://computeexpresslink.org/blog/questions-from-the-compute-express-link-exploring-coherent-memory-and-innovative-use-cases-webinar-2341/>

## 4. Terminology

Use these terms consistently:

- **SlugArch:** the architecture, policy ABI, Hardware-JIT implementation,
  record format, and replay contract.
- **Small CXL tile:** one board-level QEMU or physical Type-2 endpoint with
  compute, optional cache or host-managed device memory, and one SlugArch
  controller.
- **Hardware JIT:** verified runtime specialization of a bounded SlugArch
  policy into a tile-local software or RTL execution backend.
- **Native GPU PTX JIT:** CUDA driver compilation reached through
  `cuModuleLoadDataEx`. It is a separate diagnostic source and is not the
  SlugArch policy JIT.
- **Host-home-agent model:** the deterministic event-level model used to test
  shared-line ordering and fault localization.
- **Direct CFMWS path:** the server-authoritative QEMU Type-2 CXL.mem boundary
  path.

Do not use:

- CFR as a second system or replay-engine name;
- “small die” where it could imply literal on-package CXL wiring;
- “CXL FLIT” for the local 64-byte SlugCXL packet;
- “hardware result” for Rust or QEMU;
- “physical CXL.cache” for the event-level home-agent model; or
- “NVIDIA baseline” for a QEMU measurement.

## 5. Architecture

### 5.1 Components

The proposed physical architecture contains:

1. **Host CPU and coherence home agent**
   - owns coherence authority for the shared physical address space;
   - assigns tile IDs and epoch IDs;
   - installs one verified policy digest across participating tiles;
   - joins tile, server, and guest or model evidence; and
   - rejects partial or mismatched epochs.

2. **CXL switch**
   - fans out the host connection to multiple endpoints;
   - routes CXL.io, CXL.cache, and CXL.mem traffic; and
   - remains an uninstrumented routing and ordering boundary in version 1.

3. **Small Type-2 compute tiles**
   - contain compute, optional cache, and optional host-managed device memory;
   - receive a unique, immutable tile ID;
   - contain one local Hardware JIT and recorder;
   - expose the versioned SlugArch BAR2 capability;
   - report policy digest, event, record, metadata, reject, drop, and epoch
     counters; and
   - fail the affected covered access if a required record cannot commit.

4. **Optional Type-3 memory tile**
   - motivates composable capacity and pooling;
   - is not required for the version-1 timed matrix; and
   - remains a future recorder placement unless a live implementation is
     added and separately validated.

5. **SlugArch coordinator and validator**
   - runs on the host;
   - maintains the campaign and boot UUID;
   - brackets each phase with cross-layer counters;
   - joins records using tile ID, event ID, request ID, server sequence,
     epoch, and policy digest; and
   - seals complete or failed evidence without rewriting raw files.

### 5.2 Tile-local state

Each tile-local controller owns:

```text
tile_id
backend_id
policy_digest[32]
policy_epoch
event_count
record_count
metadata_bytes
reject_count
drop_count
last_error
bounded diagnostic buffer
```

No controller may retain a guest pointer, QEMU pointer, CUDA pointer, or
another tile's private state.

### 5.3 One recorded operation

For a covered operation:

1. The host has already installed the same policy digest on every participating
   tile and assigned a unique tile ID.
2. The tile produces a canonical request event.
3. The local Hardware JIT validates, filters, and records the request.
4. The modeled or direct memory operation executes.
5. The tile produces a canonical completion event.
6. The required completion record commits before the completion is exposed.
7. The host coordinator joins the request, completion, server sequence,
   payload commitment, tile ID, and epoch.
8. The epoch remains eligible only if all declared joins and counters agree.

## 6. QEMU and CXLMemSim Evidence Model

### 6.1 Topology

The experiment topology instantiates 1, 2, 4, or 8 independently identified
QEMU Type-2 tile objects behind one host CXL hierarchy. Every tile has its own
J-extension handle, policy state, counters, log, and client identity.

The implementation must prove:

- all requested tiles enumerate;
- tile IDs and client IDs are unique;
- the installed policy digest is identical where required;
- backend selection is exact and has no silent fallback; and
- one tile's error cannot be counted as another tile's record.

### 6.2 Direct CFMWS anchor

Every eligible boot performs a server-authoritative sentinel at DPA 80 MiB:

- an 8-byte read returns a server-owned value;
- an 8-byte write is committed and observed by the server;
- request and completion IDs join;
- server sequence is nonzero;
- the returned modeled latency is applied;
- the QEMU direct-CFMWS counter increases by two; and
- no local-shadow, BAR overlay, cache, or coherent-pool completion is accepted.

At minimum, the designated memory-bearing tile must pass this sentinel. If the
implementation provides one CFMWS per tile, each window must be one target,
one way, 256 MiB, DPA base zero, and the evidence must retain the tile/client
identity. A multi-window extension must not weaken the one-target-per-window
rule.

### 6.3 Event-level host-home-agent model

Shared-line experiments use a deterministic model, not physical CXL.cache.
The model tracks one record per 64-byte line:

```text
line_address
version
owner_tile_or_none
sharer_bitset
last_writer_tile_or_none
visible_epoch
outstanding_invalidations
```

Recognized semantic events are:

```text
read_shared
read_exclusive
writeback
invalidate
invalidate_ack
fence
completion
epoch_seal
```

The model enforces:

- one home-agent order per line;
- monotonically increasing line versions on visible writes;
- no write completion while required invalidations remain outstanding;
- no producer completion before the declared fence makes its data visible;
- no consumer observation of a version older than the visible version; and
- exact epoch and tile attribution.

The paper labels all such results “QEMU event-level home-agent model.”

### 6.4 Workload phases

Each timed boot runs:

1. **Private partitions:** each tile accesses disjoint cache lines.
2. **Read-shared fanout:** the host publishes a version and all tiles read it.
3. **Producer/consumer:** one tile publishes data, fences, and signals another
   tile.
4. **Hot-line ping-pong:** writers alternate ownership of one line.

Each phase has:

```text
warmup iterations = 100
measured events per active tile = 10,000
counter snapshot = before and after
checksum or line-version commitment = required
```

The direct CFMWS calibration additionally retains:

```text
dependent 8-byte loads = 10,000 measured iterations
64-byte transfers = 2,000 measured iterations
```

## 7. Research Questions

### RQ1: Replay correctness

**Question:** Does each tile preserve the SlugArch replay contract, and does the
system fail at the first injected mismatch?

Metrics:

- joined request/completion records;
- Rust/RTL semantic mismatches;
- policy-digest mismatches;
- line-version mismatches;
- required-record drops;
- detected and undetected injected faults;
- false positives; and
- exact first-divergent `(tile_id, event_id, fault_code)`.

Success requires zero mismatch on every supported non-fault case, detection of
every declared fault, zero false positives, and zero required-record drops.

### RQ2: Recorder scaling

**Question:** How does continuous per-tile observation scale from one to eight
tiles?

Metrics:

- per-event median, p95, and p99 processing time;
- aggregate events per second;
- guest or model phase time;
- direct CFMWS dependent-load and 64-byte-transfer time where applicable;
- configured, server-returned, QEMU-applied, and observed latency;
- metadata bytes per operation;
- record, reject, and drop counts; and
- extension-added time relative to the matching off cell.

These metrics characterize the simulator and recorder. They do not estimate
physical die frequency, CXL link bandwidth, or GPU performance.

### RQ3: Fault localization

**Question:** Do per-tile records identify a causative tile and boundary event
more precisely than host-only outcome and checksum evidence?

Baselines:

1. **Host-only:** final state, final checksum or visible line version, and
   participating tile set.
2. **SlugArch per-tile:** the complete joined record stream.

Metrics:

- fault detection rate;
- exact tile localization rate;
- exact event localization rate;
- suspect-set size;
- events inspected before first explanation; and
- metadata bytes required for that explanation.

The host-only suspect set is computed mechanically as all tiles that could have
written or completed the affected object. It must not be manually inflated or
reduced.

### RQ4: Backend portability

**Question:** Do the Rust and FPGA-Verilator implementations execute the same
bounded policy semantics?

Metrics:

- record bytes;
- payload commitment;
- epoch;
- policy digest;
- record, metadata, reject, and drop counters;
- policy-load and event-to-record cycles for RTL; and
- exact error code for every invalid program or event.

Success requires byte-exact supported-case equivalence and identical first
failure classification. Native CUDA PTX JIT diagnostics are reported
separately and never counted as policy-backend equivalence.

## 8. Timed Campaign

### 8.1 Matrix

The timed campaign has exactly 125 unique boots.

#### Latency calibration: 60 boots

```text
tiles = [1]
latency_ns = [80, 400, 2000, 10000]
backend = [off, rust, fpga-verilator]
record_mode = [validation]
repetitions = 5
```

Count: `1 * 4 * 3 * 1 * 5 = 60`.

#### Tile scaling: 45 additional boots

```text
tiles = [2, 4, 8]
latency_ns = [400]
backend = [off, rust, fpga-verilator]
record_mode = [validation]
repetitions = 5
```

Count: `3 * 1 * 3 * 1 * 5 = 45`.

The one-tile, 400 ns cells already exist in the calibration grid and are not
rerun or double-counted.

#### Record-mode cost: 20 additional boots

```text
tiles = [4]
latency_ns = [400]
backend = [rust, fpga-verilator]
record_mode = [delta, full]
repetitions = 5
```

Count: `1 * 1 * 2 * 2 * 5 = 20`.

The four-tile validation cells already exist in the scale grid.

### 8.2 Independent repetition

Every repetition uses:

- a fresh CXLMemSim server process;
- a fresh QEMU process;
- fresh J-extension handles and backend state;
- fresh event and diagnostic files;
- a unique boot UUID;
- a deterministic cell identity;
- a recorded order seed;
- zeroed counters; and
- no reused timing sample.

The campaign uses median, minimum, and maximum across the five valid boots.
Within-boot event distributions may additionally report p95 and p99, but they
do not replace the five-boot summary.

### 8.3 Calibration gate

For the one-tile off cells, the medians of direct dependent-load time must be
strictly increasing in configured latency order:

```text
80 ns < 400 ns < 2 us < 10 us
```

Failure blocks timing claims while preserving correctness evidence.

## 9. Correctness and Fault Corpus

The correctness corpus runs from reset under five fresh boots for every
eligible topology/backend pair. It is separate from timed aggregation.

Inject exactly:

1. **Missing invalidation acknowledgement**
   - leave one required invalidation outstanding;
   - expect `E_COH_INVALIDATE_PENDING`.

2. **Stale shared-line version**
   - return version `v-1` after version `v` is visible;
   - expect `E_COH_STALE_VERSION`.

3. **Reordered completion**
   - expose producer completion before its write is visible;
   - expect `E_COH_COMPLETION_ORDER`.

4. **Fence omission**
   - signal the consumer without the declared producer fence;
   - expect `E_COH_FENCE_MISSING`.

5. **Policy-digest mismatch**
   - change one tile's accepted digest;
   - expect `E_POLICY_DIGEST`.

6. **Required-record drop**
   - force one tile backend to refuse a required record;
   - expect `E_RECORD_DROP` and a failed covered access.

Each fault case records the injected tile, event, line, epoch, expected code,
observed code, and first divergent joined record.

## 10. Fail-Stop and Error Handling

### 10.1 Local failure

If a required request or completion record cannot commit:

- the affected covered access returns failure;
- the tile increments its exact error and reject or drop counter;
- the tile emits a bounded diagnostic;
- no successful completion is exposed; and
- the coordinator marks the global epoch invalid.

### 10.2 Global epoch failure

Other tiles may retain their private state, but their work cannot be reported
as a successful partial epoch. Recovery requires:

1. quiescing or ending the failed phase;
2. sealing the failed evidence;
3. creating a new epoch ID;
4. reinstalling or confirming the policy digest; and
5. taking new zero/baseline counter snapshots.

### 10.3 Backend failure

An explicitly selected backend never falls back:

- Rust failure does not select FPGA or off;
- FPGA-Verilator failure does not select Rust;
- GPU diagnostic failure does not select simulation; and
- `auto` follows only the approved capability ordering.

### 10.4 Evidence failure

Missing process identity, binary hash, server sequence, record join, checksum,
counter bracket, or seal makes the boot invalid. The runner preserves the raw
directory and first stable failure code. A retry gets a new UUID and points to
the failed attempt with `retry_of`.

## 11. Paper Rewrite

### 11.1 Title

Use:

> **SlugArch: Replayable Small CXL Tiles for Composable Accelerators**

### 11.2 Narrative order

The paper tells one five-step story:

1. One opaque GPU-class endpoint is a large debugging and failure domain.
2. Small Type-2 tiles make compute independently composable around a
   host-mediated coherent memory space.
3. More tiles distribute causality and create more memory and offload
   boundaries.
4. SlugArch places the same verified Hardware JIT at each boundary and joins
   its attributable records by epoch.
5. QEMU, CXLMemSim, Rust, and Verilator test correctness, recorder scaling,
   diagnosis, and backend equivalence.

### 11.3 Contribution list

Use four contributions:

1. a small-tile CXL architecture with a per-tile replay boundary;
2. a bounded, versioned Hardware-JIT policy and BAR2 capability;
3. a fail-stop cross-tile replay and fault-localization contract; and
4. an artifact-backed QEMU/Rust/Verilator evaluation with five-boot evidence.

### 11.4 Page budget

Main text must end no later than page 11. References start after a forced page
boundary.

| Section | Page budget |
| --- | ---: |
| Abstract and introduction | 1.25 |
| Background | 0.75 |
| Tile architecture and Figure 1 | 1.50 |
| J-extension and implementation | 1.50 |
| Replay contract and proof sketch | 1.00 |
| Evaluation and Figure 2 | 3.25 |
| Limitations, related work, conclusion | 1.25 |
| Float and transition allowance | 0.50 |
| **Total** | **11.00** |

Do not change margins, paper geometry, or use figure text below 7 pt.

### 11.5 Required cuts

The current approximately 20-page draft is reduced by:

- replacing four quantitative tables with one four-panel results figure;
- reducing the multi-page semantics treatment to one property and compact proof
  sketch;
- replacing the evaluation roadmap with the four approved RQs and a short
  limitations paragraph;
- deduplicating proof-boundary and claim-status prose;
- compressing deployment stages to at most two paragraphs;
- removing CFR terminology;
- removing unsupported or repeated CXL.cache, migration, security, recovery,
  and physical-FPGA claims; and
- retaining only related work that distinguishes the approved thesis.

## 12. Figures

The final main text has exactly two evaluation figures and no quantitative
result table.

### Figure 1: Small-tile architecture

Show:

- host CPU and coherence home agent;
- shared host memory;
- CXL switch;
- 1 to N Type-2 compute tiles;
- one local Hardware JIT per tile;
- optional Type-3 memory;
- global coordinator and log join; and
- a legend distinguishing proposed physical, implemented QEMU, event-level
  modeled, and blocked paths.

The visual contrast may show one large opaque accelerator replaced by several
small tiles, but it must not contain an NVIDIA performance, area, power, or
yield number.

### Figure 2: Critical results

Use four panels:

1. **Latency calibration**
   - one tile;
   - 80, 400, 2,000, and 10,000 ns;
   - off, Rust, and FPGA-Verilator;
   - median with min/max whiskers.

2. **Recorder cost versus tile count**
   - 1, 2, 4, and 8 tiles at 400 ns;
   - per-event and aggregate metrics;
   - visible backend legend.

3. **Metadata by record mode**
   - validation, delta, and full;
   - four tiles at 400 ns;
   - bytes per operation and zero-drop annotation.

4. **Fault localization and equivalence**
   - host-only versus SlugArch exact localization;
   - Rust/RTL semantic mismatch count;
   - detected injected faults; and
   - blocked physical-FPGA status where applicable.

Do not mix unlike units on an unlabeled axis.

## 13. References and Citation Changes

Add this BibTeX entry:

```bibtex
@inproceedings{legoos,
  author = {Yizhou Shan and Yutong Huang and Yilun Chen and Yiying Zhang},
  title = {{LegoOS}: A Disseminated, Distributed {OS} for Hardware Resource Disaggregation},
  booktitle = {13th USENIX Symposium on Operating Systems Design and Implementation (OSDI 18)},
  year = {2018},
  isbn = {978-1-939133-08-3},
  address = {Carlsbad, CA},
  pages = {69--87},
  publisher = {USENIX Association},
  month = oct,
  url = {https://www.usenix.org/conference/osdi18/presentation/shan}
}
```

Use LegoOS in background or related work to motivate component independence,
not as prior CXL or replay work.

Retain the existing CXL, DirectCXL, Pond, TMO, UCIe, replay, tracing, GPU
checkpointing, and programmable-memory references only where the compressed
text cites them.

## 14. Claim Ledger

The paper and normalized results use these statuses:

| Claim | Status required |
| --- | --- |
| QEMU Type-2 direct CFMWS request/response | QEMU artifact-backed |
| Five-boot latency calibration | QEMU artifact-backed after monotonic gate |
| Multi-tile topology and J counters | QEMU artifact-backed |
| Shared-line ordering and injected faults | event-level model |
| Rust policy execution | software artifact-backed |
| FPGA Hardware-JIT semantics | Verilator RTL |
| FPGA area/Fmax | matched fit only, otherwise blocked |
| Native PTX compiler diagnostics | real GPU only, otherwise blocked |
| Physical CXL.cache | blocked |
| Direct peer coherence | not evaluated |
| Switch ordering | blocked |
| Small-tile power, area, yield, or performance advantage | not evaluated |
| NVIDIA performance comparison | not claimed |

Every plotted numeric field resolves:

```text
paper macro or figure datum
  -> normalized JSON key
  -> campaign and cell identity
  -> five boot UUIDs
  -> raw source files
  -> SHA256SUMS
```

## 15. Acceptance Criteria

The design is implemented only when:

1. The paper uses SlugArch as the sole system name and contains no stale CFR
   system naming.
2. The title and introduction tell the approved small-tile debugging story.
3. LegoOS is cited accurately as hardware resource disaggregation, not CXL or
   replay.
4. QEMU enumerates each declared tile with a unique identity and isolated
   J-extension state.
5. The direct 80 MiB CFMWS sentinel is server-authoritative and joins exact
   counters.
6. The event-level host-home-agent model is labeled as a model everywhere.
7. Every supported non-fault Rust/RTL case is semantically exact.
8. All six fault injections produce their declared first-failure code and
   exact tile/event attribution.
9. The 125-entry expanded boot list has five valid fresh boots per matrix
   cell.
10. Calibration medians are strictly increasing with configured latency.
11. No required record is dropped in an eligible result.
12. Figure 2 is rendered only from checksum-valid normalized JSON.
13. The main text ends on or before page 11 before references.
14. Exactly two evaluation figures and no quantitative result table remain.
15. Physical CXL.cache, direct peer coherence, switch ordering, FPGA-board,
    power, area, yield, and NVIDIA performance claims remain explicitly
    blocked or not evaluated.

## 16. Non-Goals

This design does not:

- implement LegoOS or a new operating system;
- define a new CXL coherence protocol;
- replace the host home agent;
- prove physical CXL.cache behavior;
- implement direct tile-to-tile coherence;
- prove that small tiles outperform a monolithic GPU;
- estimate die yield, cost, area, frequency, or power;
- claim that the local packet encoding is a standards-compliant CXL FLIT;
- turn CUDA PTX compilation into the SlugArch replay-policy JIT; or
- broaden the paper beyond the approved debugging and replay thesis.
