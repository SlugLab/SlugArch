#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v quartus_sh >/dev/null 2>&1; then
  echo "quartus_sh not found. Install Quartus Prime Pro or set PATH." >&2
  exit 127
fi

if [[ -z "${SLUGCXL_AGILEX_DEVICE:-}" ]]; then
  echo "Set SLUGCXL_AGILEX_DEVICE to the exact Quartus device part for your board." >&2
  exit 2
fi

quartus_sh -t quartus/build_slugcxl_hj_sof.tcl
sof="output_files/slugcxl_hj_agilex.sof"
if [[ ! -f "$sof" ]]; then
  echo "Quartus completed but $sof was not produced" >&2
  exit 1
fi
echo "$sof"
