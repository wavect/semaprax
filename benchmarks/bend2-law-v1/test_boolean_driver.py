#!/usr/bin/env python3
"""Focused contracts for the bounded pinned-Bend Boolean driver."""
import importlib.util
import pathlib
import unittest


MODULE = pathlib.Path(__file__).with_name("bend_boolean_driver.py")
SPEC = importlib.util.spec_from_file_location("bend_boolean_driver", MODULE)
DRIVER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DRIVER)


class BooleanDriverTests(unittest.TestCase):
    def test_normal_requires_both_boolean_values_in_order(self):
        self.assertEqual(
            DRIVER.classify_normal({"status": "accepted", "stdout": "0\n1\n"})[
                "status"
            ],
            "accepted",
        )
        self.assertEqual(
            DRIVER.classify_normal({"status": "accepted", "stdout": "1\n0\n"})[
                "status"
            ],
            "failed",
        )

    def test_verdict_requires_its_own_kernel_success_line(self):
        self.assertEqual(
            DRIVER.classify_verdict({"status": "accepted", "stdout": "ALL PROOFS CHECK\n"})[
                "status"
            ],
            "accepted",
        )
        self.assertEqual(
            DRIVER.classify_verdict({"status": "accepted", "stdout": "0\n1\n"})[
                "status"
            ],
            "failed",
        )


if __name__ == "__main__":
    unittest.main()
