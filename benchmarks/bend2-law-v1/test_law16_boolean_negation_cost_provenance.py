#!/usr/bin/env python3
"""Regression coverage for the retained Codex cost-provenance adapter."""
import importlib.util
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("cost_provenance", ROOT / "law16_boolean_negation_cost_provenance.py")
PROVENANCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROVENANCE)


class CostProvenanceTests(unittest.TestCase):
    def test_retained_twenty_event_streams_bind_token_usage_and_typed_cost_absence(self):
        value = PROVENANCE.capture(ROOT / "evidence")
        self.assertEqual(value["status"], "provider_monetary_receipts_unavailable")
        self.assertEqual(value["scope"]["matched_pairs"], 10)
        self.assertEqual(len(value["trials"]), 20)
        self.assertEqual(value["cost_usage"]["observed_monetary_field_paths"], [])
        self.assertEqual(value["aggregate_token_usage"]["total_tokens"], sum(row["token_usage"]["total_tokens"] for row in value["trials"]))

    def test_checked_in_receipt_is_the_current_authenticated_capture(self):
        retained = json.loads((ROOT / "evidence/law16-boolean-negation-agent-cost-provenance-v1.json").read_text())
        self.assertEqual(retained, PROVENANCE.capture(ROOT / "evidence"))

    def test_monetary_provider_fields_are_not_silently_treated_as_absent(self):
        with self.assertRaisesRegex(ValueError, "provider monetary fields were observed"):
            PROVENANCE.usage_and_absence(b'{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1},"cost_usd":"0.01"}\n')

    def test_agent_message_cost_words_are_not_provider_billing_metadata(self):
        tokens, cost = PROVENANCE.usage_and_absence(b'{"type":"item.completed","item":{"type":"agent_message","text":"cost_usd: 0.01"}}\n{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1}}\n')
        self.assertEqual(tokens["total_tokens"], 2)
        self.assertEqual(cost["status"], "unavailable")


if __name__ == "__main__":
    unittest.main()
