#!/usr/bin/env python3
"""Regression checks for RI-13 nontrivial M3 raw batch evidence."""

import importlib.util
import pathlib
import tempfile
import unittest


ROOT = pathlib.Path(__file__).parent
MODULE = ROOT / "summarize_nontrivial_batch.py"
SPEC = importlib.util.spec_from_file_location("ri13_nontrivial_batch", MODULE)
SUMMARY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SUMMARY)
BASELINE = ROOT / "darwin-arm64-nontrivial-a9189183e-2026-10-04.csv"
CANDIDATE = ROOT / "darwin-arm64-nontrivial-460a7e6be-2026-10-04.csv"


class NontrivialBatchSummaryTests(unittest.TestCase):
    def test_recorded_pair_has_exact_workload_copies_and_investigation_status(self):
        result = SUMMARY.summarize(BASELINE, CANDIDATE)
        self.assertEqual(result["workload"]["samples_per_route"], 5)
        self.assertEqual(result["workload"]["operations_per_batch"], 16)
        self.assertEqual(result["workload"]["body_bytes_per_operation"], 4096)
        self.assertEqual(
            result["runs"]["baseline"]["generated_semaprax"]["p50_elapsed_ms"], 52.112458
        )
        self.assertEqual(
            result["runs"]["candidate"]["generated_semaprax"]["p50_elapsed_ms"], 39.400292
        )
        self.assertEqual(
            result["runs"]["baseline"]["direct_rust"]["p50_elapsed_ms"], 5.8845
        )
        self.assertEqual(
            result["runs"]["candidate"]["direct_rust"]["p50_elapsed_ms"], 4.996417
        )
        self.assertEqual(
            result["runs"]["baseline"]["generated_semaprax"]["p50_allocator_requests"]["allocation_calls"], 129792
        )
        self.assertEqual(
            result["runs"]["candidate"]["generated_semaprax"]["p50_allocator_requests"]["allocation_calls"], 84224
        )
        self.assertEqual(result["comparison"]["status"], "investigation_required")

    def test_changed_copy_total_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            changed = pathlib.Path(directory) / "changed.csv"
            changed.write_text(
                CANDIDATE.read_text().replace(
                    "generated_semaprax,0,16,38357750,4096,65536,65536",
                    "generated_semaprax,0,16,38357750,4096,65536,0",
                )
            )
            with self.assertRaisesRegex(ValueError, "callback copy total"):
                SUMMARY.summarize(BASELINE, changed)


if __name__ == "__main__":
    unittest.main()
