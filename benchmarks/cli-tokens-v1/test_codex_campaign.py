import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import codex_campaign


class CodexCampaignTests(unittest.TestCase):
    def test_exec_parser_keeps_cached_as_subset_and_tools_are_not_turns(self):
        events = [
            {"type": "thread.started", "thread_id": "thread-a"},
            {"type": "item.completed", "item": {"type": "command_execution"}},
            {"type": "item.completed", "item": {"type": "command_execution"}},
            {"type": "item.completed", "item": {"type": "agent_message"}},
            {"type": "turn.completed", "usage": {"input_tokens": 100, "cached_input_tokens": 30,
                "cache_write_input_tokens": 2, "output_tokens": 7, "reasoning_output_tokens": 3}},
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "stream.jsonl"
            path.write_text("\n".join(json.dumps(event) for event in events) + "\n")
            observed = codex_campaign.parse_exec_jsonl(path)
        self.assertEqual(observed["final_turn_usage"]["input_tokens"], 100)
        self.assertEqual(observed["final_turn_usage"]["cached_input_tokens"], 30)
        self.assertEqual(observed["codex_outer_turns"], 1)
        self.assertEqual(observed["tool_item_counts"], {"command_execution": 2, "agent_message": 1})
        self.assertIsNone(observed["model_observed"])
        self.assertIsNone(observed["stable_context_tokens"])

    def test_trace_deduplicates_by_response_and_reconciles_final_usage(self):
        final = {"final_turn_usage": {"input_tokens": 100, "cached_input_tokens": 30,
                 "cache_write_input_tokens": 2, "output_tokens": 7, "reasoning_output_tokens": 3}}
        events = [
            {"type": "turn_context", "payload": {"model": "gpt-6.1-sol", "effort": "medium"}},
            {"type": "token_usage_record", "payload": {"response_id": "r1", "usage": final["final_turn_usage"]}},
            {"type": "token_usage_record", "payload": {"response_id": "r1", "usage": final["final_turn_usage"]}},
            {"type": "event_msg", "payload": {"type": "token_count", "info": {"model_context_window": 258400}}},
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "rollout.jsonl"
            path.write_text("\n".join(json.dumps(event) for event in events) + "\n")
            observed = codex_campaign.trace_usage(final, path)
        self.assertTrue(observed["reconciled"])
        self.assertEqual(observed["model_request_count"], 1)
        self.assertEqual(observed["model_observed"], "gpt-6.1-sol")
        self.assertEqual(observed["model_context_window"], 258400)

    def test_multi_request_uses_response_usage_not_cumulative_turn_totals(self):
        def counts(n):
            return {"input_tokens": n, "cached_input_tokens": n // 2,
                    "cache_write_input_tokens": 0, "output_tokens": n // 10, "reasoning_output_tokens": 0}
        events = [{"type": "turn_context", "payload": {"model": "gpt-6.1-sol", "effort": "medium"}},
                  {"type": "token_usage_record", "payload": {"response_id": "r1", "usage": counts(100), "turn_token_usage": counts(100)}},
                  {"type": "token_usage_record", "payload": {"response_id": "r2", "usage": counts(200), "turn_token_usage": counts(300)}}]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "rollout.jsonl"
            path.write_text("\n".join(json.dumps(e) for e in events))
            result = codex_campaign.trace_usage({"final_turn_usage": counts(300)}, path)
        self.assertTrue(result["reconciled"])
        self.assertEqual(result["model_request_count"], 2)
        self.assertEqual(result["legacy_net_input_tokens"], 100)
        self.assertEqual(result["request_usage_sum"]["input_tokens"], 300)

    def test_task_rollout_ignores_unrelated_sessions_and_binds_cwd(self):
        thread = "01a118e1-0017-78d0-8c6f-2e391487cbc3"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); workspace = root / "worktree"; workspace.mkdir()
            owned = root / f"rollout-{thread}.jsonl"
            owned.write_text(json.dumps({"type": "session_meta", "payload": {
                "id": thread, "cwd": str(workspace), "creator_account_id": "private"}}) + "\n")
            (root / "unrelated.jsonl").write_text("not our session")
            output = root / "copy.jsonl"
            self.assertEqual(codex_campaign.copy_task_rollout([thread], workspace, output, root), output)
            self.assertNotIn("private", output.read_text())
            self.assertIsNone(codex_campaign.copy_task_rollout([thread], root, output, root))
        self.assertIsNone(codex_campaign._usage({"input_tokens": True})["input_tokens"])

    def test_command_disables_the_same_features_for_each_arm(self):
        command = codex_campaign.codex_command("write it")
        self.assertEqual(command[:3], ["codex", "exec", "--json"])
        self.assertIn("--ignore-user-config", command)
        self.assertIn("--ignore-rules", command)
        self.assertEqual([command[index + 1] for index, value in enumerate(command) if value == "--disable"],
                         ["apps", "plugins", "memories", "multi_agent", "skill_search"])
        self.assertNotIn("HOME", " ".join(command))

    def test_price_requires_each_request_and_does_not_double_count_cache(self):
        price = codex_campaign.list_price_estimate([{"usage": {
            "input_tokens": 100, "cached_input_tokens": 30, "cache_write_input_tokens": 2,
            "output_tokens": 7, "reasoning_output_tokens": 3,
        }}])
        self.assertEqual(price["standard_short_context_api_equivalent_usd"], 0.000214)
        self.assertEqual(price["cache_write_tokens"], 2)
        self.assertIsNone(codex_campaign.list_price_estimate([])["standard_short_context_api_equivalent_usd"])

    def test_capabilities_is_help_only(self):
        with patch("codex_campaign.subprocess.run") as run:
            run.return_value.returncode = 0
            run.return_value.stdout = "--json --model --sandbox --ignore-user-config --ignore-rules --disable --config"
            run.return_value.stderr = ""
            result = codex_campaign.capabilities("fixture-codex")
        self.assertEqual(run.call_args.args[0], ["fixture-codex", "exec", "--help"])
        self.assertEqual(result["status"], "ready")


if __name__ == "__main__":
    unittest.main()
