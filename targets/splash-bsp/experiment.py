#!/usr/bin/env python3
"""Host prototype of replay-checked nested BSP barriers on real Type-2 objects.

CPU arithmetic and logical barrier controllers run in Python. Data and fixed
64-byte certificates traverse QEMU device memory. No endpoint JIT, peer DMA,
concurrent compute, or physical BSP timing is implied.
"""
from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import random
import shutil
import statistics
import struct
import sys
import time
import uuid
from pathlib import Path

SHARED = Path(__file__).resolve().parents[1] / "splash-type2"
sys.path.insert(0, str(SHARED))
from campaign import git_info, sha256, write_json
from transport import Simulator

CERT = struct.Struct("<4IQ32sQ")
MAGIC = 0x53425350
POLICY = hashlib.sha256(b"slugarch-bsp-v1;exact-membership;epoch;ordered-hashes;mod64-reduction").digest()
MASK64 = (1 << 64) - 1
OUTPUT = 0x800000
LEAF = 0x1000000
GROUP = 0x1200000
FAULTS = ("missing_record", "stale_epoch", "wrong_tile", "corrupt_digest")


class BarrierError(Exception):
    def __init__(self, tile, epoch, stage, fault):
        self.detail = dict(tile=tile, epoch=epoch, stage=stage, fault=fault)
        super().__init__(str(self.detail))


def output_bytes(seed, epoch, tile, block):
    return struct.pack("<8Q", *[((seed * 3 + (epoch + 1) * 1009 + tile * 131 + block * 17 + lane) & MASK64)
                               for lane in range(8)])


def certificate(epoch, kind, owner, members, hashes, count):
    membership = sum(1 << x for x in members)
    digest = hashlib.sha256(POLICY + b"".join(hashes)).digest()
    return CERT.pack(MAGIC, epoch, kind, owner, membership, digest, count)


def corrupt(cert, fault):
    values = list(CERT.unpack(cert))
    if fault == "missing_record":
        values[-1] -= 1
    elif fault == "stale_epoch":
        values[1] -= 1
    elif fault == "wrong_tile":
        values[3] += 1
    elif fault == "corrupt_digest":
        values[5] = bytes([values[5][0] ^ 1]) + values[5][1:]
    else:
        raise ValueError(fault)
    return CERT.pack(*values)


def check_certificate(actual, expected, tile, epoch, stage):
    if actual == expected:
        return
    a, e = CERT.unpack(actual), CERT.unpack(expected)
    fields = {0: "bad_magic", 1: "stale_epoch", 2: "wrong_kind", 3: "wrong_tile",
              4: "wrong_membership", 5: "corrupt_digest", 6: "missing_record"}
    fault = next(fields[i] for i in fields if a[i] != e[i])
    raise BarrierError(tile, epoch, stage, fault)


def groups_for(n, mode):
    size = 2 if mode == "nested2" else 4 if mode == "nested4" else n
    return [list(range(i, min(n, i + size))) for i in range(0, n, size)]


class Barrier:
    def __init__(self, sim, epoch, mode, hashes, log, fault=None):
        self.sim, self.epoch, self.mode, self.hashes, self.log, self.fault = sim, epoch, mode, hashes, log, fault
        self.metrics = dict(root_reads=0, group_reads=0, certificate_writes=0, certificate_bytes_read=0,
                            certificate_bytes_written=0, released=False)

    def put(self, tile, offset, data, role):
        self.sim.devices[tile].device_write(offset, data)
        self.metrics["certificate_writes"] += 1
        self.metrics["certificate_bytes_written"] += 64
        self.log.append(dict(action="certificate_write", epoch=self.epoch, tile=tile,
                             offset=offset, role=role, data=data.hex()))

    def get(self, tile, offset, expected, role, owner):
        actual = self.sim.devices[tile].consume(offset, 64)
        self.metrics[f"{role}_reads"] += 1
        self.metrics["certificate_bytes_read"] += 64
        self.log.append(dict(action="certificate_read", epoch=self.epoch, tile=tile,
                             offset=offset, role=role, data=actual.hex(), expected=expected.hex()))
        check_certificate(actual, expected, owner, self.epoch, role)
        return actual

    def execute(self):
        n = len(self.sim.devices)
        leaves = []
        for tile in range(n):
            hs = self.hashes[tile]
            blocks = [[h] for h in hs] if self.mode == "event" else [hs]
            for index, block_hashes in enumerate(blocks):
                expected = certificate(self.epoch, 0 if self.mode == "event" else 1, tile,
                                       [tile], block_hashes, len(block_hashes))
                actual = expected
                if self.fault and tile == n - 1 and index == len(blocks) - 1:
                    actual = corrupt(actual, self.fault)
                self.put(tile, LEAF + index * 64, actual, "leaf")
                leaves.append((tile, LEAF + index * 64, expected))
        if self.mode in ("event", "tile"):
            for tile, offset, expected in leaves:
                self.get(tile, offset, expected, "root", tile)
        else:
            parents = []
            for members in groups_for(n, self.mode):
                child_certs = []
                for tile in members:
                    _, offset, expected = leaves[tile]
                    child_certs.append(self.get(tile, offset, expected, "group", tile))
                representative = members[self.epoch % len(members)]
                parent = certificate(self.epoch, 2, representative, members,
                                     [hashlib.sha256(c).digest() for c in child_certs],
                                     sum(len(self.hashes[t]) for t in members))
                self.put(representative, GROUP, parent, "group")
                parents.append((representative, parent))
            for tile, parent in parents:
                self.get(tile, GROUP, parent, "root", tile)
        self.metrics["released"] = True
        self.log.append(dict(action="release", epoch=self.epoch,
                             logical_root=self.epoch % n, participants=list(range(n))))
        return self.metrics


