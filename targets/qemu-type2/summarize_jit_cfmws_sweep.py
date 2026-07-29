#!/usr/bin/env python3
"""Validate and aggregate a multi-latency SlugArch direct-CFMWS sweep."""

from __future__ import annotations

import argparse
import json
import pathlib

import summarize_jit_cfmws


DEFAULT_LATENCIES_NS = (80, 400, 2_000, 10_000)
DEFAULT_BACKENDS = ("rust", "fpga-verilator")
EXPECTED_REPEATS = 5
SERIES_METRICS = (
    "read_applied_delay_ns",
    "write_applied_delay_ns",
    "read_delay_overshoot_ns",
    "write_delay_overshoot_ns",
    "read_host_event_pair_span_ns",
    "write_host_event_pair_span_ns",
)


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def summarize_sweep(
        root: pathlib.Path,
        latencies_ns: tuple[int, ...] = DEFAULT_LATENCIES_NS,
        expected_repeats: int = 5,
        backends: tuple[str, ...] = DEFAULT_BACKENDS) -> dict:
    root = pathlib.Path(root).resolve()
    latencies_ns = tuple(latencies_ns)
    backends = tuple(backends)
    _require(root.is_dir(), f"sweep root is not a directory: {root}")
    _require(
        latencies_ns == DEFAULT_LATENCIES_NS
        and backends == DEFAULT_BACKENDS
        and expected_repeats == EXPECTED_REPEATS,
        "exact campaign requires latencies "
        f"{DEFAULT_LATENCIES_NS}, backends {DEFAULT_BACKENDS}, "
        f"and {EXPECTED_REPEATS} repeats",
    )

    cells = {}
    series = {
        backend: {metric: [] for metric in SERIES_METRICS}
        for backend in backends
    }
    policy_digests = set()
    semantic_signatures = set()
    validation_totals = {
        "fresh_processes": 0,
        "canonical_events": 0,
        "request_completion_joins": 0,
        "direct_cfmws_completions": 0,
        "final_records": 0,
        "final_metadata_bytes": 0,
        "rejects": 0,
        "drops": 0,
        "bypass_path_completions": 0,
    }

    for latency_ns in latencies_ns:
        cell_backends = {
            backend: root / f"latency-{latency_ns}" / backend
            for backend in backends
        }
        cell = summarize_jit_cfmws.summarize(
            cell_backends,
            expected_repeats=expected_repeats,
            expected_latency_ns=latency_ns,
        )
        cells[str(latency_ns)] = cell
        policy_digests.add(cell["policy_digest"])
        semantic_signatures.add(cell["semantic_signature_sha256"])
        for name in validation_totals:
            validation_totals[name] += cell["validation"][name]
        for backend in backends:
            metrics = cell["backends"][backend]["metrics"]
            for metric in SERIES_METRICS:
                series[backend][metric].append({
                    "modeled_latency_ns": latency_ns,
                    **metrics[metric],
                })

    _require(
        len(policy_digests) == 1,
        "policy digest differs across latency cells",
    )
    _require(
        len(semantic_signatures) == 1,
        "replay semantic signature differs across latency cells",
    )
    for backend in backends:
        for operation in ("read", "write"):
            points = series[backend][f"{operation}_applied_delay_ns"]
            medians = [point["median"] for point in points]
            _require(
                all(
                    right > left
                    for left, right in zip(medians, medians[1:])
                ),
                f"{backend} {operation}: non-monotonic applied-delay medians",
            )

    return {
        "schema": "slugarch.qemu-jit-cfmws-latency-sweep.v1",
        "status": "pass",
        "source_root": "raw",
        "latencies_ns": list(latencies_ns),
        "backends": list(backends),
        "expected_repeats": expected_repeats,
        "policy_digest": next(iter(policy_digests)),
        "replay_semantic_signature_sha256":
            next(iter(semantic_signatures)),
        "measurement_scope": {
            "modeled_latency":
                "CXLMemSim-requested and QEMU-applied delay; not CXL-link latency",
            "process":
                "one filtered qtest invocation starts one fresh QEMU process",
            "backend":
                "Rust software and FPGA-Verilator RTL simulation; no physical FPGA timing",
            "service":
                "in-process fake CXLMemSim service; "
                "not a production external service",
        },
        "validation": {
            **validation_totals,
            "latency_cells": len(latencies_ns),
            "backend_latency_cells": len(latencies_ns) * len(backends),
            "semantic_signatures": len(semantic_signatures),
            "monotonic_applied_delay_medians": True,
        },
        "series": series,
        "cells": cells,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate a repeated SlugArch JIT CFMWS latency sweep",
    )
    parser.add_argument("--root", required=True, type=pathlib.Path)
    parser.add_argument(
        "--latency-ns",
        action="append",
        type=int,
        dest="latencies_ns",
    )
    parser.add_argument(
        "--backend",
        action="append",
        choices=DEFAULT_BACKENDS,
        dest="backends",
    )
    parser.add_argument("--expected-repeats", type=int, default=5)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    summary = summarize_sweep(
        args.root,
        latencies_ns=tuple(args.latencies_ns or DEFAULT_LATENCIES_NS),
        expected_repeats=args.expected_repeats,
        backends=tuple(args.backends or DEFAULT_BACKENDS),
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
