#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
HELPER_SRC="$REPO_ROOT/targets/qemu-type2/slugarch_type2_guest.c"
HEADER_DIR=${CXLMEMSIM_GUEST_HEADER_DIR:-/home/victoryang00/CXLMemSim/qemu_integration/guest_libcuda}
OUT_DIR=${1:-/tmp/slugarch-qemu-type2-helper-check}

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"
mkdir -p "$OUT_DIR/guest_libcuda"
cp "$HEADER_DIR/cxl_gpu_cmd.h" "$OUT_DIR/guest_libcuda/cxl_gpu_cmd.h"

gcc -O2 -Wall -Wextra -I"$OUT_DIR" -o "$OUT_DIR/slugarch_type2_guest" "$HELPER_SRC"
cargo run -q -p slugarch-cli -- export-cxlmemsim \
  "$REPO_ROOT/targets/qemu-type2/identity_times_const.json" --out "$OUT_DIR"
"$OUT_DIR/slugarch_type2_guest" --no-bar2 \
  --requests "$OUT_DIR/requests.bin" \
  --responses "$OUT_DIR/responses.bin" \
  --summary "$OUT_DIR/guest-summary.json"
cargo run -q -p slugarch-cli -- validate-cxlmemsim \
  "$REPO_ROOT/targets/qemu-type2/identity_times_const.json" \
  --responses "$OUT_DIR/responses.bin" --out "$OUT_DIR"
