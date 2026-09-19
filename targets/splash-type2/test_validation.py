"""Negative tests for accepting corrupted/misattributed experiment evidence."""
import copy
import json
import tempfile
import unittest
from pathlib import Path

from campaign import Recorder, payload
from validate import mutation_checks, validate_records


class TraceValidationTests(unittest.TestCase):
    def fixture(self, root, mode="full"):
        result = dict(run_id="test-epoch", config=dict(devices=2, payload_bytes=64, mode=mode))
        options = dict(total=2, per_device=1, relay=1)
        recorder = Recorder(root / "trace.jsonl", mode, "test-epoch", "host_relay", 2)
        data = payload("host_relay", 0, 0, 64)
        for tile in range(2):
            for kind in ("request", "completion"):
                recorder.record(tile, tile + 1, kind, data, tile)
        recorder.close()
        rows = [json.loads(line) for line in (root / "trace.jsonl").read_text().splitlines()]
        return rows, result, options

    def test_all_declared_corruptions_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            for mode in ("validation", "full"):
                records, result, options = self.fixture(Path(tmp), mode)
                validate_records(records, result, "host_relay", options)
                checks = mutation_checks(records, result, "host_relay", options)
                self.assertEqual(len(checks), 11 if mode == "full" else 10)

    def test_consistently_wrong_request_and_completion_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            records, result, options = self.fixture(Path(tmp))
            for row in records:
                row["payload_sha256"] = "a" * 64
            with self.assertRaisesRegex(ValueError, "tile=0 event=1"):
                validate_records(records, result, "host_relay", options)

    def test_cross_tile_dependency_required(self):
        with tempfile.TemporaryDirectory() as tmp:
            records, result, options = self.fixture(Path(tmp))
            records[2]["dependency"] = 0
            with self.assertRaisesRegex(ValueError, "tile=1 event=1"):
                validate_records(records, result, "host_relay", options)


if __name__ == "__main__":
    unittest.main()
