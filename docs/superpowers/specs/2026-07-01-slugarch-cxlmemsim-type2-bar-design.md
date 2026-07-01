# SlugArch CXLMemSim Type-2 BAR Evaluation Design

## Purpose

Evaluate SlugArch through the full CXLMemSim QEMU Type-2 path instead of
generating a hardware-JIT FPGA image. The proof boundary is guest-visible BAR2
MMIO on the `cxl-type2` QEMU device, not SlugArch's in-process Verilator
endpoint and not Quartus-generated RTL.

The first milestone evaluates the existing 4x4 GEMM workload. SlugArch remains
the source of truth for request generation and validation. CXLMemSim/QEMU
provides the Type-2 device boundary, command transport, software endpoint
execution, and simulator evidence.

## Current Baseline

SlugArch already has:

- `slugarch run-cxl <job.json>` using `CxlHost` and `VerilatedIp`.
- `build_gemm_dispatch_stream(job, first_tag)`, which emits 49 CXL messages.
- A documented 64-byte v1 FLIT layout in `slugarch-cxl-wire`.
- Replay recording and validation summaries in `slugarch-host`.

CXLMemSim already has:

- A QEMU `cxl-type2` device with BAR2 GPU command registers.
- Guest-side BAR2 discovery through PCI vendor `0x8086`, device `0x0d92`.
- `qemu_integration/smoke_type2_endpoint.sh`, which verifies Type-2 device
  realization and CXLMemSim server connectivity.
- A newer Zettai launch path in `qemu_integration/launch_qemu_vcs_dcd_gfam.sh`.

The malformed `launch_qemu_type2_gpu.sh` and `launch_qemu_type2_hetgpu.sh`
scripts are not part of this design.

## Design Choice

Use a BAR2 FLIT bridge.

SlugArch exports the exact 64-byte request FLIT stream for a `GemmJob`. A guest
helper maps the Type-2 BAR2 command window, submits each request FLIT, reads the
matching response FLIT, and writes a response artifact. QEMU's Type-2 device
implements a software SlugCXL endpoint model behind new BAR2 commands. The host
validates the returned response stream with SlugArch.

This preserves the residual-flow evaluation boundary: each SlugArch fabric
transaction crosses a guest-visible Type-2 BAR path, while avoiding FPGA
hardware-JIT generation.

## Non-Goals

- No Quartus synthesis, fit, assembly, SOF generation, or FPGA flashing.
- No CXL 2.0/3.0 spec-compliant FLIT encoding beyond SlugArch's current v1
  64-byte layout.
- No general PTX-over-CXL routing in the first milestone.
- No use of CXLMemSim's CUDA/hetGPU/PTX command path for SlugArch.
- No dependency on the stale Type-2 GPU launch shell scripts.
- No multi-in-flight request support in the first milestone.

## Components

### SlugArch Export and Validation

Add a simulator-oriented CLI path that can:

- Read a `GemmJob` JSON file.
- Emit `requests.bin`, a concatenation of 49 encoded 64-byte request FLITs.
- Emit `expected.json`, containing the expected 4x4 result, request count, tags,
  and workload metadata.
- Validate `responses.bin`, a concatenation of response FLITs captured from the
  guest run.
- Emit `summary.json` and `summary.csv` with success/failure state, result
  matrix, tag checks, request/response counts, replay bytes, and artifact paths.

The export path must reuse `build_gemm_dispatch_stream` and
`slugarch_cxl_wire::encode`. The validation path must reuse
`slugarch_cxl_wire::decode`; read-response validation must use the same result
matrix decoding rules as `slugarch-host`.

### CXLMemSim QEMU Type-2 BAR Commands

Extend the BAR2 command ABI with SlugArch-specific commands in the unused
`0xE0` range:

