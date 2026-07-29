#!/usr/bin/env python3

import hashlib
import json
import os
import pathlib
import platform
import shutil
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest import mock


SCRIPT_DIR = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))

import summarize_jit_cfmws as summarizer  # noqa: E402
import summarize_jit_cfmws_failstop as failstop_summarizer  # noqa: E402
import summarize_jit_cfmws_sweep as sweep_summarizer  # noqa: E402
import run_jit_cfmws_failstop as failstop_runner  # noqa: E402
import run_jit_cfmws_sweep as sweep_runner  # noqa: E402
import run_qemu_cxl_fault_matrix as fault_runner  # noqa: E402


summarize = summarizer.summarize


POLICY_DIGEST = "bc91e1b53305764adbaf714367cf2bf91206fbefade323b3b507307970ff81d4"
PHASE_ID = 2215048676827443796
REPO_ROOT = SCRIPT_DIR.parents[1]
ARTIFACT_ROOT = REPO_ROOT / "artifact" / "slugarch_cxlmemsim"
SWEEP_ARTIFACT = (
    ARTIFACT_ROOT / "qemu-jit-cfmws-latency-sweep-20260729"
)
FAILSTOP_ARTIFACT = (
    ARTIFACT_ROOT / "qemu-jit-cfmws-failstop-20260729"
)


def write_jsonl(path, entries):
    path.write_text(
        "".join(json.dumps(entry, separators=(",", ":")) + "\n"
                for entry in entries),
        encoding="utf-8",
    )


def mutate_jsonl(path, index, key, value):
    entries = [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
    ]
    entries[index][key] = value
    write_jsonl(path, entries)


def make_fake_qemu_inputs(root):
    qemu_build = root / "build"
    (qemu_build / "tests" / "qtest").mkdir(parents=True)
    cxl_test = qemu_build / "tests" / "qtest" / "cxl-test"
    cxl_test.write_text(
        "#!/bin/sh\n"
        "printf 'TAP version 14\\nok 1 %s # SKIP unavailable\\n"
        "1..1\\n' \"$2\"\n",
        encoding="utf-8",
    )
    cxl_test.chmod(0o755)
    (qemu_build / "qemu-system-x86_64").write_bytes(b"qemu")
    rust_library = root / "rust.so"
    fpga_library = root / "fpga.so"
    policy = root / "policy.json"
    rust_library.write_bytes(b"rust")
    fpga_library.write_bytes(b"fpga")
    policy.write_text("{}\n", encoding="utf-8")
    return qemu_build, rust_library, fpga_library, policy


def make_fake_qemu_source(root):
    qemu_source = root / "source"
    (qemu_source / "tests" / "qtest").mkdir(parents=True)
    (qemu_source / "tests" / "qtest" / "cxl-test.c").write_text(
        "/* fixture */\n",
        encoding="utf-8",
    )
    return qemu_source


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def exact_tap(test_path):
    return f"TAP version 14\nok 1 {test_path}\n1..1\n"


