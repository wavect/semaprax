#!/usr/bin/env python3
import importlib.util
import pathlib
import unittest

ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("law16_i64_list", ROOT / "law16_i64_list_proof_capsule.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class Law16I64ListProofCapsuleTests(unittest.TestCase):
    def test_bound_profile_uses_the_fixed_law15_identity_and_all_i64_domain(self):
        self.assertEqual(MODULE.PIN, "leanprover/lean4:v4.34.0")
        self.assertIn('law15.collection.sort', MODULE.SOURCE.read_text())
        self.assertNotIn('law16.collection.sort', MODULE.SOURCE.read_text())
        self.assertIn('sort_multiplicity', MODULE.PROOFS.read_text())

    def test_kernel_acceptance_requires_both_seeded_negative_controls(self):
        text = (f"test x::{MODULE.TEST} ... ok\n"
                "empty-output: count differs for value 2 on input [1, 2]\n"
                "duplicate-element: count differs for value 2 on input [1, 2]\n").encode()
        self.assertTrue(MODULE.accepted(0, text))
        self.assertFalse(MODULE.accepted(0, text.replace(b"duplicate-element", b"wrong")))
        self.assertFalse(MODULE.accepted(1, text))

    def test_command_runs_one_exact_ignored_test_from_the_supplied_binary(self):
        command = MODULE.command(pathlib.Path("/tool/language"))
        self.assertEqual(command, ["/tool/language", MODULE.TEST, "--ignored", "--exact", "--nocapture"])


if __name__ == "__main__":
    unittest.main()
