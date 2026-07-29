#!/usr/bin/env python3
"""Summarize repeated SlugArch JIT direct-CFMWS evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import statistics
from typing import Iterable


JIT_EVENT_SCHEMA = "slugarch.qemu-jit-event.v1"
JIT_JOIN_SCHEMA = "slugarch.qemu-cfmws-join.v1"
DPA = 80 * 1024 * 1024
TRANSFER_BYTES = 8
MODELED_LATENCY_NS = 400
EMPTY_FNV1A64 = "cbf29ce484222325"
READ_PREFIX = "8877665544332211"
READ_FNV1A64 = "82f950555efebd75"
WRITE_PREFIX = "1122334455667788"
WRITE_FNV1A64 = "f6fe41c3df7a3a4d"
READ_SHA256 = "804d562d22470fb7be7f06aa076621cb268932be33d8f9fb2844f958dbd74c17"
WRITE_SHA256 = "1dce6604591efb439d5e87418a1d00dbfd014327d8c4dea862815714b76ae9a5"
BYPASS_COUNTERS = (
    "bar4_overlay",
    "local_shadow",
    "local_cache",
    "bulk_overlay",
    "coherent_pool",
)

EXPECTED_EVENTS = (
    {
        "event_id": 1,
        "direction": 0,
        "event_class": 1,
        "opcode": 3,
        "tag": 2,
        "payload_len": 0,
        "payload_prefix_hex": "",
        "payload_fnv1a64": EMPTY_FNV1A64,
    },
    {
        "event_id": 2,
        "direction": 1,
        "event_class": 3,
        "opcode": 5,
        "tag": 2,
        "payload_len": 8,
        "payload_prefix_hex": READ_PREFIX,
        "payload_fnv1a64": READ_FNV1A64,
    },
    {
        "event_id": 3,
        "direction": 0,
        "event_class": 2,
        "opcode": 4,
        "tag": 3,
        "payload_len": 8,
        "payload_prefix_hex": WRITE_PREFIX,
        "payload_fnv1a64": WRITE_FNV1A64,
    },
    {
        "event_id": 4,
        "direction": 1,
        "event_class": 4,
        "opcode": 5,
        "tag": 3,
        "payload_len": 0,
        "payload_prefix_hex": "",
        "payload_fnv1a64": EMPTY_FNV1A64,
    },
)


class ContractError(ValueError):
    """Raised when a trace does not satisfy the replay contract."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ContractError(message)


def _load_jsonl(path: pathlib.Path) -> list[dict]:
    try:
        entries = [
            json.loads(line)
            for line in path.read_text(encoding="utf-8").splitlines()
            if line
        ]
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"cannot read JSONL {path}: {error}") from error
    _require(entries, f"empty JSONL file: {path}")
    _require(
        all(isinstance(entry, dict) for entry in entries),
        f"non-object JSONL entry: {path}",
    )
    return entries


def _sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _metrics(values: Iterable[int]) -> dict:
    ordered = list(values)
    return {
        "values": ordered,
        "min": min(ordered),
        "median": statistics.median(ordered),
        "max": max(ordered),
    }


def _check_fields(entry: dict, expected: dict, context: str) -> None:
    for field, value in expected.items():
        _require(
            entry.get(field) == value,
            f"{context}: expected {field}={value!r}, got {entry.get(field)!r}",
        )


