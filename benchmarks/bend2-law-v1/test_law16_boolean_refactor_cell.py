#!/usr/bin/env python3
"""Offline regression for the retained matched Boolean-refactor cell."""

import importlib.util
import pathlib
import shutil
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parent
CAPSULE = ROOT / "evidence/law16-boolean-refactor-cell-v1"
SPEC = importlib.util.spec_from_file_location("law16_boolean_refactor_cell", ROOT / "law16_boolean_refactor_cell.py")
CELL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CELL)


class BooleanRefactorCellTest(unittest.TestCase):
    def test_authenticated_capture_has_all_routes(self):
        review = CELL.verify(CAPSULE)
        self.assertEqual(review["status"], "completed_local_matched_boolean_refactor")
        self.assertEqual(review["routes"], 8)
        self.assertEqual(review["raw_streams"], 16)
        self.assertEqual(review["checked_u32"], "not_admitted_by_this_boolean_cell")

    def test_raw_drift_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            copy = pathlib.Path(directory) / "capsule"
            shutil.copytree(CAPSULE, copy)
            raw = copy / "raw/bend-candidate-verdict.stdout"
            raw.write_bytes(raw.read_bytes() + b"drift\n")
            with self.assertRaises(ValueError):
                CELL.verify(copy)


if __name__ == "__main__":
    unittest.main()