| Command | Name | Behavior |
| --- | --- | --- |
| `0xE0` | `CXL_GPU_CMD_SLUG_QUERY` | Return Slug bridge version, FLIT size, queue depth, and capability bits. |
| `0xE1` | `CXL_GPU_CMD_SLUG_RESET` | Reset endpoint model state, response queue, counters, and error state. |
| `0xE2` | `CXL_GPU_CMD_SLUG_SUBMIT_FLIT` | Read one 64-byte request FLIT from BAR2 data offset `0x1000`, execute it, enqueue one response FLIT. |
| `0xE3` | `CXL_GPU_CMD_SLUG_READ_FLIT` | Copy the oldest response FLIT into BAR2 data offset `0x1000`. |
| `0xE4` | `CXL_GPU_CMD_SLUG_GET_STATS` | Return submitted, completed, failed, load, compute, read, and bad-tag counters. |

Command completion uses the existing BAR2 `CMD_STATUS`, `CMD_RESULT`, and
`RESULT0..RESULT3` registers. `SLUG_SUBMIT_FLIT` and `SLUG_READ_FLIT` are
synchronous for the first milestone.

### Software SlugCXL Endpoint Model

The QEMU endpoint model executes the current SlugCXL 4x4 GEMM token protocol:

- Accept `M2SRwD::MemWr` to dispatch address `0x2000`.
- Extract load and compute tokens from request data bytes `[0..4]`.
- Accept `M2SReq::MemRd` with the read token in `addr[63:32]`.
- Maintain 16x16 A/B/C storage internally, using the top-left 4x4 region.
- Return `S2MNDR::Cmp` for valid load and compute requests.
- Return `S2MDRS::MemData` for valid read requests, with the 24-bit result in
  response data bytes `[0..3]`.
- Return `S2MNDR::DispatchFailed` for malformed class, opcode, address, token,
  or state-order errors.

The model must preserve request tags in responses. It should keep deterministic
state so two identical request streams produce byte-identical response streams.

### Guest Helper

Add a guest helper under `qemu_integration/` that:

- Finds the guest PCI device `8086:0d92`.
- Maps BAR2 through `/sys/bus/pci/devices/<BDF>/resource2`.
- Verifies `CXL_GPU_MAGIC == 0x43584C32`.
- Calls `SLUG_QUERY` and checks `flit_size == 64`.
- Calls `SLUG_RESET`.
- Reads `requests.bin`.
- For each 64-byte request: writes it to BAR2 data offset `0x1000`, issues
  `SLUG_SUBMIT_FLIT`, issues `SLUG_READ_FLIT`, and appends the returned 64-byte
  response to `responses.bin`.
- Writes `guest-summary.json` with PCI BDF, command counts, elapsed time,
  command failures, and BAR bridge stats.

The helper should be a small C program so it can build inside minimal guests
with `gcc`. It must not require Rust or Verilator inside the guest.

### Harness and Artifacts

Add a host harness that creates one timestamped run directory:

```text
artifact/slugarch_cxlmemsim/YYYYmmdd-HHMMSS/
  job.json
  requests.bin
  expected.json
  responses.bin
  guest-summary.json
  summary.json
  summary.csv
  qemu.log
  cxlmemsim-server.log
  commands.txt
```

The harness should support two modes:

- `--existing-guest`: use an already running guest over SSH.
- `--launch`: launch the supported CXLMemSim Type-2 QEMU path, then run the
  guest helper over SSH.

The first reliable implementation may require `--existing-guest` if the local
guest image, kernel, or SSH path is environment-specific.

## Data Flow

1. Host SlugArch reads `job.json`.
2. Host SlugArch writes `requests.bin` and `expected.json`.
3. Harness copies `requests.bin` and the guest helper into the QEMU guest.
4. Guest helper maps Type-2 BAR2 and queries the Slug bridge.
5. Guest helper submits one request FLIT at a time through BAR2.
6. QEMU Type-2 model decodes the request, updates endpoint state, and enqueues
   one response FLIT.