def make_run(
        root, backend, repetition, time_base, read_delay, write_delay,
        modeled_latency=400):
    run_dir = root / f"{backend}-run-{repetition}"
    run_dir.mkdir()
    common = {
        "schema": "slugarch.qemu-jit-event.v1",
        "client_id": 1,
        "address": 83886080,
        "phase_id": PHASE_ID,
        "status": 0,
        "policy_digest": POLICY_DIGEST,
        "result": 0,
        "effective_error": 0,
        "accepted": 1,
        "emitted": 1,
        "decision_error": 0,
        "reject_count": 0,
        "drop_count": 0,
        "epoch": PHASE_ID,
    }
    events = [
        {
            **common,
            "event_id": 1,
            "direction": 0,
            "event_class": 1,
            "opcode": 3,
            "tag": 2,
            "monotonic_ns": time_base,
            "payload_len": 0,
            "payload_prefix_hex": "",
            "payload_fnv1a64": "cbf29ce484222325",
            "event_count": 1,
            "record_count": 1,
            "metadata_bytes": 8,
        },
        {
            **common,
            "event_id": 2,
            "direction": 1,
            "event_class": 3,
            "opcode": 5,
            "tag": 2,
            "monotonic_ns": time_base + 100,
            "payload_len": 8,
            "payload_prefix_hex": "8877665544332211",
            "payload_fnv1a64": "82f950555efebd75",
            "event_count": 2,
            "record_count": 2,
            "metadata_bytes": 16,
        },
        {
            "schema": "slugarch.qemu-cfmws-join.v1",
            "request_event_id": 1,
            "completion_event_id": 2,
            "request_id": 2,
            "server_sequence": 1,
            "external_commit": False,
            "effective_error": 0,
            "policy_digest": POLICY_DIGEST,
        },
        {
            **common,
            "event_id": 3,
            "direction": 0,
            "event_class": 2,
            "opcode": 4,
            "tag": 3,
            "monotonic_ns": time_base + 300,
            "payload_len": 8,
            "payload_prefix_hex": "1122334455667788",
            "payload_fnv1a64": "f6fe41c3df7a3a4d",
            "event_count": 3,
            "record_count": 3,
            "metadata_bytes": 24,
        },
        {
            **common,
            "event_id": 4,
            "direction": 1,
            "event_class": 4,
            "opcode": 5,
            "tag": 3,
            "monotonic_ns": time_base + 500,
            "payload_len": 0,
            "payload_prefix_hex": "",
            "payload_fnv1a64": "cbf29ce484222325",
            "event_count": 4,
            "record_count": 4,
            "metadata_bytes": 32,
        },
        {
            "schema": "slugarch.qemu-cfmws-join.v1",
            "request_event_id": 3,
            "completion_event_id": 4,
            "request_id": 3,
            "server_sequence": 2,
            "external_commit": False,
            "effective_error": 0,
            "policy_digest": POLICY_DIGEST,
        },
    ]
    write_jsonl(run_dir / "jit-events.jsonl", events)

    counters = lambda count: {
        "event": "path_counters",
        "phase_id": "phase:cfmws",
        "direct_cfmws": count,
        "bar4_overlay": 0,
        "local_shadow": 0,
        "local_cache": 0,
        "bulk_overlay": 0,
        "coherent_pool": 0,
    }
    completions = [
        {
            "event": "handshake",
            "client_id": 1,
            "server_instance_id": "a0a1a2a3a4a5a6a7a8a9aaabacadaeaf",
            "capacity_bytes": 268435456,
            "configured_latency_ns": modeled_latency,
            "protocol_version": 1,
        },
        counters(0),
        {
            "event": "completion",
            "client_id": 1,
            "request_id": 2,
            "server_sequence": 1,
            "operation": "read",
            "dpa": 83886080,
            "length": 8,
            "payload_sha256":
                "804d562d22470fb7be7f06aa076621cb268932be33d8f9fb2844f958dbd74c17",
            "status": 0,
            "returned_modeled_latency_ns": modeled_latency,
            "requested_delay_ns": modeled_latency,
            "applied_delay_ns": read_delay,
            "delay_overshoot_ns": read_delay - modeled_latency,
            "delay_undershot": False,
            "path": "direct_cfmws",
            "phase_id": "phase:cfmws",
        },
        counters(1),
        {
            "event": "completion",
            "client_id": 1,
            "request_id": 3,
            "server_sequence": 2,
            "operation": "write",
            "dpa": 83886080,
            "length": 8,
            "payload_sha256":
                "1dce6604591efb439d5e87418a1d00dbfd014327d8c4dea862815714b76ae9a5",
            "status": 0,
            "returned_modeled_latency_ns": modeled_latency,
            "requested_delay_ns": modeled_latency,
            "applied_delay_ns": write_delay,
            "delay_overshoot_ns": write_delay - modeled_latency,
            "delay_undershot": False,
            "path": "direct_cfmws",
            "phase_id": "phase:cfmws",
        },
        counters(2),
    ]
    write_jsonl(run_dir / "qemu-events.jsonl", completions)