def serial_oracle(n, blocks, steps):
    seed, expected = 7, []
    for epoch in range(steps):
        values = [output_bytes(seed, epoch, tile, block) for tile in range(n) for block in range(blocks)]
        seed = sum(sum(struct.unpack("<8Q", value)) for value in values) & MASK64
        expected.append(dict(reduction=seed, sha256=hashlib.sha256(b"".join(values)).hexdigest()))
    return expected


def run_cell(args, cell, out):
    out.mkdir()
    events, seed = [], 7
    result = dict(config=cell, run_id=str(uuid.uuid4()), status="running", steps=[])
    oracle = serial_oracle(cell["devices"], cell["blocks"], args.steps)
    try:
        with Simulator(args.source, args.qemu, out, cell["devices"]) as sim:
            result["pid"] = sim.proc.pid
            started = time.perf_counter_ns()
            for epoch in range(args.steps):
                step_start = time.perf_counter_ns()
                values = {}
                hashes = {t: [] for t in range(cell["devices"])}
                data_writes = 0

                def work(tiles, retry=False):
                    nonlocal data_writes
                    for tile in tiles:
                        hashes[tile] = []
                        for block in range(cell["blocks"]):
                            data = output_bytes(seed, epoch, tile, block)
                            d = sim.devices[tile]
                            d.publish(OUTPUT + block * 64, data)
                            received = d.consume(OUTPUT + block * 64, 64)
                            if received != data:
                                raise ValueError(f"output mismatch: epoch={epoch}, tile={tile}, block={block}")
                            values[tile, block] = received
                            h = hashlib.sha256(received).digest()
                            hashes[tile].append(h)
                            data_writes += 1
                            missing = (cell["fault"] == "missing_record" and epoch == 1 and
                                       tile == cell["devices"] - 1 and block == cell["blocks"] - 1 and not retry)
                            events.append(dict(action="record_missing" if missing else "output_record", epoch=epoch,
                                               tile=tile, block=block, retry=retry, payload_hex=received.hex(),
                                               payload_sha256=h.hex()))

                work(range(cell["devices"]))
                fault = cell["fault"] if epoch == 1 else None
                barrier = Barrier(sim, epoch, cell["mode"], hashes, events, fault)
                step = dict(epoch=epoch, initial_output_writes=data_writes)
                barrier_start = time.perf_counter_ns()
                try:
                    step["barrier"] = barrier.execute()
                    if fault:
                        raise ValueError("injected fault was silently accepted")
                except BarrierError as error:
                    if error.detail["fault"] != fault or error.detail["tile"] != cell["devices"] - 1:
                        raise ValueError(f"fault was misattributed: {error}") from error
                    if barrier.metrics["released"] or any(e["action"] == "release" and e["epoch"] == epoch for e in events):
                        raise ValueError("failed epoch was released")
                    # A completion-only barrier's arrival predicate is satisfied;
                    # actual output values exist, while replay acceptance failed.
                    completion_only_release = len(values) == cell["devices"] * cell["blocks"]
                    target = cell["devices"] - 1
                    committed = sim.devices[target].consume(OUTPUT + (cell["blocks"] - 1) * 64, 64)
                    if committed != values[target, cell["blocks"] - 1]:
                        raise ValueError("post-write failure lost committed output evidence")
                    step["fault"] = dict(**error.detail, completion_only_would_release=completion_only_release,
                                         replay_barrier_released=False, committed_output_sha256=hashlib.sha256(committed).hexdigest(),
                                         attempted_barrier=barrier.metrics.copy())
                    retry_tiles = next(g for g in groups_for(cell["devices"], cell["mode"]) if target in g)
                    step["retry_tiles"] = retry_tiles
                    preserved = {k: v for k, v in values.items() if k[0] not in retry_tiles}
                    work(retry_tiles, retry=True)
                    # Read the unchanged siblings back from QEMU after recovery.
                    for (tile, block), previous in preserved.items():
                        if sim.devices[tile].consume(OUTPUT + block * 64, 64) != previous:
                            raise ValueError("unaffected sibling changed during recovery")
                    step["preserved_blocks_checked"] = len(preserved)
                    step["barrier"] = Barrier(sim, epoch, cell["mode"], hashes, events).execute()
                step["barrier_wall_ns"] = time.perf_counter_ns() - barrier_start
                actual = b"".join(values[t, b] for t in range(cell["devices"]) for b in range(cell["blocks"]))
                seed = sum(struct.unpack(f"<{len(actual)//8}Q", actual)) & MASK64
                step["output_sha256"] = hashlib.sha256(actual).hexdigest()
                step["reduction"] = seed
                if step["output_sha256"] != oracle[epoch]["sha256"] or seed != oracle[epoch]["reduction"]:
                    raise ValueError("serial BSP oracle disagrees")
                step["retry_output_writes"] = data_writes - step["initial_output_writes"]
                step["wall_ns"] = time.perf_counter_ns() - step_start
                result["steps"].append(step)
            result["wall_ns"] = time.perf_counter_ns() - started
            result["status"] = "pass"
    except Exception as error:
        result.update(status="fail", error=repr(error))
        raise
    finally:
        (out / "events.jsonl").write_text("".join(json.dumps(e, sort_keys=True, separators=(",", ":")) + "\n" for e in events))
        result["events_sha256"] = sha256(out / "events.jsonl")
        write_json(out / "result.json", result)


