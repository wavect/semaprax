#!/usr/bin/env python3
"""Offline controls for RI-13 retained combined-measure artifacts."""

import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path


MEASURE = Path(__file__).with_name("measure.py")
SPEC = importlib.util.spec_from_file_location("ri13_combined_measure", MEASURE)
measure = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(measure)


class RawArtifactReceiptTests(unittest.TestCase):
    def make_receipt(self, root):
        artifacts = measure.RawArtifacts(root / "raw")
        commands = {}
        for stage in measure.BUILD_STAGE_NAMES:
            commands[f"cold-{stage}"] = artifacts.capture(f"cold-{stage}", stage.encode(), b"")
        for label in (
            "m1-batch-throughput",
            "m2-batch-throughput",
            "m3-route-measurement",
            "m3-batch-throughput",
        ):
            commands[label] = artifacts.capture(label, label.encode(), b"")
        def command_result(pair):
            return {
                "raw_artifacts": pair,
                "stdout_sha256": pair["stdout"]["sha256"].removeprefix("sha256:"),
                "stderr_sha256": pair["stderr"]["sha256"].removeprefix("sha256:"),
            }

        receipt = {
            "raw_artifacts": artifacts.manifest(),
            "full_build_and_consumer_stages": [
                {"stage": stage, **command_result(commands[f"cold-{stage}"])}
                for stage in measure.BUILD_STAGE_NAMES
            ],
            "warm_build_and_consumer_stages": None,
            "m1_batch_throughput_measurement_command": command_result(commands["m1-batch-throughput"]),
            "m2_batch_throughput_measurement_command": command_result(commands["m2-batch-throughput"]),
            "route_measurement_command": command_result(commands["m3-route-measurement"]),
            "batch_throughput_measurement_command": command_result(commands["m3-batch-throughput"]),
        }
        path = root / "receipt.json"
        path.write_text(json.dumps(receipt), encoding="utf-8")
        return path, receipt

    def test_verifier_accepts_complete_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            path, receipt = self.make_receipt(Path(directory))
            self.assertEqual(measure.verify_receipt_raw_artifacts(path), receipt)

    def test_verifier_accepts_relocated_raw_artifact_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path, receipt = self.make_receipt(root)
            relocated = root / "copied-raw-artifacts"
            shutil.copytree(receipt["raw_artifacts"]["directory"], relocated)
            self.assertEqual(
                measure.verify_receipt_raw_artifacts(path, relocated), receipt
            )

    def test_verifier_rejects_missing_command_or_forged_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path, receipt = self.make_receipt(root)
            receipt["full_build_and_consumer_stages"].pop()
            path.write_text(json.dumps(receipt), encoding="utf-8")
            with self.assertRaises(ValueError):
                measure.verify_receipt_raw_artifacts(path)

            path, receipt = self.make_receipt(root / "second")
            entry = receipt["raw_artifacts"]["files"][0]
            (Path(receipt["raw_artifacts"]["directory"]) / entry["path"]).write_bytes(b"forged")
            with self.assertRaises(ValueError):
                measure.verify_receipt_raw_artifacts(path)


if __name__ == "__main__":
    unittest.main()
