#!/usr/bin/env python3
import pathlib
import subprocess
import unittest

import full_u32_equal_spec as profile


class FullU32EqualSpecTests(unittest.TestCase):
    def test_profile_binds_full_domain_sources_and_attack_shapes(self):
        value = profile.profile()
        self.assertEqual(value["schema"], profile.SCHEMA)
        self.assertEqual(set(value["attacks"].values()), {"no-op-transfer", "empty-sort"})
        self.assertIn("fixed length four", value["nonclaims"][1])

    def test_universal_sort_model_checks_sortedness_and_multiplicity(self):
        z3 = __import__("shutil").which("z3")
        if not z3:
            self.skipTest("z3 is not installed")
        source = profile.FIXTURES / "sort-equal-spec.smt2"
        result = subprocess.run([z3, "-smt2", str(source)], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.split(), ["unsat", "sat"])


if __name__ == "__main__":
    unittest.main()
