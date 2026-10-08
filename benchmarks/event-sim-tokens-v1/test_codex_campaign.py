import json
import subprocess
import tempfile
import unittest
from argparse import Namespace
from contextlib import ExitStack
from pathlib import Path
from unittest.mock import patch

import codex_campaign as adapter
import test_campaign_adapter as fixtures


class CodexShiftSimTests(unittest.TestCase):
    def setUp(self):
        # These fixtures fake model/acceptance execution; resource admission is
        # independently exercised with low-space and ENOSPC controls.
        self.enterContext(patch("campaign_resources.snapshot", return_value=[
            {"device": 1, "path": "/fixture", "free_bytes": 10 * 1024**3}]))

    def settings(self, root):
        return {"timeout_seconds": 1800, "model": adapter.MODEL, "effort": adapter.EFFORT,
                "codex_binary": "/fixture/codex", "authored_source_tokenizer": None,
                "qualification": {"scored_trials_allowed": True},
                "seed_files_sha256": {adapter.shiftsim.SPEC_RELATIVE:
                    adapter.shiftsim.common.digest(adapter.BENCHMARK / "SPEC.md")}}

    def seed(self, root):
        result = adapter.shiftsim.common.create_seed_repository(
            adapter.REPO, adapter.shiftsim.resolve_commit(adapter.REPO, "HEAD"),
            root / "seed", adapter.shiftsim.SEED_FILES)
        return root / "seed", result["seed_repository_commit"]

    def process(self, root, mutation=None, exit_code=0, reply="READY", tools=False, reconciled=True):
        """Synthetic model process, real current telemetry parser and task/cwd authentication."""
        sessions = root / "sessions"
        sessions.mkdir(exist_ok=True)
        count = [0]
        def execute(command, workspace, env, stream, stderr, timeout):
            count[0] += 1
            self.assertEqual(command[0], "/fixture/codex")
            self.assertEqual(command[-2], "--")
            self.assertEqual(timeout, 1800)
            self.assertEqual(env["SEMAPRAX_BIN"], str(root / "semaprax"))
            # The real seed helper exposes only SPEC, never corpus/oracle/history.
            self.assertEqual(subprocess.check_output(["git", "ls-files"], cwd=workspace, text=True).strip(),
                             adapter.shiftsim.SPEC_RELATIVE)
            thread = f"00000000-0000-0000-0000-{count[0]:012d}"
            usage = {"input_tokens": 100, "cached_input_tokens": 30,
                     "cache_write_input_tokens": 2, "output_tokens": 7, "reasoning_output_tokens": 3}
            events = [{"type": "thread.started", "thread_id": thread},
                      {"type": "item.completed", "item": {"type": "agent_message", "text": reply}},
                      {"type": "turn.completed", "usage": usage}]
            if tools:
                events.insert(1, {"type": "item.completed", "item": {"type": "command_execution"}})
            stream.write_text("\n".join(json.dumps(event) for event in events) + "\n")
            stderr.write_text("")
            trace_usage = dict(usage)
            if not reconciled:
                trace_usage["input_tokens"] += 1
            records = [{"type": "session_meta", "payload": {"id": thread, "cwd": str(workspace)}},
                       {"type": "turn_context", "payload": {"model": adapter.MODEL, "effort": adapter.EFFORT}},
                       {"type": "token_usage_record", "payload": {"response_id": "r1", "usage": trace_usage}}]
            (sessions / f"rollout-{thread}.jsonl").write_text("\n".join(json.dumps(r) for r in records) + "\n")
            candidate = workspace / "benchmarks/event-sim-tokens-v1/candidate"
            self.assertFalse(candidate.exists(), "candidate leaf must be absent when the model starts")
            if workspace.name != "calibration":
                candidate.mkdir()
                (candidate / "main.ts").write_text("// authored fixture\n")
            if mutation:
                mutation(workspace)
            return {"timed_out": False, "process_exit_code": exit_code, "elapsed_seconds": 0.1}
        return execute, sessions

    def patches(self, stack, root, **kwargs):
        process, sessions = self.process(root, **kwargs)
        stack.enter_context(patch.object(adapter.codex, "run_codex", side_effect=process))
        original = adapter.codex.copy_task_rollout
        stack.enter_context(patch.object(adapter.codex, "copy_task_rollout", side_effect=lambda ids, cwd, dest:
            original(ids, cwd, dest, sessions)))

    def test_plan_replaces_all_provider_metadata_and_binds_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            evidence, _, _, repo = fixtures.ShiftSimCampaignTests()._qualification_evidence(root)
            binary = root / "semaprax"
            binary.write_bytes(b"fixture compiler")
            args = Namespace(repo=str(repo), base_ref="HEAD", compiler_source_ref="HEAD",
                semaprax_bin=str(binary), qualification_evidence=str(evidence), artifacts=str(root / "artifacts"),
                round=2, trials_per_arm=5, model=adapter.MODEL, effort=adapter.EFFORT,
                timeout_seconds=1800, max_budget_usd=None, tokenizer_dir=None, codex_binary="/fixture/codex")
            digest = adapter.shiftsim.common.digest
            with patch.object(adapter.codex, "capabilities", return_value={"status": "ready"}) as caps, \
                 patch.object(adapter.subprocess, "run", wraps=subprocess.run) as run, \
                 patch.object(adapter.shiftsim.common, "digest", side_effect=lambda p: "a" * 64 if p == binary else digest(p)):
                def version_or_real(command, **kwargs):
                    if command == ["/fixture/codex", "--version"]:
                        return subprocess.CompletedProcess(command, 0, "codex fixture-version\n", "")
                    return run._mock_wraps(command, **kwargs)
                run.side_effect = version_or_real
                settings = adapter.plan(args)
            caps.assert_called_once_with("/fixture/codex")
            self.assertEqual(settings["model"], adapter.MODEL)
            self.assertEqual(settings["effort"], adapter.EFFORT)
            self.assertEqual(settings["codex_version"], "codex fixture-version")
            self.assertEqual(settings["codex_execution"]["command_configuration"][0], "/fixture/codex")
            self.assertEqual(settings["price_book"]["standard_short_context_usd_per_million"], adapter.codex.PRICE_USD_PER_MTOK)
            self.assertNotIn("claude", json.dumps(settings))
            self.assertEqual(settings["attempt_denominator"], 10)
            self.assertEqual(settings["benchmark_inputs_sha256"], adapter.shiftsim.FROZEN_BENCHMARK_SHA256)
            self.assertTrue(settings["qualification"]["scored_trials_allowed"])
            self.assertFalse((root / "artifacts").exists())
            with patch.object(adapter.shiftsim, "resolve_commit", side_effect=[settings["repository_commit"], "b" * 40]), \
                 patch.object(adapter.shiftsim.common, "digest", return_value="a" * 64):
                with self.assertRaisesRegex(ValueError, "qualified compiler source"):
                    adapter.plan(args)
            args.max_budget_usd = 1
            with self.assertRaisesRegex(ValueError, "monetary cap"):
                adapter.plan(args)

    def test_mock_trial_uses_current_seed_prompt_usage_archive_and_cleanup_apis(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory).resolve()
            seed, commit = self.seed(root)
            artifacts = root / "artifacts"
            artifacts.mkdir()
            settings = self.settings(root)
            self.patches(stack, root)
            acceptance = stack.enter_context(patch.object(adapter.shiftsim, "check_program", return_value={"accepted": True}))
            row = adapter.launch_trial(seed, artifacts, commit, {"arm": "semaprax", "number": 1}, settings, root / "semaprax")
            self.assertEqual(row["status"], "accepted")
            self.assertTrue(row["observed"]["reconciled"])
            self.assertEqual(row["observed"]["model_request_count"], 1)
            self.assertEqual(row["list_price"]["standard_short_context_api_equivalent_usd"], 0.000214)
            self.assertIsNone(row["provider_receipt_actual_usd"])
            self.assertEqual(row["final_candidate_source_metrics"]["status"], "unmeasured")
            self.assertTrue(row["worktree_removed_after_archive"])
            self.assertFalse(Path(row["workspace"]).exists())
            self.assertEqual(set(row["candidate_source_sha256"]), {"main.ts"})
            self.assertEqual(acceptance.call_args.args[-1], "evidence_gated_scored")
            prompt = (artifacts / "prompts/semaprax-01.txt").read_text()
            self.assertEqual(row["prompt_sha256"], adapter.shiftsim.sha_text(prompt))
            self.assertIn("native Project v24", prompt)
            self.assertIn("language-command-io.stream.v2", prompt)
            self.assertIn("65,536", prompt)
            self.assertIn(str(root / "semaprax"), prompt)
            other = adapter.launch_trial(seed, artifacts, commit, {"arm": "typescript", "number": 1}, settings, root / "semaprax")
            self.assertEqual(other["status"], "accepted")
            self.assertTrue(other["worktree_removed_after_archive"])
            self.assertTrue(other["observed"]["reconciled"])
            other_prompt = (artifacts / "prompts/typescript-01.txt").read_text()
            self.assertEqual(other_prompt, adapter.shiftsim.prompt_for("typescript",
                Path(other["workspace"]) / "benchmarks/event-sim-tokens-v1/candidate", root / "semaprax"))

    def test_external_writes_and_spec_changes_refuse_before_acceptance_or_archive(self):
        for phase in ("model", "acceptance"):
            for path in ("outside.txt", adapter.shiftsim.SPEC_RELATIVE):
                with self.subTest(phase=phase, path=path), tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
                    root = Path(directory).resolve()
                    seed, commit = self.seed(root)
                    artifacts = root / "artifacts"
                    artifacts.mkdir()
                    mutation = lambda workspace: (workspace / path).write_text("changed\n")
                    self.patches(stack, root, mutation=mutation if phase == "model" else None)
                    def check(candidate, *_):
                        mutation(candidate.parents[2])
                        return {"accepted": True}
                    acceptance = stack.enter_context(patch.object(adapter.shiftsim, "check_program", side_effect=check))
                    row = adapter.launch_trial(seed, artifacts, commit, {"arm": "typescript", "number": 1}, self.settings(root), root / "semaprax")
                    self.assertEqual(row["status"], "not_accepted")
                    self.assertTrue(row["workspace_retained_for_review"])
                    self.assertTrue(Path(row["workspace"]).exists())
                    self.assertNotIn("candidate_archive", row)
                    self.assertEqual(acceptance.call_count, 0 if phase == "model" else 1)
                    if phase == "acceptance":
                        self.assertFalse(row["acceptance"]["accepted"])

    def test_unreconciled_usage_and_measurement_failure_are_reported_without_losing_attempt(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory).resolve()
            seed, commit = self.seed(root)
            artifacts = root / "artifacts"
            artifacts.mkdir()
            self.patches(stack, root, reconciled=False)
            acceptance = stack.enter_context(patch.object(adapter.shiftsim, "check_program"))
            stack.enter_context(patch.object(adapter.shiftsim.common, "authored_source_metrics", side_effect=UnicodeError("bad text")))
            row = adapter.launch_trial(seed, artifacts, commit, {"arm": "semaprax", "number": 1}, self.settings(root), root / "semaprax")
            self.assertEqual(row["status"], "failed")
            self.assertFalse(row["telemetry_valid"])
            self.assertIn("unreconciled", row["failure"])
            self.assertEqual(row["final_candidate_source_metrics"]["status"], "measurement_failed")
            self.assertTrue(row["worktree_removed_after_archive"])
            acceptance.assert_not_called()

    def test_calibration_is_separate_tool_free_and_cleans_its_real_worktree(self):
        for kwargs, status in (({}, "ready"), ({"tools": True}, "failed"), ({"reply": "wrong"}, "failed"),
                               ({"reconciled": False}, "failed")):
            with self.subTest(kwargs=kwargs), tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
                root = Path(directory).resolve()
                seed, commit = self.seed(root)
                artifacts = root / "artifacts"
                artifacts.mkdir()
                self.patches(stack, root, **kwargs)
                row = adapter.launch_calibration(seed, artifacts, commit, self.settings(root), root / "semaprax")
                self.assertEqual(row["status"], status)
                self.assertTrue(row["separate_from_trials"])
                self.assertFalse(row["subtracted_from_trials"])
                self.assertTrue(row["worktree_removed_after_archive"])
                self.assertFalse((artifacts / "worktrees/calibration").exists())

    def test_campaign_persists_attempts_and_stops_after_provider_failure(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory).resolve()
            settings = {**self.settings(root), "artifacts": str(root / "artifacts"),
                        "repository_commit": adapter.shiftsim.resolve_commit(adapter.REPO, "HEAD"),
                        "trial_order": ["semaprax", "typescript"] * 5, "attempt_denominator": 10,
                        "harness_source_files_sha256": adapter.codex.harness_source_inventory(adapter.REPO, adapter.HARNESS_SOURCE_FILES)}
            stack.enter_context(patch.object(adapter, "plan", return_value=settings))
            # First process is calibration, second refuses. No model is invoked.
            process, sessions = self.process(root)
            def execute(*args):
                row = process(*args)
                if "calibration" not in args[1].name:
                    row["process_exit_code"] = 1
                return row
            stack.enter_context(patch.object(adapter.codex, "run_codex", side_effect=execute))
            original = adapter.codex.copy_task_rollout
            stack.enter_context(patch.object(adapter.codex, "copy_task_rollout", side_effect=lambda ids, cwd, dest: original(ids, cwd, dest, sessions)))
            report = adapter.run_campaign(Namespace(repo=str(adapter.REPO), semaprax_bin=str(root / "semaprax")))
            self.assertEqual(report["campaign_status"], "interrupted")
            self.assertEqual(report["campaign"]["harness_source_snapshot"]["files_sha256"], settings["harness_source_files_sha256"])
            self.assertEqual(report["recorded_attempts"], 1)
            self.assertEqual(len(report["unlaunched_trial_order"]), 9)
            self.assertEqual(report["attempt_denominator"], 10)
            self.assertEqual(json.loads((root / "artifacts/results.json").read_text())["recorded_attempts"], 1)
            self.assertEqual(json.loads((root / "artifacts/attempts/semaprax-01.json").read_text())["process_exit_code"], 1)

    def test_ordinary_timeout_is_archived_cleaned_and_does_not_truncate_matched_attempts(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory).resolve()
            settings = {**self.settings(root), "artifacts": str(root / "artifacts"),
                        "repository_commit": adapter.shiftsim.resolve_commit(adapter.REPO, "HEAD"),
                        "trial_order": ["semaprax", "typescript"] * 5, "attempt_denominator": 10,
                        "harness_source_files_sha256": adapter.codex.harness_source_inventory(
                            adapter.REPO, adapter.HARNESS_SOURCE_FILES)}
            stack.enter_context(patch.object(adapter, "plan", return_value=settings))
            process, sessions = self.process(root)
            attempts = [0]
            def execute(*args):
                row = process(*args)
                if "calibration" not in args[1].name:
                    attempts[0] += 1
                    if attempts[0] == 1:
                        row.update({"timed_out": True, "process_exit_code": None})
                return row
            stack.enter_context(patch.object(adapter.codex, "run_codex", side_effect=execute))
            original = adapter.codex.copy_task_rollout
            stack.enter_context(patch.object(adapter.codex, "copy_task_rollout", side_effect=lambda ids, cwd, dest:
                original(ids, cwd, dest, sessions)))
            stack.enter_context(patch.object(adapter.shiftsim, "check_program", return_value={"accepted": True}))
            report = adapter.run_campaign(Namespace(repo=str(adapter.REPO), semaprax_bin=str(root / "semaprax")))
            self.assertEqual(report["campaign_status"], "complete")
            self.assertEqual(report["recorded_attempts"], 10)
            first = report["trials"][0]
            self.assertTrue(first["timed_out"])
            self.assertTrue(first["worktree_removed_after_archive"])
            self.assertIn("candidate_archive", first)
            self.assertFalse(Path(first["workspace"]).exists())

    def test_main_reports_interrupted_campaign_as_failure(self):
        args = ["codex_campaign.py", "run", "--base-ref", "HEAD", "--compiler-source-ref", "HEAD",
                "--semaprax-bin", "fixture", "--qualification-evidence", "fixture", "--artifacts", "fixture",
                "--acknowledge-paid-attempts"]
        with patch("sys.argv", args), patch.object(adapter, "plan", return_value={}), \
             patch.object(adapter, "run_campaign", return_value={"campaign_status": "interrupted"}), \
             patch("builtins.print"):
            self.assertEqual(adapter.main(), 2)

    def test_command_disables_same_features_for_both_arms_and_selects_requested_binary(self):
        command = adapter.command_for({"codex_binary": "chosen"}, "prompt")
        self.assertEqual(command[0], "chosen")
        self.assertIn("--ignore-user-config", command)
        self.assertEqual([command[i + 1] for i, value in enumerate(command) if value == "--disable"],
                         ["apps", "plugins", "memories", "multi_agent", "skill_search"])
        self.assertEqual(adapter.ARMS, ("semaprax", "typescript"))


if __name__ == "__main__":
    unittest.main()
