#!/usr/bin/env python3
"""Run repeated real-backend SlugArch pre-commit fail-stop probes."""

from __future__ import annotations

import argparse
import datetime
import json
import os
import pathlib
import subprocess
import time

import run_jit_cfmws_sweep
import summarize_jit_cfmws_failstop
from qemu_cxl_artifact_common import build_provenance, validate_tap_success


TEST_PATH = "/x86_64/pci/cxl/type2_jext_cfmws_external_reject"
BACKENDS = ("rust", "fpga-verilator")
MODELED_LATENCY_NS = 400
EXPECTED_REPEATS = 5


def run_failstop(
        qemu_build: pathlib.Path,
        rust_library: pathlib.Path,
        fpga_library: pathlib.Path,
        policy: pathlib.Path,
        output_root: pathlib.Path,
        qemu_source: pathlib.Path,
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
        raise ValueError(f"missing fail-stop inputs: {missing}")
    if output_root.exists():
        raise ValueError(f"output root already exists: {output_root}")
    if repeats != EXPECTED_REPEATS:
        raise ValueError(
            "exact campaign requires Rust and FPGA-Verilator backends "
            f"with {EXPECTED_REPEATS} repeats each"
        )

    output_root.mkdir(parents=True)
    started_at = datetime.datetime.now(datetime.UTC)
    runs = []
    for backend in BACKENDS:
        evidence_root = output_root / "raw" / backend
        evidence_root.mkdir(parents=True)
        for repetition in range(1, repeats + 1):
            invocation = run_jit_cfmws_sweep.build_invocation(
                qemu_build=qemu_build,
                evidence_root=evidence_root,
                library=libraries[backend],
                policy=policy,
                backend=backend,
                latency_ns=MODELED_LATENCY_NS,
                repetition=repetition,
                test_path=TEST_PATH,
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
            tap_path = evidence_root / f"{backend}-run-{repetition}.tap"
            tap_path.write_text(result.stdout, encoding="utf-8")
            tap_error = None
            try:
                validate_tap_success(result.stdout, TEST_PATH)
            except ValueError as error:
                tap_error = str(error)
            runs.append({
                "backend": backend,
                "repetition": repetition,
                "exit_code": result.returncode,
                "wall_seconds": elapsed_seconds,
                "tap_sha256": run_jit_cfmws_sweep._sha256(tap_path),
            })
            if result.returncode != 0 or tap_error is not None:
                raise RuntimeError(
                    f"fail-stop qtest failed for {backend}, "
                    f"repetition={repetition}: "
                    f"{tap_error or 'nonzero exit code'}; see {tap_path}"
                )

    raw_root = output_root / "raw"
    summary = summarize_jit_cfmws_failstop.summarize_failstop(
        {backend: raw_root / backend for backend in BACKENDS},
        expected_repeats=repeats,
    )
    summary_path = output_root / "summary.json"
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    finished_at = datetime.datetime.now(datetime.UTC)
    manifest = {
        "schema": "slugarch.qemu-jit-cfmws-failstop-run.v1",
        "status": "pass",
        "started_at_utc": started_at.isoformat(),
        "finished_at_utc": finished_at.isoformat(),
        "qemu_build": str(qemu_build),
        "qemu_source": str(qemu_source),
        "output_root": str(output_root),
        "repeats": repeats,
        "test_path": TEST_PATH,
        "provenance": build_provenance(
            pathlib.Path(__file__),
            pathlib.Path(summarize_jit_cfmws_failstop.__file__),
            qemu_source,
        ),
        "inputs": {
            "qemu_system_x86_64_sha256":
                run_jit_cfmws_sweep._sha256(
                    qemu_build / "qemu-system-x86_64"
                ),
            "cxl_test_sha256":
                run_jit_cfmws_sweep._sha256(
                    qemu_build / "tests/qtest/cxl-test"
                ),
            "policy_sha256": run_jit_cfmws_sweep._sha256(policy),
            "rust_library_sha256":
                run_jit_cfmws_sweep._sha256(libraries["rust"]),
            "fpga_verilator_library_sha256":
                run_jit_cfmws_sweep._sha256(
                    libraries["fpga-verilator"]
                ),
            "cxl_test_source_sha256":
                run_jit_cfmws_sweep._sha256(
                    qemu_source / "tests/qtest/cxl-test.c"
                ),
        },
        "runs": runs,
        "summary_sha256": run_jit_cfmws_sweep._sha256(summary_path),
    }
    (output_root / "run-manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Run SlugArch QEMU real-backend fail-stop probes",
    )
    parser.add_argument("--qemu-build", required=True, type=pathlib.Path)
    parser.add_argument("--qemu-source", required=True, type=pathlib.Path)
    parser.add_argument("--rust-library", required=True, type=pathlib.Path)
    parser.add_argument("--fpga-library", required=True, type=pathlib.Path)
    parser.add_argument("--policy", required=True, type=pathlib.Path)
    parser.add_argument("--output-root", required=True, type=pathlib.Path)
    parser.add_argument("--repeats", type=int, default=5)
    args = parser.parse_args(argv)
    run_failstop(
        qemu_build=args.qemu_build,
        rust_library=args.rust_library,
        fpga_library=args.fpga_library,
        policy=args.policy,
        output_root=args.output_root,
        repeats=args.repeats,
        qemu_source=args.qemu_source,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
