#!/usr/bin/env python3
"""Run a repeated, table-oriented QEMU CXL fail-stop fault matrix."""

from __future__ import annotations

import argparse
import datetime
import json
import os
import pathlib
import subprocess
import time

from qemu_cxl_artifact_common import (
    build_provenance,
    sha256 as _sha256,
    validate_tap_success,
)

FAULT_CASES = (
    {
        "name": "ack-capacity-mismatch",
        "test_path": "/x86_64/pci/cxl/type2_sync_bad_capacity",
        "injection": "HELLO/ACK capacity field",
        "asserted_invariant":
            "device realization is rejected with a capacity error",
    },
    {
        "name": "ack-latency-out-of-range",
        "test_path": "/x86_64/pci/cxl/type2_sync_bad_latency",
        "injection": "HELLO/ACK modeled latency > 1 ms",
        "asserted_invariant":
            "device realization is rejected with a latency error",
    },
    {
        "name": "ack-request-id-mismatch",
        "test_path": "/x86_64/pci/cxl/type2_sync_bad_request_id",
        "injection": "HELLO/ACK request ID",
        "asserted_invariant":
            "device realization is rejected with a request-ID error",
    },
    {
        "name": "ack-crc32c-corruption",
        "test_path": "/x86_64/pci/cxl/type2_sync_bad_crc",
        "injection": "HELLO/ACK CRC32C",
        "asserted_invariant":
            "device realization is rejected with a CRC32C error",
    },
    {
        "name": "cfmws-two-targets",
        "test_path": "/x86_64/pci/cxl/type2_cfmws_reject_two_targets",
        "injection": "unsupported two-target CFMWS",
        "asserted_invariant":
            "zero CXLMemSim requests and zero direct completions",
    },
    {
        "name": "cfmws-size-mismatch",
        "test_path": "/x86_64/pci/cxl/type2_cfmws_reject_512m",
        "injection": "512 MiB CFMWS for a 256 MiB endpoint",
        "asserted_invariant":
            "zero CXLMemSim requests and zero direct completions",
    },
    {
        "name": "cfmws-switch-route",
        "test_path": "/x86_64/pci/cxl/type2_cfmws_reject_switch",
        "injection": "unsupported switch-routed CFMWS",
        "asserted_invariant":
            "zero CXLMemSim requests and zero direct completions",
    },
    {
        "name": "jit-request-reject",
        "test_path": "/x86_64/pci/cxl/type2_jext_cfmws_reject",
        "injection": "strict JIT reject before request transmission",
        "asserted_invariant":
            "JIT error 14, zero server requests, and zero completions",
    },
    {
        "name": "jit-request-drop",
        "test_path": "/x86_64/pci/cxl/type2_jext_cfmws_drop",
        "injection": "strict record drop before request transmission",
        "asserted_invariant":
            "JIT error 15, one drop, zero server requests, and zero completions",
    },
    {
        "name": "jit-post-commit-drop",
        "test_path":
            "/x86_64/pci/cxl/type2_jext_cfmws_post_commit_drop",
        "injection": "strict completion-record drop after server write",
        "asserted_invariant":
            "external commit is joined to error 15; zero host completions",
    },
)
EXPECTED_REPEATS = 5
EVIDENCE_SCOPE = (
    "QEMU qtest behavioral assertions; not physical CXL or FPGA timing"
)


