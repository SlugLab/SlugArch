#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 RUN_DIR" >&2
    exit 2
fi

RUN_DIR=$1
GUEST_SSH_CMD=${CXLMEMSIM_GUEST_SSH_CMD:?set CXLMEMSIM_GUEST_SSH_CMD, for example: ssh root@192.168.122.10}
GUEST_SCP_TARGET=${CXLMEMSIM_GUEST_SCP_TARGET:?set CXLMEMSIM_GUEST_SCP_TARGET, for example: root@192.168.122.10}
CXLMEMSIM_ROOT=${CXLMEMSIM_ROOT:-/home/victoryang00/CXLMemSim}
GUEST_DIR=${CXLMEMSIM_GUEST_DIR:-/root/slugarch-qemu-type2}
read -r -a GUEST_SSH_ARR <<<"$GUEST_SSH_CMD"

mkdir -p "$RUN_DIR"
{
    echo "guest_ssh_cmd=$GUEST_SSH_CMD"
    echo "guest_scp_target=$GUEST_SCP_TARGET"
    echo "cxlmemsim_root=$CXLMEMSIM_ROOT"
    echo "guest_dir=$GUEST_DIR"
} >>"$RUN_DIR/commands.txt"

command -v scp >/dev/null || { echo "scp not found" >&2; exit 1; }
command -v cargo >/dev/null || true
test -r "$RUN_DIR/requests.bin" || { echo "requests.bin is not readable" >&2; exit 1; }

if [[ ! -f "$CXLMEMSIM_ROOT/qemu_integration/slugarch_type2_guest.c" ]]; then
    echo "missing CXLMemSim guest helper source; implement qemu_integration/slugarch_type2_guest.c first" >&2
    exit 1
fi

append_command() {
    local label=$1
    shift
    printf '%s=' "$label" >>"$RUN_DIR/commands.txt"
    printf '%q ' "$@" >>"$RUN_DIR/commands.txt"
    printf '\n' >>"$RUN_DIR/commands.txt"
}

append_command mkdir_guest_dir "${GUEST_SSH_ARR[@]}" "mkdir -p '$GUEST_DIR'"
"${GUEST_SSH_ARR[@]}" "mkdir -p '$GUEST_DIR'"
append_command scp_requests scp "$RUN_DIR/requests.bin" "$CXLMEMSIM_ROOT/qemu_integration/slugarch_type2_guest.c" "$GUEST_SCP_TARGET:$GUEST_DIR/"
scp "$RUN_DIR/requests.bin" "$CXLMEMSIM_ROOT/qemu_integration/slugarch_type2_guest.c" "$GUEST_SCP_TARGET:$GUEST_DIR/"
append_command guest_build_run "${GUEST_SSH_ARR[@]}" "cd '$GUEST_DIR' && gcc -O2 -Wall -Wextra -o slugarch_type2_guest slugarch_type2_guest.c && sudo ./slugarch_type2_guest --requests requests.bin --responses responses.bin --summary guest-summary.json"
"${GUEST_SSH_ARR[@]}" "cd '$GUEST_DIR' && gcc -O2 -Wall -Wextra -o slugarch_type2_guest slugarch_type2_guest.c && sudo ./slugarch_type2_guest --requests requests.bin --responses responses.bin --summary guest-summary.json"
append_command scp_responses scp "$GUEST_SCP_TARGET:$GUEST_DIR/responses.bin" "$RUN_DIR/responses.bin"
scp "$GUEST_SCP_TARGET:$GUEST_DIR/responses.bin" "$RUN_DIR/responses.bin"
append_command scp_guest_summary scp "$GUEST_SCP_TARGET:$GUEST_DIR/guest-summary.json" "$RUN_DIR/guest-summary.json"
scp "$GUEST_SCP_TARGET:$GUEST_DIR/guest-summary.json" "$RUN_DIR/guest-summary.json"
