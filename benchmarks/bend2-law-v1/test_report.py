#!/usr/bin/env python3
"""Focused contracts for the LAW-16 transparent report renderer."""
import importlib.util
import json
import pathlib
import tempfile
import unittest


MODULE = pathlib.Path(__file__).with_name("report.py")
SPEC = importlib.util.spec_from_file_location("bend2_law_report", MODULE)
REPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORT)


class ReportTests(unittest.TestCase):
    def result(self):
        paths = {
            path: {"status": "unavailable", "reason": "no local tool"}
            for path in REPORT.PATHS
        }
        paths["bend_normal"] = {
            "status": "ok",
            "cold": {"status": "accepted", "wall_ms": 9.0},
            "attacks": {"weakened-postcondition": {"status": "rejected"}},
            "warm": {"p50_ms": 2.0, "p95_ms": 3.0, "samples_ms": [1.0, 2.0, 3.0]},
        }
        return {
            "schema": REPORT.INPUT_SCHEMA,
            "status": "unavailable",
            "identities": {"bend": {"status": "ok"}, "semaprax": {"status": "ok"}},
            "environment": {"hardware": "test host"},
            "configuration": {"samples": 30},
            "manifest": {"sha256": "sha256:manifest"},
            "commands_sha256": "sha256:commands",
            "fixture_digests": {"boolean": "sha256:fixture"},
            "cells": [{"id": "boolean", "numeric_domain": "bool exact", "laws": ["postcondition"], "paths": paths}],
        }

    def write(self, root, value):
        path = root / "result.json"
        path.write_text(json.dumps(value))
        return path

    def test_report_retains_samples_variation_nonresults_and_distinct_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self.write(pathlib.Path(directory), self.result())
            document = REPORT.render(path)
        paths = document["cells"][0]["paths"]
        normal = next(row for row in paths if row["path"] == "bend_normal")
        self.assertEqual(normal["warm"]["variation"]["raw_samples_ms"], [1.0, 2.0, 3.0])
        self.assertEqual(normal["warm"]["variation"]["population_stddev_ms"], 0.816)
        unavailable = next(row for row in paths if row["path"] == "semaprax_lean")
        self.assertEqual(unavailable["status"], "unavailable")
        self.assertNotIn("winner", document)
        self.assertIn("trusted_computing_base", document)

    def test_missing_execution_path_or_raw_samples_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            value = self.result()
            del value["cells"][0]["paths"]["bend_verdict"]
            with self.assertRaisesRegex(ValueError, "every distinct"):
                REPORT.render(self.write(root, value))
            value = self.result()
            value["cells"][0]["paths"]["bend_normal"]["warm"]["samples_ms"] = []
            with self.assertRaisesRegex(ValueError, "raw warm samples"):
                REPORT.render(self.write(root, value))


if __name__ == "__main__":
    unittest.main()
