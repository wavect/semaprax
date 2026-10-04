#!/usr/bin/env python3
"""Focused contracts for the Boolean law-gaming receipt driver."""
import importlib.util
import pathlib
import unittest


MODULE = pathlib.Path(__file__).with_name("boolean_negative_controls.py")
SPEC = importlib.util.spec_from_file_location("bend2_boolean_controls", MODULE)
CONTROLS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CONTROLS)


class BooleanNegativeControlTests(unittest.TestCase):
    def test_bend_rejection_requires_an_actual_kernel_failure_marker(self):
        self.assertEqual(
            CONTROLS.expected_reject({"status": "rejected"}, "SOME PROOFS FAIL\n", "SOME PROOFS FAIL")["status"],
            "rejected",
        )
        self.assertEqual(
            CONTROLS.expected_reject({"status": "rejected"}, "ordinary output\n", "SOME PROOFS FAIL")["status"],
            "failed",
        )

    def test_runtime_control_requires_success_witness_and_rejects_a_zero_exit_mutant(self):
        self.assertEqual(CONTROLS.expected_accept({"status": "accepted"}, "0\n")["status"], "accepted")
        self.assertEqual(CONTROLS.expected_accept({"status": "accepted"}, "1\n")["status"], "failed")
        self.assertEqual(
            CONTROLS.expected_reject({"status": "accepted"}, "language status", "language status")["status"],
            "failed",
        )


if __name__ == "__main__":
    unittest.main()
