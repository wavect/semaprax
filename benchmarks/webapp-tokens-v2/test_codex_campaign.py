import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import codex_campaign as campaign


ROOT = Path(__file__).resolve().parents[2]


class WebappCampaignTests(unittest.TestCase):
    def test_plan_pins_public_seed_receipt_and_matched_order(self):
        args = type("Args", (), {
            "repo": str(ROOT), "base_ref": "HEAD", "compiler_source_ref": "HEAD",
            "artifacts": "/private/tmp/teamdesk-offline-plan-test", "semaprax_bin": "/bin/sh",
            "tokenizer_dir": "/private/tmp/semaprax-opt-tokenizer", "codex_binary": "/usr/bin/true",
            "model": campaign.MODEL, "effort": campaign.EFFORT, "trials_per_arm": 5,
            "timeout_seconds": campaign.TIMEOUT_SECONDS,
        })
        settings = campaign.plan(args)
        self.assertEqual(settings["attempt_denominator"], 10)
        self.assertEqual(settings["trial_order"], ["semaprax", "typescript", "typescript", "semaprax",
                                                      "semaprax", "typescript", "typescript", "semaprax",
                                                      "semaprax", "typescript"])
        self.assertEqual(settings["qualification"]["required_cases"], 912)
        self.assertEqual(set(settings["seed_files_sha256"]), {
            "benchmarks/webapp-tokens-v2/SPEC.md",
            "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md",
        })

    def test_seed_contains_only_spec_and_launch_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            seed = Path(directory) / "seed"
            source_commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT,
                                           text=True, capture_output=True, check=True).stdout.strip()
            info = campaign.create_seed_repository(ROOT, source_commit, seed)
            self.assertEqual(info["seed_files_sha256"], info["source_files_sha256"])
            listed = subprocess.run(["git", "ls-tree", "-r", "--name-only", "HEAD"], cwd=seed,
                                    text=True, capture_output=True, check=True).stdout.splitlines()
            self.assertEqual(listed, [path.lstrip("/") for path in campaign.SEED_FILES])
            self.assertNotIn("benchmarks/webapp-tokens-v2/acceptance/run.mjs", listed)

    def test_rollout_usage_reconciles_only_matching_final_total(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            stream = root / "exec.jsonl"
            rollout = root / "rollout.jsonl"
            usage = {"input_tokens": 20, "cached_input_tokens": 4,
                     "cache_write_input_tokens": 8, "output_tokens": 3,
                     "reasoning_output_tokens": 1}
            stream.write_text(json.dumps({"type": "turn.completed", "usage": usage}) + "\n")
            rollout.write_text("\n".join([
                json.dumps({"type": "turn_context", "payload": {"model": campaign.MODEL, "effort": campaign.EFFORT}}),
                json.dumps({"type": "token_usage_record", "payload": {"response_id": "r1", "usage": usage}}),
            ]) + "\n")
            observed = campaign.trace_usage(campaign.parse_exec_jsonl(stream), rollout)
            self.assertTrue(observed["reconciled"])
            self.assertEqual(observed["model_request_count"], 1)
            self.assertEqual(campaign.list_price_estimate(observed["model_requests"])["actual_billed_usd"], None)


if __name__ == "__main__":
    unittest.main()
