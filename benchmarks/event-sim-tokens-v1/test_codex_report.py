import json
import tempfile
import unittest
from pathlib import Path

from codex_report import recount, summarize


def row(arm, number, status="accepted", cost=1):
    return {
        "arm": arm, "number": number, "status": status,
        "model_request_turns": 2, "raw_input_tokens": 10,
        "cached_input_tokens": 3, "cache_write_input_tokens": 1,
        "output_tokens": 4, "legacy_net_input_tokens": 8,
        "final_authored_tokens_proxy": 20, "agent_wall_seconds": 1.5,
        "acceptance_wall_seconds": 0.5,
        "conditional_api_equivalent_usd": cost,
    }


class ReportTests(unittest.TestCase):
    def test_recount_uses_real_codex_trace_adapter_and_preserves_missing_trace(self):
        usage = {"input_tokens": 100, "cached_input_tokens": 30,
                 "cache_write_input_tokens": 2, "output_tokens": 7,
                 "reasoning_output_tokens": 3}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            stream = root / "trial.jsonl"
            trace = root / "rollout.jsonl"
            stream.write_text(json.dumps({"type": "thread.started", "thread_id": "00000000-0000-0000-0000-000000000001"}) + "\n"
                              + json.dumps({"type": "turn.completed", "usage": usage}) + "\n")
            trace.write_text("\n".join(json.dumps(record) for record in (
                {"type": "session_meta", "payload": {"id": "00000000-0000-0000-0000-000000000001", "cwd": str(root)}},
                {"type": "turn_context", "payload": {"model": "gpt-6.1-sol", "effort": "medium"}},
                {"type": "token_usage_record", "payload": {"response_id": "r1", "usage": usage}},
            )) + "\n")
            results = root / "results.json"
            results.write_text(json.dumps({
                "campaign": {"trial_order": ["semaprax", "typescript"]},
                "trials": [
                    {"arm": "semaprax", "number": 1, "status": "accepted",
                     "transcript": str(stream), "rollout_trace": str(trace),
                     "elapsed_seconds": 1.25, "acceptance_elapsed_seconds": 0.5,
                     "final_candidate_source_metrics": {"total_tokens": 19}},
                    {"arm": "typescript", "number": 1, "status": "failed"},
                ],
            }))
            report = recount(results)
            recorded = report["trials"][0]
            self.assertEqual(recorded["model_request_turns"], 1)
            self.assertEqual(recorded["model_observed"], "gpt-6.1-sol")
            self.assertEqual(recorded["effort_observed"], "medium")
            self.assertIsNone(recorded["provider_resolved_model"])
            self.assertEqual(recorded["raw_input_tokens"], 100)
            self.assertEqual(recorded["cached_input_tokens"], 30)
            self.assertEqual(recorded["output_tokens"], 7)
            self.assertEqual(recorded["final_authored_tokens_proxy"], 19)
            self.assertEqual(recorded["evidence_sha256"].keys(), {"exec", "rollout"})
            self.assertIsNone(report["trials"][1]["model_request_turns"])

    def test_recount_rejects_malformed_trial(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "results.json"
            path.write_text(json.dumps({"campaign": {"trial_order": []}, "trials": [None]}))
            with self.assertRaisesRegex(ValueError, "malformed trial"):
                recount(path)

    def test_failed_attempt_cost_is_in_per_accepted_denominator(self):
        data = {"campaign": {"trial_order": ["semaprax", "typescript", "typescript", "semaprax"]}}
        result = summarize(data, [row("semaprax", 1, cost=2), row("typescript", 1),
                                  row("typescript", 2), row("semaprax", 2, "failed", 4)])
        arm = result["arms"]["semaprax"]
        self.assertEqual(arm["accepted"], 1)
        self.assertEqual(arm["conditional_short_context_api_equivalent_usd_per_accepted_task"], 6)
        self.assertEqual(arm["metrics_all_recorded_attempts"]["raw_input_tokens"]["total"], 20)

    def test_unknown_cost_stays_null(self):
        data = {"campaign": {"trial_order": ["semaprax", "typescript", "semaprax"]}}
        result = summarize(data, [row("semaprax", 1), row("typescript", 1), row("semaprax", 2, "failed", None)])
        self.assertIsNone(result["arms"]["semaprax"]["conditional_short_context_api_equivalent_usd_per_accepted_task"])
        self.assertIsNone(result["arms"]["semaprax"]["actual_billed_usd"])

    def test_partial_campaign_cannot_have_winner(self):
        data = {"campaign": {"trial_order": ["semaprax", "typescript", "typescript", "semaprax"]}}
        result = summarize(data, [row("semaprax", 1)])
        self.assertFalse(result["complete"])
        self.assertFalse(result["all_arms_have_minimum_trials"])
        self.assertIsNone(result["winner"])
        self.assertEqual(result["unlaunched_order"], ["typescript", "typescript", "semaprax"])
        with self.assertRaisesRegex(ValueError, "order"):
            summarize(data, [row("typescript", 1)])


if __name__ == "__main__":
    unittest.main()
