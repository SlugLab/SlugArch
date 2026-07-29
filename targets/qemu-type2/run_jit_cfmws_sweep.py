#!/usr/bin/env python3
"""Run fresh-process SlugArch direct-CFMWS latency calibration cells."""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import pathlib
import subprocess
import time

import summarize_jit_cfmws_sweep
from qemu_cxl_artifact_common import build_provenance, validate_tap_success


TEST_PATH = "/x86_64/pci/cxl/type2_jext_cfmws_records"
DEFAULT_LATENCIES_NS = summarize_jit_cfmws_sweep.DEFAULT_LATENCIES_NS
DEFAULT_BACKENDS = summarize_jit_cfmws_sweep.DEFAULT_BACKENDS
EXPECTED_REPEATS = summarize_jit_cfmws_sweep.EXPECTED_REPEATS


def _sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def build_invocation(
        qemu_build: pathlib.Path,
        evidence_root: pathlib.Path,
        library: pathlib.Path,
        policy: pathlib.Path,
        backend: str,
        latency_ns: int,
        repetition: int,
        test_path: str = TEST_PATH) -> dict:
    qemu_build = pathlib.Path(qemu_build).resolve()
    evidence_root = pathlib.Path(evidence_root).resolve()
    library = pathlib.Path(library).resolve()
    policy = pathlib.Path(policy).resolve()
    return {
        "cwd": qemu_build,
        "command": (
            str(qemu_build / "tests/qtest/cxl-test"),
            "-p",
            test_path,
        ),
        "environment": {
            "QTEST_QEMU_BINARY":
                str(qemu_build / "qemu-system-x86_64"),
            "MESON_TEST_ITERATION": str(repetition),
            "SLUGARCH_QTEST_CFMWS_EVIDENCE_DIR": str(evidence_root),
            "SLUGARCH_QTEST_CFMWS_JIT_LIBRARY": str(library),
            "SLUGARCH_QTEST_CFMWS_JIT_POLICY": str(policy),
            "SLUGARCH_QTEST_CFMWS_JIT_MODE": backend,
            "SLUGARCH_QTEST_CFMWS_LATENCY_NS": str(latency_ns),
        },
    }


