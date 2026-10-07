import json
import os
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path
import subprocess
from unittest.mock import patch

import live_campaign


class LiveCampaignTests(unittest.TestCase):
    def test_stream_usage_keeps_observed_model_and_deduplicates_message_updates(self):
        events = [
            {
                "type": "assistant",
                "message": {
                    "id": "turn-1",
                    "model": live_campaign.MODEL,
                    "usage": {"input_tokens": 100, "output_tokens": 20},
                    "content": [{"type": "text", "text": "partial"}],
                },
            },
            {
                "type": "assistant",
                "message": {
                    "id": "turn-1",
                    "model": live_campaign.MODEL,
                    "usage": {
                        "input_tokens": 120,
                        "cache_creation_input_tokens": 10,
                        "cache_read_input_tokens": 30,
                        "output_tokens": 25,
                    },
                    "content": [{"type": "text", "text": "final"}],
                },
            },
            {
                "type": "assistant",
                "message": {
                    "id": "turn-2",
                    "model": live_campaign.MODEL,
                    "usage": {"input_tokens": 40, "output_tokens": 10},
                    "content": [],
                },
            },
            {"type": "result", "subtype": "success", "usage": {
                "input_tokens": 160,
                "cache_creation_input_tokens": 10,
                "cache_read_input_tokens": 30,
                "output_tokens": 34,
            }},
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "stream.jsonl"
            path.write_text("\n".join(json.dumps(event) for event in events) + "\n")
            result = live_campaign.stream_usage(path)
        self.assertEqual(result["models_observed"], [live_campaign.MODEL])
        self.assertEqual(result["turns_with_usage"], 2)
        self.assertEqual(result["usage"]["input_tokens"], 160)
        self.assertEqual(result["usage"]["cache_creation_input_tokens"], 10)
        self.assertEqual(result["first_turn_usage"]["input_tokens"], 120)
        self.assertEqual(result["result_event"]["subtype"], "success")
        self.assertEqual(result["usage_updates_per_message"]["turn-1"], 2)
        self.assertEqual(result["usage_discrepancies"]["output_tokens"], {
            "per_turn_sum": 35, "final_result": 34,
        })
        self.assertEqual(result["usage"]["output_tokens"], 34)

    def test_missing_usage_fields_remain_unknown_and_model_usage_aliases_parse(self):
        events = [
            {"type": "assistant", "message": {
                "id": "turn-1", "model": live_campaign.MODEL,
                "usage": {"input_tokens": 42, "output_tokens": 3},
            }},
            {"type": "result", "modelUsage": {live_campaign.MODEL: {
                "inputTokens": 42, "outputTokens": 3,
            }}},
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "stream.jsonl"
            path.write_text("\n".join(json.dumps(event) for event in events) + "\n")
            result = live_campaign.stream_usage(path)
        self.assertEqual(result["usage"]["input_tokens"], 42)
        self.assertEqual(result["usage"]["cache_read_input_tokens"], None)
        self.assertEqual(result["usage"]["cache_creation_input_tokens"], None)
        self.assertIsNone(result["fixed_context_tokens_in_this_session"])
        self.assertIsNone(live_campaign.rate_card_estimate(result["usage"]))

    def test_calibration_subtracts_one_prompt_proxy_from_first_turn_input_and_cache(self):
        first_turn = {
            "input_tokens": 100,
            "cache_creation_input_tokens": 20,
            "cache_read_input_tokens": 30,
            "output_tokens": 8,
        }
        self.assertEqual(live_campaign.input_tokens_total(first_turn), 150)
        self.assertEqual(live_campaign.inherited_context_proxy(first_turn, 12), 138)
        self.assertIsNone(live_campaign.inherited_context_proxy({"input_tokens": 100}, 12))
        self.assertIsNone(live_campaign.inherited_context_proxy(first_turn, None))
        self.assertIsNone(live_campaign.inherited_context_proxy(first_turn, 151))

    def test_calibration_runner_uses_sparse_checkout_and_removes_clean_worktree(self):
        repo = live_campaign.REPO
        commit = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=repo, text=True, capture_output=True, check=True
        ).stdout.strip()
        with tempfile.TemporaryDirectory() as directory:
            artifacts = Path(directory) / "artifacts"
            workspace = artifacts / "worktrees" / "calibration"
            settings = {
                "model": live_campaign.MODEL,
                "effort": live_campaign.EFFORT,
                "timeout_seconds": 30,
                "max_budget_usd": None,
                "authored_source_tokenizer": None,
            }

            def fake_cli(_command, cwd, _env, stream_path, stderr_path, _timeout):
                self.assertEqual(sorted(
                    str(path.relative_to(cwd)) for path in cwd.rglob("*")
                    if path.is_file() and ".git" not in path.parts
                ), [path.lstrip("/") for path in live_campaign.SPARSE_FILES])
                events = [
                    {"type": "assistant", "message": {
                        "id": "calibration-turn", "model": live_campaign.MODEL,
                        "usage": {"input_tokens": 100, "cache_creation_input_tokens": 20,
                                  "cache_read_input_tokens": 30, "output_tokens": 1},
                        "content": [{"type": "text", "text": "READY"}],
                    }},
                    {"type": "result", "subtype": "success"},
                ]
                stream_path.write_text("\n".join(json.dumps(event) for event in events) + "\n")
                stderr_path.write_text("")
                return {"process_exit_code": 0, "timed_out": False, "elapsed_seconds": 0.01, "failure": None}

            try:
                with patch.object(live_campaign, "run_claude", side_effect=fake_cli):
                    row = live_campaign.launch_calibration(
                        repo, artifacts, commit, settings, Path("/bin/true")
                    )
                self.assertEqual(row["status"], "ready")
                self.assertEqual(row["first_turn_provider_input_plus_cache_tokens"], 150)
                self.assertIsNone(row["inherited_context_input_tokens_proxy"])
                self.assertTrue(row["workspace_removed"])
                self.assertFalse(workspace.exists())
            finally:
                if workspace.exists():
                    subprocess.run(
                        ["git", "worktree", "remove", "--force", str(workspace)],
                        cwd=repo, text=True, capture_output=True, check=False,
                    )

    def test_authored_source_token_proxy_has_pinned_identity_and_excludes_generated_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = root / "node_modules" / "@anthropic-ai" / "tokenizer"
            dependency = root / "node_modules" / "tiktoken"
            package.mkdir(parents=True)
            dependency.mkdir(parents=True)
            (package / "package.json").write_text(json.dumps({"name": "@anthropic-ai/tokenizer", "version": "0.0.4"}))
            (package / "index.js").write_text("exports.countTokens = text => text.length;\n")
            (dependency / "package.json").write_text(json.dumps({"version": "1.0.10"}))
            candidate = root / "candidate"
            (candidate / "dist").mkdir(parents=True)
            (candidate / "node_modules" / "library").mkdir(parents=True)
            (candidate / "acceptance-fixtures").mkdir()
            (candidate / "main.spx").write_text("abc\n")
            (candidate / "main.ts").write_text("defg\n")
            (candidate / "main.js").write_text("generated js\n")
            (candidate / "build.sh").write_text("hi\n")
            (candidate / "package.json").write_text("{}\n")
            (candidate / "compiler.c").write_text("generated c\n")
            (candidate / "dist" / "output.ts").write_text("generated ts\n")
            (candidate / "node_modules" / "library" / "index.ts").write_text("dependency\n")
            (candidate / "acceptance-fixtures" / "expected.ts").write_text("fixture\n")
            metadata = live_campaign.tokenizer_metadata(root)
            result = live_campaign.authored_source_metrics(candidate, metadata)
        self.assertEqual(result["status"], "measured_proxy")
        self.assertEqual(result["total_tokens"], len("abc\ndefg\nhi\n{}\n"))
        self.assertEqual([row["path"] for row in result["files"]], [
            "build.sh", "main.spx", "main.ts", "package.json",
        ])
        self.assertEqual(result["tokenizer"]["package"], "@anthropic-ai/tokenizer")
        self.assertEqual(result["tokenizer"]["version"], "0.0.4")
        self.assertIn("legacy-Claude tokenizer proxy", result["tokenizer"]["claim"])
        self.assertEqual(len(result["tokenizer"]["fingerprint_sha256"]), 64)

    def test_failures_remain_in_each_arm_denominator(self):
        rows = [
            {"arm": "semaprax", "number": 1, "status": "accepted"},
            {"arm": "semaprax", "number": 2, "status": "failed", "failure": "timeout"},
            {"arm": "typescript", "number": 1, "status": "accepted"},
        ]
        result = live_campaign.summarize(rows)
        semaprax = result["arms"][0]
        self.assertEqual(result["attempt_denominator"], 3)
        self.assertEqual(semaprax["attempts"], 2)
        self.assertEqual(semaprax["accepted"], 1)
        self.assertEqual(semaprax["acceptance_rate"], 0.5)
        self.assertEqual(semaprax["failures"][0]["reason"], "timeout")

    def test_rate_card_is_labeled_as_an_estimate_and_prices_each_usage_bucket(self):
        estimate = live_campaign.rate_card_estimate({
            "input_tokens": 1_000_000,
            "cache_creation_input_tokens": 1_000_000,
            "cache_read_input_tokens": 1_000_000,
            "output_tokens": 1_000_000,
        })
        self.assertEqual(estimate, 14.7)
        self.assertEqual(live_campaign.PRICE_BOOK_DATE, "2026-09-25")

    def test_summary_includes_failed_attempt_costs_and_keeps_unknown_usage(self):
        rows = [
            {"arm": "semaprax", "number": 1, "status": "accepted", "elapsed_seconds": 10,
             "list_price_estimate_usd": 0.2, "provider_input_plus_cache_tokens_gross": 150,
             "provider_input_plus_cache_tokens_after_context_proxy": 100,
             "authored_source_metrics": {"total_tokens": 75},
             "observed": {"usage": {"input_tokens": 100}, "turns_with_usage": 2}},
            {"arm": "semaprax", "number": 2, "status": "failed", "elapsed_seconds": 20,
             "list_price_estimate_usd": 0.1, "provider_input_plus_cache_tokens_gross": None,
             "provider_input_plus_cache_tokens_after_context_proxy": None,
             "authored_source_metrics": {"total_tokens": None},
             "observed": {"usage": {"input_tokens": 50}, "turns_with_usage": 1}},
        ]
        calibration = {"inherited_context_input_tokens_proxy": 50, "list_price_estimate_usd": 0.03}
        summary = live_campaign.summarize(rows, calibration)
        arm = summary["arms"][0]
        self.assertEqual(arm["provider_input_plus_cache_tokens_gross_known_subtotal"], 150)
        self.assertEqual(arm["provider_input_plus_cache_tokens_after_context_proxy_known_subtotal"], 100)
        self.assertEqual(arm["authored_source_token_proxy_known_subtotal"], 75)
        self.assertEqual(summary["campaign_list_price_estimate_including_calibration_usd"], 0.33)
        self.assertEqual(summary["campaign_estimated_cost_per_accepted_task_including_calibration_usd"], 0.33)
        summary = arm
        self.assertEqual(summary["provider_usage_totals_known_subtotal"]["input_tokens"], 150)
        self.assertEqual(summary["provider_usage_missing_trial_counts"]["output_tokens"], 2)
        self.assertEqual(summary["aggregate_model_session_wall_seconds"], 30)
        self.assertEqual(summary["estimated_cost_per_accepted_task_usd"], 0.3)
        self.assertTrue(summary["list_price_estimate_complete"])

    def test_campaign_plan_requires_five_trials_and_external_artifacts(self):
        args = Namespace(
            repo=str(live_campaign.REPO),
            base_ref="HEAD",
            artifacts=str(live_campaign.REPO / "campaign-artifacts"),
            trials_per_arm=4,
            timeout_seconds=1800,
            max_budget_usd=None,
            model=live_campaign.MODEL,
            effort=live_campaign.EFFORT,
        )
        with self.assertRaisesRegex(ValueError, "outside the repository"):
            live_campaign.plan(args)
        args.artifacts = "/tmp/loglens-campaign-plan-test"
        with self.assertRaisesRegex(ValueError, "at least 5 trials"):
            live_campaign.plan(args)

    def test_independent_acceptance_uses_relative_in_cwd_fixtures_and_covers_spec_edges(self):
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory)
            (candidate / "build.sh").write_text("exit 0\n", encoding="utf-8")
            program = candidate / "run.py"
            program.write_text(
                """import pathlib, sys
from oracle import analyse, json_line, text
args = sys.argv[1:]
if not args:
    print('usage error', file=sys.stderr); raise SystemExit(2)
path = args.pop(0)
top, as_json = 5, False
while args:
    item = args.pop(0)
    if item == '--json': as_json = True
    elif item == '--top' and args:
        try: top = int(args.pop(0))
        except ValueError: print('usage error', file=sys.stderr); raise SystemExit(2)
    else: print('usage error', file=sys.stderr); raise SystemExit(2)
    if item == '--top' and not 1 <= top <= 50:
        print('usage error', file=sys.stderr); raise SystemExit(2)
try: content = pathlib.Path(path).read_text()
except OSError:
    print('cannot read file', file=sys.stderr); raise SystemExit(1)
report = analyse(content, top)
sys.stdout.write(json_line(report) if as_json else text(report))
""",
                encoding="utf-8",
            )
            (candidate / "run.sh").write_text(
                'exec python3 "$(dirname "$0")/run.py" "$@"\n', encoding="utf-8"
            )
            result = live_campaign.check_program(
                candidate,
                5,
                {**os.environ, "PYTHONPATH": str(live_campaign.BENCHMARK)},
            )
        self.assertTrue(result["accepted"], result)
        self.assertEqual(len(result["checks"]), 32)
        self.assertTrue(all(row["status"] == "passed" for row in result["checks"]))


if __name__ == "__main__":
    unittest.main()