def make_failstop_run(root, backend, repetition, time_base):
    run_dir = root / f"{backend}-run-{repetition}"
    run_dir.mkdir()
    jit_backend = 1 if backend == "rust" else 3
    write_jsonl(
        run_dir / "jit-events.jsonl",
        [
            {
                "schema": "slugarch.qemu-jit-event.v1",
                "event_id": 1,
                "client_id": 1,
                "direction": 0,
                "event_class": 1,
                "opcode": 3,
                "address": 83886080,
                "tag": 2,
                "phase_id": PHASE_ID,
                "monotonic_ns": time_base,
                "status": 0,
                "payload_len": 0,
                "payload_prefix_hex": "",
                "payload_fnv1a64": "cbf29ce484222325",
                "policy_digest": POLICY_DIGEST,
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
            {
                "schema": "slugarch.qemu-cfmws-join.v1",
                "request_event_id": 1,
                "completion_event_id": 0,
                "request_id": 2,
                "server_sequence": 0,
                "external_commit": False,
                "effective_error": 14,
                "policy_digest": POLICY_DIGEST,
            },
        ],
    )
    write_jsonl(
        run_dir / "qemu-events.jsonl",
        [
            {
                "event": "handshake",
                "client_id": 1,
                "server_instance_id":
                    "a0a1a2a3a4a5a6a7a8a9aaabacadaeaf",
                "capacity_bytes": 268435456,
                "configured_latency_ns": 400,
                "protocol_version": 1,
            },
            {
                "event": "path_counters",
                "phase_id": "phase:reject",
                "direct_cfmws": 0,
                "bar4_overlay": 0,
                "local_shadow": 0,
                "local_cache": 0,
                "bulk_overlay": 0,
                "coherent_pool": 0,
            },
        ],
    )
    (run_dir / "failstop-outcome.json").write_text(
        json.dumps({
            "schema": "slugarch.qemu-cfmws-failstop.v1",
            "backend": backend,
            "configured_latency_ns": 400,
            "jit_status": 3,
            "jit_backend": jit_backend,
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
        }) + "\n",
        encoding="utf-8",
    )


class SummarizeJitCfmwsTest(unittest.TestCase):
    def test_replay_signature_ignores_latency_only_fields(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            signatures = []
            for latency in (80, 2_000):
                rust = root / f"latency-{latency}" / "rust"
                rust.mkdir(parents=True)
                make_run(
                    rust,
                    "rust",
                    1,
                    1_000,
                    latency + 11,
                    latency + 12,
                    modeled_latency=latency,
                )
                summary = summarize(
                    {"rust": rust},
                    expected_repeats=1,
                    expected_latency_ns=latency,
                )
                signatures.append(summary["semantic_signature_sha256"])

        self.assertEqual(len(set(signatures)), 1)

    def test_accepts_parameterized_modeled_latency(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            rust = root / "rust"
            rust.mkdir()
            make_run(rust, "rust", 1, 1_000, 91, 92, modeled_latency=80)
            make_run(rust, "rust", 2, 2_000, 93, 94, modeled_latency=80)

            summary = summarize(
                {"rust": rust},
                expected_repeats=2,
                expected_latency_ns=80,
            )

        self.assertEqual(summary["modeled_latency_ns"], 80)
        metrics = summary["backends"]["rust"]["metrics"]
        self.assertEqual(metrics["read_applied_delay_ns"]["values"], [91, 93])
        self.assertEqual(metrics["write_delay_overshoot_ns"]["max"], 14)

    def test_summarizes_two_valid_repeats(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            rust = root / "rust"
            rust.mkdir()
            make_run(rust, "rust", 1, 1_000, 410, 420)
            make_run(rust, "rust", 2, 2_000, 430, 440)

            summary = summarize({"rust": rust}, expected_repeats=2)

        self.assertEqual(summary["status"], "pass")
        self.assertEqual(summary["validation"]["fresh_processes"], 2)
        self.assertEqual(summary["validation"]["canonical_events"], 8)
        self.assertEqual(summary["validation"]["request_completion_joins"], 4)
        self.assertEqual(summary["validation"]["direct_cfmws_completions"], 4)
        self.assertEqual(summary["policy_digest"], POLICY_DIGEST)
        metrics = summary["backends"]["rust"]["metrics"]
        self.assertEqual(metrics["read_host_event_pair_span_ns"]["median"], 100)
        self.assertEqual(metrics["write_host_event_pair_span_ns"]["median"], 200)
        self.assertEqual(metrics["read_applied_delay_ns"]["values"], [410, 430])
        self.assertEqual(metrics["write_delay_overshoot_ns"]["max"], 40)

    def test_rejects_broken_replay_contracts(self):
        cases = [
            (
                "server sequence",
                "jit-events.jsonl",
                2,
                "server_sequence",
                7,
            ),
            (
                "bypass counter",
                "qemu-events.jsonl",
                5,
                "local_shadow",
                1,
            ),
            (
                "drop",
                "jit-events.jsonl",
                4,
                "drop_count",
                1,
            ),
            (
                "undershot",
                "qemu-events.jsonl",
                2,
                "delay_undershot",
                True,
            ),
            (
                "policy digest",
                "jit-events.jsonl",
                0,
                "policy_digest",
                "0" * 64,
            ),
        ]
        for error, filename, index, key, value in cases:
            with self.subTest(error=error), tempfile.TemporaryDirectory() as tmp:
                root = pathlib.Path(tmp)
                rust = root / "rust"
                rust.mkdir()
                make_run(rust, "rust", 1, 1_000, 410, 420)
                make_run(rust, "rust", 2, 2_000, 430, 440)
                mutate_jsonl(
                    rust / "rust-run-2" / filename,
                    index,
                    key,
                    value,
                )

                with self.assertRaisesRegex(ValueError, error):
                    summarize({"rust": rust}, expected_repeats=2)

    def test_requires_exact_repeat_directories(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            rust = root / "rust"
            rust.mkdir()
            make_run(rust, "rust", 1, 1_000, 410, 420)

            with self.assertRaisesRegex(ValueError, "run directories"):
                summarize({"rust": rust}, expected_repeats=2)

    def test_cli_writes_canonical_summary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            rust = root / "rust"
            rust.mkdir()
            make_run(rust, "rust", 1, 1_000, 410, 420)
            make_run(rust, "rust", 2, 2_000, 430, 440)
            output = root / "summary.json"

            result = summarizer.main([
                "--backend", f"rust={rust}",
                "--expected-repeats", "2",
                "--output", str(output),
            ])

            self.assertEqual(result, 0)
            summary = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(summary["status"], "pass")
            self.assertTrue(output.read_text(encoding="utf-8").endswith("\n"))


class SummarizeJitCfmwsSweepTest(unittest.TestCase):
    def test_sweep_manifest_records_runner_provenance(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            qemu_build, rust_library, fpga_library, policy = (
                make_fake_qemu_inputs(root)
            )
            qemu_source = make_fake_qemu_source(root)
            cxl_source = qemu_source / "tests" / "qtest" / "cxl-test.c"
            completed = SimpleNamespace(
                returncode=0,
                stdout=exact_tap(sweep_runner.TEST_PATH),
            )
            with (
                mock.patch.object(
                    sweep_runner.subprocess,
                    "run",
                    return_value=completed,
                ),
                mock.patch.object(
                    sweep_runner.summarize_jit_cfmws_sweep,
                    "summarize_sweep",
                    return_value={"status": "pass"},
                ),
            ):
                try:
                    sweep_runner.run_sweep(
                        qemu_build,
                        rust_library,
                        fpga_library,
                        policy,
                        root / "output",
                        qemu_source=qemu_source,
                    )
                except TypeError as error:
                    self.fail(f"qemu_source is not accepted: {error}")

            manifest = json.loads(
                (root / "output" / "run-manifest.json").read_text(
                    encoding="utf-8"
                )
            )
            provenance = manifest.get("provenance")
            self.assertIsNotNone(provenance)
            self.assertEqual(
                provenance["python_version"],
                platform.python_version(),
            )
            self.assertEqual(
                provenance["runner_sha256"],
                sha256(pathlib.Path(sweep_runner.__file__)),
            )
            self.assertEqual(
                provenance["summarizer_sha256"],
                sha256(pathlib.Path(sweep_summarizer.__file__)),
            )
            self.assertEqual(
                provenance["qemu_cxl_test_source_sha256"],
                sha256(cxl_source),
            )

    def test_sweep_runner_rejects_tap_skip(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            qemu_build, rust_library, fpga_library, policy = (
                make_fake_qemu_inputs(root)
            )
            qemu_source = make_fake_qemu_source(root)
            with self.assertRaisesRegex(Exception, "SKIP"):
                sweep_runner.run_sweep(
                    qemu_build,
                    rust_library,
                    fpga_library,
                    policy,
                    root / "output",
                    qemu_source=qemu_source,
                )

    def test_sweep_runner_requires_qemu_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            qemu_build, rust_library, fpga_library, policy = (
                make_fake_qemu_inputs(root)
            )
            with self.assertRaisesRegex(ValueError, "qemu_source is required"):
                sweep_runner.run_sweep(
                    qemu_build,
                    rust_library,
                    fpga_library,
                    policy,
                    root / "output",
                    qemu_source=None,
                )

    def test_requires_exact_four_latency_two_backend_five_repeat_contract(self):
        raw = SWEEP_ARTIFACT / "raw"
        invalid_contracts = (
            {
                "latencies_ns": (80, 400),
                "expected_repeats": 5,
                "backends": ("rust", "fpga-verilator"),
            },
            {
                "latencies_ns": (80, 400, 2_000, 10_000),
                "expected_repeats": 5,
                "backends": ("rust",),
            },
            {
                "latencies_ns": (80, 400, 2_000, 10_000),
                "expected_repeats": 4,
                "backends": ("rust", "fpga-verilator"),
            },
        )
        for contract in invalid_contracts:
            with self.subTest(contract=contract):
                with self.assertRaisesRegex(ValueError, "exact campaign"):
                    sweep_summarizer.summarize_sweep(raw, **contract)

    def test_summary_source_roots_are_relocation_stable(self):
        summary = sweep_summarizer.summarize_sweep(
            SWEEP_ARTIFACT / "raw"
        )

        self.assertEqual(summary["source_root"], "raw")
        self.assertEqual(
            summary["cells"]["80"]["backends"]["rust"]["source_root"],
            "rust",
        )
        self.assertEqual(
            summary["cells"]["10000"]["backends"]["fpga-verilator"][
                "source_root"
            ],
            "fpga-verilator",
        )

    def test_sweep_summary_states_fake_cxlmemsim_scope(self):
        summary = sweep_summarizer.summarize_sweep(
            SWEEP_ARTIFACT / "raw"
        )

        self.assertEqual(
            summary["measurement_scope"]["service"],
            "in-process fake CXLMemSim service; "
            "not a production external service",
        )

    def test_runner_builds_one_filtered_fresh_process_invocation(self):
        invocation = sweep_runner.build_invocation(
            qemu_build=pathlib.Path("/build/qemu"),
            evidence_root=pathlib.Path("/artifact/latency-2000/rust"),
            library=pathlib.Path("/lib/slugarch-rust.so"),
            policy=pathlib.Path("/src/policy.json"),
            backend="rust",
            latency_ns=2_000,
            repetition=4,
        )

        self.assertEqual(invocation["cwd"], pathlib.Path("/build/qemu"))
        self.assertEqual(
            invocation["command"],
            (
                "/build/qemu/tests/qtest/cxl-test",
                "-p",
                "/x86_64/pci/cxl/type2_jext_cfmws_records",
            ),
        )
        self.assertEqual(invocation["environment"]["MESON_TEST_ITERATION"], "4")
        self.assertEqual(
            invocation["environment"]["SLUGARCH_QTEST_CFMWS_LATENCY_NS"],
            "2000",
        )
        self.assertEqual(
            invocation["environment"]["SLUGARCH_QTEST_CFMWS_JIT_MODE"],
            "rust",
        )
        reject_invocation = sweep_runner.build_invocation(
            qemu_build=pathlib.Path("/build/qemu"),
            evidence_root=pathlib.Path("/artifact/reject/rust"),
            library=pathlib.Path("/lib/slugarch-rust.so"),
            policy=pathlib.Path("/src/reject.json"),
            backend="rust",
            latency_ns=400,
            repetition=1,
            test_path="/x86_64/pci/cxl/type2_jext_cfmws_external_reject",
        )
        self.assertEqual(
            reject_invocation["command"][-1],
            "/x86_64/pci/cxl/type2_jext_cfmws_external_reject",
        )

    def test_aggregates_latency_cells_and_preserves_all_points(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            for latency in (80, 400, 2_000, 10_000):
                for backend, offset in (("rust", 10), ("fpga-verilator", 20)):
                    backend_root = root / f"latency-{latency}" / backend
                    backend_root.mkdir(parents=True)
                    for repetition in range(1, 6):
                        make_run(
                            backend_root,
                            backend,
                            repetition,
                            latency * 10 + repetition * 1_000,
                            latency + offset + repetition,
                            latency + offset + repetition + 3,
                            modeled_latency=latency,
                        )

            summary = sweep_summarizer.summarize_sweep(
                root,
            )

        self.assertEqual(summary["status"], "pass")
        self.assertEqual(summary["validation"]["fresh_processes"], 40)
        self.assertEqual(summary["validation"]["canonical_events"], 160)
        self.assertEqual(summary["validation"]["request_completion_joins"], 80)
        self.assertEqual(summary["validation"]["direct_cfmws_completions"], 80)
        self.assertEqual(summary["validation"]["semantic_signatures"], 1)
        self.assertTrue(
            summary["validation"]["monotonic_applied_delay_medians"]
        )
        points = summary["series"]["rust"]["read_applied_delay_ns"]
        self.assertEqual(points[0]["values"], [91, 92, 93, 94, 95])
        self.assertEqual(points[3]["values"], [
            10_011, 10_012, 10_013, 10_014, 10_015,
        ])

    def test_rejects_non_monotonic_applied_delay_medians(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            for latency in (80, 400, 2_000, 10_000):
                for backend in ("rust", "fpga-verilator"):
                    backend_root = root / f"latency-{latency}" / backend
                    backend_root.mkdir(parents=True)
                    for repetition in range(1, 6):
                        delay = latency + 10 + repetition
                        if backend == "rust" and latency == 80:
                            delay = 3_000 + repetition
                        make_run(
                            backend_root,
                            backend,
                            repetition,
                            latency * 10 + repetition * 1_000,
                            delay,
                            delay,
                            modeled_latency=latency,
                        )

            with self.assertRaisesRegex(ValueError, "non-monotonic"):
                sweep_summarizer.summarize_sweep(root)


class SummarizeJitCfmwsFailstopTest(unittest.TestCase):
    def test_failstop_manifest_records_runner_provenance(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            qemu_build, rust_library, fpga_library, policy = (
                make_fake_qemu_inputs(root)
            )
            qemu_source = make_fake_qemu_source(root)
            cxl_source = qemu_source / "tests" / "qtest" / "cxl-test.c"
            completed = SimpleNamespace(
                returncode=0,
                stdout=exact_tap(failstop_runner.TEST_PATH),
            )
            with (
                mock.patch.object(
                    failstop_runner.subprocess,
                    "run",
                    return_value=completed,
                ),
                mock.patch.object(
                    failstop_runner.summarize_jit_cfmws_failstop,
                    "summarize_failstop",
                    return_value={"status": "pass"},
                ),
            ):
                try:
                    failstop_runner.run_failstop(
                        qemu_build,
                        rust_library,
                        fpga_library,
                        policy,
                        root / "output",
                        qemu_source=qemu_source,
                    )
                except TypeError as error:
                    self.fail(f"qemu_source is not accepted: {error}")

            manifest = json.loads(
                (root / "output" / "run-manifest.json").read_text(
                    encoding="utf-8"
                )
            )
            provenance = manifest.get("provenance")
            self.assertIsNotNone(provenance)
            self.assertEqual(
                provenance["python_version"],
                platform.python_version(),
            )
            self.assertEqual(
                provenance["runner_sha256"],
                sha256(pathlib.Path(failstop_runner.__file__)),
            )
            self.assertEqual(
                provenance["summarizer_sha256"],
                sha256(pathlib.Path(failstop_summarizer.__file__)),
            )
            self.assertEqual(
                provenance["qemu_cxl_test_source_sha256"],
                sha256(cxl_source),
            )

    def test_fault_matrix_manifest_records_runner_provenance(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            qemu_build = root / "build"
            qemu_source = root / "source"
            (qemu_build / "tests" / "qtest").mkdir(parents=True)
            (qemu_source / "tests" / "qtest").mkdir(parents=True)
            (qemu_build / "tests" / "qtest" / "cxl-test").write_bytes(
                b"cxl-test"
            )
            (qemu_build / "qemu-system-x86_64").write_bytes(b"qemu")
            cxl_source = qemu_source / "tests" / "qtest" / "cxl-test.c"
            cxl_source.write_text("/* fixture */\n", encoding="utf-8")

            def complete(command, **_kwargs):
                return SimpleNamespace(
                    returncode=0,
                    stdout=exact_tap(command[-1]),
                )

            with mock.patch.object(
                fault_runner.subprocess,
                "run",
                side_effect=complete,
            ):
                fault_runner.run_fault_matrix(
                    qemu_build,
                    qemu_source,
                    root / "output",
                )

            manifest = json.loads(
                (root / "output" / "run-manifest.json").read_text(
                    encoding="utf-8"
                )
            )
            provenance = manifest.get("provenance")
            self.assertIsNotNone(provenance)
            self.assertEqual(
                provenance["python_version"],
                platform.python_version(),
            )
            self.assertEqual(
                provenance["runner_sha256"],
                sha256(pathlib.Path(fault_runner.__file__)),
            )
            self.assertEqual(
                provenance["summarizer_sha256"],
                sha256(pathlib.Path(fault_runner.__file__)),
            )
            self.assertEqual(
                provenance["qemu_cxl_test_source_sha256"],
                sha256(cxl_source),
            )

    def test_failstop_runner_rejects_tap_skip(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            qemu_build, rust_library, fpga_library, policy = (
                make_fake_qemu_inputs(root)
            )
            qemu_source = make_fake_qemu_source(root)
            with self.assertRaisesRegex(Exception, "SKIP"):
                failstop_runner.run_failstop(
                    qemu_build,
                    rust_library,
                    fpga_library,
                    policy,
                    root / "output",
                    qemu_source=qemu_source,
                )

    def test_failstop_runner_requires_qemu_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            qemu_build, rust_library, fpga_library, policy = (
                make_fake_qemu_inputs(root)
            )
            with self.assertRaisesRegex(ValueError, "qemu_source is required"):
                failstop_runner.run_failstop(
                    qemu_build,
                    rust_library,
                    fpga_library,
                    policy,
                    root / "output",
                    qemu_source=None,
                )

    def test_requires_exact_two_backend_five_repeat_contract(self):
        raw = FAILSTOP_ARTIFACT / "raw"
        with self.assertRaisesRegex(ValueError, "exact campaign"):
            failstop_summarizer.summarize_failstop(
                {"rust": raw / "rust"},
                expected_repeats=5,
            )

        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            for backend in ("rust", "fpga-verilator"):
                backend_root = root / backend
                backend_root.mkdir()
                source = raw / backend / f"{backend}-run-1"
                (backend_root / f"{backend}-run-1").symlink_to(
                    source,
                    target_is_directory=True,
                )
            with self.assertRaisesRegex(ValueError, "exact campaign"):
                failstop_summarizer.summarize_failstop(
                    {
                        "rust": root / "rust",
                        "fpga-verilator": root / "fpga-verilator",
                    },
                    expected_repeats=1,
                )

    def test_failstop_summary_states_qtest_simulation_scope(self):
        raw = FAILSTOP_ARTIFACT / "raw"
        summary = failstop_summarizer.summarize_failstop({
            "rust": raw / "rust",
            "fpga-verilator": raw / "fpga-verilator",
        })

        self.assertIn("QEMU qtest", summary["evidence_scope"])
        self.assertIn("FPGA-Verilator", summary["evidence_scope"])
        self.assertIn("no physical FPGA", summary["evidence_scope"])
        self.assertEqual(
            summary["backends"]["rust"]["source_root"],
            "rust",
        )

    def test_fault_matrix_rejects_tap_skip_as_failure(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            qemu_build = root / "build"
            qemu_source = root / "source"
            (qemu_build / "tests" / "qtest").mkdir(parents=True)
            (qemu_source / "tests" / "qtest").mkdir(parents=True)
            cxl_test = qemu_build / "tests" / "qtest" / "cxl-test"
            cxl_test.write_text(
                "#!/bin/sh\n"
                "printf 'TAP version 14\\nok 1 %s # SKIP unavailable\\n"
                "1..1\\n' \"$2\"\n",
                encoding="utf-8",
            )
            cxl_test.chmod(0o755)
            (qemu_build / "qemu-system-x86_64").write_bytes(b"qemu")
            (qemu_source / "tests" / "qtest" / "cxl-test.c").write_text(
                "/* fixture */\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(RuntimeError, "SKIP"):
                fault_runner.run_fault_matrix(
                    qemu_build,
                    qemu_source,
                    root / "output",
                    repeats=5,
                )

    def test_fault_matrix_requires_exact_five_repeats(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            qemu_build = root / "build"
            qemu_source = root / "source"
            (qemu_build / "tests" / "qtest").mkdir(parents=True)
            (qemu_source / "tests" / "qtest").mkdir(parents=True)
            cxl_test = qemu_build / "tests" / "qtest" / "cxl-test"
            cxl_test.write_text(
                "#!/bin/sh\n"
                "printf 'TAP version 14\\nok 1 %s\\n1..1\\n' \"$2\"\n",
                encoding="utf-8",
            )
            cxl_test.chmod(0o755)
            (qemu_build / "qemu-system-x86_64").write_bytes(b"qemu")
            (qemu_source / "tests" / "qtest" / "cxl-test.c").write_text(
                "/* fixture */\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "exact campaign"):
                fault_runner.run_fault_matrix(
                    qemu_build,
                    qemu_source,
                    root / "output",
                    repeats=1,
                )

    def test_fault_matrix_has_unique_table_worthy_contracts(self):
        names = [case["name"] for case in fault_runner.FAULT_CASES]
        paths = [case["test_path"] for case in fault_runner.FAULT_CASES]

        self.assertEqual(len(fault_runner.FAULT_CASES), 10)
        self.assertEqual(len(set(names)), 10)
        self.assertEqual(len(set(paths)), 10)
        self.assertTrue(
            all(case["asserted_invariant"] for case in fault_runner.FAULT_CASES)
        )

    def test_validates_real_backend_pre_commit_rejection(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            for backend in ("rust", "fpga-verilator"):
                backend_root = root / backend
                backend_root.mkdir()
                for repetition in range(1, 6):
                    make_failstop_run(
                        backend_root,
                        backend,
                        repetition,
                        repetition * 1_000,
                    )

            summary = failstop_summarizer.summarize_failstop(
                {
                    "rust": root / "rust",
                    "fpga-verilator": root / "fpga-verilator",
                },
            )

        self.assertEqual(summary["status"], "pass")
        self.assertEqual(summary["validation"]["fresh_processes"], 10)
        self.assertEqual(summary["validation"]["rejected_requests"], 10)
        self.assertEqual(summary["validation"]["external_commits"], 0)
        self.assertEqual(summary["validation"]["server_memory_requests"], 0)
        self.assertEqual(summary["validation"]["server_read_requests"], 0)
        self.assertEqual(summary["validation"]["server_write_requests"], 0)
        self.assertEqual(summary["validation"]["server_sequence"], 0)
        self.assertEqual(
            summary["validation"]["direct_cfmws_completions"], 0
        )
        self.assertEqual(summary["validation"]["semantic_signatures"], 1)

    def test_rejects_any_external_side_effect(self):
        for field in (
                "server_memory_requests",
                "server_read_requests",
                "server_write_requests",
                "server_sequence"):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = pathlib.Path(tmp)
                for backend in ("rust", "fpga-verilator"):
                    backend_root = root / backend
                    backend_root.mkdir()
                    for repetition in range(1, 6):
                        make_failstop_run(
                            backend_root,
                            backend,
                            repetition,
                            repetition * 1_000,
                        )
                rust = root / "rust"
                outcome = rust / "rust-run-1" / "failstop-outcome.json"
                data = json.loads(outcome.read_text(encoding="utf-8"))
                data[field] = 1
                outcome.write_text(
                    json.dumps(data) + "\n",
                    encoding="utf-8",
                )

                with self.assertRaisesRegex(ValueError, field):
                    failstop_summarizer.summarize_failstop(
                        {
                            "rust": rust,
                            "fpga-verilator":
                                root / "fpga-verilator",
                        },
                    )


if __name__ == "__main__":
    unittest.main()