def _validate_jit_entries(entries: list[dict], context: str) -> dict:
    _require(len(entries) == 6, f"{context}: expected 6 JIT log entries")
    events = [entries[0], entries[1], entries[3], entries[4]]
    joins = [entries[2], entries[5]]
    _require(
        all(event.get("schema") == JIT_EVENT_SCHEMA for event in events),
        f"{context}: canonical event schema mismatch",
    )
    _require(
        all(join.get("schema") == JIT_JOIN_SCHEMA for join in joins),
        f"{context}: request/completion join schema mismatch",
    )

    digests = {
        entry.get("policy_digest")
        for entry in events + joins
    }
    _require(
        len(digests) == 1 and None not in digests,
        f"{context}: policy digest mismatch",
    )
    digest = next(iter(digests))
    _require(
        len(digest) == 64 and all(character in "0123456789abcdef"
                                  for character in digest),
        f"{context}: policy digest is not lowercase SHA-256",
    )

    phases = {event.get("phase_id") for event in events}
    _require(
        len(phases) == 1 and next(iter(phases), 0) > 0,
        f"{context}: phase/epoch mismatch",
    )
    phase_id = next(iter(phases))
    times = []
    for index, (event, expected) in enumerate(
            zip(events, EXPECTED_EVENTS, strict=True), start=1):
        _check_fields(event, expected, f"{context}: event {index}")
        _check_fields(
            event,
            {
                "client_id": 1,
                "address": DPA,
                "status": 0,
                "result": 0,
                "effective_error": 0,
                "accepted": 1,
                "emitted": 1,
                "decision_error": 0,
                "event_count": index,
                "record_count": index,
                "metadata_bytes": index * 8,
                "reject_count": 0,
                "drop_count": 0,
                "epoch": phase_id,
            },
            f"{context}: event {index}",
        )
        _require(
            isinstance(event.get("monotonic_ns"), int),
            f"{context}: event timestamp is not an integer",
        )
        times.append(event["monotonic_ns"])
    _require(
        all(right > left for left, right in zip(times, times[1:])),
        f"{context}: event timestamps are not strictly increasing",
    )

    expected_joins = (
        {
            "request_event_id": 1,
            "completion_event_id": 2,
            "request_id": 2,
            "server_sequence": 1,
        },
        {
            "request_event_id": 3,
            "completion_event_id": 4,
            "request_id": 3,
            "server_sequence": 2,
        },
    )
    for index, (join, expected) in enumerate(
            zip(joins, expected_joins, strict=True), start=1):
        _require(
            join.get("server_sequence") == expected["server_sequence"],
            f"{context}: join {index}: server sequence mismatch",
        )
        _check_fields(
            join,
            {
                key: value
                for key, value in expected.items()
                if key != "server_sequence"
            },
            f"{context}: join {index}",
        )
        _check_fields(
            join,
            {
                "external_commit": False,
                "effective_error": 0,
                "policy_digest": digest,
            },
            f"{context}: join {index}",
        )
    return {
        "events": events,
        "joins": joins,
        "policy_digest": digest,
        "phase_id": phase_id,
    }


def _validate_counter(entry: dict, expected_count: int, context: str) -> None:
    _check_fields(
        entry,
        {
            "event": "path_counters",
            "phase_id": "phase:cfmws",
            "direct_cfmws": expected_count,
        },
        context,
    )
    _require(
        all(entry.get(counter) == 0 for counter in BYPASS_COUNTERS),
        f"{context}: bypass counter is nonzero",
    )