7. Guest helper reads each response FLIT and writes `responses.bin`.
8. Harness copies `responses.bin` and `guest-summary.json` back to the host.
9. Host SlugArch validates tags, response classes, matrix result, and replay
   summary, then writes `summary.json` and `summary.csv`.

## Error Handling

SlugArch validation fails if:

- `responses.bin` is not a multiple of 64 bytes.
- Response count is not 49.
- Any response has an unknown class/opcode or nonzero reserved bytes.
- Any response tag differs from the matching request tag.
- Any load/compute response is not `S2MNDR::Cmp`.
- Any read response is not `S2MDRS::MemData`.
- The decoded 4x4 result does not match `expected.json`.

Guest helper failures include:

- Type-2 device not found.
- BAR2 cannot be opened or mapped.
- BAR2 magic/version mismatch.
- Slug bridge query fails or reports an unsupported FLIT size.
- BAR command timeout or non-success `CMD_RESULT`.

QEMU model failures return `DispatchFailed` response FLITs and increment bridge
error counters. Fatal internal state errors should also be printed to `qemu.log`
with a `CXL Type2 SlugBridge:` prefix.

## Evidence Requirements

A successful run must provide:

- `summary.json` with `status: "pass"`.
- `summary.csv` with one row for the run.
- `requests.bin` containing 49 request FLITs.
- `responses.bin` containing 49 response FLITs.
- Matching request and response tags.
- Result matrix equal to `expected.json`.
- QEMU log lines showing Type-2 realization and Slug bridge activity.
- CXLMemSim server log showing the server was started or connected.
- `commands.txt` recording the exact host and guest commands.

## Testing Plan

### SlugArch Unit Tests

- Exporting the identity-times-constant job writes exactly `49 * 64` request
  bytes.
- Validation rejects truncated response artifacts.
- Validation rejects tag mismatch.
- Validation rejects a `DispatchFailed` response in a load or compute slot.
- Validation accepts a known-good response stream generated by the existing
  Verilator path or a checked-in fixture.

### CXLMemSim Unit and Shell Tests

- QEMU Slug bridge decode rejects bad class, bad opcode, bad dispatch address,
  and nonzero reserved bytes.
- QEMU Slug bridge returns deterministic responses for the identity GEMM stream.
- Guest helper can be syntax-checked and built with `gcc -O2 -Wall`.
- `smoke_type2_endpoint.sh` continues to pass after adding Slug commands.

### End-to-End Test

Run:

```bash
slugarch export-cxlmemsim tests/fixtures/identity_times_const.json --out RUN_DIR
CXLMEMSIM_GUEST_SSH="ssh root@GUEST" \
  /home/victoryang00/CXLMemSim/qemu_integration/slugarch_type2_bar_run.sh \
  --requests RUN_DIR/requests.bin --out RUN_DIR
slugarch validate-cxlmemsim tests/fixtures/identity_times_const.json \
  --responses RUN_DIR/responses.bin --out RUN_DIR
```

Expected result:

- Host validation passes.
- `summary.json` reports 49 requests, 49 responses, zero tag mismatches, zero
  dispatch failures, and the expected matrix.
- QEMU and CXLMemSim logs are copied into the run directory.

## Open Constraints

- The implementation must stay path-limited because the CXLMemSim checkout is
  already dirty.
- The first live run may need an existing guest with SSH because disk image,
  kernel, and network setup are machine-specific.
- The design assumes BAR2 command IDs `0xE0..0xE4` are unused by current
  CXLMemSim commands.
- If the QEMU server protocol does not expose useful counters for Slug events,
  the first run may rely on QEMU bridge stats plus server connection evidence.

## Success Criteria

The work is complete when a user can run one command sequence that produces a
timestamped artifact directory proving the identity GEMM workload crossed the
guest-visible CXLMemSim QEMU Type-2 BAR path and validated successfully in
SlugArch, without invoking Quartus or generating hardware-JIT FPGA collateral.
