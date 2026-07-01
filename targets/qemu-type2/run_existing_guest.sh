#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 RUN_DIR" >&2
    exit 2
fi

RUN_DIR=$1
GUEST_SSH=${CXLMEMSIM_GUEST_SSH:?set CXLMEMSIM_GUEST_SSH, for example: ssh root@192.168.122.10}
CXLMEMSIM_ROOT=${CXLMEMSIM_ROOT:-/home/victoryang00/CXLMemSim}
GUEST_DIR=${CXLMEMSIM_GUEST_DIR:-/root/slugarch-qemu-type2}

mkdir -p "$RUN_DIR"
{
    echo "guest_ssh=$GUEST_SSH"
    echo "cxlmemsim_root=$CXLMEMSIM_ROOT"
    echo "guest_dir=$GUEST_DIR"
} >>"$RUN_DIR/commands.txt"

if [[ ! -f "$RUN_DIR/requests.bin" ]]; then
    echo "missing $RUN_DIR/requests.bin; run slugarch export-cxlmemsim first" >&2
    exit 1
fi

if [[ ! -f "$CXLMEMSIM_ROOT/qemu_integration/slugarch_type2_guest.c" ]]; then
    echo "missing CXLMemSim guest helper source; implement qemu_integration/slugarch_type2_guest.c first" >&2
    exit 1
fi

$GUEST_SSH "mkdir -p '$GUEST_DIR'"
scp "$RUN_DIR/requests.bin" "$CXLMEMSIM_ROOT/qemu_integration/slugarch_type2_guest.c" "$GUEST_SSH:$GUEST_DIR/"
$GUEST_SSH "cd '$GUEST_DIR' && gcc -O2 -Wall -Wextra -o slugarch_type2_guest slugarch_type2_guest.c && sudo ./slugarch_type2_guest --requests requests.bin --responses responses.bin --summary guest-summary.json"
scp "$GUEST_SSH:$GUEST_DIR/responses.bin" "$RUN_DIR/responses.bin"
scp "$GUEST_SSH:$GUEST_DIR/guest-summary.json" "$RUN_DIR/guest-summary.json"
