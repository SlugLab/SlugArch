#!/usr/bin/env python3
"""Validate repeated real-backend SlugArch pre-commit fail-stop evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib


JIT_EVENT_SCHEMA = "slugarch.qemu-jit-event.v1"
JIT_JOIN_SCHEMA = "slugarch.qemu-cfmws-join.v1"
OUTCOME_SCHEMA = "slugarch.qemu-cfmws-failstop.v1"
MODELED_LATENCY_NS = 400
DPA = 80 * 1024 * 1024
EMPTY_FNV1A64 = "cbf29ce484222325"
BYPASS_COUNTERS = (
    "bar4_overlay",
    "local_shadow",
    "local_cache",
    "bulk_overlay",
    "coherent_pool",
)
BACKEND_IDS = {
    "rust": 1,
    "fpga-verilator": 3,
}
EXPECTED_BACKENDS = ("rust", "fpga-verilator")
EXPECTED_REPEATS = 5
EVIDENCE_SCOPE = (
    "QEMU qtest with an in-process fake CXLMemSim service, using "
    "the Rust software and FPGA-Verilator RTL-simulation backends; "
    "no physical FPGA or production external-service evidence"
)


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _load_json(path: pathlib.Path) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read JSON {path}: {error}") from error
    _require(isinstance(value, dict), f"JSON root is not an object: {path}")
    return value


def _load_jsonl(path: pathlib.Path) -> list[dict]:
    try:
        entries = [
            json.loads(line)
            for line in path.read_text(encoding="utf-8").splitlines()
            if line
        ]
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read JSONL {path}: {error}") from error
    _require(
        entries and all(isinstance(entry, dict) for entry in entries),
        f"empty or non-object JSONL: {path}",
    )
    return entries


def _sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _check_fields(entry: dict, expected: dict, context: str) -> None:
    for field, value in expected.items():
        _require(
            entry.get(field) == value,
            f"{context}: expected {field}={value!r}, "
            f"got {entry.get(field)!r}",
        )


def _validate_run(run_dir: pathlib.Path, backend: str, context: str) -> dict:
    jit_path = run_dir / "jit-events.jsonl"
    qemu_path = run_dir / "qemu-events.jsonl"
    outcome_path = run_dir / "failstop-outcome.json"
    jit = _load_jsonl(jit_path)
    qemu = _load_jsonl(qemu_path)
    outcome = _load_json(outcome_path)
    _require(len(jit) == 2, f"{context}: expected one event and one join")
    _require(
        len(qemu) == 2,
        f"{context}: expected handshake and zero-path counters",
    )
    event, join = jit
    handshake, counters = qemu
    _check_fields(
        event,
        {
            "schema": JIT_EVENT_SCHEMA,
            "event_id": 1,
            "client_id": 1,
            "direction": 0,
            "event_class": 1,
            "opcode": 3,
            "address": DPA,
            "tag": 2,
            "status": 0,
            "payload_len": 0,
            "payload_prefix_hex": "",
            "payload_fnv1a64": EMPTY_FNV1A64,
            "result": 0,
            "effective_error": 14,
            "accepted": 0,
            "emitted": 0,
            "decision_error": 14,
            "event_count": 1,
            "record_count": 0,
            "metadata_bytes": 0,
            "reject_count": 1,
            "drop_count": 0,
            "epoch": 0,
        },
        f"{context}: rejected request event",
    )
    _require(
        isinstance(event.get("phase_id"), int) and event["phase_id"] > 0,
        f"{context}: phase ID is invalid",
    )
    _require(
        isinstance(event.get("monotonic_ns"), int),
        f"{context}: monotonic timestamp is invalid",
    )
    digest = event.get("policy_digest")
    _require(
        isinstance(digest, str)
        and len(digest) == 64
        and all(character in "0123456789abcdef" for character in digest),
        f"{context}: invalid policy digest",
    )
    _check_fields(
        join,
        {
            "schema": JIT_JOIN_SCHEMA,
            "request_event_id": 1,
            "completion_event_id": 0,
            "request_id": 2,
            "server_sequence": 0,
            "external_commit": False,
            "effective_error": 14,
            "policy_digest": digest,
        },
        f"{context}: pre-commit join",
    )
    _check_fields(
        handshake,
        {
            "event": "handshake",
            "client_id": 1,
            "capacity_bytes": 256 * 1024 * 1024,
            "configured_latency_ns": MODELED_LATENCY_NS,
            "protocol_version": 1,
        },
        f"{context}: handshake",
    )
    _require(
        isinstance(handshake.get("server_instance_id"), str)
        and len(handshake["server_instance_id"]) == 32,
        f"{context}: invalid server instance identity",
    )
    _check_fields(
        counters,
        {
            "event": "path_counters",
            "phase_id": "phase:reject",
            "direct_cfmws": 0,
        },
        f"{context}: path counters",
    )
    _require(
        all(counters.get(counter) == 0 for counter in BYPASS_COUNTERS),
        f"{context}: bypass counter is nonzero",
    )
    _check_fields(
        outcome,
        {
            "schema": OUTCOME_SCHEMA,
            "backend": backend,
            "configured_latency_ns": MODELED_LATENCY_NS,
            "jit_status": 3,
            "jit_backend": BACKEND_IDS[backend],
            "last_error": 14,
            "event_count": 1,
            "record_count": 0,
            "reject_count": 1,
            "drop_count": 0,
            "completed_reads": 0,
            "completed_writes": 0,
            "direct_cfmws_completions": 0,
            "server_memory_requests": 0,
            "server_read_requests": 0,
            "server_write_requests": 0,
            "server_sequence": 0,
            "external_commit": False,
        },
        f"{context}: fail-stop outcome",
    )
    signature_payload = {
        "event": {
            key: value
            for key, value in event.items()
            if key != "monotonic_ns"
        },
        "join": join,
    }
    signature = hashlib.sha256(json.dumps(
        signature_payload,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")).hexdigest()
    return {
        "policy_digest": digest,
        "semantic_signature_sha256": signature,
        "jit_events_sha256": _sha256(jit_path),
        "qemu_events_sha256": _sha256(qemu_path),
        "outcome_sha256": _sha256(outcome_path),
    }


def summarize_failstop(
        backends: dict[str, pathlib.Path],
        expected_repeats: int = 5) -> dict:
    _require(backends, "at least one backend is required")
    _require(
        tuple(sorted(backends)) == tuple(sorted(EXPECTED_BACKENDS))
        and expected_repeats == EXPECTED_REPEATS,
        "exact campaign requires Rust and FPGA-Verilator backends "
        f"with {EXPECTED_REPEATS} repeats each",
    )
    policy_digests = set()
    signatures = set()
    backend_summaries = {}
    total_processes = 0

    for backend, source_root in sorted(backends.items()):
        _require(backend in BACKEND_IDS, f"unsupported backend: {backend}")
        source_root = pathlib.Path(source_root).resolve()
        expected_names = [
            f"{backend}-run-{repetition}"
            for repetition in range(1, expected_repeats + 1)
        ]
        actual_names = sorted(
            path.name for path in source_root.iterdir() if path.is_dir()
        ) if source_root.is_dir() else []
        _require(
            actual_names == sorted(expected_names),
            f"{backend}: run directories do not match {expected_names}",
        )
        runs = []
        for repetition in range(1, expected_repeats + 1):
            run = _validate_run(
                source_root / f"{backend}-run-{repetition}",
                backend,
                f"{backend} repetition {repetition}",
            )
            policy_digests.add(run["policy_digest"])
            signatures.add(run["semantic_signature_sha256"])
            runs.append({"repetition": repetition, **run})
            total_processes += 1
        backend_summaries[backend] = {
            "source_root": backend,
            "repeats": expected_repeats,
            "runs": runs,
        }

    _require(
        len(policy_digests) == 1,
        "policy digest differs across runs or backends",
    )
    _require(
        len(signatures) == 1,
        "fail-stop semantic signature differs across runs or backends",
    )
    return {
        "schema": "slugarch.qemu-jit-cfmws-failstop-five.v1",
        "status": "pass",
        "evidence_scope": EVIDENCE_SCOPE,
        "fault": {
            "name": "verified-policy-strict-reject",
            "injection_point": "before CXLMemSim request transmission",
            "expected_error": 14,
            "expected_external_commit": False,
        },
        "policy_digest": next(iter(policy_digests)),
        "semantic_signature_sha256": next(iter(signatures)),
        "validation": {
            "fresh_processes": total_processes,
            "rejected_requests": total_processes,
            "external_commits": 0,
            "server_memory_requests": 0,
            "server_read_requests": 0,
            "server_write_requests": 0,
            "server_sequence": 0,
            "direct_cfmws_completions": 0,
            "records": 0,
            "drops": 0,
            "bypass_path_completions": 0,
            "semantic_signatures": len(signatures),
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
    return name, pathlib.Path(path)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate repeated SlugArch real-backend fail-stop traces",
    )
    parser.add_argument(
        "--backend",
        action="append",
        required=True,
        type=_backend_argument,
        metavar="NAME=PATH",
    )
    parser.add_argument("--expected-repeats", type=int, default=5)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    backends = dict(args.backend)
    _require(
        len(backends) == len(args.backend),
        "duplicate backend argument",
    )
    summary = summarize_failstop(backends, args.expected_repeats)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
