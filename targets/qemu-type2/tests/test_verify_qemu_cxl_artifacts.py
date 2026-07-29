#!/usr/bin/env python3

import hashlib
import json
import pathlib
import platform
import shutil
import subprocess
import sys
import tempfile
import unittest


SCRIPT_DIR = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = SCRIPT_DIR.parents[1]
VERIFY_SCRIPT = SCRIPT_DIR / "verify_qemu_cxl_artifacts.py"
ARTIFACT_ROOT = REPO_ROOT / "artifact" / "slugarch_cxlmemsim"
ARTIFACT_NAMES = (
    "qemu-jit-cfmws-latency-sweep-20260729",
    "qemu-jit-cfmws-failstop-20260729",
    "qemu-cxl-fault-matrix-20260729",
)
FAILSTOP_SCOPE = (
    "QEMU qtest with an in-process fake CXLMemSim service, using "
    "the Rust software and FPGA-Verilator RTL-simulation backends; "
    "no physical FPGA or production external-service evidence"
)
FAULT_SCOPE = (
    "QEMU qtest behavioral assertions; not physical CXL or FPGA timing"
)


def copy_artifacts(destination):
    copied = {}
    for name in ARTIFACT_NAMES:
        target = destination / name
        shutil.copytree(ARTIFACT_ROOT / name, target)
        copied[name] = target
    canonicalize_metadata(copied)
    return copied


def run_verifier(copied):
    return subprocess.run(
        (
            sys.executable,
            str(VERIFY_SCRIPT),
            "--sweep",
            str(copied[ARTIFACT_NAMES[0]]),
            "--failstop",
            str(copied[ARTIFACT_NAMES[1]]),
            "--fault-matrix",
            str(copied[ARTIFACT_NAMES[2]]),
        ),
        cwd=REPO_ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )


def write_json(path, value):
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def provenance(runner, summarizer, qemu_source_digest=None):
    result = {
        "python_version": platform.python_version(),
        "runner_sha256": sha256(runner),
        "summarizer_sha256": sha256(summarizer),
    }
    if qemu_source_digest is not None:
        result["qemu_cxl_test_source_sha256"] = qemu_source_digest
    return result


def canonicalize_metadata(copied):
    sweep = copied[ARTIFACT_NAMES[0]]
    sweep_summary_path = sweep / "summary.json"
    sweep_summary = json.loads(
        sweep_summary_path.read_text(encoding="utf-8")
    )
    sweep_summary["source_root"] = "raw"
    for cell in sweep_summary["cells"].values():
        for backend, value in cell["backends"].items():
            value["source_root"] = backend
    write_json(sweep_summary_path, sweep_summary)
    sweep_manifest_path = sweep / "run-manifest.json"
    sweep_manifest = json.loads(
        sweep_manifest_path.read_text(encoding="utf-8")
    )
    sweep_manifest["provenance"] = provenance(
        SCRIPT_DIR / "run_jit_cfmws_sweep.py",
        SCRIPT_DIR / "summarize_jit_cfmws_sweep.py",
        sweep_manifest["inputs"].get("cxl_test_source_sha256"),
    )
    sweep_manifest["summary_sha256"] = sha256(sweep_summary_path)
    write_json(sweep_manifest_path, sweep_manifest)

    failstop = copied[ARTIFACT_NAMES[1]]
    failstop_summary_path = failstop / "summary.json"
    failstop_summary = json.loads(
        failstop_summary_path.read_text(encoding="utf-8")
    )
    failstop_summary["evidence_scope"] = FAILSTOP_SCOPE
    for backend, value in failstop_summary["backends"].items():
        value["source_root"] = backend
    write_json(failstop_summary_path, failstop_summary)
    failstop_manifest_path = failstop / "run-manifest.json"
    failstop_manifest = json.loads(
        failstop_manifest_path.read_text(encoding="utf-8")
    )
    failstop_manifest["provenance"] = provenance(
        SCRIPT_DIR / "run_jit_cfmws_failstop.py",
        SCRIPT_DIR / "summarize_jit_cfmws_failstop.py",
        failstop_manifest["inputs"].get("cxl_test_source_sha256"),
    )
    failstop_manifest["summary_sha256"] = sha256(failstop_summary_path)
    write_json(failstop_manifest_path, failstop_manifest)

    fault = copied[ARTIFACT_NAMES[2]]
    fault_summary_path = fault / "summary.json"
    fault_summary = json.loads(
        fault_summary_path.read_text(encoding="utf-8")
    )
    fault_summary["evidence_scope"] = FAULT_SCOPE
    write_json(fault_summary_path, fault_summary)
    fault_manifest_path = fault / "run-manifest.json"
    fault_manifest = json.loads(
        fault_manifest_path.read_text(encoding="utf-8")
    )
    fault_manifest["provenance"] = provenance(
        SCRIPT_DIR / "run_qemu_cxl_fault_matrix.py",
        SCRIPT_DIR / "run_qemu_cxl_fault_matrix.py",
        fault_manifest["inputs"].get("cxl_test_source_sha256"),
    )
    fault_manifest["summary_sha256"] = sha256(fault_summary_path)
    write_json(fault_manifest_path, fault_manifest)