def _validate_qemu_entries(
        entries: list[dict], context: str, modeled_latency_ns: int) -> dict:
    _require(len(entries) == 6, f"{context}: expected 6 QEMU log entries")
    handshake = entries[0]
    counters = [entries[1], entries[3], entries[5]]
    completions = [entries[2], entries[4]]
    _check_fields(
        handshake,
        {
            "event": "handshake",
            "client_id": 1,
            "capacity_bytes": 256 * 1024 * 1024,
            "configured_latency_ns": modeled_latency_ns,
            "protocol_version": 1,
        },
        f"{context}: handshake",
    )
    _require(
        isinstance(handshake.get("server_instance_id"), str)
        and len(handshake["server_instance_id"]) == 32,
        f"{context}: invalid server instance identity",
    )
    for count, counter in enumerate(counters):
        _validate_counter(counter, count, f"{context}: counter {count}")

    expected_completions = (
        {
            "request_id": 2,
            "server_sequence": 1,
            "operation": "read",
            "payload_sha256": READ_SHA256,
        },
        {
            "request_id": 3,
            "server_sequence": 2,
            "operation": "write",
            "payload_sha256": WRITE_SHA256,
        },
    )
    for completion, expected in zip(
            completions, expected_completions, strict=True):
        operation = expected["operation"]
        _check_fields(
            completion,
            {
                "event": "completion",
                "client_id": 1,
                "dpa": DPA,
                "length": TRANSFER_BYTES,
                "status": 0,
                "returned_modeled_latency_ns": modeled_latency_ns,
                "requested_delay_ns": modeled_latency_ns,
                "path": "direct_cfmws",
                "phase_id": "phase:cfmws",
                **expected,
            },
            f"{context}: {operation} completion",
        )
        applied = completion.get("applied_delay_ns")
        overshoot = completion.get("delay_overshoot_ns")
        _require(
            isinstance(applied, int) and applied >= modeled_latency_ns,
            f"{context}: {operation} applied delay is below the model",
        )
        _require(
            overshoot == applied - modeled_latency_ns,
            f"{context}: {operation} delay overshoot is inconsistent",
        )
        _require(
            completion.get("delay_undershot") is False,
            f"{context}: {operation} delay undershot",
        )
    return {
        "handshake": handshake,
        "completions": completions,
    }


