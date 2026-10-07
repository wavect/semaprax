import hashlib
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
    def test_claude_command_ends_options_before_prompt(self):
        settings = {
            "model": live_campaign.MODEL,
            "effort": live_campaign.EFFORT,
            "max_budget_usd": 3.5,
        }
        prompt = "Implement the benchmark application."
        command = live_campaign.claude_command(settings, prompt)
        self.assertEqual(command[-2:], ["--", prompt])
        self.assertLess(command.index("--allowedTools"), command.index("--"))
        self.assertEqual(command[command.index("--allowedTools") + 1], "Bash,Read,Edit,Write,Glob,Grep")
        self.assertEqual(command[command.index("--max-budget-usd") + 1], "3.5")

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
                    "usage": {"input_tokens": 40, "cache_creation_input_tokens": 0,
                              "cache_read_input_tokens": 0, "output_tokens": 10},
                    "content": [],
                },
            },
            {"type": "result", "subtype": "success", "usage": {
                "input_tokens": 160,
                "cache_creation_input_tokens": 10,
                "cache_creation": {
                    "ephemeral_5m_input_tokens": 6,
                    "ephemeral_1h_input_tokens": 4,
                },
                "cache_read_input_tokens": 30,
                "output_tokens": 34,
            }, "total_cost_usd": 0.0042},
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "stream.jsonl"
            path.write_text("\n".join(json.dumps(event) for event in events) + "\n")
            result = live_campaign.stream_usage(path)
        self.assertEqual(result["models_observed"], [live_campaign.MODEL])
        self.assertEqual(result["turns_with_usage"], 2)
        self.assertEqual(result["usage"]["input_tokens"], 160)
        self.assertEqual(result["usage"]["cache_creation_input_tokens"], 10)
        self.assertEqual(result["usage"]["cache_creation_ephemeral_5m_input_tokens"], 6)
        self.assertEqual(result["usage"]["cache_creation_ephemeral_1h_input_tokens"], 4)
        self.assertEqual(result["provider_reported_api_equivalent_total_cost_usd"], 0.0042)
        self.assertIn("not a receipt", result["provider_reported_api_equivalent_cost_note"])
        self.assertEqual(result["first_turn_usage"]["input_tokens"], 120)
        self.assertEqual(result["result_event"]["subtype"], "success")
        self.assertEqual(result["usage_updates_per_message"]["turn-1"], 2)
        self.assertEqual(result["usage_discrepancies"]["output_tokens"], {
            "per_turn_sum": 35, "final_result": 34,
        })
        self.assertEqual(result["usage"]["output_tokens"], 34)
        self.assertEqual(result["legacy_net_input"], {
            "first_turn_input_plus_cache_tokens": 160,
            "per_turn_input_plus_cache_tokens_sum": 200,
            "baseline_tokens_subtracted": 320,
            "net_input_tokens": -120,
        })

    def test_stream_usage_preserves_provider_cache_ttls_and_rejects_inconsistent_split(self):
        def read_result(cache_creation: dict[str, int]) -> dict:
            events = [{
                "type": "result",
                "usage": {
                    "input_tokens": 2,
                    "cache_creation_input_tokens": 1948,
                    "cache_creation": cache_creation,
                    "cache_read_input_tokens": 4773,
                    "output_tokens": 4,
                },
                "modelUsage": {live_campaign.MODEL: {
                    "inputTokens": 2,
                    "cacheCreationInputTokens": 1948,
                    "cacheReadInputTokens": 4773,
                    "outputTokens": 4,
                }},
            }]
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "stream.jsonl"
                path.write_text("\n".join(json.dumps(event) for event in events) + "\n")
                return live_campaign.stream_usage(path)

        consistent = read_result({"ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 1948})
        self.assertEqual(consistent["usage"]["cache_creation_ephemeral_5m_input_tokens"], 0)
        self.assertEqual(consistent["usage"]["cache_creation_ephemeral_1h_input_tokens"], 1948)
        estimate = live_campaign.rate_card_estimate_details(consistent["usage"])
        self.assertEqual(estimate["cache_write_pricing"]["basis"], "provider_ttl_breakdown")
        self.assertEqual(estimate["usd"], 0.008791)

        inconsistent = read_result({"ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 1947})
        self.assertEqual(inconsistent["usage"]["cache_creation_ephemeral_1h_input_tokens"], 1947)
        rejected = live_campaign.rate_card_estimate_details(inconsistent["usage"])
        self.assertEqual(rejected["cache_write_pricing"]["basis"], "inconsistent_provider_ttl_breakdown")
        self.assertIsNone(rejected["usd"])

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

    def test_calibration_context_proxy_is_one_turn_diagnostic_only(self):
        first_turn = {
            "input_tokens": 100,
            "cache_creation_input_tokens": 20,
            "cache_read_input_tokens": 30,
            "output_tokens": 8,
        }
        self.assertEqual(live_campaign.input_tokens_total(first_turn), 150)
        self.assertEqual(live_campaign.one_turn_context_proxy(first_turn, 12), 138)
        self.assertIsNone(live_campaign.one_turn_context_proxy({"input_tokens": 100}, 12))
        self.assertIsNone(live_campaign.one_turn_context_proxy(first_turn, None))
        self.assertIsNone(live_campaign.one_turn_context_proxy(first_turn, 151))

    def test_legacy_net_input_reproduces_first_turn_subtraction_and_preserves_negative(self):
        first = {
            "input_tokens": 100, "cache_creation_input_tokens": 20,
            "cache_read_input_tokens": 30, "output_tokens": 4,
        }
        second = {
            "input_tokens": 120, "cache_creation_input_tokens": 10,
            "cache_read_input_tokens": 40, "output_tokens": 5,
        }
        result = live_campaign.legacy_net_input_metrics([first, second])
        self.assertEqual(result, {
            "first_turn_input_plus_cache_tokens": 150,
            "per_turn_input_plus_cache_tokens_sum": 320,
            "baseline_tokens_subtracted": 300,
            "net_input_tokens": 20,
        })
        lower_second_turn = {**second, "input_tokens": 10, "cache_creation_input_tokens": 0,
                             "cache_read_input_tokens": 0}
        negative = live_campaign.legacy_net_input_metrics([first, lower_second_turn])
        self.assertEqual(negative["net_input_tokens"], -140)

    def test_legacy_net_input_is_unknown_when_any_turn_bucket_is_missing(self):
        first = {
            "input_tokens": 100, "cache_creation_input_tokens": 20,
            "cache_read_input_tokens": 30, "output_tokens": 4,
        }
        second = {
            "input_tokens": 120, "cache_creation_input_tokens": 10,
            "cache_read_input_tokens": None, "output_tokens": 5,
        }
        result = live_campaign.legacy_net_input_metrics([first, second])
        self.assertEqual(result["first_turn_input_plus_cache_tokens"], 150)
        self.assertIsNone(result["per_turn_input_plus_cache_tokens_sum"])
        self.assertEqual(result["baseline_tokens_subtracted"], 300)
        self.assertIsNone(result["net_input_tokens"])

    def test_model_identity_uses_observed_message_id_not_requested_alias_or_usage_key(self):
        dated_model = "claude-sonnet-4-5-20260929"
        events = [
            {"type": "assistant", "message": {
                "id": "turn-1", "model": dated_model, "usage": {"input_tokens": 5},
            }},
            {"type": "result", "modelUsage": {"claude-sonnet-4-5": {"inputTokens": 5}}},
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "stream.jsonl"
            path.write_text("\n".join(json.dumps(event) for event in events) + "\n")
            result = live_campaign.stream_usage(path)
        self.assertEqual(result["models_observed"], [dated_model])
        self.assertEqual(result["model_usage_keys_observed"], ["claude-sonnet-4-5"])
        self.assertTrue(live_campaign.observed_model_matches(result["models_observed"], dated_model))
        self.assertFalse(live_campaign.observed_model_matches(result["models_observed"], "claude-sonnet-4-5"))
        self.assertFalse(live_campaign.observed_model_matches([dated_model, "other"], dated_model))

    def test_seed_repository_and_worktree_reveal_only_pinned_public_inputs(self):
        source_repo = live_campaign.REPO
        source_commit = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=source_repo, text=True, capture_output=True, check=True
        ).stdout.strip()
        with tempfile.TemporaryDirectory() as directory:
            artifacts = Path(directory) / "artifacts"
            artifacts.mkdir()
            seed_repo = artifacts / "seed-repository"
            seed_info = live_campaign.create_seed_repository(source_repo, source_commit, seed_repo)
            seed_commit = seed_info["seed_repository_commit"]
            workspace = artifacts / "worktrees" / "inventory-test"
            error = live_campaign.add_seed_worktree(seed_repo, workspace, seed_commit)
            self.assertIsNone(error, error)
            expected_files = sorted(path.lstrip("/") for path in live_campaign.SEED_FILES)

            def git(*arguments):
                return subprocess.run(
                    ["git", *arguments], cwd=workspace, text=True, capture_output=True, check=False
                )

            self.assertEqual(git("ls-tree", "-r", "--name-only", "HEAD").stdout.splitlines(), expected_files)
            self.assertEqual(git("read-tree", "HEAD").returncode, 0)
            self.assertEqual(git("ls-files").stdout.splitlines(), expected_files)
            self.assertEqual(git("rev-list", "--count", "HEAD").stdout.strip(), "1")
            self.assertEqual(git("log", "--all", "--format=%H").stdout.splitlines(), [seed_commit])
            self.assertNotEqual(git("cat-file", "-e", source_commit).returncode, 0)
            self.assertNotEqual(git("show", "HEAD:benchmarks/cli-tokens-v1/oracle.py").returncode, 0)
            self.assertNotEqual(git("show", "HEAD:benchmarks/webapp-tokens-v2/README.md").returncode, 0)
            self.assertNotEqual(git(
                "show", f"{source_commit}:benchmarks/cli-tokens-v1/oracle.py"
            ).returncode, 0)
            self.assertNotEqual(git(
                "show", f"{source_commit}:benchmarks/cli-tokens-v1/candidate/run.sh"
            ).returncode, 0)
            self.assertEqual(seed_info["source_repository_commit"], source_commit)
            self.assertEqual(seed_info["source_files_sha256"], seed_info["seed_files_sha256"])
            subprocess.run(["git", "worktree", "remove", "--force", str(workspace)],
                           cwd=seed_repo, check=True, capture_output=True)

    def test_calibration_runner_uses_minimal_seed_checkout_and_removes_clean_worktree(self):
        source_repo = live_campaign.REPO
        source_commit = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=source_repo, text=True, capture_output=True, check=True
        ).stdout.strip()
        with tempfile.TemporaryDirectory() as directory:
            artifacts = Path(directory) / "artifacts"
            artifacts.mkdir()
            seed_repo = artifacts / "seed-repository"
            seed_info = live_campaign.create_seed_repository(source_repo, source_commit, seed_repo)
            seed_commit = seed_info["seed_repository_commit"]
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
                ), sorted(path.lstrip("/") for path in live_campaign.SEED_FILES))
                self.assertEqual(subprocess.run(
                    ["git", "ls-files"], cwd=cwd, text=True, capture_output=True, check=True
                ).stdout.splitlines(), sorted(path.lstrip("/") for path in live_campaign.SEED_FILES))
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
                        seed_repo, artifacts, seed_commit, settings, Path("/bin/true")
                    )
                self.assertEqual(row["status"], "ready")
                self.assertEqual(row["first_turn_provider_input_plus_cache_tokens"], 150)
                self.assertIsNone(row["one_turn_context_input_tokens_proxy"])
                self.assertEqual(row["observed_model_id"], live_campaign.MODEL)
                self.assertTrue(row["workspace_removed"])
                self.assertFalse(workspace.exists())
            finally:
                if workspace.exists():
                    subprocess.run(
                        ["git", "worktree", "remove", "--force", str(workspace)],
                        cwd=seed_repo, text=True, capture_output=True, check=False,
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
        usage = {
            "input_tokens": 1_000_000,
            "cache_creation_input_tokens": 1_000_000,
            "cache_read_input_tokens": 1_000_000,
            "output_tokens": 1_000_000,
        }
        estimate = live_campaign.rate_card_estimate(usage)
        self.assertEqual(estimate, 14.7)
        details = live_campaign.rate_card_estimate_details(usage)
        self.assertEqual(details["cache_write_pricing"]["basis"], "assumed_all_cache_writes_5m")
        self.assertEqual(live_campaign.PRICE_BOOK_DATE, "2026-10-07")
        self.assertEqual(live_campaign.PRICE_USD_PER_MTOK["cache_write_1h"], 4.0)

    def test_rate_card_prices_reported_cache_ttls_at_distinct_rates(self):
        usage = {
            "input_tokens": 1_000_000,
            "cache_creation_input_tokens": 1_000_000,
            "cache_creation_ephemeral_5m_input_tokens": 600_000,
            "cache_creation_ephemeral_1h_input_tokens": 400_000,
            "cache_read_input_tokens": 1_000_000,
            "output_tokens": 1_000_000,
        }
        details = live_campaign.rate_card_estimate_details(usage)
        self.assertEqual(details["usd"], 15.3)
        self.assertEqual(details["cache_write_pricing"], {
            "basis": "provider_ttl_breakdown",
            "five_minute_tokens": 600_000,
            "one_hour_tokens": 400_000,
        })

    def test_summary_includes_failed_attempt_costs_and_keeps_unknown_usage(self):
        rows = [
            {"arm": "semaprax", "number": 1, "status": "accepted", "elapsed_seconds": 10,
             "list_price_estimate_usd": 0.2, "provider_input_plus_cache_tokens_raw": 150,
             "legacy_net_input_tokens": 20, "legacy_net_first_turn_input_plus_cache_tokens": 150,
             "legacy_net_baseline_tokens_subtracted": 300,
             "final_candidate_source_metrics": {"total_tokens": 75},
             "observed": {"usage": {"input_tokens": 100}, "turns_with_usage": 2}},
            {"arm": "semaprax", "number": 2, "status": "failed", "elapsed_seconds": 20,
             "list_price_estimate_usd": 0.1, "provider_input_plus_cache_tokens_raw": None,
             "legacy_net_input_tokens": None,
             "legacy_net_first_turn_input_plus_cache_tokens": None,
             "legacy_net_baseline_tokens_subtracted": None,
             "final_candidate_source_metrics": {"total_tokens": None},
             "observed": {"usage": {"input_tokens": 50}, "turns_with_usage": 1}},
        ]
        calibration = {"one_turn_context_input_tokens_proxy": 50, "list_price_estimate_usd": 0.03}
        summary = live_campaign.summarize(rows, calibration)
        arm = summary["arms"][0]
        self.assertEqual(arm["provider_input_plus_cache_tokens_raw_known_subtotal"], 150)
        self.assertEqual(arm["legacy_net_input_tokens_per_trial"], [20, None])
        self.assertEqual(arm["legacy_net_input_tokens_known_subtotal"], 20)
        self.assertEqual(arm["legacy_net_input_tokens_incomplete_trials"], 1)
        self.assertFalse(arm["context_baseline_applied_to_trial_totals"])
        self.assertNotIn("provider_input_plus_cache_tokens_after_context_proxy_known_subtotal", arm)
        self.assertEqual(arm["final_candidate_source_token_proxy_known_subtotal"], 75)
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
            (candidate / "test.sh").write_text(
                "#!/bin/sh\nprintf 'goldens and status tests passed\\n'\n", encoding="utf-8"
            )
            result = live_campaign.check_program(
                candidate,
                5,
                {**os.environ, "PYTHONPATH": str(live_campaign.BENCHMARK)},
            )
        self.assertTrue(result["accepted"], result)
        self.assertEqual(result["candidate_tests"]["status"], "passed")
        self.assertIn("goldens and status tests passed", result["candidate_tests"]["stdout"])
        self.assertEqual(len(result["checks"]), 33)
        self.assertTrue(all(row["status"] == "passed" for row in result["checks"]))

    def test_candidate_acceptance_requires_test_script_and_retains_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory)
            (candidate / "build.sh").write_text("exit 0\n", encoding="utf-8")
            (candidate / "run.sh").write_text("exit 0\n", encoding="utf-8")
            missing = live_campaign.check_program(candidate, 5, os.environ.copy())
            self.assertFalse(missing["accepted"])
            self.assertEqual(missing["candidate_tests"]["status"], "missing")
            self.assertEqual(missing["checks"], [])

            (candidate / "test.sh").write_text(
                "#!/bin/sh\nprintf 'self test failed\\n' >&2\nexit 7\n",
                encoding="utf-8",
            )
            failed = live_campaign.check_program(candidate, 5, os.environ.copy())
        self.assertFalse(failed["accepted"])
        self.assertEqual(failed["candidate_tests"]["status"], "failed")
        self.assertEqual(failed["candidate_tests"]["exit_code"], 7)
        self.assertIn("self test failed", failed["candidate_tests"]["stderr"])
        self.assertEqual(failed["checks"], [])

    def test_candidate_archive_excludes_only_known_caches_and_hashes_preserved_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate = root / "candidate"
            archive = root / "archive"
            (candidate / "nested" / "node_modules" / "pkg").mkdir(parents=True)
            (candidate / ".cache" / "compiler").mkdir(parents=True)
            (candidate / "__pycache__").mkdir()
            (candidate / ".pytest_cache" / "v" / "cache").mkdir(parents=True)
            (candidate / "target").mkdir()
            (candidate / "dist").mkdir()
            (candidate / "generated").mkdir()
            authored = {
                "main.spx": "fn main() {}\n",
                "build.sh": "#!/bin/sh\n",
                "run.sh": "#!/bin/sh\n",
                "pnpm-lock.yaml": "lockfileVersion: '9.0'\n",
                "manual.wasm": "handwritten binary payload\n",
                "parser.rs": "// authored implementation\n",
                "target/keep.rs": "// preserve until provenance is known\n",
                "dist/keep.ts": "// do not drop by directory name alone\n",
                "generated/keep.spx": "// generated-looking path, retain safely\n",
            }
            for relative, content in authored.items():
                path = candidate / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
            (candidate / "nested" / "node_modules" / "pkg" / "index.js").write_text("dependency\n")
            (candidate / ".cache" / "compiler" / "artifact").write_text("cache\n")
            (candidate / "__pycache__" / "module.pyc").write_text("cache\n")
            (candidate / ".pytest_cache" / "v" / "cache" / "nodeids").write_text("cache\n")

            file_hashes, excluded = live_campaign.archive_candidate(candidate, archive)

        self.assertEqual(excluded, [".cache", ".pytest_cache", "__pycache__", "nested/node_modules"])
        self.assertEqual(set(file_hashes), set(authored))
        self.assertEqual(file_hashes["main.spx"], hashlib.sha256(authored["main.spx"].encode()).hexdigest())


if __name__ == "__main__":
    unittest.main()
