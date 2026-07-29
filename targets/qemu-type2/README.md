# SlugArch QEMU Type-2 Target

This target evaluates SlugArch through the CXLMemSim QEMU `cxl-type2` BAR2
path. It is the simulator-backed replacement for `targets/agilex-vr2` hardware
JIT evaluation.

## One-Guest Existing-VM Flow

This host harness assumes a supported CXLMemSim Type-2 guest is already
running and reachable over SSH. It depends on the future
`qemu_integration/slugarch_type2_guest.c` helper described in the design; it
does not launch QEMU or capture QEMU/CXLMemSim logs itself.

```bash
RUN_DIR=artifact/slugarch_cxlmemsim/$(date -u +%Y%m%d-%H%M%S)
cargo run -p slugarch-cli -- export-cxlmemsim \
  targets/qemu-type2/identity_times_const.json --out "$RUN_DIR"
CXLMEMSIM_GUEST_SSH_CMD="ssh root@GUEST" \
CXLMEMSIM_GUEST_SCP_CMD="scp" \
CXLMEMSIM_GUEST_SCP_TARGET="root@GUEST" \
  targets/qemu-type2/run_existing_guest.sh "$RUN_DIR"
cargo run -p slugarch-cli -- validate-cxlmemsim \
  targets/qemu-type2/identity_times_const.json \
  --responses "$RUN_DIR/responses.bin" --out "$RUN_DIR"
```

For this existing-guest harness, the CLI validation pass is `summary.json`
reporting `status: "pass"`. For the full evaluation evidence, also archive the
QEMU and CXLMemSim logs from the supported Type-2 launch and smoke path.

## Live Sweep Flow

`run_live_knob_sweep.sh` launches the compatible CXLMemSim server and QEMU
Type-2 guest, waits for guest SSH, runs five BAR2 helper passes through
`run_existing_guest.sh`, and then emits the simulator-feasible replay-policy
report:

```bash
targets/qemu-type2/run_live_knob_sweep.sh
```

The current guest image reaches SSH reliably with QEMU user networking through
`virtio-net-pci`. A previous `e1000` launch reached the serial login prompt but
timed out during SSH banner exchange, so the live sweep script keeps
`virtio-net-pci` as the default SSH NIC.

## Formal direct-CFMWS campaigns

The paper artifacts use filtered qtests that launch one fresh QEMU process per
observation. They exercise the QEMU Type-2 endpoint, its protocol-valid fake
CXLMemSim service, and either the Rust policy backend or the Verilated RTL
model. They do not measure a physical CXL link or post-fit FPGA timing.

Build both policy libraries before running the campaigns:

```bash
cargo build -p slugarch-jit-ffi --release
env CCACHE_DISABLE=1 \
  CARGO_TARGET_DIR=target/fpga-formal \
  VERILATOR_ROOT=/path/to/verilator/share/verilator \
  cargo build -p slugarch-jit-ffi --release --features fpga-verilator
```

Set `QEMU_BUILD` to a configured QEMU build directory and `QEMU_SOURCE` to the
matching source checkout. The output directories must not already exist.

```bash
QEMU_BUILD=/path/to/qemu-build
QEMU_SOURCE=/path/to/qemu
ARTIFACT_ROOT=artifact/slugarch_cxlmemsim

python3 targets/qemu-type2/run_jit_cfmws_sweep.py \
  --qemu-build "$QEMU_BUILD" \
  --qemu-source "$QEMU_SOURCE" \
  --rust-library target/release/libslugarch_jit_ffi.so \
  --fpga-library target/fpga-formal/release/libslugarch_jit_ffi.so \
  --policy targets/qemu-type2/cfmws_validation_policy.json \
  --output-root \
    "$ARTIFACT_ROOT/qemu-jit-cfmws-latency-sweep-20260729"

python3 targets/qemu-type2/run_jit_cfmws_failstop.py \
  --qemu-build "$QEMU_BUILD" \
  --qemu-source "$QEMU_SOURCE" \
  --rust-library target/release/libslugarch_jit_ffi.so \
  --fpga-library target/fpga-formal/release/libslugarch_jit_ffi.so \
  --policy targets/qemu-type2/cfmws_reject_policy.json \
  --output-root \
    "$ARTIFACT_ROOT/qemu-jit-cfmws-failstop-20260729"

python3 targets/qemu-type2/run_qemu_cxl_fault_matrix.py \
  --qemu-build "$QEMU_BUILD" \
  --qemu-source "$QEMU_SOURCE" \
  --output-root "$ARTIFACT_ROOT/qemu-cxl-fault-matrix-20260729"
```

The campaign shapes are fail-closed: four latency points, both backends, and
five repeats for the sweep; both backends and five repeats for real-backend
rejection; and ten fault cases with five repeats for the matrix. A zero qtest
exit is insufficient: the runners require one exact, non-skipped TAP result.
After collection, verify every manifest, raw-file hash, TAP result, campaign
shape, and deterministic summary:

```bash
python3 targets/qemu-type2/verify_qemu_cxl_artifacts.py \
  --sweep "$ARTIFACT_ROOT/qemu-jit-cfmws-latency-sweep-20260729" \
  --failstop "$ARTIFACT_ROOT/qemu-jit-cfmws-failstop-20260729" \
  --fault-matrix "$ARTIFACT_ROOT/qemu-cxl-fault-matrix-20260729"
```
