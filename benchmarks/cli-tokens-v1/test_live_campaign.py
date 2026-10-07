import json
import os
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path

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
            {"type": "result", "subtype": "success"},
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

    def test_independent_acceptance_runner_checks_build_goldens_and_error_statuses(self):
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory)
            (candidate / "build.sh").write_text("exit 0\n", encoding="utf-8")
            program = candidate / "run.py"
            program.write_text(
                """import os, pathlib, sys
args = sys.argv[1:]
root = pathlib.Path(os.environ['LOG_LENS_BENCHMARK_DIR'])
if not args:
    print('missing file', file=sys.stderr); raise SystemExit(2)
if args[0] == 'missing-loglens-input.log':
    print('cannot read file', file=sys.stderr); raise SystemExit(1)
if args[0] == str(root / 'sample.log') and args[1:] == []:
    sys.stdout.buffer.write((root / 'expected.txt').read_bytes()); raise SystemExit(0)
if args[0] == str(root / 'sample.log') and args[1:] == ['--top', '3', '--json']:
    sys.stdout.buffer.write((root / 'expected.json').read_bytes()); raise SystemExit(0)
print('usage error', file=sys.stderr); raise SystemExit(2)
""",
                encoding="utf-8",
            )
            (candidate / "run.sh").write_text(
                'exec python3 "$(dirname "$0")/run.py" "$@"\n', encoding="utf-8"
            )
            result = live_campaign.check_program(
                candidate,
                live_campaign.BENCHMARK / "sample.log",
                5,
                {**os.environ, "LOG_LENS_BENCHMARK_DIR": str(live_campaign.BENCHMARK)},
            )
        self.assertTrue(result["accepted"], result)
        self.assertEqual(len(result["checks"]), 7)


if __name__ == "__main__":
    unittest.main()
