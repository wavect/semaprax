#!/usr/bin/env python3
"""Focused contracts for Claude monetary-cost pilot sanitization."""
import importlib.util
import json
import pathlib
import tempfile
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("claude_cost_pilot", ROOT / "law16_claude_cost_pilot.py")
PILOT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PILOT)


class ClaudeCostPilotTests(unittest.TestCase):
    def plan(self, root):
        path = root / "plan.json"
        path.write_text(json.dumps({"schema": PILOT.PLAN_SCHEMA, "provider": {"id": "anthropic-claude-code-cli", "model_id": "claude-haiku-4-5-20251001", "runner": "claude-print-stream-json-v1"}, "trial": {"id": "trial:1", "language": "probe-only", "repository_access": "none", "source_outcome": "required_before_campaign"}, "budget": {"max_cost_usd": "0.01", "scope": "one"}, "campaign_admission": "requires success"}))
        return path

    def stream(self, root, result):
        path = root / "events.jsonl"
        path.write_text(json.dumps({"type": "system", "subtype": "init", "model": "claude-haiku-4-5-20251001", "claude_code_version": "2.1.289"}) + "\n" + json.dumps(result) + "\n")
        return path

    def test_error_budget_result_retains_provider_charge_but_cannot_admit_campaign(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            value = PILOT.capture(self.plan(root), self.stream(root, {"type": "result", "subtype": "error_max_budget_usd", "is_error": True, "total_cost_usd": 0.023741, "duration_ms": 1, "usage": {"input_tokens": 0, "output_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0}}))
        self.assertEqual(value["provider_cost_usd"], 0.023741)
        self.assertFalse(value["campaign_admission"])
        self.assertEqual(value["status"], "blocked_before_source_outcome_review")

    def test_success_over_budget_is_not_admitted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            value = PILOT.capture(self.plan(root), self.stream(root, {"type": "result", "subtype": "success", "is_error": False, "total_cost_usd": 0.02, "duration_ms": 1, "usage": {"input_tokens": 1, "output_tokens": 1, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0}}))
        self.assertFalse(value["campaign_admission"])

    def test_success_in_budget_still_requires_source_outcome_review(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            value = PILOT.capture(self.plan(root), self.stream(root, {"type": "result", "subtype": "success", "is_error": False, "total_cost_usd": 0.001, "duration_ms": 1, "usage": {"input_tokens": 1, "output_tokens": 1, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0}}))
        self.assertEqual(value["status"], "eligible_for_source_outcome_review")
        self.assertFalse(value["campaign_admission"])


if __name__ == "__main__":
    unittest.main()
