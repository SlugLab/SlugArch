#!/usr/bin/env bash
set -euo pipefail

REPO=${REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}
CXLMEMSIM_ROOT=${CXLMEMSIM_ROOT:-/home/victoryang00/CXLMemSim}
QEMU_BINARY=${QEMU_BINARY:-$CXLMEMSIM_ROOT/lib/qemu/build/qemu-system-x86_64}
SERVER_BINARY=${SERVER_BINARY:-$CXLMEMSIM_ROOT/build/cxlmemsim_server}
KERNEL_IMAGE=${KERNEL_IMAGE:-/home/victoryang00/cxl/arch/x86/boot/bzImage}
DISK_IMAGE=${DISK_IMAGE:-$CXLMEMSIM_ROOT/build/qemu1.img}
CXL_MEMSIM_HOST=${CXL_MEMSIM_HOST:-127.0.0.1}
CXL_MEMSIM_PORT=${CXL_MEMSIM_PORT:-10199}
SSH_PORT=${SSH_PORT:-12022}
CXL_CAPACITY_MB=${CXL_CAPACITY_MB:-256}
CXL_DEFAULT_LATENCY=${CXL_DEFAULT_LATENCY:-100}
CXL_TYPE2_CACHE_SIZE=${CXL_TYPE2_CACHE_SIZE:-16M}
CXL_TYPE2_MEM_SIZE=${CXL_TYPE2_MEM_SIZE:-64M}
CXL_TYPE2_COHERENCY=${CXL_TYPE2_COHERENCY:-true}
REPLAY_REPEATS=${REPLAY_REPEATS:-20}
RUN_ID=${RUN_ID:-qemu-type2-knob-sweep-20260704-$(date -u +%H%M%S)}
ARTIFACT_DIR=${ARTIFACT_DIR:-$REPO/artifact/slugarch_cxlmemsim/$RUN_ID}

SERVER_LOG="$ARTIFACT_DIR/cxlmemsim-server.log"
QEMU_LOG="$ARTIFACT_DIR/qemu-guest.log"
HOST_LOG="$ARTIFACT_DIR/host-run.log"
LAUNCH_JSON="$ARTIFACT_DIR/launch.json"
SERVER_PID=
QEMU_PID=
STARTED_SERVER=0

mkdir -p "$ARTIFACT_DIR"

log() {
    printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*" | tee -a "$HOST_LOG"
}

server_is_up() {
    timeout 1 bash -c "</dev/tcp/$CXL_MEMSIM_HOST/$CXL_MEMSIM_PORT" >/dev/null 2>&1
}

ssh_guest() {
    ssh -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        -o ConnectTimeout=2 \
        -p "$SSH_PORT" root@127.0.0.1 "$@"
}

cleanup() {
    set +e
    if [[ -n "${QEMU_PID:-}" ]] && kill -0 "$QEMU_PID" >/dev/null 2>&1; then
        ssh_guest "sync; poweroff" >/dev/null 2>&1
        for _ in $(seq 1 30); do
            kill -0 "$QEMU_PID" >/dev/null 2>&1 || break
            sleep 1
        done
        if kill -0 "$QEMU_PID" >/dev/null 2>&1; then
            kill "$QEMU_PID" >/dev/null 2>&1
            wait "$QEMU_PID" >/dev/null 2>&1
        fi
    fi
    if [[ "$STARTED_SERVER" == 1 ]] && [[ -n "${SERVER_PID:-}" ]] &&
       kill -0 "$SERVER_PID" >/dev/null 2>&1; then
        kill "$SERVER_PID" >/dev/null 2>&1
        wait "$SERVER_PID" >/dev/null 2>&1
    fi
}
trap cleanup EXIT

cd "$REPO"

log "artifact_dir=$ARTIFACT_DIR"
log "checking inputs"
test -x "$QEMU_BINARY"
test -x "$SERVER_BINARY"
test -r "$KERNEL_IMAGE"
test -r "$DISK_IMAGE"

"$QEMU_BINARY" --version >"$ARTIFACT_DIR/qemu-version.txt"

if ! server_is_up; then
    log "starting cxlmemsim server on $CXL_MEMSIM_HOST:$CXL_MEMSIM_PORT"
    "$SERVER_BINARY" \
        --comm-mode=tcp \
        --port="$CXL_MEMSIM_PORT" \
        --capacity="$CXL_CAPACITY_MB" \
        --default_latency="$CXL_DEFAULT_LATENCY" \
        >"$SERVER_LOG" 2>&1 &
    SERVER_PID=$!
    STARTED_SERVER=1
    for _ in $(seq 1 100); do
        server_is_up && break
        if ! kill -0 "$SERVER_PID" >/dev/null 2>&1; then
            tail -100 "$SERVER_LOG" >&2 || true
            exit 1
        fi
        sleep 0.1
    done
fi
server_is_up

