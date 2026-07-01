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
CXLMEMSIM_GUEST_SCP_TARGET="root@GUEST" \
  targets/qemu-type2/run_existing_guest.sh "$RUN_DIR"
cargo run -p slugarch-cli -- validate-cxlmemsim \
  targets/qemu-type2/identity_times_const.json \
  --responses "$RUN_DIR/responses.bin" --out "$RUN_DIR"
```

For this existing-guest harness, the CLI validation pass is `summary.json`
reporting `status: "pass"`. For the full evaluation evidence, also archive the
QEMU and CXLMemSim logs from the supported Type-2 launch and smoke path.