def run_sweep(
        qemu_build: pathlib.Path,
        rust_library: pathlib.Path,
        fpga_library: pathlib.Path,
        policy: pathlib.Path,
        output_root: pathlib.Path,
        qemu_source: pathlib.Path,
        latencies_ns: tuple[int, ...] = DEFAULT_LATENCIES_NS,
        repeats: int = 5) -> dict:
    qemu_build = pathlib.Path(qemu_build).resolve()
    output_root = pathlib.Path(output_root).resolve()
    policy = pathlib.Path(policy).resolve()
    if qemu_source is None:
        raise ValueError("qemu_source is required for formal provenance")
    qemu_source = pathlib.Path(qemu_source).resolve()
    libraries = {
        "rust": pathlib.Path(rust_library).resolve(),
        "fpga-verilator": pathlib.Path(fpga_library).resolve(),
    }
    required_files = (
        qemu_build / "tests/qtest/cxl-test",
        qemu_build / "qemu-system-x86_64",
        policy,
        *libraries.values(),
        qemu_source / "tests/qtest/cxl-test.c",
    )
    missing = [str(path) for path in required_files if not path.is_file()]
    if missing:
        raise ValueError(f"missing benchmark inputs: {missing}")
    if output_root.exists():
        raise ValueError(f"output root already exists: {output_root}")
    if tuple(latencies_ns) != DEFAULT_LATENCIES_NS or repeats != EXPECTED_REPEATS:
        raise ValueError(
            "exact campaign requires latencies "
            f"{DEFAULT_LATENCIES_NS}, both backends, and "
            f"{EXPECTED_REPEATS} repeats"
        )

    output_root.mkdir(parents=True)
    started_at = datetime.datetime.now(datetime.UTC)
    runs = []
    for latency_ns in latencies_ns:
        for backend in DEFAULT_BACKENDS:
            evidence_root = (
                output_root / "raw" / f"latency-{latency_ns}" / backend
            )
            evidence_root.mkdir(parents=True)
            for repetition in range(1, repeats + 1):
                invocation = build_invocation(
                    qemu_build=qemu_build,
                    evidence_root=evidence_root,
                    library=libraries[backend],
                    policy=policy,
                    backend=backend,
                    latency_ns=latency_ns,
                    repetition=repetition,
                )
                environment = os.environ.copy()
                environment.update(invocation["environment"])
                before = time.perf_counter()
                result = subprocess.run(
                    invocation["command"],
                    cwd=invocation["cwd"],
                    env=environment,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    check=False,
                )
                elapsed_seconds = time.perf_counter() - before
                tap_path = (
                    evidence_root / f"{backend}-run-{repetition}.tap"
                )
                tap_path.write_text(result.stdout, encoding="utf-8")
                tap_error = None
                try:
                    validate_tap_success(result.stdout, TEST_PATH)
                except ValueError as error:
                    tap_error = str(error)
                run = {
                    "backend": backend,
                    "modeled_latency_ns": latency_ns,
                    "repetition": repetition,
                    "exit_code": result.returncode,
                    "wall_seconds": elapsed_seconds,
                    "tap_sha256": _sha256(tap_path),
                }
                runs.append(run)
                if result.returncode != 0 or tap_error is not None:
                    raise RuntimeError(
                        "qtest failed for "
                        f"{backend}, latency={latency_ns}, "
                        f"repetition={repetition}: "
                        f"{tap_error or 'nonzero exit code'}; see {tap_path}"
                    )

    raw_root = output_root / "raw"
    summary = summarize_jit_cfmws_sweep.summarize_sweep(
        raw_root,
        latencies_ns=tuple(latencies_ns),
        expected_repeats=repeats,
    )
    summary_path = output_root / "summary.json"
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    finished_at = datetime.datetime.now(datetime.UTC)
    manifest = {
        "schema": "slugarch.qemu-jit-cfmws-sweep-run.v1",
        "status": "pass",
        "started_at_utc": started_at.isoformat(),
        "finished_at_utc": finished_at.isoformat(),
        "qemu_build": str(qemu_build),
        "qemu_source": str(qemu_source),
        "output_root": str(output_root),
        "latencies_ns": list(latencies_ns),
        "repeats": repeats,
        "test_path": TEST_PATH,
        "provenance": build_provenance(
            pathlib.Path(__file__),
            pathlib.Path(summarize_jit_cfmws_sweep.__file__),
            qemu_source,
        ),
        "inputs": {
            "qemu_system_x86_64_sha256":
                _sha256(qemu_build / "qemu-system-x86_64"),
            "cxl_test_sha256":
                _sha256(qemu_build / "tests/qtest/cxl-test"),
            "policy_sha256": _sha256(policy),
            "rust_library_sha256": _sha256(libraries["rust"]),
            "fpga_verilator_library_sha256":
                _sha256(libraries["fpga-verilator"]),
            "cxl_test_source_sha256": _sha256(
                qemu_source / "tests/qtest/cxl-test.c"
            ),
        },
        "runs": runs,
        "summary_sha256": _sha256(summary_path),
    }
    (output_root / "run-manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Run the SlugArch QEMU JIT direct-CFMWS latency sweep",
    )
    parser.add_argument("--qemu-build", required=True, type=pathlib.Path)
    parser.add_argument("--qemu-source", required=True, type=pathlib.Path)
    parser.add_argument("--rust-library", required=True, type=pathlib.Path)
    parser.add_argument("--fpga-library", required=True, type=pathlib.Path)
    parser.add_argument("--policy", required=True, type=pathlib.Path)
    parser.add_argument("--output-root", required=True, type=pathlib.Path)
    parser.add_argument(
        "--latency-ns",
        action="append",
        type=int,
        dest="latencies_ns",
    )
    parser.add_argument("--repeats", type=int, default=5)
    args = parser.parse_args(argv)
    run_sweep(
        qemu_build=args.qemu_build,
        rust_library=args.rust_library,
        fpga_library=args.fpga_library,
        policy=args.policy,
        output_root=args.output_root,
        latencies_ns=tuple(args.latencies_ns or DEFAULT_LATENCIES_NS),
        repeats=args.repeats,
        qemu_source=args.qemu_source,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