def _semantic_signature(jit: dict, qemu: dict) -> str:
    events = [
        {
            key: value
            for key, value in event.items()
            if key != "monotonic_ns"
        }
        for event in jit["events"]
    ]
    completions = [
        {
            key: value
            for key, value in completion.items()
            if key not in {
                "returned_modeled_latency_ns",
                "requested_delay_ns",
                "applied_delay_ns",
                "delay_overshoot_ns",
                "delay_undershot",
            }
        }
        for completion in qemu["completions"]
    ]
    payload = {
        "events": events,
        "joins": jit["joins"],
        "completions": completions,
    }
    encoded = json.dumps(
        payload, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def summarize(
        backends: dict[str, pathlib.Path],
        expected_repeats: int = 5,
        expected_latency_ns: int = MODELED_LATENCY_NS) -> dict:
    _require(backends, "at least one backend is required")
    _require(expected_repeats > 0, "expected repeats must be positive")
    _require(expected_latency_ns > 0, "modeled latency must be positive")
    backend_summaries = {}
    policy_digests = set()
    semantic_signatures = set()
    total_processes = 0
    total_events = 0
    total_joins = 0
    total_completions = 0

    for backend, root in sorted(backends.items()):
        _require(
            backend in {"rust", "fpga-verilator"},
            f"unsupported backend: {backend}",
        )
        root = pathlib.Path(root).resolve()
        expected_names = [
            f"{backend}-run-{repetition}"
            for repetition in range(1, expected_repeats + 1)
        ]
        actual_names = sorted(
            path.name for path in root.iterdir() if path.is_dir()
        ) if root.is_dir() else []
        _require(
            actual_names == sorted(expected_names),
            f"{backend}: run directories do not match {expected_names}",
        )
        runs = []
        for repetition in range(1, expected_repeats + 1):
            run_dir = root / f"{backend}-run-{repetition}"
            jit_path = run_dir / "jit-events.jsonl"
            qemu_path = run_dir / "qemu-events.jsonl"
            jit_entries = _load_jsonl(jit_path)
            qemu_entries = _load_jsonl(qemu_path)
            context = f"{backend} repetition {repetition}"
            jit = _validate_jit_entries(jit_entries, context)
            qemu = _validate_qemu_entries(
                qemu_entries, context, expected_latency_ns
            )
            events = jit["events"]
            joins = jit["joins"]
            completions = qemu["completions"]
            digest = jit["policy_digest"]
            policy_digests.add(digest)
            semantic_signatures.add(_semantic_signature(jit, qemu))
            runs.append({
                "repetition": repetition,
                "jit_events_sha256": _sha256(jit_path),
                "qemu_events_sha256": _sha256(qemu_path),
                "policy_digest": digest,
                "phase_id": events[0]["phase_id"],
                "canonical_events": 4,
                "request_completion_joins": 2,
                "direct_cfmws_completions": 2,
                "final_record_count": events[-1]["record_count"],
                "final_metadata_bytes": events[-1]["metadata_bytes"],
                "final_reject_count": events[-1]["reject_count"],
                "final_drop_count": events[-1]["drop_count"],
                "read_host_event_pair_span_ns":
                    events[1]["monotonic_ns"] - events[0]["monotonic_ns"],
                "write_host_event_pair_span_ns":
                    events[3]["monotonic_ns"] - events[2]["monotonic_ns"],
                "read_applied_delay_ns": completions[0]["applied_delay_ns"],
                "write_applied_delay_ns": completions[1]["applied_delay_ns"],
                "read_delay_overshoot_ns":
                    completions[0]["delay_overshoot_ns"],
                "write_delay_overshoot_ns":
                    completions[1]["delay_overshoot_ns"],
            })
            total_processes += 1
            total_events += len(events)
            total_joins += len(joins)
            total_completions += len(completions)

        metric_names = [
            "read_host_event_pair_span_ns",
            "write_host_event_pair_span_ns",
            "read_applied_delay_ns",
            "write_applied_delay_ns",
            "read_delay_overshoot_ns",
            "write_delay_overshoot_ns",
        ]
        backend_summaries[backend] = {
            "source_root": root.name,
            "repeats": expected_repeats,
            "runs": runs,
            "metrics": {
                name: _metrics(run[name] for run in runs)
                for name in metric_names
            },
        }

    _require(
        len(policy_digests) == 1,
        "policy digest differs across runs or backends",
    )
    _require(
        len(semantic_signatures) == 1,
        "backend semantic signature mismatch",
    )
    return {
        "schema": "slugarch.qemu-jit-cfmws-five.v1",
        "status": "pass",
        "modeled_latency_ns": expected_latency_ns,
        "policy_digest": next(iter(policy_digests)),
        "semantic_signature_sha256": next(iter(semantic_signatures)),
        "measurement_scope": {
            "modeled_latency":
                "CXLMemSim-requested and QEMU-applied delay; not CXL-link latency",
            "host_event_pair_span":
                "host monotonic interval between canonical request and completion records",
            "backend":
                "Rust software and FPGA-Verilator RTL simulation; no physical FPGA timing",
        },
        "validation": {
            "fresh_processes": total_processes,
            "canonical_events": total_events,
            "request_completion_joins": total_joins,
            "direct_cfmws_completions": total_completions,
            "final_records": total_processes * 4,
            "final_metadata_bytes": total_processes * 32,
            "rejects": 0,
            "drops": 0,
            "bypass_path_completions": 0,
            "semantic_signatures": len(semantic_signatures),
        },
        "backends": backend_summaries,
    }


def _backend_argument(value: str) -> tuple[str, pathlib.Path]:
    try:
        name, path = value.split("=", 1)
    except ValueError as error:
        raise argparse.ArgumentTypeError(
            "backend must use NAME=PATH"
        ) from error
    if not name or not path:
        raise argparse.ArgumentTypeError("backend must use NAME=PATH")
    return name, pathlib.Path(path)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate and summarize repeated JIT direct-CFMWS traces",
    )
    parser.add_argument(
        "--backend",
        action="append",
        required=True,
        type=_backend_argument,
        metavar="NAME=PATH",
    )
    parser.add_argument("--expected-repeats", type=int, default=5)
    parser.add_argument(
        "--modeled-latency-ns",
        type=int,
        default=MODELED_LATENCY_NS,
    )
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    backends = dict(args.backend)
    _require(
        len(backends) == len(args.backend),
        "duplicate backend argument",
    )
    summary = summarize(
        backends,
        args.expected_repeats,
        args.modeled_latency_ns,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
