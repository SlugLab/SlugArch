#!/usr/bin/env python3
"""Reproducible correctness and serialized scaling on Splash Type-2 devices.

Records belong to the host test harness, not to the endpoint Hardware JIT.
All cells are declared before execution; failed attempts are preserved.
"""
from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import platform
import random
import shutil
import statistics
import subprocess
import time
import uuid
from pathlib import Path

from transport import MEM_SIZE, Simulator

ROOT = Path(__file__).resolve().parents[2]
POLICY = {"schema": "slugarch.splash-host-trace.v1", "coverage": "publish-consume",
          "hash": "sha256", "offset": 0x400000, "schedule": "serialized-round-robin"}
POLICY_SHA = hashlib.sha256(json.dumps(POLICY, sort_keys=True).encode()).hexdigest()


def write_json(path: Path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def sha256(path: Path) -> str:
    with path.open("rb") as src:
        return hashlib.file_digest(src, "sha256").hexdigest()


def git_info(path: Path) -> dict:
    def read(*args):
        return subprocess.check_output(["git", "-C", str(path), *args])
    return {"commit": read("rev-parse", "HEAD").decode().strip(),
            "branch": read("branch", "--show-current").decode().strip(),
            "status": read("status", "--short", "--untracked-files=no").decode(),
            "diff_sha256": hashlib.sha256(read("diff", "HEAD")).hexdigest()}


def payload(phase: str, tile: int, iteration: int, size: int) -> bytes:
    seed = f"slugarch-splash-v1/{phase}/{tile}/{iteration}".encode()
    return hashlib.shake_256(seed).digest(size)


def schedule(phase: str, count: int, total: int, per_device: int, relay: int):
    if phase == "fixed_total":
        return [(index % count, index // count) for index in range(total)]
    rounds = per_device if phase == "fixed_per_device" else relay
    return [(tile, iteration) for iteration in range(rounds) for tile in range(count)]


class Recorder:
    def __init__(self, path: Path, mode: str, run_id: str, phase: str, count: int):
        self.mode, self.run_id, self.phase = mode, run_id, phase
        self.ids = [0] * count
        self.events = 0
        self.file = path.open("w") if mode != "off" else None

    def record(self, tile: int, tx: int, kind: str, data: bytes, dependency: int):
        if not self.file:
            return
        self.ids[tile] += 1
        row = dict(run_id=self.run_id, phase=self.phase, tile=tile,
                   event_id=self.ids[tile], transaction=tx, kind=kind,
                   dependency=dependency, policy_sha256=POLICY_SHA,
                   offset=POLICY["offset"], size=len(data), result=0,
                   payload_sha256=hashlib.sha256(data).hexdigest())
        if self.mode == "full":
            row["payload_hex"] = data.hex()
        self.file.write(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n")
        self.events += 1

    def close(self):
        if self.file:
            self.file.close()


def correctness(sim: Simulator) -> list[dict]:
    rows = []

    def check(case, tile, actual, expected):
        row = dict(case=case, tile=tile, actual=actual, expected=expected,
                   status="pass" if actual == expected else "fail")
        rows.append(row)
        if actual != expected:
            raise AssertionError(row)

    ds = sim.devices
    for d in ds:
        d.publish(0x100000, bytes([d.tile + 1]) * 64)
    for d in ds:
        want = bytes([d.tile + 1]) * 64
        check("same_offset_host_isolation", d.tile, d.host_read(0x100000, 64).hex(), want.hex())
        check("host_to_device_visibility", d.tile, d.consume(0x100000, 64).hex(), want.hex())
        d.device_write(0x100000, bytes([200 + d.tile]) * 64)
    for d in ds:
        check("device_to_cached_host_visibility", d.tile, d.host_read(0x100000, 64).hex(),
              (bytes([200 + d.tile]) * 64).hex())
        d.publish(0x200000, b"\xaa" * 192)
        d.host_read(0x200000, 192)
        d.device_write(0x200000 + 60, bytes(range(80)))
        check("unaligned_write_preserves_neighbors", d.tile, d.host_read(0x200000, 192).hex(),
              (b"\xaa" * 60 + bytes(range(80)) + b"\xaa" * 52).hex())
        d.command(0xA4, 0x300000, 64, d.tile % 2)
    for d in ds:
        check("bias_state_isolation", d.tile, d.command(0xA5, 0x300000)[1][0], d.tile % 2)
    for target in ds:
        for d in ds:
            d.reset_stats()
        target.publish(0x310000, bytes([target.tile + 1]) * 8)
        for d in ds:
            check(f"counter_isolation_from_{target.tile}", d.tile,
                  d.stats()["coherency_requests"], int(d.tile == target.tile))
        check("invalid_command_rejected", target.tile,
              target.command(0xDEAD, allow_error=True)[0], 1)
        for d in ds:
            # Inspect status before any subsequent command can clear it.
            check(f"error_state_isolation_from_{target.tile}", d.tile,
                  sim.qt.readl(d.bar2 + 0x18), int(d.tile == target.tile))
        check("invalid_range_rejected", target.tile,
              target.command(0x23, MEM_SIZE - 4, 8, allow_error=True)[0], 1)
        for d in ds:
            check(f"recovery_and_peer_data_from_{target.tile}", d.tile,
                  d.consume(0x100000, 64).hex(), (bytes([200 + d.tile]) * 64).hex())
    return rows


def run_phase(sim, out, config, run_id, phase, args):
    count, size, mode = config["devices"], config["payload_bytes"], config["mode"]
    # Warm the same footprint and command path for every policy and topology.
    for iteration in range(args.warmup):
        for d in sim.devices:
            data = payload("warmup", d.tile, iteration, size)
            d.publish(POLICY["offset"], data)
            if d.consume(POLICY["offset"], size) != data:
                raise AssertionError("warmup mismatch")
    for d in sim.devices:
        d.reset_stats()
    work = schedule(phase, count, args.total, args.per_device, args.relay)
    recorder = Recorder(out / f"{phase}.jsonl", mode, run_id, phase, count)
    samples = []
    digest = hashlib.sha256()
    relay_data = None
    started = time.perf_counter_ns()
    try:
        for tx, (tile, iteration) in enumerate(work, 1):
            tick = time.perf_counter_ns()
            d = sim.devices[tile]
            if phase == "host_relay":
                if tile == 0:
                    relay_data = payload(phase, 0, iteration, size)
                data = relay_data
            else:
                data = payload(phase, tile, iteration, size)
            dependency = tx - 1 if phase == "host_relay" and tile > 0 else 0
            recorder.record(tile, tx, "request", data, dependency)
            d.publish(POLICY["offset"], data)
            received = d.consume(POLICY["offset"], size)
            if received != data:
                raise AssertionError(f"payload mismatch: phase={phase}, tile={tile}, tx={tx}")
            recorder.record(tile, tx, "completion", received, dependency)
            if phase == "host_relay":
                relay_data = received  # Consumer input actually comes from QEMU.
            digest.update(received)
            samples.append(time.perf_counter_ns() - tick)
    finally:
        # Include buffer flush/close in end-to-end harness wall time; no fsync
        # or crash-durability claim. Percentiles describe operations in a run.
        recorder.close()
    elapsed = time.perf_counter_ns() - started
    stats = [dict(tile=d.tile, **d.stats(), modeled_ns=d.modeled_ns()) for d in sim.devices]
    counts = [sum(tile == d.tile for tile, _ in work) for d in sim.devices]
    # BAR writes are split into 8-byte MMIO accesses, one coherency request each.
    # Device consume contributes one GPU-read request per command.
    for row, transfers in zip(stats, counts):
        expected = transfers * (size // 8 + 1)
        if row["coherency_requests"] != expected:
            raise AssertionError(f"counter coverage {phase}: {row}, expected={expected}")
    trace = out / f"{phase}.jsonl"
    write_json(out / f"{phase}-samples.json", samples)
    return dict(phase=phase, transactions=len(work), exact_payload_checks=len(work),
                wall_ns=elapsed, ns_per_transaction=elapsed / len(work),
                transactions_per_second=len(work) * 1e9 / elapsed,
                payload_bytes=size * len(work), output_sha256=digest.hexdigest(),
                trace_events=recorder.events, trace_bytes=trace.stat().st_size if trace.exists() else 0,
                trace_sha256=sha256(trace) if trace.exists() else None,
                device_stats=stats)


def run_cell(args, config, out):
    out.mkdir()
    run_id = str(uuid.uuid4())
    result = dict(config=config, run_id=run_id, status="running")
    write_json(out / "result.json", result)
    try:
        with Simulator(args.source, args.qemu, out, config["devices"]) as sim:
            result["pid"] = sim.proc.pid
            result["correctness"] = correctness(sim)
            result["phases"] = [run_phase(sim, out, config, run_id, phase, args)
                                for phase in ("fixed_total", "fixed_per_device", "host_relay")]
            result["status"] = "pass"
    except Exception as error:
        result["status"] = "fail"
        result["error"] = repr(error)
        raise
    finally:
        write_json(out / "result.json", result)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--source", type=Path, default=Path("/root/CXLMemSim"))
    ap.add_argument("--splash", type=Path, default=Path("/root/Splash"))
    ap.add_argument("--qemu", type=Path, default=Path("/root/CXLMemSim/lib/qemu/build-perf/qemu-system-x86_64"))
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--devices", type=int, nargs="+", default=[1, 2, 4, 8])
    ap.add_argument("--sizes", type=int, nargs="+", default=[64, 4096])
    ap.add_argument("--modes", nargs="+", choices=["off", "validation", "full"], default=["off", "validation", "full"])
    ap.add_argument("--repeats", type=int, default=5)
    ap.add_argument("--total", type=int, default=256)
    ap.add_argument("--per-device", type=int, default=64)
    ap.add_argument("--relay", type=int, default=16)
    ap.add_argument("--warmup", type=int, default=8)
    ap.add_argument("--seed", type=int, default=20260919)
    args = ap.parse_args()
    if any(n not in (1, 2, 4, 8) for n in args.devices) or any(s < 64 or s > 65536 or s % 64 for s in args.sizes):
        ap.error("devices must be 1/2/4/8; payload sizes must be multiples of 64 up to 65536")
    if min(args.repeats, args.total, args.per_device, args.relay, args.warmup) < 1:
        ap.error("repeat and workload sizes must be positive")
    if any(args.total % n for n in args.devices):
        ap.error("fixed total must divide evenly across all device counts")
    for values in (args.devices, args.sizes, args.modes):
        if len(set(values)) != len(values):
            ap.error("duplicate matrix values")
    args.out.mkdir(parents=True, exist_ok=False)
    cells = [dict(devices=n, payload_bytes=size, mode=mode, repeat=rep)
             for n, size, mode, rep in itertools.product(args.devices, args.sizes, args.modes, range(args.repeats))]
    random.Random(args.seed).shuffle(cells)
    for i, cell in enumerate(cells):
        cell["directory"] = f"run-{i:03d}"
    sources = args.out / "sources"
    sources.mkdir()
    source_files = list(Path(__file__).parent.glob("*.py"))
    source_files += [args.source / "qemu_integration" / name for name in ("qtest_switch_offload.py", "qtest_type2_paper.py")]
    source_files += [args.source / "lib/qemu/hw/cxl" / name for name in ("cxl_type2.c", "cxl_type2_coherency.c")]
    source_files += [args.splash / "bench/swebench_type2/transport.py"]
    identities = {}
    for p in source_files:
        name = "splash_transport.py" if p == source_files[-1] else p.name
        shutil.copyfile(p, sources / name)
        identities[str(p)] = sha256(p)
    manifest = dict(schema="slugarch.splash-multidevice.v1", created_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                    options={k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()},
                    cells=cells, policy=POLICY, policy_sha256=POLICY_SHA,
                    qemu_sha256=sha256(args.qemu), source_sha256=identities,
                    repositories={"slugarch": git_info(ROOT), "splash": git_info(args.splash),
                                  "cxlmemsim": git_info(args.source), "qemu": git_info(args.source / "lib/qemu")},
                    environment=dict(platform=platform.platform(), python=platform.python_version(),
                                     cpu=Path("/proc/cpuinfo").read_text().split("model name", 1)[-1].splitlines()[0]),
                    scope="One real QEMU process per cell; local BAR device memory; sequential host qtest; host trace recorder; no endpoint JIT or physical timing")
    write_json(args.out / "manifest.json", manifest)
    for index, cell in enumerate(cells, 1):
        print(f"[{index}/{len(cells)}] {cell}", flush=True)
        run_cell(args, cell, args.out / cell["directory"])
    for name, digest in identities.items():
        if sha256(Path(name)) != digest:
            raise RuntimeError(f"source changed during campaign: {name}")
    if sha256(args.qemu) != manifest["qemu_sha256"]:
        raise RuntimeError("QEMU binary changed during campaign")
    files = sorted(p for p in args.out.rglob("*") if p.is_file())
    (args.out / "SHA256SUMS").write_text("".join(f"{sha256(p)}  {p.relative_to(args.out)}\n" for p in files))
    print(f"Completed {len(cells)} fresh QEMU processes: {args.out}")


if __name__ == "__main__":
    main()
