#!/usr/bin/env python3
"""Shared fail-closed helpers for QEMU CXL evidence artifacts."""

from __future__ import annotations

import hashlib
import pathlib
import platform
import re


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with pathlib.Path(path).open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def validate_tap_success(output: str, test_path: str) -> None:
    """Require one exact, non-skipped TAP success for the selected test."""
    lines = [line.strip() for line in output.splitlines() if line.strip()]
    result_lines = [
        line for line in lines
        if re.match(r"^(?:not )?ok(?:\s|$)", line)
    ]
    expected_result = f"ok 1 {test_path}"
    if any("# SKIP" in line.upper() for line in result_lines):
        raise ValueError(f"{test_path}: TAP result contains SKIP")
    if any("# TODO" in line.upper() for line in result_lines):
        raise ValueError(f"{test_path}: TAP result contains TODO")
    if result_lines != [expected_result]:
        raise ValueError(
            f"{test_path}: expected exact TAP result {expected_result!r}, "
            f"got {result_lines!r}"
        )
    plans = [line for line in lines if re.match(r"^1\.\.", line)]
    if plans != ["1..1"]:
        raise ValueError(
            f"{test_path}: expected exact TAP plan '1..1', got {plans!r}"
        )


def build_provenance(
        runner: pathlib.Path,
        summarizer: pathlib.Path,
        qemu_source: pathlib.Path | None = None) -> dict:
    provenance = {
        "python_version": platform.python_version(),
        "runner_sha256": sha256(runner),
        "summarizer_sha256": sha256(summarizer),
    }
    if qemu_source is not None:
        cxl_source = (
            pathlib.Path(qemu_source).resolve()
            / "tests" / "qtest" / "cxl-test.c"
        )
        if not cxl_source.is_file():
            raise ValueError(f"missing QEMU cxl-test source: {cxl_source}")
        provenance["qemu_cxl_test_source_sha256"] = sha256(cxl_source)
    return provenance
