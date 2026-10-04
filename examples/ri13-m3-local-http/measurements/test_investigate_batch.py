#!/usr/bin/env python3
"""Focused regression tests for RI-13 batch regression investigation."""

import importlib.util
import json
import pathlib
import tempfile
import unittest


MODULE = pathlib.Path(__file__).with_name("investigate_batch.py")
SPEC = importlib.util.spec_from_file_location("ri13_batch_investigation", MODULE)
INVESTIGATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INVESTIGATE)


def report():
    routes = {}
    for name, throughput, allocations in (
        ("direct_rust", 4700.0, 88),
        ("handwritten_adapter", 4900.0, 88),
        ("generated_semaprax", 245.0, 8358),
    ):
        routes[name] = {
            "operations_per_sample": 64,
            "body_bytes_per_operation": 2,
            "normalized_operations_per_second": throughput,
            "allocator_requests_per_batch": {
                "allocation_calls": allocations,
                "deallocation_calls": allocations,
                "reallocation_calls": 0,
                "allocated_bytes": allocations * 2,
                "deallocated_bytes": allocations * 2,
            },
        }
    return {
        "schema": INVESTIGATE.REPORT_SCHEMA,
        "checkout": "reviewed",
        "batch_throughput_measurement_command": {"command": ["cargo", "run", "--", "batch"]},
        "batch_throughput": {"routes": routes},
    }


SOURCE = """
fn run_batch_measurement() { for _ in 0..BATCH_OPERATIONS { execute(route, runtime, client, &endpoint, revision); } }
fn execute() { let registered = generated::register(Arc::clone(revision), move |_| async { 0 }).unwrap(); let pending = registered.call_typed(seed, 10_000).unwrap(); }
"""


class BatchInvestigationTests(unittest.TestCase):
    def test_low_generated_throughput_requires_investigation_and_binds_source(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            receipt = root / "receipt.json"
            source = root / "measure.rs"
            receipt.write_text(json.dumps(report()))
            source.write_text(SOURCE)
            result = INVESTIGATE.investigate(receipt, source)
        self.assertEqual(result["status"], "investigation_required")
        self.assertEqual(result["measured"]["generated_to_handwritten_throughput_ratio"], 0.05)
        self.assertGreater(result["measured"]["generated_minus_handwritten_allocator_requests_per_operation"]["allocation_calls"], 0)
        self.assertTrue(result["inspected_source"]["sha256"].startswith("sha256:"))

    def test_wrong_batch_workload_and_missing_registration_shape_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            receipt = root / "receipt.json"
            source = root / "measure.rs"
            invalid = report()
            invalid["batch_throughput"]["routes"]["direct_rust"]["operations_per_sample"] = 1
            receipt.write_text(json.dumps(invalid))
            source.write_text(SOURCE)
            with self.assertRaisesRegex(ValueError, "reviewed workload"):
                INVESTIGATE.investigate(receipt, source)
            receipt.write_text(json.dumps(report()))
            source.write_text("fn main() {}")
            with self.assertRaisesRegex(ValueError, "repeated-registration"):
                INVESTIGATE.investigate(receipt, source)


if __name__ == "__main__":
    unittest.main()
