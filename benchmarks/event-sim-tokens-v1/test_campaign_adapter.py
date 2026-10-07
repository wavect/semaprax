import json
import subprocess
import sys
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import live_campaign_common as common
import campaign as live_campaign


class ShiftSimCampaignTests(unittest.TestCase):
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
