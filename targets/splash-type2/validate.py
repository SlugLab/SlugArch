#!/usr/bin/env python3
"""Validate every declared cell and every host record before aggregation."""
from __future__ import annotations

import argparse
import copy
import hashlib
import itertools
import json
import statistics
from pathlib import Path

from campaign import POLICY, POLICY_SHA, payload, schedule, sha256, write_json


def require(value, message):
    if not value:
        raise ValueError(message)


def validate_records(records, result, phase, options):
    config = result["config"]
    count, size = config["devices"], config["payload_bytes"]
    work = schedule(phase, count, options["total"], options["per_device"], options["relay"])
    require(len(records) == 2 * len(work), "record count: missing or extra event")
    event_ids = [0] * count
    digest = hashlib.sha256()
    for tx, (tile, iteration) in enumerate(work, 1):
        data = payload(phase, 0 if phase == "host_relay" else tile, iteration, size)
        for index, kind in enumerate(("request", "completion")):
            row = records[2 * (tx - 1) + index]
            event_ids[tile] += 1
            expected = dict(run_id=result["run_id"], phase=phase, tile=tile,
                            event_id=event_ids[tile], transaction=tx, kind=kind,
                            dependency=tx - 1 if phase == "host_relay" and tile > 0 else 0,
                            policy_sha256=POLICY_SHA, offset=POLICY["offset"], size=size,
                            result=0, payload_sha256=hashlib.sha256(data).hexdigest())
            if config["mode"] == "full":
                expected["payload_hex"] = data.hex()
            require(row == expected, f"tile={tile} event={event_ids[tile]}: record mismatch")
        digest.update(data)
    return digest.hexdigest()


def mutation_checks(records, result, phase, options):
    mutations = {}
    for field, value in [("tile", 999), ("event_id", 999), ("payload_sha256", "0" * 64),
                         ("run_id", "wrong-epoch"), ("policy_sha256", "0" * 64),
                         ("dependency", 999), ("result", 1)]:
        damaged = copy.deepcopy(records)
        damaged[1][field] = value
        mutations[field] = damaged
    mutations["missing_completion"] = records[:-1]
    mutations["duplicate_event"] = records[:1] + records
    mutations["reorder"] = records[1:2] + records[:1] + records[2:]
    if result["config"]["mode"] == "full":
        damaged = copy.deepcopy(records)
        damaged[1]["payload_hex"] = "00" * records[1]["size"]
        mutations["payload_bytes"] = damaged
    checks = []
    for name, damaged in mutations.items():
        try:
            validate_records(damaged, result, phase, options)
        except ValueError as error:
            checks.append(dict(mutation=name, detected=True, diagnostic=str(error)))
        else:
            raise ValueError(f"false accept: {name}")
    return checks


def distribution(values):
    return dict(median=statistics.median(values), min=min(values), max=max(values), values=values)


