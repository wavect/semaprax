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
    def test_every_application_milestone_has_hashed_files_and_no_manual_abi_escape(self):
        document = LEDGER.ledger()
        self.assertEqual(document["schema"], "semaprax.ri13.developer-friction-ledger.v2")
        self.assertEqual(set(document["milestones"]), {"m1", "m2", "m3", "linked"})
        for row in document["milestones"].values():
            self.assertGreater(row["authored_noncomment_lines"], 0)
            self.assertEqual(row["handwritten_abi_or_unsafe_escape_hatches"], {"count": 0, "status": "refused"})
            self.assertTrue(all(entry["sha256"].startswith("sha256:") for entry in row["authored_rust_files"]))

    def test_comparison_harnesses_disclose_handwritten_adapters_and_instrumentation(self):
        document = LEDGER.ledger()
        comparisons = document["comparison_harnesses"]
        self.assertEqual(set(comparisons), {"m1", "m2", "m3"})
        self.assertIn("HandwrittenRegex", comparisons["m1"]["handwritten_adapter_symbols"])
        self.assertIn("HandwrittenRecordAdapter", comparisons["m2"]["handwritten_adapter_symbols"])
        for row in comparisons.values():
            self.assertTrue(row["comparison_rust_files"])
            self.assertTrue(all(entry["sha256"].startswith("sha256:") for entry in row["comparison_rust_files"]))
        self.assertGreater(
            comparisons["m1"]["comparison_rust_files"][0]["escape_hatch_tokens"].get("unsafe", 0),
            0,
        )

    def test_explicit_host_configuration_and_generated_inventory_contract_remain_visible(self):
        document = LEDGER.ledger()
        self.assertEqual(document["caller_owned_host_configuration"]["m3"]["count"], 3)
        self.assertEqual(document["caller_owned_host_configuration"]["linked"]["count"], 2)
        generated = document["generated_code"]
        self.assertEqual(generated["status"], "prepared_then_inventory_required")
        self.assertEqual(generated["measurement_receipt_field"], "generated_code_inventory")
        self.assertEqual(set(generated["locations"]), {"m1", "m2", "m3", "linked"})


if __name__ == "__main__":
    unittest.main()
