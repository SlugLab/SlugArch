#!/usr/bin/env python3
"""Post-hoc fail-closed verification for SlugArch QEMU CXL artifacts."""

from __future__ import annotations

import argparse
import copy
import json
import pathlib
import sys

from qemu_cxl_artifact_common import sha256, validate_tap_success
import run_jit_cfmws_failstop
import run_jit_cfmws_sweep
import run_qemu_cxl_fault_matrix
import summarize_jit_cfmws_failstop
import summarize_jit_cfmws_sweep


SWEEP_SCHEMA = "slugarch.qemu-jit-cfmws-sweep-run.v1"
SWEEP_SUMMARY_SCHEMA = "slugarch.qemu-jit-cfmws-latency-sweep.v1"
FAILSTOP_SCHEMA = "slugarch.qemu-jit-cfmws-failstop-run.v1"
FAILSTOP_SUMMARY_SCHEMA = "slugarch.qemu-jit-cfmws-failstop-five.v1"
FAULT_SCHEMA = "slugarch.qemu-cxl-fault-matrix-run.v1"
FAULT_SUMMARY_SCHEMA = "slugarch.qemu-cxl-fault-matrix.v1"
HEX_DIGITS = frozenset("0123456789abcdef")
JIT_REQUIRED_INPUT_KEYS = frozenset({
    "qemu_system_x86_64_sha256",
    "cxl_test_sha256",
    "cxl_test_source_sha256",
    "policy_sha256",
    "rust_library_sha256",
    "fpga_verilator_library_sha256",
})
FAULT_REQUIRED_INPUT_KEYS = frozenset({
    "qemu_system_x86_64_sha256",
    "cxl_test_sha256",
    "cxl_test_source_sha256",
})
SHARED_QEMU_INPUT_KEYS = (
    "qemu_system_x86_64_sha256",
    "cxl_test_sha256",
    "cxl_test_source_sha256",
)
SHARED_JIT_INPUT_KEYS = (
    "rust_library_sha256",
    "fpga_verilator_library_sha256",
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


def _require_sha256(value: object, context: str) -> None:
    _require(
        isinstance(value, str)
        and len(value) == 64
        and set(value) <= HEX_DIGITS,
        f"{context}: expected lowercase SHA-256",
    )


def _verify_hash(path: pathlib.Path, expected: object, context: str) -> None:
    _require_sha256(expected, context)
    _require(path.is_file(), f"{context}: missing file {path}")
    actual = sha256(path)
    _require(
        actual == expected,
        f"{context}: SHA-256 mismatch for {path}: "
        f"expected {expected}, got {actual}",
    )


def _verify_common_manifest(
        root: pathlib.Path,
        manifest: dict,
        summary: dict,
        manifest_schema: str,
        summary_schema: str,
        runner_path: pathlib.Path,
        summarizer_path: pathlib.Path,
        required_input_keys: frozenset[str]) -> None:
    _require(
        manifest.get("schema") == manifest_schema,
        f"{root}: manifest schema mismatch",
    )
    _require(
        summary.get("schema") == summary_schema,
        f"{root}: summary schema mismatch",
    )
    _require(
        manifest.get("status") == "pass"
        and summary.get("status") == "pass",
        f"{root}: artifact status is not pass",
    )
    _verify_hash(
        root / "summary.json",
        manifest.get("summary_sha256"),
        f"{root}: summary",
    )
    inputs = manifest.get("inputs")
    _require(isinstance(inputs, dict) and inputs, f"{root}: missing inputs")
    actual_input_keys = frozenset(inputs)
    missing_input_keys = sorted(required_input_keys - actual_input_keys)
    unexpected_input_keys = sorted(actual_input_keys - required_input_keys)
    _require(
        not missing_input_keys and not unexpected_input_keys,
        f"{root}: input keys mismatch; missing={missing_input_keys}, "
        f"unexpected={unexpected_input_keys}",
    )
    for name, digest in inputs.items():
        _require_sha256(digest, f"{root}: input {name}")
    provenance = manifest.get("provenance")
    _require(
        isinstance(provenance, dict),
        f"{root}: provenance is required",
    )
    _require(
        isinstance(provenance.get("python_version"), str)
        and provenance["python_version"],
        f"{root}: missing Python provenance",
    )
    _verify_script_provenance(
        provenance.get("runner_sha256"),
        runner_path,
        f"{root}: runner provenance",
    )
    _verify_script_provenance(
        provenance.get("summarizer_sha256"),
        summarizer_path,
        f"{root}: summarizer provenance",
    )
    source_digest = provenance.get("qemu_cxl_test_source_sha256")
    input_source_digest = inputs.get("cxl_test_source_sha256")
    _require_sha256(
        source_digest,
        f"{root}: QEMU cxl-test source provenance",
    )
    _require(
        source_digest == input_source_digest,
        f"{root}: QEMU cxl-test source provenance mismatch",
    )


def _verify_script_provenance(
        declared: object,
        path: pathlib.Path,
        context: str) -> None:
    _require_sha256(declared, context)
    actual = sha256(path)
    _require(
        declared == actual,
        f"{context}: expected {actual}, got {declared}",
    )


def _require_wall_seconds(value: object, context: str) -> None:
    _require(
        not isinstance(value, bool)
        and isinstance(value, (int, float))
        and value >= 0,
        f"{context}: invalid wall_seconds",
    )


def _verify_sweep_source_roots(summary: dict, context: str) -> None:
    _require(
        summary.get("source_root") == "raw",
        f"{context}: source_root must be 'raw'",
    )
    cells = summary.get("cells", {})
    for latency in summarize_jit_cfmws_sweep.DEFAULT_LATENCIES_NS:
        cell = cells.get(str(latency), {})
        for backend in summarize_jit_cfmws_sweep.DEFAULT_BACKENDS:
            try:
                source_root = cell["backends"][backend]["source_root"]
            except (KeyError, TypeError) as error:
                raise ValueError(
                    f"sweep summary missing {latency}/{backend}"
                ) from error
            _require(
                source_root == backend,
                f"{context}: {latency}/{backend} source_root "
                f"must be {backend!r}",
            )


def _verify_failstop_source_roots(summary: dict, context: str) -> None:
    backends = summary.get("backends", {})
    for backend in summarize_jit_cfmws_failstop.EXPECTED_BACKENDS:
        try:
            source_root = backends[backend]["source_root"]
        except (KeyError, TypeError) as error:
            raise ValueError(
                f"fail-stop summary missing backend {backend}"
            ) from error
        _require(
            source_root == backend,
            f"{context}: {backend} source_root must be {backend!r}",
        )


def _require_equal(actual: dict, expected: dict, context: str) -> None:
    _require(
        actual == expected,
        f"{context}: deterministic resummarization mismatch",
    )


def verify_sweep(root: pathlib.Path) -> int:
    root = pathlib.Path(root).resolve()
    _require(root.is_dir(), f"sweep artifact is not a directory: {root}")
    manifest = _load_json(root / "run-manifest.json")
    summary = _load_json(root / "summary.json")
    _verify_common_manifest(
        root,
        manifest,
        summary,
        SWEEP_SCHEMA,
        SWEEP_SUMMARY_SCHEMA,
        pathlib.Path(run_jit_cfmws_sweep.__file__),
        pathlib.Path(summarize_jit_cfmws_sweep.__file__),
        JIT_REQUIRED_INPUT_KEYS,
    )
    _verify_sweep_source_roots(summary, f"{root}: sweep summary")
    expected_latencies = list(
        summarize_jit_cfmws_sweep.DEFAULT_LATENCIES_NS
    )
    expected_backends = list(
        summarize_jit_cfmws_sweep.DEFAULT_BACKENDS
    )
    _require(
        manifest.get("latencies_ns") == expected_latencies
        and manifest.get("repeats")
        == summarize_jit_cfmws_sweep.EXPECTED_REPEATS
        and manifest.get("test_path") == run_jit_cfmws_sweep.TEST_PATH,
        f"{root}: sweep manifest violates the exact campaign contract",
    )
    _require(
        summary.get("latencies_ns") == expected_latencies
        and summary.get("backends") == expected_backends
        and summary.get("expected_repeats")
        == summarize_jit_cfmws_sweep.EXPECTED_REPEATS,
        f"{root}: sweep summary violates the exact campaign contract",
    )
    runs = manifest.get("runs")
    _require(isinstance(runs, list), f"{root}: sweep runs are not a list")
    expected_keys = {
        (latency, backend, repetition)
        for latency in summarize_jit_cfmws_sweep.DEFAULT_LATENCIES_NS
        for backend in summarize_jit_cfmws_sweep.DEFAULT_BACKENDS
        for repetition in range(
            1, summarize_jit_cfmws_sweep.EXPECTED_REPEATS + 1
        )
    }
    actual_keys = set()
    expected_taps = set()
    for run in runs:
        _require(isinstance(run, dict), f"{root}: invalid sweep run")
        key = (
            run.get("modeled_latency_ns"),
            run.get("backend"),
            run.get("repetition"),
        )
        _require(key not in actual_keys, f"{root}: duplicate sweep run {key}")
        actual_keys.add(key)
        _require(
            run.get("exit_code") == 0,
            f"{root}: nonzero sweep exit for {key}",
        )
        _require_wall_seconds(run.get("wall_seconds"), f"{root}: {key}")
        latency, backend, repetition = key
        tap_path = (
            root / "raw" / f"latency-{latency}" / str(backend)
            / f"{backend}-run-{repetition}.tap"
        )
        expected_taps.add(tap_path)
        _verify_hash(
            tap_path,
            run.get("tap_sha256"),
            f"{root}: sweep TAP {key}",
        )
        validate_tap_success(
            tap_path.read_text(encoding="utf-8"),
            run_jit_cfmws_sweep.TEST_PATH,
        )
    _require(
        actual_keys == expected_keys,
        f"{root}: sweep run set violates the exact campaign contract",
    )
    actual_taps = set((root / "raw").rglob("*.tap"))
    _require(
        actual_taps == expected_taps,
        f"{root}: unexpected or missing sweep TAP files",
    )
    fresh = summarize_jit_cfmws_sweep.summarize_sweep(root / "raw")
    _verify_sweep_source_roots(fresh, f"{root}: fresh sweep summary")
    _require_equal(
        summary,
        fresh,
        f"{root}: sweep",
    )
    _require(
        summary.get("validation", {}).get("fresh_processes") == 40,
        f"{root}: sweep does not contain 40 fresh processes",
    )
    return 40


def verify_failstop(root: pathlib.Path) -> int:
    root = pathlib.Path(root).resolve()
    _require(root.is_dir(), f"fail-stop artifact is not a directory: {root}")
    manifest = _load_json(root / "run-manifest.json")
    summary = _load_json(root / "summary.json")
    _verify_common_manifest(
        root,
        manifest,
        summary,
        FAILSTOP_SCHEMA,
        FAILSTOP_SUMMARY_SCHEMA,
        pathlib.Path(run_jit_cfmws_failstop.__file__),
        pathlib.Path(summarize_jit_cfmws_failstop.__file__),
        JIT_REQUIRED_INPUT_KEYS,
    )
    _require(
        summary.get("evidence_scope")
        == summarize_jit_cfmws_failstop.EVIDENCE_SCOPE,
        f"{root}: fail-stop evidence_scope mismatch",
    )
    _verify_failstop_source_roots(
        summary,
        f"{root}: fail-stop summary",
    )
    _require(
        manifest.get("repeats")
        == summarize_jit_cfmws_failstop.EXPECTED_REPEATS
        and manifest.get("test_path") == run_jit_cfmws_failstop.TEST_PATH,
        f"{root}: fail-stop manifest violates the exact campaign contract",
    )
    runs = manifest.get("runs")
    _require(
        isinstance(runs, list),
        f"{root}: fail-stop runs are not a list",
    )
    expected_keys = {
        (backend, repetition)
        for backend in summarize_jit_cfmws_failstop.EXPECTED_BACKENDS
        for repetition in range(
            1, summarize_jit_cfmws_failstop.EXPECTED_REPEATS + 1
        )
    }
    actual_keys = set()
    expected_taps = set()
    for run in runs:
        _require(isinstance(run, dict), f"{root}: invalid fail-stop run")
        key = (run.get("backend"), run.get("repetition"))
        _require(
            key not in actual_keys,
            f"{root}: duplicate fail-stop run {key}",
        )
        actual_keys.add(key)
        _require(
            run.get("exit_code") == 0,
            f"{root}: nonzero fail-stop exit for {key}",
        )
        _require_wall_seconds(run.get("wall_seconds"), f"{root}: {key}")
        backend, repetition = key
        tap_path = (
            root / "raw" / str(backend)
            / f"{backend}-run-{repetition}.tap"
        )
        expected_taps.add(tap_path)
        _verify_hash(
            tap_path,
            run.get("tap_sha256"),
            f"{root}: fail-stop TAP {key}",
        )
        validate_tap_success(
            tap_path.read_text(encoding="utf-8"),
            run_jit_cfmws_failstop.TEST_PATH,
        )
    _require(
        actual_keys == expected_keys,
        f"{root}: fail-stop run set violates the exact campaign contract",
    )
    actual_taps = set((root / "raw").rglob("*.tap"))
    _require(
        actual_taps == expected_taps,
        f"{root}: unexpected or missing fail-stop TAP files",
    )
    fresh = summarize_jit_cfmws_failstop.summarize_failstop({
        backend: root / "raw" / backend
        for backend in summarize_jit_cfmws_failstop.EXPECTED_BACKENDS
    })
    _verify_failstop_source_roots(
        fresh,
        f"{root}: fresh fail-stop summary",
    )
    _require_equal(
        summary,
        fresh,
        f"{root}: fail-stop",
    )
    _require(
        summary.get("validation", {}).get("fresh_processes") == 10,
        f"{root}: fail-stop does not contain 10 fresh processes",
    )
    return 10


def _fault_summary_without_wall_seconds(summary: dict) -> dict:
    normalized = copy.deepcopy(summary)
    for case in normalized.get("cases", []):
        for run in case.get("runs", []):
            run.pop("wall_seconds", None)
    return normalized


def _resummarize_fault_matrix(root: pathlib.Path) -> dict:
    cases = []
    for case in run_qemu_cxl_fault_matrix.FAULT_CASES:
        runs = []
        for repetition in range(
                1, run_qemu_cxl_fault_matrix.EXPECTED_REPEATS + 1):
            tap_path = (
                root / "raw" / case["name"] / f"run-{repetition}.tap"
            )
            _require(tap_path.is_file(), f"missing fault TAP: {tap_path}")
            validate_tap_success(
                tap_path.read_text(encoding="utf-8"),
                case["test_path"],
            )
            runs.append({
                "assertion_passed": True,
                "exit_code": 0,
                "repetition": repetition,
                "tap_sha256": sha256(tap_path),
            })
        cases.append({
            **case,
            "assertion_failures": 0,
            "fresh_processes":
                run_qemu_cxl_fault_matrix.EXPECTED_REPEATS,
            "runs": runs,
        })
    return {
        "schema": FAULT_SUMMARY_SCHEMA,
        "status": "pass",
        "evidence_scope": run_qemu_cxl_fault_matrix.EVIDENCE_SCOPE,
        "repeats_per_case": run_qemu_cxl_fault_matrix.EXPECTED_REPEATS,
        "validation": {
            "fault_cases": len(run_qemu_cxl_fault_matrix.FAULT_CASES),
            "fresh_test_processes":
                len(run_qemu_cxl_fault_matrix.FAULT_CASES)
                * run_qemu_cxl_fault_matrix.EXPECTED_REPEATS,
            "assertion_failures": 0,
        },
        "cases": cases,
    }


def verify_fault_matrix(root: pathlib.Path) -> int:
    root = pathlib.Path(root).resolve()
    _require(root.is_dir(), f"fault artifact is not a directory: {root}")
    manifest = _load_json(root / "run-manifest.json")
    summary = _load_json(root / "summary.json")
    _verify_common_manifest(
        root,
        manifest,
        summary,
        FAULT_SCHEMA,
        FAULT_SUMMARY_SCHEMA,
        pathlib.Path(run_qemu_cxl_fault_matrix.__file__),
        pathlib.Path(run_qemu_cxl_fault_matrix.__file__),
        FAULT_REQUIRED_INPUT_KEYS,
    )
    _require(
        summary.get("evidence_scope")
        == run_qemu_cxl_fault_matrix.EVIDENCE_SCOPE,
        f"{root}: fault-matrix evidence_scope mismatch",
    )
    _require(
        summary.get("repeats_per_case")
        == run_qemu_cxl_fault_matrix.EXPECTED_REPEATS
        and summary.get("validation", {}).get("fault_cases") == 10
        and summary.get("validation", {}).get("fresh_test_processes") == 50,
        f"{root}: fault matrix violates the exact campaign contract",
    )
    cases = summary.get("cases")
    _require(
        isinstance(cases, list) and len(cases) == 10,
        f"{root}: fault matrix must contain 10 cases",
    )
    expected_taps = set()
    for expected_case, actual_case in zip(
            run_qemu_cxl_fault_matrix.FAULT_CASES, cases, strict=True):
        for field in ("name", "test_path", "injection", "asserted_invariant"):
            _require(
                actual_case.get(field) == expected_case[field],
                f"{root}: fault case {expected_case['name']} "
                f"has wrong {field}",
            )
        runs = actual_case.get("runs")
        _require(
            isinstance(runs, list) and len(runs) == 5,
            f"{root}: fault case {expected_case['name']} needs 5 runs",
        )
        repetitions = set()
        for run in runs:
            repetition = run.get("repetition")
            _require(
                repetition not in repetitions,
                f"{root}: duplicate fault repetition "
                f"{expected_case['name']}/{repetition}",
            )
            repetitions.add(repetition)
            _require(
                run.get("exit_code") == 0
                and run.get("assertion_passed") is True,
                f"{root}: failed fault run "
                f"{expected_case['name']}/{repetition}",
            )
            _require_wall_seconds(
                run.get("wall_seconds"),
                f"{root}: {expected_case['name']}/{repetition}",
            )
            tap_path = (
                root / "raw" / expected_case["name"]
                / f"run-{repetition}.tap"
            )
            expected_taps.add(tap_path)
            _verify_hash(
                tap_path,
                run.get("tap_sha256"),
                f"{root}: fault TAP "
                f"{expected_case['name']}/{repetition}",
            )
            validate_tap_success(
                tap_path.read_text(encoding="utf-8"),
                expected_case["test_path"],
            )
        _require(
            repetitions == set(range(1, 6)),
            f"{root}: fault case {expected_case['name']} "
            "has wrong repetitions",
        )
    actual_taps = set((root / "raw").rglob("*.tap"))
    _require(
        actual_taps == expected_taps,
        f"{root}: unexpected or missing fault TAP files",
    )
    fresh = _resummarize_fault_matrix(root)
    _require_equal(
        _fault_summary_without_wall_seconds(summary),
        fresh,
        f"{root}: fault matrix",
    )
    return 50


def _cross_check_declared_inputs(
        sweep: pathlib.Path,
        failstop: pathlib.Path,
        fault_matrix: pathlib.Path) -> None:
    manifests = {
        "sweep": _load_json(
            pathlib.Path(sweep).resolve() / "run-manifest.json"
        ),
        "failstop": _load_json(
            pathlib.Path(failstop).resolve() / "run-manifest.json"
        ),
        "fault-matrix": _load_json(
            pathlib.Path(fault_matrix).resolve() / "run-manifest.json"
        ),
    }

    def require_match(field: str, campaign_names: tuple[str, ...]) -> None:
        values = {
            name: manifests[name]["inputs"][field]
            for name in campaign_names
        }
        _require(
            len(set(values.values())) == 1,
            f"cross-campaign input mismatch for {field}: {values}",
        )

    for field in SHARED_QEMU_INPUT_KEYS:
        require_match(field, ("sweep", "failstop", "fault-matrix"))
    for field in SHARED_JIT_INPUT_KEYS:
        require_match(field, ("sweep", "failstop"))


def verify_artifacts(
        sweep: pathlib.Path,
        failstop: pathlib.Path,
        fault_matrix: pathlib.Path) -> dict:
    sweep_processes = verify_sweep(sweep)
    failstop_processes = verify_failstop(failstop)
    fault_processes = verify_fault_matrix(fault_matrix)
    _cross_check_declared_inputs(sweep, failstop, fault_matrix)
    return {
        "schema": "slugarch.qemu-cxl-artifact-verification.v1",
        "status": "pass",
        "validation": {
            "artifacts": 3,
            "sweep_processes": sweep_processes,
            "failstop_processes": failstop_processes,
            "fault_matrix_processes": fault_processes,
            "total_processes":
                sweep_processes + failstop_processes + fault_processes,
            "manifests": 3,
            "summaries": 3,
            "artifact_file_hashes_verified": True,
            "declared_input_hashes_validated":
                "required, SHA-256-formatted, "
                "and cross-campaign-consistent",
            "tap_skips": 0,
            "deterministic_resummarization": True,
        },
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Verify SlugArch QEMU CXL artifact manifests and raw data",
    )
    parser.add_argument("--sweep", required=True, type=pathlib.Path)
    parser.add_argument("--failstop", required=True, type=pathlib.Path)
    parser.add_argument("--fault-matrix", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        report = verify_artifacts(
            args.sweep,
            args.failstop,
            args.fault_matrix,
        )
    except (OSError, ValueError) as error:
        print(f"verification failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