def validate_campaign(root: Path):
    manifest = json.loads((root / "manifest.json").read_text())
    options = manifest["options"]
    require(manifest["policy"] == POLICY and manifest["policy_sha256"] == POLICY_SHA, "unknown policy")
    expected_cells = set(itertools.product(options["devices"], options["sizes"], options["modes"], range(options["repeats"])))
    observed_cells = [(c["devices"], c["payload_bytes"], c["mode"], c["repeat"]) for c in manifest["cells"]]
    require(set(observed_cells) == expected_cells and len(observed_cells) == len(expected_cells), "incomplete matrix")
    require({p.name for p in root.glob("run-*")} == {c["directory"] for c in manifest["cells"]}, "extra/missing runs")
    sums = {}
    for line in (root / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split("  ", 1)
        require(name not in sums and not Path(name).is_absolute() and ".." not in Path(name).parts, "invalid checksum path")
        require(sha256(root / name) == digest, f"checksum mismatch: {name}")
        sums[name] = digest
    actual_files = {str(p.relative_to(root)) for p in root.rglob("*") if p.is_file() and p.name != "SHA256SUMS"}
    require(set(sums) == actual_files, "unsealed files or missing checksums")
    rows, mutations, run_ids = [], [], set()
    correctness_count = 0
    signatures = {}
    for cell in manifest["cells"]:
        run = root / cell["directory"]
        result = json.loads((run / "result.json").read_text())
        require(result["config"] == cell and result["status"] == "pass", f"failed/wrong cell: {run}")
        require(result["run_id"] not in run_ids, "duplicate run identity")
        run_ids.add(result["run_id"])
        n = cell["devices"]
        topology = json.loads((run / "topology.json").read_text())
        require(len(topology) == n and [x["tile"] for x in topology] == list(range(n)), "wrong topology")
        for key in ("bus", "serial", "bar2", "bar4"):
            require(len({x[key] for x in topology}) == n, f"aliased {key}")
        require(all(t["pci_identity"] == 0x0D928086 and t["bar2_magic"] == 0x43584C32 for t in topology), "device identity")
        checks = result["correctness"]
        require(len(checks) == 3 * n * n + 7 * n, "missing correctness checks")
        require(len({(c["case"], c["tile"]) for c in checks}) == len(checks), "duplicate correctness check")
        require(all(c["status"] == "pass" and c["actual"] == c["expected"] for c in checks), "correctness failure")
        correctness_count += len(checks)
        require([p["phase"] for p in result["phases"]] == ["fixed_total", "fixed_per_device", "host_relay"], "phase coverage")
        for phase in result["phases"]:
            name, size = phase["phase"], cell["payload_bytes"]
            work = schedule(name, n, options["total"], options["per_device"], options["relay"])
            require(phase["transactions"] == phase["exact_payload_checks"] == len(work), "transaction coverage")
            require(phase["payload_bytes"] == size * len(work), "payload count")
            expected_output = hashlib.sha256()
            for tile, iteration in work:
                expected_output.update(payload(name, 0 if name == "host_relay" else tile, iteration, size))
            require(phase["output_sha256"] == expected_output.hexdigest(), "wrong output commitment")
            samples = json.loads((run / f"{name}-samples.json").read_text())
            require(len(samples) == len(work) and all(x > 0 for x in samples), "missing timing samples")
            require(phase["wall_ns"] >= sum(samples), "wall time shorter than nested samples")
            require(phase["ns_per_transaction"] == phase["wall_ns"] / len(work), "timing denominator")
            require(phase["transactions_per_second"] == len(work) * 1e9 / phase["wall_ns"], "throughput denominator")
            require([x["tile"] for x in phase["device_stats"]] == list(range(n)), "counter identity")
            for stat in phase["device_stats"]:
                expected_requests = sum(tile == stat["tile"] for tile, _ in work) * (size // 8 + 1)
                require(stat["coherency_requests"] == expected_requests, "counter coverage")
                require(stat["modeled_ns"] > 0, "missing modeled counter")
            if cell["mode"] == "off":
                require(phase["trace_events"] == phase["trace_bytes"] == 0 and phase["trace_sha256"] is None, "off recorded events")
                require(not (run / f"{name}.jsonl").exists(), "unexpected off trace")
            else:
                trace = run / f"{name}.jsonl"
                require(phase["trace_sha256"] == sha256(trace) and phase["trace_bytes"] == trace.stat().st_size, "trace identity")
                records = [json.loads(line) for line in trace.read_text().splitlines()]
                require(phase["trace_events"] == len(records) == 2 * len(work), "event count")
                require(validate_records(records, result, name, options) == phase["output_sha256"], "trace output")
                if name == "host_relay":
                    mutations += [dict(run=cell["directory"], **m) for m in mutation_checks(records, result, name, options)]
            key = (n, size, name)
            signatures.setdefault(key, set()).add(phase["output_sha256"])
            rows.append({**cell, "run_id": result["run_id"], **phase,
                         "transfer_bytes": size,
                         "trace_bytes_per_transaction": phase["trace_bytes"] / len(work)})
    require(all(len(v) == 1 for v in signatures.values()), "modes or repeats disagree")
    groups = []
    for n, size, mode, phase in itertools.product(options["devices"], options["sizes"], options["modes"], ["fixed_total", "fixed_per_device", "host_relay"]):
        group = [r for r in rows if (r["devices"], r["transfer_bytes"], r["mode"], r["phase"]) == (n, size, mode, phase)]
        require(len(group) == options["repeats"], "missing repeat in aggregate")
        groups.append(dict(devices=n, transfer_bytes=size, mode=mode, phase=phase,
                           runs=[r["directory"] for r in group], run_ids=[r["run_id"] for r in group],
                           transactions=group[0]["transactions"],
                           metrics={field: distribution([r[field] for r in group]) for field in
                                    ("wall_ns", "ns_per_transaction", "transactions_per_second", "trace_bytes_per_transaction")}))
    return dict(schema="slugarch.splash-multidevice-summary.v1", status="pass", scope=manifest["scope"],
                manifest_sha256=sha256(root / "manifest.json"), seal_sha256=sha256(root / "SHA256SUMS"),
                qemu_sha256=manifest["qemu_sha256"], options=options,
                fresh_processes=len(run_ids), correctness_checks=correctness_count,
                measured_transactions=sum(r["transactions"] for r in rows),
                recorded_events=sum(r["trace_events"] for r in rows),
                offline_mutations=len(mutations), mutations=mutations,
                unique_output_signatures=len(signatures), groups=groups, runs=rows)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("campaign", type=Path)
    ap.add_argument("--output", type=Path, required=True)
    args = ap.parse_args()
    summary = validate_campaign(args.campaign)
    write_json(args.output, summary)
    print({k: summary[k] for k in ("status", "fresh_processes", "correctness_checks", "measured_transactions", "recorded_events", "offline_mutations")})


if __name__ == "__main__":
    main()