class VerifyQemuCxlArtifactsTest(unittest.TestCase):
    def test_verifies_relocated_complete_artifacts(self):
        with tempfile.TemporaryDirectory() as tmp:
            copied = copy_artifacts(pathlib.Path(tmp))
            result = run_verifier(copied)

        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["status"], "pass")
        self.assertEqual(report["validation"]["artifacts"], 3)
        self.assertEqual(report["validation"]["sweep_processes"], 40)
        self.assertEqual(report["validation"]["failstop_processes"], 10)
        self.assertEqual(report["validation"]["fault_matrix_processes"], 50)
        self.assertTrue(
            report["validation"]["artifact_file_hashes_verified"]
        )
        self.assertEqual(
            report["validation"]["declared_input_hashes_validated"],
            "required, SHA-256-formatted, and cross-campaign-consistent",
        )
        self.assertNotIn("hashes_verified", report["validation"])

    def test_rejects_tap_hash_corruption(self):
        with tempfile.TemporaryDirectory() as tmp:
            copied = copy_artifacts(pathlib.Path(tmp))
            tap = (
                copied[ARTIFACT_NAMES[0]]
                / "raw/latency-80/rust/rust-run-1.tap"
            )
            tap.write_text(
                tap.read_text(encoding="utf-8") + "tampered\n",
                encoding="utf-8",
            )
            result = run_verifier(copied)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SHA-256 mismatch", result.stderr)

    def test_rejects_skip_even_when_tap_hash_is_updated(self):
        with tempfile.TemporaryDirectory() as tmp:
            copied = copy_artifacts(pathlib.Path(tmp))
            sweep = copied[ARTIFACT_NAMES[0]]
            tap = sweep / "raw/latency-80/rust/rust-run-1.tap"
            tap.write_text(
                tap.read_text(encoding="utf-8").replace(
                    "ok 1 /x86_64/pci/cxl/type2_jext_cfmws_records",
                    "ok 1 /x86_64/pci/cxl/type2_jext_cfmws_records "
                    "# SKIP unavailable",
                ),
                encoding="utf-8",
            )
            manifest_path = sweep / "run-manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["runs"][0]["tap_sha256"] = sha256(tap)
            write_json(manifest_path, manifest)
            result = run_verifier(copied)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SKIP", result.stderr)

    def test_rejects_corrupted_failstop_outcome(self):
        with tempfile.TemporaryDirectory() as tmp:
            copied = copy_artifacts(pathlib.Path(tmp))
            outcome = (
                copied[ARTIFACT_NAMES[1]]
                / "raw/rust/rust-run-1/failstop-outcome.json"
            )
            value = json.loads(outcome.read_text(encoding="utf-8"))
            value["external_commit"] = True
            outcome.write_text(
                json.dumps(value, separators=(",", ":")) + "\n",
                encoding="utf-8",
            )
            result = run_verifier(copied)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("external_commit", result.stderr)

    def test_rejects_resigned_failstop_server_side_effect(self):
        with tempfile.TemporaryDirectory() as tmp:
            copied = copy_artifacts(pathlib.Path(tmp))
            failstop = copied[ARTIFACT_NAMES[1]]
            outcome = (
                failstop
                / "raw/rust/rust-run-1/failstop-outcome.json"
            )
            value = json.loads(outcome.read_text(encoding="utf-8"))
            value["server_read_requests"] = 1
            write_json(outcome, value)

            summary_path = failstop / "summary.json"
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["backends"]["rust"]["runs"][0][
                "outcome_sha256"
            ] = sha256(outcome)
            write_json(summary_path, summary)

            manifest_path = failstop / "run-manifest.json"
            manifest = json.loads(
                manifest_path.read_text(encoding="utf-8")
            )
            manifest["summary_sha256"] = sha256(summary_path)
            write_json(manifest_path, manifest)
            result = run_verifier(copied)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("server_read_requests", result.stderr)

    def test_rejects_resigned_nondeterministic_summary(self):
        with tempfile.TemporaryDirectory() as tmp:
            copied = copy_artifacts(pathlib.Path(tmp))
            sweep = copied[ARTIFACT_NAMES[0]]
            summary_path = sweep / "summary.json"
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["validation"]["canonical_events"] = 159
            write_json(summary_path, summary)
            manifest_path = sweep / "run-manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["summary_sha256"] = sha256(summary_path)
            write_json(manifest_path, manifest)
            result = run_verifier(copied)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("deterministic resummarization mismatch", result.stderr)

    def test_rejects_resigned_absolute_source_root(self):
        with tempfile.TemporaryDirectory() as tmp:
            copied = copy_artifacts(pathlib.Path(tmp))
            sweep = copied[ARTIFACT_NAMES[0]]
            summary_path = sweep / "summary.json"
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["source_root"] = "/tmp/forged/raw"
            write_json(summary_path, summary)
            manifest_path = sweep / "run-manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["summary_sha256"] = sha256(summary_path)
            write_json(manifest_path, manifest)
            result = run_verifier(copied)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("source_root", result.stderr)

    def test_rejects_resigned_wrong_evidence_scope(self):
        with tempfile.TemporaryDirectory() as tmp:
            copied = copy_artifacts(pathlib.Path(tmp))
            failstop = copied[ARTIFACT_NAMES[1]]
            summary_path = failstop / "summary.json"
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["evidence_scope"] = "physical FPGA proof"
            write_json(summary_path, summary)
            manifest_path = failstop / "run-manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["summary_sha256"] = sha256(summary_path)
            write_json(manifest_path, manifest)
            result = run_verifier(copied)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("evidence_scope", result.stderr)

    def test_rejects_wrong_runner_provenance(self):
        with tempfile.TemporaryDirectory() as tmp:
            copied = copy_artifacts(pathlib.Path(tmp))
            sweep = copied[ARTIFACT_NAMES[0]]
            manifest_path = sweep / "run-manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["provenance"]["runner_sha256"] = "0" * 64
            write_json(manifest_path, manifest)
            result = run_verifier(copied)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("runner provenance", result.stderr)

    def test_rejects_missing_required_input_hash(self):
        with tempfile.TemporaryDirectory() as tmp:
            copied = copy_artifacts(pathlib.Path(tmp))
            sweep = copied[ARTIFACT_NAMES[0]]
            manifest_path = sweep / "run-manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            del manifest["inputs"]["qemu_system_x86_64_sha256"]
            write_json(manifest_path, manifest)
            result = run_verifier(copied)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("input keys", result.stderr)
        self.assertIn("qemu_system_x86_64_sha256", result.stderr)

    def test_rejects_conflicting_cross_campaign_input_hash(self):
        with tempfile.TemporaryDirectory() as tmp:
            copied = copy_artifacts(pathlib.Path(tmp))
            sweep = copied[ARTIFACT_NAMES[0]]
            manifest_path = sweep / "run-manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["inputs"]["rust_library_sha256"] = "0" * 64
            write_json(manifest_path, manifest)
            result = run_verifier(copied)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("cross-campaign input mismatch", result.stderr)
        self.assertIn("rust_library_sha256", result.stderr)


if __name__ == "__main__":
    unittest.main()
