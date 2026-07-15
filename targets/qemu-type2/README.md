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