cat >"$LAUNCH_JSON" <<EOF
{
  "run_id": "$RUN_ID",
  "artifact_dir": "$ARTIFACT_DIR",
  "qemu_binary": "$QEMU_BINARY",
  "server_binary": "$SERVER_BINARY",
  "kernel_image": "$KERNEL_IMAGE",
  "disk_image": "$DISK_IMAGE",
  "cxlmemsim_host": "$CXL_MEMSIM_HOST",
  "cxlmemsim_port": $CXL_MEMSIM_PORT,
  "ssh_port": $SSH_PORT,
  "type2_cache_size": "$CXL_TYPE2_CACHE_SIZE",
  "type2_mem_size": "$CXL_TYPE2_MEM_SIZE",
  "type2_coherency": "$CXL_TYPE2_COHERENCY",
  "replay_repeats": $REPLAY_REPEATS
}
EOF

log "starting qemu guest on ssh port $SSH_PORT"
env CXL_TRANSPORT_MODE=tcp \
    CXL_MEMSIM_HOST="$CXL_MEMSIM_HOST" \
    CXL_MEMSIM_PORT="$CXL_MEMSIM_PORT" \
    "$QEMU_BINARY" \
        -accel tcg \
        -cpu max \
        -M q35,cxl=on,cxl-fmw.0.targets.0=cxl.0,cxl-fmw.0.size=1G \
        -m 2G \
        -smp 2 \
        -kernel "$KERNEL_IMAGE" \
        -append "root=/dev/vda rw console=ttyS0,115200 nokaslr systemd.mask=cxl-numa-setup.service" \
        -drive "file=$DISK_IMAGE,if=none,id=bootdisk,format=raw" \
        -device virtio-blk-pci,drive=bootdisk,bus=pcie.0 \
        -netdev "user,id=net0,hostfwd=tcp:127.0.0.1:$SSH_PORT-:22" \
        -device virtio-net-pci,netdev=net0,bus=pcie.0,mac=52:54:00:00:10:22 \
        -device pxb-cxl,bus_nr=12,bus=pcie.0,id=cxl.0 \
        -device cxl-rp,port=0,bus=cxl.0,id=type2_rp,chassis=0,slot=2 \
        -device "cxl-type2,bus=type2_rp,id=cxl-type2-slugarch,sn=200,gpu-mode=0,cache-size=$CXL_TYPE2_CACHE_SIZE,mem-size=$CXL_TYPE2_MEM_SIZE,cxlmemsim-addr=$CXL_MEMSIM_HOST,cxlmemsim-port=$CXL_MEMSIM_PORT,coherency-enabled=$CXL_TYPE2_COHERENCY" \
        -nographic \
        >"$QEMU_LOG" 2>&1 &
QEMU_PID=$!
echo "$QEMU_PID" >"$ARTIFACT_DIR/qemu.pid"

for i in $(seq 1 180); do
    if ssh_guest true >/dev/null 2>&1; then
        log "guest ssh is ready after ${i}s"
        break
    fi
    if ! kill -0 "$QEMU_PID" >/dev/null 2>&1; then
        tail -120 "$QEMU_LOG" >&2 || true
        exit 1
    fi
    if [[ "$i" == 180 ]]; then
        tail -120 "$QEMU_LOG" >&2 || true
        exit 1
    fi
    sleep 1
done

log "guest topology"
ssh_guest "uname -a; lspci -nn | grep -i -E 'cxl|8086:0d92' || true; ls /dev/dax* /dev/cxl* 2>/dev/null || true" \
    >"$ARTIFACT_DIR/guest-topology.txt" 2>&1 || true

for run in $(seq 1 5); do
    RUN_DIR="$ARTIFACT_DIR/run-$run"
    mkdir -p "$RUN_DIR"
    log "live BAR2 run $run"
    cargo run -q -p slugarch-cli -- export-cxlmemsim \
        targets/qemu-type2/identity_times_const.json --out "$RUN_DIR" \
        >>"$HOST_LOG" 2>&1
    CXLMEMSIM_GUEST_SSH_CMD="ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=2 -p $SSH_PORT root@127.0.0.1" \
    CXLMEMSIM_GUEST_SCP_CMD="scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -P $SSH_PORT" \
    CXLMEMSIM_GUEST_SCP_TARGET="root@127.0.0.1" \
    CXLMEMSIM_ROOT="$CXLMEMSIM_ROOT" \
        targets/qemu-type2/run_existing_guest.sh "$RUN_DIR" \
        >>"$HOST_LOG" 2>&1
    cargo run -q -p slugarch-cli -- validate-cxlmemsim \
        targets/qemu-type2/identity_times_const.json \
        --responses "$RUN_DIR/responses.bin" --out "$RUN_DIR" \
        >>"$HOST_LOG" 2>&1
done

log "replay architecture knob sweep"
cargo run -q -p slugarch-cli -- measure-sim-feasible \
    targets/qemu-type2/identity_times_const.json \
    --out "$ARTIFACT_DIR/sim-feasible" \
    --qemu-repeatability-dir "$ARTIFACT_DIR" \
    --dev-root /dev \
    --replay-repeats "$REPLAY_REPEATS" \
    >>"$HOST_LOG" 2>&1

log "complete"
echo "$ARTIFACT_DIR"