def run_fault_matrix(
        qemu_build: pathlib.Path,
        qemu_source: pathlib.Path,
        output_root: pathlib.Path,
        repeats: int = 5) -> dict:
    qemu_build = pathlib.Path(qemu_build).resolve()
    qemu_source = pathlib.Path(qemu_source).resolve()
    output_root = pathlib.Path(output_root).resolve()
    if repeats != EXPECTED_REPEATS:
        raise ValueError(
            f"exact campaign requires {EXPECTED_REPEATS} repeats per case"
        )
    cxl_test = qemu_build / "tests/qtest/cxl-test"
    qemu_binary = qemu_build / "qemu-system-x86_64"
    cxl_source = qemu_source / "tests/qtest/cxl-test.c"
    missing = [
        str(path)
        for path in (cxl_test, qemu_binary, cxl_source)
        if not path.is_file()
    ]
    if missing:
        raise ValueError(f"missing QEMU fault-matrix inputs: {missing}")
    if output_root.exists():
        raise ValueError(f"output root already exists: {output_root}")

    output_root.mkdir(parents=True)
    raw_root = output_root / "raw"
    raw_root.mkdir()
    started_at = datetime.datetime.now(datetime.UTC)
    case_summaries = []
    all_runs = []
    for case in FAULT_CASES:
        case_root = raw_root / case["name"]
        case_root.mkdir()
        runs = []
        for repetition in range(1, repeats + 1):
            environment = os.environ.copy()
            environment["QTEST_QEMU_BINARY"] = str(qemu_binary)
            environment["MESON_TEST_ITERATION"] = str(repetition)
            for key in (
                "SLUGARCH_QTEST_CFMWS_EVIDENCE_DIR",
                "SLUGARCH_QTEST_CFMWS_JIT_LIBRARY",
                "SLUGARCH_QTEST_CFMWS_JIT_POLICY",
                "SLUGARCH_QTEST_CFMWS_JIT_MODE",
                "SLUGARCH_QTEST_CFMWS_LATENCY_NS",
            ):
                environment.pop(key, None)
            before = time.perf_counter()
            result = subprocess.run(
                (str(cxl_test), "-p", case["test_path"]),
                cwd=qemu_build,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                check=False,
            )
            elapsed_seconds = time.perf_counter() - before
            tap_path = case_root / f"run-{repetition}.tap"
            tap_path.write_text(result.stdout, encoding="utf-8")
            tap_error = None
            try:
                validate_tap_success(result.stdout, case["test_path"])
            except ValueError as error:
                tap_error = str(error)
            passed = result.returncode == 0 and tap_error is None
            run = {
                "repetition": repetition,
                "exit_code": result.returncode,
                "assertion_passed": passed,
                "wall_seconds": elapsed_seconds,
                "tap_sha256": _sha256(tap_path),
            }
            runs.append(run)
            all_runs.append({"case": case["name"], **run})
            if not passed:
                raise RuntimeError(
                    f"fault case {case['name']} repetition {repetition} "
                    f"failed: {tap_error or 'nonzero exit code'}; "
                    f"see {tap_path}"
                )
        case_summaries.append({
            **case,
            "fresh_processes": repeats,
            "assertion_failures": 0,
            "runs": runs,
        })

    summary = {
        "schema": "slugarch.qemu-cxl-fault-matrix.v1",
        "status": "pass",
        "evidence_scope": EVIDENCE_SCOPE,
        "repeats_per_case": repeats,
        "validation": {
            "fault_cases": len(FAULT_CASES),
            "fresh_test_processes": len(all_runs),
            "assertion_failures": 0,
        },
        "cases": case_summaries,
    }
    summary_path = output_root / "summary.json"
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    finished_at = datetime.datetime.now(datetime.UTC)
    manifest = {
        "schema": "slugarch.qemu-cxl-fault-matrix-run.v1",
        "status": "pass",
        "started_at_utc": started_at.isoformat(),
        "finished_at_utc": finished_at.isoformat(),
        "qemu_build": str(qemu_build),
        "qemu_source": str(qemu_source),
        "output_root": str(output_root),
        "provenance": build_provenance(
            pathlib.Path(__file__),
            pathlib.Path(__file__),
            qemu_source,
        ),
        "inputs": {
            "qemu_system_x86_64_sha256": _sha256(qemu_binary),
            "cxl_test_sha256": _sha256(cxl_test),
            "cxl_test_source_sha256": _sha256(cxl_source),
        },
        "summary_sha256": _sha256(summary_path),
    }
    (output_root / "run-manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Run repeated table-oriented QEMU CXL fault cases",
    )
    parser.add_argument("--qemu-build", required=True, type=pathlib.Path)
    parser.add_argument("--qemu-source", required=True, type=pathlib.Path)
    parser.add_argument("--output-root", required=True, type=pathlib.Path)
    parser.add_argument("--repeats", type=int, default=5)
    args = parser.parse_args(argv)
    run_fault_matrix(
        args.qemu_build,
        args.qemu_source,
        args.output_root,
        args.repeats,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
