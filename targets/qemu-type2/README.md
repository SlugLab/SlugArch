# SlugArch QEMU Type-2 Target

This target evaluates SlugArch through the CXLMemSim QEMU `cxl-type2` BAR2
path. It is the simulator-backed replacement for `targets/agilex-vr2` hardware
JIT evaluation.

## One-Guest Existing-VM Flow

```bash
RUN_DIR=artifact/slugarch_cxlmemsim/$(date -u +%Y%m%d-%H%M%S)
cargo run -p slugarch-cli -- export-cxlmemsim \
  targets/qemu-type2/identity_times_const.json --out "$RUN_DIR"
CXLMEMSIM_GUEST_SSH="ssh root@GUEST" \
  targets/qemu-type2/run_existing_guest.sh "$RUN_DIR"
cargo run -p slugarch-cli -- validate-cxlmemsim \
  targets/qemu-type2/identity_times_const.json \
  --responses "$RUN_DIR/responses.bin" --out "$RUN_DIR"
```

The run passes only when `summary.json` reports `status: "pass"` and the QEMU
log shows Type-2 device realization plus Slug bridge activity.
