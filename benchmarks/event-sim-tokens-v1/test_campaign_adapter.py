import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from argparse import Namespace
from contextlib import ExitStack
from pathlib import Path
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import live_campaign_common as common
import campaign as live_campaign


class ShiftSimCampaignTests(unittest.TestCase):
    def _run_trial_with_spec_edit(self, root: Path, edit_stage: str):
        spec_text = "# Frozen public spec\n"
        spec_hash = hashlib.sha256(spec_text.encode()).hexdigest()
        workspace = root / "artifacts" / "worktrees" / "semaprax-01"
        settings = {
            "timeout_seconds": 10,
            "observed_model_id": live_campaign.MODEL,
            "model": live_campaign.MODEL,
            "seed_files_sha256": {"benchmarks/event-sim-tokens-v1/SPEC.md": spec_hash},
        }

        def add_worktree(_seed, path, _commit, _files):
            (path / "benchmarks/event-sim-tokens-v1").mkdir(parents=True)
            (path / "benchmarks/event-sim-tokens-v1/SPEC.md").write_text(spec_text, encoding="utf-8")
            return None

        def run_process(_command, _cwd, _env, stream, _stderr, _timeout):
            stream.write_text("{}\n", encoding="utf-8")
            if edit_stage == "before_acceptance":
                (workspace / "benchmarks/event-sim-tokens-v1/SPEC.md").write_text("# Changed spec\n")
            return {"timed_out": False, "process_exit_code": 0, "elapsed_seconds": 0.1}

        def check_program(_candidate, _timeout, _env):
            if edit_stage == "during_acceptance":
                (workspace / "benchmarks/event-sim-tokens-v1/SPEC.md").write_text("# Changed spec\n")
            return {"accepted": True}

        with ExitStack() as stack:
            stack.enter_context(patch.object(common, "add_seed_worktree", side_effect=add_worktree))
            stack.enter_context(patch.object(live_campaign, "prompt_for", return_value="test prompt"))
            stack.enter_context(patch.object(live_campaign, "run_process", side_effect=run_process))
            stack.enter_context(patch.object(common, "claude_command", return_value=[]))
            stack.enter_context(patch.object(live_campaign, "trial_environment", return_value={}))
            stack.enter_context(patch.object(common, "stream_usage", return_value={
                "models_observed": [live_campaign.MODEL], "usage": {}, "turns_with_usage": 1,
                "legacy_net_input": {"net_input_tokens": 0},
            }))
            stack.enter_context(patch.object(common, "observed_model_matches", return_value=True))
            stack.enter_context(patch.object(common, "rate_card_estimate_details", return_value={"usd": 0}))
            stack.enter_context(patch.object(common, "authored_source_metrics", return_value={"total_tokens": 0}))
            acceptance = stack.enter_context(patch.object(live_campaign, "check_program", side_effect=check_program))
            archive = stack.enter_context(patch.object(common, "archive_candidate", return_value=({}, [])))
            row = live_campaign.launch_trial(
                root / "seed", root / "artifacts", "commit", {"arm": "semaprax", "number": 1},
                settings, root / "semaprax",
            )
        return row, acceptance, archive, workspace

    def test_plan_marks_campaign_unqualified_for_open_stdin_boundary(self):
        with tempfile.TemporaryDirectory() as directory:
            settings = live_campaign.plan(Namespace(
                repo=str(live_campaign.REPO), base_ref="HEAD", artifacts=str(Path(directory) / "campaign"),
                trials_per_arm=5, model=live_campaign.MODEL, effort=live_campaign.EFFORT,
                timeout_seconds=1800, max_budget_usd=None,
            ))
        self.assertEqual(settings["qualification"]["status"], "preflight_not_qualified")
        self.assertEqual(settings["qualification"]["blocking_issue"], 611)
        self.assertEqual(settings["trial_order"], [
            "semaprax", "typescript", "typescript", "semaprax", "semaprax",
            "typescript", "typescript", "semaprax", "semaprax", "typescript",
        ])

    def test_seed_history_exposes_only_public_spec_not_acceptance_oracle(self):
        source = live_campaign.REPO
        commit = live_campaign.resolve_commit(source, "HEAD")
        with tempfile.TemporaryDirectory() as directory:
            artifacts = Path(directory)
            seed = artifacts / "seed"
            metadata = common.create_seed_repository(source, commit, seed, live_campaign.SEED_FILES)
            workspace = artifacts / "worktree"
            error = common.add_seed_worktree(seed, workspace, metadata["seed_repository_commit"],
                                             live_campaign.SEED_FILES)
            self.assertIsNone(error, error)
            files = subprocess.run(["git", "ls-tree", "-r", "--name-only", "HEAD"], cwd=workspace,
                                   text=True, capture_output=True, check=True).stdout.splitlines()
            self.assertEqual(files, ["benchmarks/event-sim-tokens-v1/SPEC.md"])
            self.assertNotEqual(subprocess.run(
                ["git", "show", "HEAD:benchmarks/event-sim-tokens-v1/acceptance/corpus.json"],
                cwd=workspace, capture_output=True, check=False,
            ).returncode, 0)
            self.assertEqual(metadata["source_files_sha256"], metadata["seed_files_sha256"])
            subprocess.run(["git", "worktree", "remove", "--force", str(workspace)], cwd=seed,
                           check=True, capture_output=True)

    def test_common_accounting_keeps_missing_usage_unknown_and_reports_proxy_separately(self):
        with tempfile.TemporaryDirectory() as directory:
            stream = Path(directory) / "stream.jsonl"
            stream.write_text(json.dumps({"type": "assistant", "message": {
                "id": "turn", "model": live_campaign.MODEL, "usage": {"input_tokens": 12},
                "content": [{"type": "text", "text": "done"}],
            }}) + "\n", encoding="utf-8")
            usage = common.stream_usage(stream, live_campaign.MODEL)
        self.assertEqual(usage["usage"]["input_tokens"], 12)
        self.assertIsNone(usage["usage"]["cache_creation_input_tokens"])
        self.assertIsNone(usage["usage"]["cache_read_input_tokens"])
        self.assertIsNone(usage["legacy_net_input"]["net_input_tokens"])
        self.assertEqual(usage["provider_output_tokens"], None)

    def test_summary_preserves_turn_counts_per_attempt_and_total(self):
        summary = live_campaign.summarize([
            {"arm": "semaprax", "number": 1, "status": "accepted",
             "observed": {"turns_with_usage": 2}},
            {"arm": "semaprax", "number": 2, "status": "failed",
             "observed": {"turns_with_usage": 1}},
        ])
        self.assertEqual(summary["arms"]["semaprax"]["turns"], 3)
        self.assertEqual(summary["arms"]["semaprax"]["turns_with_usage_per_attempt"], [2, 1])

    def test_spec_drift_before_acceptance_is_rejected_and_workspace_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            row, acceptance, archive, workspace = self._run_trial_with_spec_edit(
                Path(directory), "before_acceptance"
            )
            acceptance.assert_not_called()
            archive.assert_not_called()
            self.assertEqual(row["seeded_spec_integrity"]["status"], "failed")
            self.assertEqual(row["status"], "failed")
            self.assertTrue(row["workspace_retained_for_review"])
            self.assertTrue((workspace / "benchmarks/event-sim-tokens-v1/SPEC.md").is_file())

    def test_spec_drift_during_acceptance_invalidates_and_preserves_workspace(self):
        with tempfile.TemporaryDirectory() as directory:
            row, acceptance, archive, workspace = self._run_trial_with_spec_edit(
                Path(directory), "during_acceptance"
            )
            acceptance.assert_called_once()
            archive.assert_not_called()
            self.assertEqual(row["status"], "not_accepted")
            self.assertTrue(row["acceptance"]["accepted"])
            self.assertEqual(row["seeded_spec_integrity_before_archive"]["status"], "failed")
            self.assertTrue(row["workspace_retained_for_review"])
            self.assertTrue((workspace / "benchmarks/event-sim-tokens-v1/SPEC.md").is_file())

    def test_candidate_archive_retains_sources_and_records_only_cache_omissions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate, archive = root / "candidate", root / "archive"
            candidate.mkdir()
            (candidate / "main.ts").write_text("export {}\n", encoding="utf-8")
            (candidate / "node_modules").mkdir()
            (candidate / "node_modules" / "hidden.js").write_text("cache", encoding="utf-8")
            hashes, excluded = common.archive_candidate(candidate, archive)
        self.assertEqual(set(hashes), {"main.ts"})
        self.assertEqual(excluded, ["node_modules"])


if __name__ == "__main__":
    unittest.main()
