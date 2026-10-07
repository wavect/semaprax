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
import measurement_evidence


class LiveCampaignTests(unittest.TestCase):
    def test_provider_session_turns_remain_separate_and_unknown_when_invalid(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "stream.jsonl"
            for supplied, expected in [(0, 0), (7, 7), (None, None), (-1, None),
                                       (True, None), ("7", None)]:
                path.write_text(json.dumps({"type": "result", "num_turns": supplied}) + "\n")
                observed = live_campaign.stream_usage(path)
                self.assertEqual(observed["provider_reported_session_turns"], expected)
                self.assertEqual(observed["turns_with_usage"], 0)

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
                    "usage": {"input_tokens": 100, "output_tokens": 20, "thinking_tokens": 3},
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
        self.assertEqual(
            [row["message_id"] for row in result["turn_usage_by_message"]],
            ["turn-1", "turn-2"],
        )
        self.assertEqual(result["turn_usage_by_message"][0]["usage"]["output_tokens"], 25)
        self.assertEqual(result["turn_usage_by_message"][0]["usage"]["thinking_tokens"], 3)

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

    def _measurement_binding(self, root):
        campaign = root / "campaign.json"
        results = root / "results.json"
        transcript = root / "transcripts" / "semaprax-01.jsonl"
        prompt = "task prompt"
        campaign.write_text("{}\n", encoding="utf-8")
        results.write_text("{}\n", encoding="utf-8")
        transcript.parent.mkdir(parents=True)
        transcript.write_text("{\"type\":\"assistant\"}\n", encoding="utf-8")
        return {
            "campaign_sha256": live_campaign.digest(campaign),
            "results_sha256": live_campaign.digest(results),
            "trial_id": "semaprax-01",
            "arm": "semaprax",
            "number": 1,
            "model_id": live_campaign.MODEL,
            "prompt_sha256": live_campaign.sha_text(prompt),
            "transcript_sha256": live_campaign.digest(transcript),
        }

    def _write_receipt(self, root, binding, amount="0.1250", document=b"provider export bytes"):
        folder = root / "provider-receipts"
        folder.mkdir(exist_ok=True)
        document_path = folder / "source.pdf"
        document_path.write_bytes(document)
        sidecar = {
            "schema": measurement_evidence.RECEIPT_SCHEMA,
            "binding": binding,
            "provenance": {
                "provider": "anthropic",
                "source_kind": "caller_supplied_provider_export",
                "reference": "invoice-row-7",
                "document_path": "provider-receipts/source.pdf",
                "document_sha256": hashlib.sha256(document).hexdigest(),
            },
            "billed": {"currency": "USD", "amount_decimal": amount},
        }
        path = folder / "semaprax-01.json"
        path.write_text(json.dumps(sidecar), encoding="utf-8")
        return path

    def test_receipt_import_binds_exact_trial_bytes_but_never_claims_provider_authentication(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binding = self._measurement_binding(root)
            sidecar = self._write_receipt(root, binding)
            imported = measurement_evidence._receipt(root, sidecar, binding)
        self.assertEqual(imported["reported_billed_usd"], "0.1250")
        self.assertEqual(imported["status"], "bound_caller_supplied_origin_unverified")
        self.assertFalse(imported["provider_origin_verified"])
        self.assertIsNone(imported["actual_billed_usd"])

    def test_measurement_sidecars_support_symlinked_artifact_root(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = base / "artifact-alias"
            (base / "artifacts").mkdir()
            root.symlink_to(base / "artifacts", target_is_directory=True)
            binding = self._measurement_binding(root)
            receipt = self._write_receipt(root, binding)
            imported_receipt = measurement_evidence._receipt(root, receipt, binding)
            trace, turns = self._trace_fixture(root, binding)
            imported_trace = measurement_evidence._trace(root, trace, binding, turns)
        self.assertEqual(imported_receipt["sidecar_path"], "provider-receipts/semaprax-01.json")
        self.assertEqual(imported_trace["sidecar_path"], "trace.json")

    def test_receipt_import_rejects_binding_document_and_decimal_drift(self):
        for mutation in ("binding", "document", "decimal", "path"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                binding = self._measurement_binding(root)
                sidecar = self._write_receipt(root, binding, amount="0.1250")
                value = json.loads(sidecar.read_text(encoding="utf-8"))
                if mutation == "binding":
                    value["binding"]["results_sha256"] = "0" * 64
                elif mutation == "document":
                    (root / "provider-receipts" / "source.pdf").write_bytes(b"changed")
                elif mutation == "decimal":
                    value["billed"]["amount_decimal"] = True
                else:
                    value["provenance"]["document_path"] = "../outside.pdf"
                sidecar.write_text(json.dumps(value), encoding="utf-8")
                with self.assertRaises(ValueError):
                    measurement_evidence._receipt(root, sidecar, binding)

    def _trace_fixture(self, root, binding, schema_change=False, missing_bucket=False):
        root.mkdir(parents=True, exist_ok=True)
        turns = [
            {"message_id": "turn-1", "usage": {
                "input_tokens": 10, "cache_creation_input_tokens": 2,
                "cache_read_input_tokens": 3, "output_tokens": 5,
            }},
            {"message_id": "turn-2", "usage": {
                "input_tokens": 4, "cache_creation_input_tokens": 1,
                "cache_read_input_tokens": 5, "output_tokens": 7,
            }},
        ]
        rows = []
        for index, observed in enumerate(turns):
            composition = {
                "system_tokens": 2,
                "tool_schema_tokens": 3,
                "task_prompt_tokens": 4,
                "conversation_history_tokens": 6 if index == 0 else 1,
            }
            # The second turn has smaller explicit composition counts so each
            # bucket still totals its raw input plus cache counters.
            if index == 1:
                composition.update({
                    "system_tokens": 1,
                    "tool_schema_tokens": 2,
                    "task_prompt_tokens": 1,
                    "conversation_history_tokens": 6,
                })
            if missing_bucket and index == 1:
                composition["tool_schema_tokens"] = None
            rows.append({
                "message_id": observed["message_id"],
                "request_id": f"provider-request-{index + 1}",
                "system_prompt_sha256": "1" * 64,
                "tool_schema_sha256": ("2" if not schema_change or index == 0 else "3") * 64,
                "task_prompt_sha256": binding["prompt_sha256"],
                "composition": composition,
                "thinking_output_tokens": 1 if index == 0 else None,
            })
        value = {
            "schema": measurement_evidence.TRACE_SCHEMA,
            "binding": binding,
            "provenance": {
                "provider": "anthropic",
                "source_kind": "caller_supplied_request_trace",
                "reference": "request-dump-1",
            },
            "turns": rows,
        }
        path = root / "trace.json"
        path.write_text(json.dumps(value), encoding="utf-8")
        return path, turns

    def test_request_trace_records_complete_composition_and_raw_counters_separately(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binding = self._measurement_binding(root)
            path, turns = self._trace_fixture(root, binding)
            imported = measurement_evidence._trace(root, path, binding, turns)
        self.assertEqual(imported["status"], "complete")
        self.assertEqual(imported["complete_composition_totals"], {
            "system_tokens": 3, "tool_schema_tokens": 5,
            "task_prompt_tokens": 5, "conversation_history_tokens": 12,
            "fixed_harness_context_tokens": 8,
        })
        self.assertEqual(imported["turns"][0]["raw_provider_usage"]["cache_read_input_tokens"], 3)
        self.assertEqual(imported["turns"][0]["thinking_output_tokens_reported_by_trace"], 1)
        self.assertFalse(imported["provider_origin_verified"])

    def test_request_trace_missing_bucket_and_changed_schema_preserve_raw_but_fail_closed(self):
        for change, expected in (("missing", "incomplete"), ("schema", "schema_drift")):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                binding = self._measurement_binding(root)
                path, turns = self._trace_fixture(
                    root, binding, schema_change=change == "schema", missing_bucket=change == "missing",
                )
                imported = measurement_evidence._trace(root, path, binding, turns)
            self.assertEqual(imported["status"], expected)
            self.assertIsNone(imported["complete_composition_totals"]["system_tokens"])
            self.assertEqual(imported["turns"][0]["raw_provider_usage"]["output_tokens"], 5)
            self.assertEqual(imported["turns"][1]["raw_provider_usage"]["cache_creation_input_tokens"], 1)

    def test_request_trace_missing_provider_bucket_keeps_null_and_cannot_complete(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binding = self._measurement_binding(root)
            path, turns = self._trace_fixture(root, binding)
            turns[1]["usage"]["cache_read_input_tokens"] = None
            imported = measurement_evidence._trace(root, path, binding, turns)
        self.assertEqual(imported["status"], "incomplete")
        self.assertIsNone(imported["turns"][1]["raw_provider_usage"]["cache_read_input_tokens"])
        self.assertIsNone(imported["complete_composition_totals"]["task_prompt_tokens"])

    def test_request_trace_rejects_task_prompt_and_bucket_sum_drift(self):
        for mutation in ("prompt", "sum"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                binding = self._measurement_binding(root)
                path, turns = self._trace_fixture(root, binding)
                value = json.loads(path.read_text(encoding="utf-8"))
                if mutation == "prompt":
                    value["turns"][0]["task_prompt_sha256"] = "0" * 64
                else:
                    value["turns"][0]["composition"]["system_tokens"] += 1
                path.write_text(json.dumps(value), encoding="utf-8")
                with self.assertRaises(ValueError):
                    measurement_evidence._trace(root, path, binding, turns)

    def test_trial_import_uses_optional_conventional_sidecars_and_leaves_absence_unknown(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binding = self._measurement_binding(root)
            row = {"status": "accepted", "observed": {"turn_usage_by_message": []}}
            measurement_evidence.attach_trial(row, root, binding)
            self.assertEqual(row["provider_receipt_evidence"]["status"], "missing")
            self.assertIsNone(row["provider_receipt_reported_billed_usd"])
            self.assertEqual(row["request_context_trace"]["status"], "missing")

            self._write_receipt(root, binding)
            trace_root = root / "request-context-traces"
            trace_root.mkdir()
            trace_binding = binding
            path, turns = self._trace_fixture(root, trace_binding)
            (trace_root / "semaprax-01.json").write_bytes(path.read_bytes())
            row["observed"]["turn_usage_by_message"] = turns
            measurement_evidence.attach_trial(row, root, binding)
        self.assertEqual(row["provider_receipt_reported_billed_usd"], "0.1250")
        self.assertEqual(row["request_context_trace"]["status"], "complete")
        self.assertIsNone(row["provider_receipt_actual_usd"])

    def test_reported_receipt_cost_requires_five_complete_attempts_and_keeps_actual_null(self):
        rows = [
            {"status": "accepted" if index < 2 else "failed",
             "provider_receipt_reported_billed_usd": f"0.0{index + 1}",
             "provider_receipt_evidence": {"provider_origin_verified": False},
             "request_context_trace": {"status": "missing"}}
            for index in range(5)
        ]
        result = measurement_evidence.arm_summary(rows)
        self.assertTrue(result["provider_receipt_reported_billed_usd_complete"])
        self.assertEqual(result["provider_receipt_reported_billed_usd_known_subtotal"], "0.15")
        self.assertEqual(result["reported_receipt_cost_per_accepted_task_usd"], "0.075")
        self.assertIsNone(result["provider_receipt_actual_usd"])
        missing = measurement_evidence.arm_summary(rows[:-1])
        self.assertFalse(missing["provider_receipt_reported_billed_usd_complete"])
        self.assertIsNone(missing["reported_receipt_cost_per_accepted_task_usd"])

    def test_offline_recount_refreshes_only_accounting_from_saved_transcripts(self):
        with tempfile.TemporaryDirectory() as directory:
            artifacts = Path(directory)
            transcripts = artifacts / "transcripts"
            transcripts.mkdir()
            event = {
                "type": "result",
                "usage": {
                    "input_tokens": 2,
                    "cache_creation_input_tokens": 1948,
                    "cache_creation": {
                        "ephemeral_5m_input_tokens": 0,
                        "ephemeral_1h_input_tokens": 1948,
                    },
                    "cache_read_input_tokens": 4773,
                    "output_tokens": 4,
                },
                "modelUsage": {live_campaign.MODEL: {
                    "inputTokens": 2,
                    "cacheCreationInputTokens": 1948,
                    "cacheReadInputTokens": 4773,
                    "outputTokens": 4,
                }},
                "total_cost_usd": 0.0087906,
            }
            (transcripts / "calibration.jsonl").write_text(json.dumps(event) + "\n")
            (transcripts / "semaprax-01.jsonl").write_text(json.dumps(event) + "\n")
            (artifacts / "calibration.json").write_text(json.dumps({
                "status": "ready",
                "transcript": "transcripts/calibration.jsonl",
                "calibration_prompt_tokens_legacy_proxy": 10,
                "observed": {"usage": {"cache_creation_ephemeral_1h_input_tokens": None}},
                "list_price_estimate_usd": 0.005869,
            }))
            trials = [{
                "arm": "semaprax", "number": 1, "status": "not_accepted",
                "elapsed_seconds": 42.5, "transcript": "transcripts/semaprax-01.jsonl",
                "acceptance": {"accepted": False, "checks": [
                    {"name": "checked-in-sample-text", "status": "passed"},
                    {"name": "spec-sample-top3-json", "status": "passed"},
                    {"name": "options-json-before-top", "status": "passed"},
                    {"name": "crlf-line-endings-text", "status": "failed"},
                    {"name": "crlf-line-endings-json", "status": "failed"},
                    {"name": "cr-line-endings-text", "status": "failed"},
                    {"name": "cr-line-endings-json", "status": "failed"},
                ]},
                "candidate_archive": "/campaign/candidates/semaprax-01",
                "list_price_estimate_usd": 0.005869,
            }, {
                "arm": "typescript", "number": 1, "status": "failed",
                "elapsed_seconds": 3.0, "failure": "checkout failed",
            }]
            source = {
                "campaign": {
                    "attempt_denominator": 10,
                    "calibration_result": {"list_price_estimate_usd": 0.005869},
                },
                "calibration": {}, "trials": trials,
                "summary": {"stale": True}, "campaign_elapsed_wall_seconds": 50.0,
            }
            source_path = artifacts / "results.json"
            source_path.write_text(json.dumps(source))
            original_bytes = source_path.read_bytes()

            with self.assertRaisesRegex(ValueError, "does not match campaign round"):
                live_campaign.recount_results(artifacts, expected_round=4)

            output = live_campaign.recount_results(artifacts)
            report = json.loads(output.read_text())
            self.assertEqual(source_path.read_bytes(), original_bytes)

        self.assertEqual(report["accounting"]["schema"], "semaprax.cli-tokens.accounting.v1")
        self.assertEqual(report["accounting"]["usage_source"], "saved Claude Code stream-json transcripts")
        self.assertEqual(report["calibration"]["observed"]["usage"]["cache_creation_ephemeral_1h_input_tokens"], 1948)
        self.assertEqual(report["calibration"]["list_price_estimate_usd"], 0.008791)
        accepted, missing = report["trials"]
        self.assertEqual(accepted["list_price_estimate_usd"], 0.008791)
        self.assertEqual(accepted["acceptance"], trials[0]["acceptance"])
        self.assertEqual(accepted["elapsed_seconds"], 42.5)
        self.assertEqual(accepted["candidate_archive"], "/campaign/candidates/semaprax-01")
        self.assertEqual(accepted["status"], "not_accepted")
        assessment = accepted["post_run_acceptance_scope_assessment"]
        self.assertFalse(assessment["original_full_corpus_accepted"])
        self.assertEqual(assessment["explicit_spec_checks"]["status"], "passed")
        self.assertEqual(assessment["additional_robustness_checks"]["status"], "failed")
        self.assertEqual(len(assessment["additional_robustness_checks"]["failed"]), 4)
        self.assertEqual(missing["accounting_status"], "transcript_missing")
        self.assertIsNone(missing["list_price_estimate_usd"])
        self.assertEqual(report["campaign_elapsed_wall_seconds"], 50.0)
        self.assertEqual(report["summary"]["attempt_denominator"], 2)
        self.assertEqual(report["accounting"]["planned_attempt_denominator"], 10)
        self.assertFalse(report["accounting"]["campaign_complete"])
        self.assertEqual(
            len(report["acceptance_scope_assessment"]["additional_robustness_checks"]["failed_cases"]),
            4,
        )
        self.assertFalse(report["summary"]["arms"][1]["list_price_estimate_complete"])

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
            round=4,
        )
        with self.assertRaisesRegex(ValueError, "outside the repository"):
            live_campaign.plan(args)
        args.artifacts = "/tmp/loglens-campaign-plan-test"
        with self.assertRaisesRegex(ValueError, "at least 5 trials"):
            live_campaign.plan(args)

    def test_plan_persists_explicit_round_and_rejects_wrong_frozen_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            args = Namespace(
                repo=str(live_campaign.REPO),
                base_ref="HEAD",
                artifacts=str(Path(directory) / "round4"),
                trials_per_arm=5,
                timeout_seconds=1800,
                max_budget_usd=None,
                model=live_campaign.MODEL,
                effort=live_campaign.EFFORT,
                round=4,
            )
            settings = live_campaign.plan(args)
            self.assertEqual(settings["round"], 4)
            self.assertEqual(settings["seed_files_sha256"], live_campaign.ROUND_SEED_SHA256[4])
            args.round = 3
            args.artifacts = str(Path(directory) / "round3")
            with self.assertRaisesRegex(ValueError, "round 3 requires its frozen SPEC/sample identity"):
                live_campaign.plan(args)

    def test_newline_scope_tracks_frozen_round_identity(self):
        checks = [{"name": name, "status": "failed"}
                  for name in sorted(live_campaign.NEWLINE_CHECKS)]
        checks.append({"name": "spec-sample-top3-json", "status": "passed"})
        checks.append({"name": "options-json-before-top", "status": "passed"})
        row = {"acceptance": {"accepted": False, "checks": checks}}
        round3 = live_campaign.acceptance_scope_assessment(row)
        self.assertEqual(len(round3["additional_robustness_checks"]["failed"]), 4)
        self.assertEqual(round3["explicit_spec_checks"]["status"], "passed")
        round4 = live_campaign.acceptance_scope_assessment(
            row, 4, live_campaign.ROUND_SEED_SHA256[4]
        )
        self.assertEqual(round4["additional_robustness_checks"]["failed"], [])
        self.assertEqual(round4["explicit_spec_checks"]["status"], "failed")
        self.assertEqual(len(round4["explicit_spec_checks"]["failed"]), 4)
        self.assertEqual(round4["unclassified_checks"]["names"], [])

    def test_recount_requires_matching_round_four_input_hashes(self):
        with tempfile.TemporaryDirectory() as directory:
            artifacts = Path(directory)
            (artifacts / "calibration.json").write_text(json.dumps({"status": "ready"}))
            source = {
                "campaign": {
                    "round": 4,
                    "seed_files_sha256": live_campaign.ROUND_SEED_SHA256[4],
                    "attempt_denominator": 1,
                },
                "calibration": {},
                "trials": [{"arm": "semaprax", "number": 1, "status": "not_accepted",
                            "acceptance": {"accepted": False, "checks": [
                                {"name": name, "status": "failed"}
                                for name in sorted(live_campaign.NEWLINE_CHECKS)
                            ]}}],
            }
            results_path = artifacts / "results.json"
            results_path.write_text(json.dumps(source))
            output = live_campaign.recount_results(artifacts, expected_round=4)
            report = json.loads(output.read_text())
            scope = report["trials"][0]["post_run_acceptance_scope_assessment"]
            self.assertEqual(len(scope["explicit_spec_checks"]["failed"]), 4)
            self.assertEqual(scope["additional_robustness_checks"]["status"], "not_tested")

            source["campaign"]["seed_files_sha256"] = live_campaign.ROUND_SEED_SHA256[3]
            results_path.write_text(json.dumps(source))
            with self.assertRaisesRegex(ValueError, "round 4 requires its frozen SPEC/sample identity"):
                live_campaign.recount_results(artifacts, expected_round=4)

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

    def _launch_trial_with_workspace_mutation(self, root, mutation_stage, mutated_path=None):
        spec = b"# Public frozen spec\n"
        sample = b"public sample input\n"
        seed_hashes = {
            "benchmarks/cli-tokens-v1/SPEC.md": hashlib.sha256(spec).hexdigest(),
            "benchmarks/cli-tokens-v1/sample.log": hashlib.sha256(sample).hexdigest(),
        }
        workspace = root / "artifacts" / "worktrees" / "semaprax-01"
        artifacts = root / "artifacts"
        settings = {
            "timeout_seconds": 5,
            "model": live_campaign.MODEL,
            "observed_model_id": live_campaign.MODEL,
            "seed_files_sha256": seed_hashes,
            "authored_source_tokenizer": None,
        }

        def add_worktree(_seed, path, _commit):
            (path / "benchmarks/cli-tokens-v1/candidate").mkdir(parents=True)
            (path / "benchmarks/cli-tokens-v1/SPEC.md").write_bytes(spec)
            (path / "benchmarks/cli-tokens-v1/sample.log").write_bytes(sample)
            return None

        def run_claude(_command, _cwd, _env, stream, _stderr, _timeout):
            stream.write_text("{}\n", encoding="utf-8")
            if mutation_stage == "before_acceptance":
                (workspace / mutated_path).write_bytes(b"changed public input\n")
            return {"timed_out": False, "process_exit_code": 0, "elapsed_seconds": 0.1, "failure": None}

        def check_program(_candidate, _timeout, _env):
            if mutation_stage == "during_acceptance" and mutated_path:
                (workspace / mutated_path).write_bytes(b"changed public input\n")
            elif mutation_stage == "outside_candidate":
                (workspace / "unexpected.txt").write_text("outside candidate\n", encoding="utf-8")
            return {"accepted": True}

        def archive_candidate(_candidate, _archive):
            if mutation_stage == "during_archive":
                (workspace / mutated_path).write_bytes(b"changed public input\n")
            return {}, []

        with patch.object(live_campaign, "add_seed_worktree", side_effect=add_worktree), \
             patch.object(live_campaign, "prompt_for", return_value="test prompt"), \
             patch.object(live_campaign, "run_claude", side_effect=run_claude), \
             patch.object(live_campaign, "claude_command", return_value=[]), \
             patch.object(live_campaign, "trial_environment", return_value={}), \
             patch.object(live_campaign, "stream_usage", return_value={
                 "models_observed": [live_campaign.MODEL], "usage": {}, "legacy_net_input": {},
             }), \
             patch.object(live_campaign, "rate_card_estimate_details", return_value={"usd": 0, "cache_write_pricing": {}}), \
             patch.object(live_campaign, "authored_source_metrics", return_value={"status": "ok", "total_tokens": 1, "files": []}), \
             patch.object(live_campaign, "check_program", side_effect=check_program) as check, \
             patch.object(live_campaign, "archive_candidate", side_effect=archive_candidate) as archive, \
             patch.object(live_campaign.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "", "")) as git_remove:
            row = live_campaign.launch_trial(
                root / "seed", artifacts, "seed-commit", {"arm": "semaprax", "number": 1}, settings,
                Path("/bin/true"),
            )
        return row, check, archive, git_remove, workspace

    def test_launch_trial_fails_closed_if_public_input_drifts_before_acceptance(self):
        for relative in ("benchmarks/cli-tokens-v1/SPEC.md", "benchmarks/cli-tokens-v1/sample.log"):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as directory:
                row, check, archive, cleanup, workspace = self._launch_trial_with_workspace_mutation(
                    Path(directory), "before_acceptance", relative,
                )
                self.assertEqual(row["status"], "not_accepted")
                self.assertFalse(row["acceptance"]["accepted"])
                self.assertTrue(row["acceptance_invalidated"])
                self.assertFalse(check.called)
                self.assertTrue(archive.called)
                self.assertIn("candidate_archive", row)
                self.assertFalse(cleanup.called)
                self.assertTrue(row["workspace_retained_for_review"])
                self.assertTrue(workspace.exists())
                self.assertIn(relative, row["workspace_integrity_before_acceptance"]["public_inputs"]["failed_files"])
                self.assertIn("final_candidate_source_metrics", row)

    def test_launch_trial_invalidates_acceptance_if_public_input_drifts_during_acceptance(self):
        for relative in ("benchmarks/cli-tokens-v1/SPEC.md", "benchmarks/cli-tokens-v1/sample.log"):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as directory:
                row, check, archive, cleanup, workspace = self._launch_trial_with_workspace_mutation(
                    Path(directory), "during_acceptance", relative,
                )
                self.assertTrue(check.called)
                self.assertEqual(row["status"], "not_accepted")
                self.assertFalse(row["acceptance"]["accepted"])
                self.assertTrue(row["acceptance_invalidated"])
                self.assertTrue(row["acceptance"]["invalidated"])
                self.assertTrue(row["workspace_retained_for_review"])
                self.assertTrue(workspace.exists())
                self.assertTrue(archive.called)
                self.assertFalse(cleanup.called)

    def test_launch_trial_invalidates_acceptance_and_retains_external_workspace_writes(self):
        with tempfile.TemporaryDirectory() as directory:
            row, check, archive, cleanup, workspace = self._launch_trial_with_workspace_mutation(
                Path(directory), "outside_candidate",
            )
            self.assertTrue(check.called)
            self.assertEqual(row["status"], "not_accepted")
            self.assertFalse(row["acceptance"]["accepted"])
            self.assertTrue(row["acceptance_invalidated"])
            self.assertIn("unexpected.txt", row["workspace_integrity_before_archive"]["external_writes"]["paths"])
            self.assertTrue(row["workspace_retained_for_review"])
            self.assertTrue(workspace.exists())
            self.assertTrue(archive.called)
            self.assertIn("candidate_archive", row)
            self.assertFalse(cleanup.called)

    def test_launch_trial_rechecks_public_inputs_after_archive_before_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            row, check, archive, cleanup, workspace = self._launch_trial_with_workspace_mutation(
                Path(directory), "during_archive", "benchmarks/cli-tokens-v1/sample.log",
            )
            self.assertTrue(check.called)
            self.assertTrue(archive.called)
            self.assertEqual(row["status"], "not_accepted")
            self.assertFalse(row["acceptance"]["accepted"])
            self.assertIn("sample.log", row["failure"])
            self.assertTrue(row["workspace_retained_for_review"])
            self.assertTrue(row["candidate_archive"])
            self.assertTrue(workspace.exists())
            self.assertFalse(cleanup.called)

    def test_launch_trial_accepts_and_cleans_up_when_inputs_and_workspace_are_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            row, check, archive, cleanup, workspace = self._launch_trial_with_workspace_mutation(
                Path(directory), "clean",
            )
        self.assertTrue(check.called)
        self.assertEqual(row["status"], "accepted")
        self.assertTrue(row["workspace_integrity_before_acceptance"]["status"] == "passed")
        self.assertTrue(row["workspace_integrity_before_cleanup"]["status"] == "passed")
        self.assertTrue(archive.called)
        self.assertTrue(cleanup.called)


if __name__ == "__main__":
    unittest.main()
