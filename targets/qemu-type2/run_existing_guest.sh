#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 RUN_DIR" >&2
    exit 2
fi

RUN_DIR=$1
GUEST_SSH_CMD=${CXLMEMSIM_GUEST_SSH_CMD:?set CXLMEMSIM_GUEST_SSH_CMD, for example: ssh root@192.168.122.10}
GUEST_SCP_CMD=${CXLMEMSIM_GUEST_SCP_CMD:-scp}
GUEST_SCP_TARGET=${CXLMEMSIM_GUEST_SCP_TARGET:?set CXLMEMSIM_GUEST_SCP_TARGET, for example: root@192.168.122.10}
CXLMEMSIM_ROOT=${CXLMEMSIM_ROOT:-/home/victoryang00/CXLMemSim}
GUEST_DIR=${CXLMEMSIM_GUEST_DIR:-/root/slugarch-qemu-type2}
read -r -a GUEST_SSH_ARR <<<"$GUEST_SSH_CMD"
read -r -a GUEST_SCP_ARR <<<"$GUEST_SCP_CMD"

mkdir -p "$RUN_DIR"
{
    echo "guest_ssh_cmd=$GUEST_SSH_CMD"
    echo "guest_scp_cmd=$GUEST_SCP_CMD"
    echo "guest_scp_target=$GUEST_SCP_TARGET"
    echo "cxlmemsim_root=$CXLMEMSIM_ROOT"
    echo "guest_dir=$GUEST_DIR"
} >>"$RUN_DIR/commands.txt"

command -v "${GUEST_SCP_ARR[0]}" >/dev/null || { echo "${GUEST_SCP_ARR[0]} not found" >&2; exit 1; }
command -v cargo >/dev/null || true
test -r "$RUN_DIR/requests.bin" || { echo "requests.bin is not readable" >&2; exit 1; }

if [[ ! -f "$CXLMEMSIM_ROOT/qemu_integration/slugarch_type2_guest.c" ]]; then
    echo "missing CXLMemSim guest helper source; implement qemu_integration/slugarch_type2_guest.c first" >&2
    exit 1
fi
if [[ ! -f "$CXLMEMSIM_ROOT/qemu_integration/guest_libcuda/cxl_gpu_cmd.h" ]]; then
    echo "missing CXLMemSim guest command header; implement qemu_integration/guest_libcuda/cxl_gpu_cmd.h first" >&2
    exit 1
fi

append_command() {
    local label=$1
    shift
    printf '%s=' "$label" >>"$RUN_DIR/commands.txt"
    printf '%q ' "$@" >>"$RUN_DIR/commands.txt"
    printf '\n' >>"$RUN_DIR/commands.txt"
}

append_command mkdir_guest_dir "${GUEST_SSH_ARR[@]}" "mkdir -p '$GUEST_DIR/guest_libcuda'"
"${GUEST_SSH_ARR[@]}" "mkdir -p '$GUEST_DIR/guest_libcuda'"
append_command scp_requests "${GUEST_SCP_ARR[@]}" "$RUN_DIR/requests.bin" "$CXLMEMSIM_ROOT/qemu_integration/slugarch_type2_guest.c" "$CXLMEMSIM_ROOT/qemu_integration/guest_libcuda/cxl_gpu_cmd.h" "$GUEST_SCP_TARGET:$GUEST_DIR/"
"${GUEST_SCP_ARR[@]}" "$RUN_DIR/requests.bin" "$CXLMEMSIM_ROOT/qemu_integration/slugarch_type2_guest.c" "$CXLMEMSIM_ROOT/qemu_integration/guest_libcuda/cxl_gpu_cmd.h" "$GUEST_SCP_TARGET:$GUEST_DIR/"
append_command move_guest_header "${GUEST_SSH_ARR[@]}" "mv '$GUEST_DIR/cxl_gpu_cmd.h' '$GUEST_DIR/guest_libcuda/cxl_gpu_cmd.h'"
"${GUEST_SSH_ARR[@]}" "mv '$GUEST_DIR/cxl_gpu_cmd.h' '$GUEST_DIR/guest_libcuda/cxl_gpu_cmd.h'"
GUEST_RUN_CMD="cd '$GUEST_DIR' && gcc -O2 -Wall -Wextra -o slugarch_type2_guest slugarch_type2_guest.c && if command -v sudo >/dev/null 2>&1; then sudo ./slugarch_type2_guest --requests requests.bin --responses responses.bin --summary guest-summary.json; else ./slugarch_type2_guest --requests requests.bin --responses responses.bin --summary guest-summary.json; fi"
append_command guest_build_run "${GUEST_SSH_ARR[@]}" "$GUEST_RUN_CMD"
"${GUEST_SSH_ARR[@]}" "$GUEST_RUN_CMD"
append_command scp_responses "${GUEST_SCP_ARR[@]}" "$GUEST_SCP_TARGET:$GUEST_DIR/responses.bin" "$RUN_DIR/responses.bin"
"${GUEST_SCP_ARR[@]}" "$GUEST_SCP_TARGET:$GUEST_DIR/responses.bin" "$RUN_DIR/responses.bin"
append_command scp_guest_summary "${GUEST_SCP_ARR[@]}" "$GUEST_SCP_TARGET:$GUEST_DIR/guest-summary.json" "$RUN_DIR/guest-summary.json"
"${GUEST_SCP_ARR[@]}" "$GUEST_SCP_TARGET:$GUEST_DIR/guest-summary.json" "$RUN_DIR/guest-summary.json"
