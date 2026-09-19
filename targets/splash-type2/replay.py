#!/usr/bin/env python3
"""Replay full host traces through fresh multi-device Splash processes.

This is active functional re-execution with full payloads, not endpoint JIT
recording, scheduler replay, or prevention of externally visible effects.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import time
import uuid
from pathlib import Path

from campaign import sha256, write_json
from transport import Simulator
from validate import validate_campaign


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("campaign", type=Path)
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    original = validate_campaign(args.campaign)
    manifest = json.loads((args.campaign / "manifest.json").read_text())
    options = manifest["options"]
    source, qemu = Path(options["source"]), Path(options["qemu"])
    if sha256(qemu) != manifest["qemu_sha256"]:
        raise ValueError("replay QEMU differs from recorded QEMU")
    args.out.mkdir(parents=True, exist_ok=False)
    shutil.copyfile(Path(__file__), args.out / "replay.py")
    cells = [c for c in manifest["cells"] if c["mode"] == "full"]
    results = []
    for index, cell in enumerate(cells, 1):
        print(f"replay [{index}/{len(cells)}] {cell['directory']}", flush=True)
        out = args.out / cell["directory"]
        recorded = json.loads((args.campaign / cell["directory"] / "result.json").read_text())
        result = dict(source_run=cell["directory"], source_run_id=recorded["run_id"],
                      replay_run_id=str(uuid.uuid4()), devices=cell["devices"],
                      transfer_bytes=cell["payload_bytes"], status="running", phases=[])
        try:
            with Simulator(source, qemu, out, cell["devices"]) as sim:
                result["pid"] = sim.proc.pid
                for phase in recorded["phases"]:
                    name = phase["phase"]
                    records = [json.loads(line) for line in (args.campaign / cell["directory"] / f"{name}.jsonl").read_text().splitlines()]
                    digest = hashlib.sha256()
                    started = time.perf_counter_ns()
                    previous = None
                    for req, cmp in zip(records[::2], records[1::2]):
                        data = bytes.fromhex(req["payload_hex"])
                        if req["dependency"] and previous != data:
                            raise ValueError("host relay dependency differs on replay")
                        d = sim.devices[req["tile"]]
                        d.publish(req["offset"], data)
                        received = d.consume(req["offset"], req["size"])
                        if received != bytes.fromhex(cmp["payload_hex"]):
                            raise ValueError(f"replay mismatch tile={d.tile}, event={cmp['event_id']}")
                        digest.update(received)
                        previous = received
                    if digest.hexdigest() != phase["output_sha256"]:
                        raise ValueError("replay output commitment differs")
                    result["phases"].append(dict(phase=name, exact_payload_checks=len(records) // 2,
                                                  output_sha256=digest.hexdigest(), wall_ns=time.perf_counter_ns() - started))
                result["status"] = "pass"
        except Exception as error:
            result.update(status="fail", error=repr(error))
            raise
        finally:
            write_json(out / "replay.json", result)
        results.append(result)
    if sha256(qemu) != manifest["qemu_sha256"]:
        raise ValueError("QEMU changed during replay")
    summary = dict(schema="slugarch.splash-functional-replay.v1", status="pass",
                   scope="Fresh-process host-driven full-trace re-execution through BAR2/BAR4; no endpoint JIT",
                   campaign_manifest_sha256=original["manifest_sha256"], qemu_sha256=manifest["qemu_sha256"],
                   fresh_processes=len(results), exact_payload_checks=sum(p["exact_payload_checks"] for r in results for p in r["phases"]),
                   results=results)
    write_json(args.out / "summary.json", summary)
    files = sorted(p for p in args.out.rglob("*") if p.is_file())
    (args.out / "SHA256SUMS").write_text("".join(f"{sha256(p)}  {p.relative_to(args.out)}\n" for p in files))
    print(f"Replay passed: {summary['fresh_processes']} fresh processes, {summary['exact_payload_checks']} exact payload checks")


if __name__ == "__main__":
    main()
