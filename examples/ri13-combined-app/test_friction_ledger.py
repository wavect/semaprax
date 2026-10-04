#!/usr/bin/env python3
"""Static contracts for RI-13 developer-friction disclosure."""
import importlib.util
import pathlib
import unittest

MODULE = pathlib.Path(__file__).with_name("friction-ledger.py")
SPEC = importlib.util.spec_from_file_location("ri13_friction", MODULE)
LEDGER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LEDGER)


class FrictionLedgerTests(unittest.TestCase):
    def test_every_milestone_has_hashed_authored_files_and_no_manual_abi_escape(self):
        document = LEDGER.ledger()
        self.assertEqual(document["schema"], "semaprax.ri13.developer-friction-ledger.v1")
        self.assertEqual(set(document["milestones"]), {"m1", "m2", "m3", "linked"})
        for row in document["milestones"].values():
            self.assertGreater(row["authored_noncomment_lines"], 0)
            self.assertEqual(row["handwritten_abi_or_unsafe_escape_hatches"], {"count": 0, "status": "refused"})
            self.assertTrue(all(entry["sha256"].startswith("sha256:") for entry in row["authored_rust_files"]))

    def test_explicit_host_configuration_and_generated_exclusion_remain_visible(self):
        document = LEDGER.ledger()
        self.assertEqual(document["caller_owned_host_configuration"]["m3"]["count"], 3)
        self.assertEqual(document["caller_owned_host_configuration"]["linked"]["count"], 2)
        self.assertEqual(document["generated_artifacts"]["status"], "excluded_from_authored_count")


if __name__ == "__main__":
    unittest.main()
