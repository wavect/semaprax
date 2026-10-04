#!/usr/bin/env python3
"""Structural tests for the bounded RI-13 combined throughput record."""

import importlib.util
import json
import pathlib
import tempfile
import unittest


MODULE = pathlib.Path(__file__).with_name("investigate-throughput.py")
SPEC = importlib.util.spec_from_file_location("ri13_combined_throughput", MODULE)
THROUGHPUT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(THROUGHPUT)


def receipt():
    routes = {}
    for name, rate in (("direct_rust", 4_700.0), ("handwritten_adapter", 4_900.0), ("generated_semaprax", 245.0)):
        routes[name] = {
            "operations_per_sample": 64,
            "body_bytes_per_operation": 2,
            "normalized_operations_per_second": rate,
        }
    return {
        "schema": THROUGHPUT.COMBINED_SCHEMA,
        "checkout": "reviewed",
        "full_build_and_consumer_stages": [{"stage": stage} for stage in THROUGHPUT.STAGES],
        "batch_throughput_measurement_command": {"command": ["cargo", "run", "--", "batch"]},
        "batch_throughput": {"routes": routes},
    }


def m3(path):
    return {
        "schema": THROUGHPUT.M3_SCHEMA,
        "status": "investigation_required",
        "receipt": {"sha256": THROUGHPUT.digest(path)},
        "measured": {
            "generated_operations_per_second": 245.0,
            "handwritten_operations_per_second": 4_900.0,
            "direct_operations_per_second": 4_700.0,
        },
    }


class CombinedThroughputTests(unittest.TestCase):
    def test_binds_m3_investigation_and_marks_unmatched_profiles_unavailable(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            report = root / "receipt.json"
            investigation = root / "m3.json"
            report.write_text(json.dumps(receipt()))
            investigation.write_text(json.dumps(m3(report)))
            result = THROUGHPUT.investigate(report, investigation)
        self.assertEqual(result["profiles"]["m1"]["status"], "unavailable")
        self.assertEqual(result["profiles"]["m2"]["status"], "unavailable")
        self.assertEqual(result["profiles"]["m3"]["status"], "investigation_required")
        self.assertEqual(result["profiles"]["m3"]["generated_to_direct_throughput_ratio"], 0.0521)

    def test_refuses_wrong_stage_set_and_unbound_m3_investigation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            report = root / "receipt.json"
            investigation = root / "m3.json"
            invalid = receipt()
            invalid["full_build_and_consumer_stages"] = invalid["full_build_and_consumer_stages"][:-1]
            report.write_text(json.dumps(invalid))
            investigation.write_text(json.dumps(m3(report)))
            with self.assertRaisesRegex(ValueError, "exact M1/M2/M3 stages"):
                THROUGHPUT.investigate(report, investigation)
            invalid = receipt()
            invalid["full_build_and_consumer_stages"].append({"stage": "extra"})
            report.write_text(json.dumps(invalid))
            investigation.write_text(json.dumps(m3(report)))
            with self.assertRaisesRegex(ValueError, "exact M1/M2/M3 stages"):
                THROUGHPUT.investigate(report, investigation)
            report.write_text(json.dumps(receipt()))
            investigation.write_text(json.dumps({**m3(report), "receipt": {"sha256": "sha256:wrong"}}))
            with self.assertRaisesRegex(ValueError, "not bound"):
                THROUGHPUT.investigate(report, investigation)


if __name__ == "__main__":
    unittest.main()
