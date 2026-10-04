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
    m2_tasks = {}
    for task in THROUGHPUT.M2_TASKS:
        m2_tasks[task] = {
            "routes": {
                name: {
                    "operations_per_sample": 32,
                    "normalized_operations_per_second": rate["normalized_operations_per_second"],
                    "adapter_buffer_copied_bytes_per_batch": 0,
                }
                for name, rate in routes.items()
            }
        }
    return {
        "schema": THROUGHPUT.COMBINED_SCHEMA,
        "checkout": "reviewed",
        "full_build_and_consumer_stages": [{"stage": stage} for stage in THROUGHPUT.STAGES],
        "m2_batch_throughput_measurement_command": {"command": ["cargo", "run", "--bin", "measure"]},
        "m2_batch_throughput": {"tasks": m2_tasks},
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
    def test_binds_m3_investigation_and_retains_only_m1_unavailable(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            report = root / "receipt.json"
            investigation = root / "m3.json"
            report.write_text(json.dumps(receipt()))
            investigation.write_text(json.dumps(m3(report)))
            result = THROUGHPUT.investigate(report, investigation)
        m1 = result["profiles"]["m1"]
        self.assertEqual(m1["status"], "unavailable")
        self.assertEqual(m1["fixed_authored_workload"]["input_bytes"], 28)
        self.assertEqual(m1["fixed_authored_workload"]["exports"], ["regex.run", "url.run"])
        self.assertEqual(m1["generated_batch_api"]["name"], "checked-export-repeat.v1")
        self.assertEqual(m1["generated_batch_api"]["operations_bound"], 4096)
        self.assertIn("accepts no new foreign input", m1["generated_batch_api"]["input_authority"])
        self.assertEqual(
            m1["generated_batch_api"]["metrics"],
            [
                "exact borrowed-input bytes",
                "adapter copy events and copied bytes",
                "post-run owner/view/string cleanup counts",
            ],
        )
        self.assertIn(
            "locked measurement consumer",
            m1["missing_measurement_consumer"]["requirement"],
        )
        self.assertEqual(result["profiles"]["m2"]["status"], "measured")
        self.assertEqual(
            result["profiles"]["m2"]["tasks"]["generic_record"]["generated_to_direct_throughput_ratio"],
            0.0521,
        )
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


    def test_refuses_m2_copy_or_workload_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            report = root / "receipt.json"
            investigation = root / "m3.json"
            invalid = receipt()
            invalid["m2_batch_throughput"]["tasks"]["generic_record"]["routes"]["generated_semaprax"]["adapter_buffer_copied_bytes_per_batch"] = 1
            report.write_text(json.dumps(invalid))
            investigation.write_text(json.dumps(m3(report)))
            with self.assertRaisesRegex(ValueError, "M2 scalar copy accounting"):
                THROUGHPUT.investigate(report, investigation)
            invalid = receipt()
            invalid["m2_batch_throughput"]["tasks"]["stateful_callback"]["routes"]["direct_rust"]["operations_per_sample"] = 31
            report.write_text(json.dumps(invalid))
            investigation.write_text(json.dumps(m3(report)))
            with self.assertRaisesRegex(ValueError, "reviewed M2 workload"):
                THROUGHPUT.investigate(report, investigation)


if __name__ == "__main__":
    unittest.main()