def validate(root):
    manifest = json.loads((root / "manifest.json").read_text())
    expected_cells = set(itertools.product(manifest["devices"], manifest["blocks"],
                                          manifest["modes"], range(manifest["repeats"]), [None]))
    expected_cells |= set(itertools.product(manifest["fault_devices"], [16], manifest["modes"],
                                           range(manifest["repeats"]), manifest["faults"]))
    observed_cells = [(c["devices"], c["blocks"], c["mode"], c["repeat"], c["fault"]) for c in manifest["cells"]]
    if set(observed_cells) != expected_cells or len(observed_cells) != len(expected_cells):
        raise ValueError("missing or duplicate matrix cell")
    # Check the complete tree, including failures, commands, topology and sources.
    sums = {}
    for line in (root / "SHA256SUMS").read_text().splitlines():
        h, name = line.split("  ", 1)
        if name in sums or Path(name).is_absolute() or ".." in Path(name).parts or sha256(root / name) != h:
            raise ValueError(f"bad checksum {name}")
        sums[name] = h
    if set(sums) != {str(p.relative_to(root)) for p in root.rglob("*") if p.is_file() and p.name != "SHA256SUMS"}:
        raise ValueError("unsealed files")
    if {p.name for p in root.glob("run-*")} != {c["directory"] for c in manifest["cells"]}:
        raise ValueError("extra or missing run")
    results, ids = [], set()
    for cell in manifest["cells"]:
        out = root / cell["directory"]
        r = json.loads((out / "result.json").read_text())
        if r["config"] != cell or r["status"] != "pass" or r["run_id"] in ids:
            raise ValueError("bad run identity/status")
        ids.add(r["run_id"])
        n, blocks, mode = cell["devices"], cell["blocks"], cell["mode"]
        topology = json.loads((out / "topology.json").read_text())
        if len(topology) != n or [t["tile"] for t in topology] != list(range(n)):
            raise ValueError("topology coverage")
        if any(len({t[key] for t in topology}) != n for key in ("bus", "serial", "bar2", "bar4")):
            raise ValueError("aliased device identity")
        expected_root = n * blocks if mode == "event" else n if mode == "tile" else len(groups_for(n, mode))
        expected_group = n if mode.startswith("nested") else 0
        oracle = serial_oracle(n, blocks, manifest["steps"])
        events = [json.loads(line) for line in (out / "events.jsonl").read_text().splitlines()]
        if sha256(out / "events.jsonl") != r["events_sha256"] or len(r["steps"]) != manifest["steps"]:
            raise ValueError("missing evidence")
        seed = 7
        for epoch, step in enumerate(r["steps"]):
            evs = [e for e in events if e["epoch"] == epoch]
            if step["epoch"] != epoch or step["reduction"] != oracle[epoch]["reduction"] or step["output_sha256"] != oracle[epoch]["sha256"]:
                raise ValueError("BSP result differs from serial oracle")
            b = step["barrier"]
            if b["root_reads"] != expected_root or b["group_reads"] != expected_group or not b["released"]:
                raise ValueError("barrier coverage")
            if b["certificate_bytes_read"] != 64 * (expected_root + expected_group):
                raise ValueError("receipt byte coverage")
            if b["certificate_writes"] != expected_root + expected_group or b["certificate_bytes_written"] != 64 * b["certificate_writes"]:
                raise ValueError("receipt write coverage")
            if len([e for e in evs if e["action"] == "release"]) != 1:
                raise ValueError("missing or extra root release")
            for e in evs:
                if e["action"] in ("output_record", "record_missing"):
                    data = output_bytes(seed, epoch, e["tile"], e["block"])
                    if e["payload_hex"] != data.hex() or e["payload_sha256"] != hashlib.sha256(data).hexdigest():
                        raise ValueError("raw output evidence differs")
            expected_retry = 0
            if cell["fault"] and epoch == 1:
                fault = step["fault"]
                if fault["fault"] != cell["fault"] or fault["tile"] != n - 1 or fault["epoch"] != epoch or fault["replay_barrier_released"] or not fault["completion_only_would_release"]:
                    raise ValueError("fault attribution/release failure")
                tiles = next(g for g in groups_for(n, mode) if n - 1 in g)
                expected_retry = len(tiles) * blocks
                if step["retry_tiles"] != tiles or step["preserved_blocks_checked"] != (n - len(tiles)) * blocks:
                    raise ValueError("recovery coverage")
                mismatches = [e for e in evs if e["action"] == "certificate_read" and e["data"] != e["expected"]]
                if len(mismatches) != 1:
                    raise ValueError("fault lacks a mismatching device receipt")
            if step["retry_output_writes"] != expected_retry or step["initial_output_writes"] != n * blocks:
                raise ValueError("data write accounting")
            outputs = [e for e in evs if e["action"] in ("output_record", "record_missing")]
            if len(outputs) != n * blocks + expected_retry:
                raise ValueError("missing output records")
            originals = [e for e in outputs if not e["retry"]]
            if len({(e["tile"], e["block"]) for e in originals}) != n * blocks:
                raise ValueError("duplicate or omitted original output")
            for role in ("root", "group"):
                actual_reads = sum(e["action"] == "certificate_read" and e["role"] == role for e in evs)
                attempted = step.get("fault", {}).get("attempted_barrier", {}).get(f"{role}_reads", 0)
                if actual_reads != b[f"{role}_reads"] + attempted:
                    raise ValueError("raw certificate IO differs from counters")
            release_index = next(i for i,e in enumerate(evs) if e["action"] == "release")
            if release_index != len(evs) - 1:
                raise ValueError("work or evidence arrived after root release")
            seed = step["reduction"]
        results.append(r)
    groups = []
    for n, blocks, mode in itertools.product(manifest["devices"], manifest["blocks"], manifest["modes"]):
        rs = [r for r in results if (r["config"]["devices"], r["config"]["blocks"], r["config"]["mode"], r["config"]["fault"]) == (n, blocks, mode, None)]
        if len(rs) != manifest["repeats"]:
            raise ValueError("incomplete healthy group")
        groups.append(dict(devices=n, blocks=blocks, mode=mode, runs=[r["config"]["directory"] for r in rs],
                           root_reads_per_step=rs[0]["steps"][0]["barrier"]["root_reads"],
                           total_reads_per_step=rs[0]["steps"][0]["barrier"]["root_reads"] + rs[0]["steps"][0]["barrier"]["group_reads"],
                           wall_ns=dict(values=[r["wall_ns"] for r in rs], median=statistics.median(r["wall_ns"] for r in rs))))
    faults = [r for r in results if r["config"]["fault"]]
    return dict(schema="slugarch.nested-bsp.v1", status="pass", manifest_sha256=sha256(root / "manifest.json"),
                seal_sha256=sha256(root / "SHA256SUMS"), fresh_processes=len(results),
                healthy_processes=len(results)-len(faults), fault_processes=len(faults),
                checked_supersteps=sum(len(r["steps"]) for r in results),
                incorrect_releases=0, recovery_oracle_mismatches=0,
                groups=groups, fault_results=[dict(config=r["config"], **r["steps"][1]) for r in faults])


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--source", type=Path, default=Path("/root/CXLMemSim"))
    ap.add_argument("--qemu", type=Path, default=Path("/root/CXLMemSim/lib/qemu/build-perf/qemu-system-x86_64"))
    ap.add_argument("--repeats", type=int, default=5)
    ap.add_argument("--steps", type=int, default=4)
    ap.add_argument("--devices", type=int, nargs="+", default=[1, 2, 4, 8])
    ap.add_argument("--blocks", type=int, nargs="+", default=[1, 16])
    ap.add_argument("--fault-devices", type=int, nargs="+", default=[2, 8])
    ap.add_argument("--validate-only", action="store_true")
    ap.add_argument("--summary", type=Path, required=True)
    args = ap.parse_args()
    if args.validate_only:
        summary = validate(args.out)
        write_json(args.summary, summary)
        print({k:v for k,v in summary.items() if k not in ("groups", "fault_results")})
        return
    if args.repeats < 1 or args.steps < 3 or any(n not in (1,2,4,8) for n in args.devices + args.fault_devices) or any(b not in (1,16) for b in args.blocks):
        ap.error("invalid matrix: positive repeats, at least 3 steps, 1/2/4/8 devices, 1/16 blocks")
    modes = ["event", "tile", "nested2", "nested4"]
    cells = [dict(devices=n, blocks=b, mode=m, repeat=r, fault=None)
             for n,b,m,r in itertools.product(args.devices,args.blocks,modes,range(args.repeats))]
    cells += [dict(devices=n,blocks=16,mode=m,repeat=r,fault=f)
              for n,m,r,f in itertools.product(args.fault_devices,modes,range(args.repeats),FAULTS)]
    random.Random(2026091902).shuffle(cells)
    for index,c in enumerate(cells):
        c["directory"] = f"run-{index:03d}"
    args.out.mkdir(parents=True,exist_ok=False)
    sources = args.out / "sources"
    sources.mkdir()
    identities = {}
    for p in [Path(__file__), SHARED/"transport.py", SHARED/"campaign.py", args.source/"qemu_integration/qtest_switch_offload.py",
              args.source/"lib/qemu/hw/cxl/cxl_type2.c", args.source/"lib/qemu/hw/cxl/cxl_type2_coherency.c"]:
        shutil.copyfile(p,sources/p.name)
        identities[str(p)] = sha256(p)
    manifest = dict(schema="slugarch.nested-bsp-campaign.v1",devices=args.devices,blocks=args.blocks,
                    modes=modes,repeats=args.repeats,steps=args.steps,cells=cells,
                    fault_devices=args.fault_devices,faults=list(FAULTS),policy_sha256=POLICY.hex(),
                    qemu_sha256=sha256(args.qemu),source_sha256=identities,
                    source=git_info(args.source),qemu_source=git_info(args.source/"lib/qemu"),
                    scope="Host-driven logical BSP hierarchy and CPU arithmetic; real serialized Type-2 BAR data/certificate IO")
    write_json(args.out/"manifest.json",manifest)
    for i,c in enumerate(cells,1):
        print(f"[{i}/{len(cells)}] {c}",flush=True)
        run_cell(args,c,args.out/c["directory"])
    if any(sha256(Path(p)) != h for p,h in identities.items()) or sha256(args.qemu) != manifest["qemu_sha256"]:
        raise ValueError("source/binary changed during campaign")
    files=sorted(p for p in args.out.rglob("*") if p.is_file())
    (args.out/"SHA256SUMS").write_text("".join(f"{sha256(p)}  {p.relative_to(args.out)}\n" for p in files))
    summary=validate(args.out)
    write_json(args.summary,summary)
    print(f"PASS {summary['fresh_processes']} processes, {summary['checked_supersteps']} supersteps")


if __name__ == "__main__":
    main()
