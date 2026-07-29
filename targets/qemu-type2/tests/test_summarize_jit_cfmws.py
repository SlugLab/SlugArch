#!/usr/bin/env python3

import json
import pathlib
import sys
import tempfile
import unittest


SCRIPT_DIR = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))

import summarize_jit_cfmws as summarizer  # noqa: E402


summarize = summarizer.summarize


POLICY_DIGEST = "bc91e1b53305764adbaf714367cf2bf91206fbefade323b3b507307970ff81d4"
PHASE_ID = 2215048676827443796


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


def make_run(root, backend, repetition, time_base, read_delay, write_delay):
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
            "configured_latency_ns": 400,
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
            "returned_modeled_latency_ns": 400,
            "requested_delay_ns": 400,
            "applied_delay_ns": read_delay,
            "delay_overshoot_ns": read_delay - 400,
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
            "returned_modeled_latency_ns": 400,
            "requested_delay_ns": 400,
            "applied_delay_ns": write_delay,
            "delay_overshoot_ns": write_delay - 400,
            "delay_undershot": False,
            "path": "direct_cfmws",
            "phase_id": "phase:cfmws",
        },
        counters(2),
    ]
    write_jsonl(run_dir / "qemu-events.jsonl", completions)


class SummarizeJitCfmwsTest(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
