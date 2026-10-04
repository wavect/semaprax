#!/usr/bin/env python3
"""Focused no-provider contracts for the Claude Boolean campaign runner."""
import importlib.util
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("claude_campaign", ROOT / "law16_claude_boolean_campaign.py")
RUNNER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(RUNNER)


class ClaudeBooleanCampaignTests(unittest.TestCase):
    def test_preregistered_provider_plan_binds_all_twenty_trials(self):
        plan = json.loads(RUNNER.CAMPAIGN_PLAN.read_text())
        self.assertEqual(len(plan["trials"]), 20)
        self.assertEqual(plan["provider"]["model_id"], "claude-haiku-4-5-20251001")
        self.assertEqual(plan["source_task_plan_sha256"], RUNNER.digest(RUNNER.PLAN.read_bytes()))
        revised = json.loads(RUNNER.CAMPAIGN_PLAN_V2.read_text())
        self.assertEqual(revised["trials"], plan["trials"])
        self.assertEqual(revised["execution"]["per_trial_max_cost_usd"], "0.06")
        self.assertTrue(revised["execution"]["continue_after_trial_failure"])

    def test_retained_budget_overrun_is_an_adverse_partial_campaign(self):
        value = RUNNER.review(ROOT / "evidence/law16-claude-campaign-stopped-v1")
        self.assertEqual(value["status"], "stopped_nonadmitted")
        self.assertEqual(value["accepted_trials"], 8)
        self.assertEqual(value["trials"], 9)

    def test_full_campaign_retains_all_trials_and_the_failed_bend_candidate(self):
        value = RUNNER.review(ROOT / "evidence/law16-claude-campaign-twenty-v2")
        self.assertEqual(value["status"], "twenty_trials_authenticated")
        self.assertEqual(value["trials"], 20)
        self.assertEqual(value["accepted_trials"], 19)
        self.assertEqual(value["matched_pairs"], 9)

    def test_schema_binds_trial_and_attack_digest(self):
        trial = RUNNER.selected_trial("bend2", 1)
        attack, _ = RUNNER.fixtures("bend2")
        schema = RUNNER.response_schema(trial, attack)
        self.assertEqual(schema["properties"]["trial_id"]["const"], trial["id"])
        self.assertEqual(schema["properties"]["seeded_attack"]["properties"]["decision"]["const"], "reject")
        self.assertEqual(schema["properties"]["seeded_attack"]["properties"]["source_sha256"]["const"], RUNNER.digest(attack.read_bytes()))

    def test_prompt_supplies_buggy_source_without_the_accepted_answer(self):
        trial = RUNNER.selected_trial("bend2", 1)
        attack, success = RUNNER.fixtures("bend2")
        text = RUNNER.prompt(trial, attack)
        self.assertIn(json.dumps(attack.read_text()), text)
        self.assertNotIn(json.dumps(success.read_text()), text)
        self.assertNotIn("accepted_source_shape", text)

    def test_terminal_event_and_secret_redaction_preserve_cost_fields(self):
        stream = (json.dumps({"type": "system", "subtype": "init", "model": "claude-haiku-4-5-20251001", "api_key": "secret"}) + "\n" +
                  json.dumps({"type": "result", "is_error": False, "total_cost_usd": 0.001, "usage": {"input_tokens": 1}}) + "\n").encode()
        rows, result = RUNNER.terminal_event(stream)
        self.assertEqual(result["total_cost_usd"], 0.001)
        self.assertEqual(RUNNER.redact(rows)[0]["api_key"], "<redacted>")
        self.assertEqual(RUNNER.money(result["total_cost_usd"]), RUNNER.Decimal("0.001"))

    def test_rate_limited_result_is_a_provider_refusal_shape(self):
        rows, result = RUNNER.terminal_event((json.dumps({"type": "result", "is_error": True, "subtype": "success", "terminal_reason": "api_error", "total_cost_usd": 0}) + "\n").encode())
        self.assertEqual(len(rows), 1)
        self.assertTrue(result["is_error"])
        self.assertEqual(RUNNER.money(result["total_cost_usd"]), RUNNER.Decimal(0))


if __name__ == "__main__":
    unittest.main()
