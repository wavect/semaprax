#!/usr/bin/env python3
"""Focused structural tests for the 30-repetition Boolean measurement runner."""
import importlib.util
import pathlib
import unittest


MODULE = pathlib.Path(__file__).with_name("boolean_measure.py")
SPEC = importlib.util.spec_from_file_location("bend2_boolean_measure", MODULE)
MEASURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MEASURE)


class BooleanMeasurementTests(unittest.TestCase):
    def sample(self, elapsed, memory=123):
        return {"status": "accepted", "elapsed_ns": elapsed, "memory": {"status": "observed", "peak_rss": memory}}

    def test_summary_retains_raw_warm_samples_and_separates_p50_p95_and_peak_rss(self):
        warm = [self.sample(value, value * 10) for value in range(1, 31)]
        row = MEASURE.summarize(self.sample(42, 42), warm)
        self.assertEqual(row["status"], "completed")
        self.assertEqual(row["summary"]["warm_sample_count"], 30)
        self.assertEqual(len(row["warm_samples"]), 30)
        self.assertEqual(row["summary"]["p50_ns"], 15.5)
        self.assertEqual(row["summary"]["p95_ns"], 28.5)
        self.assertEqual(row["summary"]["peak_rss_bytes"]["max"], 300)

    def test_missing_memory_or_failed_warm_sample_cannot_be_reported_as_completed(self):
        warm = [self.sample(1) for _ in range(30)]
        warm[0]["memory"] = {"status": "unavailable", "reason": "wrapper absent"}
        row = MEASURE.summarize(self.sample(1), warm)
        self.assertEqual(row["status"], "completed")
        self.assertEqual(row["summary"]["peak_rss_bytes"]["status"], "unavailable")
        warm[1]["status"] = "rejected"
        self.assertEqual(MEASURE.summarize(self.sample(1), warm)["status"], "unavailable")

    def test_exact_output_is_required_for_a_successful_route(self):
        self.assertEqual(MEASURE.expected({"status": "accepted"}, pathlib.Path(__file__), "not this") ["status"], "failed")


if __name__ == "__main__":
    unittest.main()
