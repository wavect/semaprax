#!/usr/bin/env python3
import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location(
    "law16_checked_u32_admission", ROOT / "law16_checked_u32_admission.py"
)
ADMISSION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADMISSION)


class CheckedU32AdmissionTests(unittest.TestCase):
    def test_fixed_probes_are_distinct_and_bind_u32_success_and_overflow(self):
        success = ADMISSION.SUCCESS.read_text()
        overflow = ADMISSION.OVERFLOW.read_text()
        self.assertIn("fn successor(value: u32) -> u32", success)
        self.assertIn("requires value < 4294967295u32", success)
        self.assertIn("ensures result == value + 1u32", success)
        self.assertIn("4294967295u32 + 1u32", overflow)
        self.assertNotEqual(ADMISSION.sha256(success.encode()), ADMISSION.sha256(overflow.encode()))

    def test_commands_require_a_bound_source_placeholder(self):
        self.assertEqual(ADMISSION.command('["semaprax", "check", "{source}"]', "success")[2], "{source}")
        with self.assertRaises(ValueError):
            ADMISSION.command('["semaprax", "check"]', "success")
        with self.assertRaises(ValueError):
            ADMISSION.command("{}", "success")


if __name__ == "__main__":
    unittest.main()
