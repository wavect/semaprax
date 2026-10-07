import json
import unittest
from pathlib import Path

EVIDENCE = json.loads((Path(__file__).parent / "round3-accounted-evidence.json").read_text())
BASE = Path(__file__).parent
LIVE_RESULTS = json.loads((BASE / "results-live.json").read_text())


class Round3EvidenceTests(unittest.TestCase):
    def test_preserves_original_no_acceptance_outcome_and_post_run_scope(self):
        self.assertEqual(EVIDENCE["campaign"]["attempt_denominator"], 10)
        self.assertIsNone(EVIDENCE["campaign_cost_per_accepted_task_usd"])
        self.assertEqual(EVIDENCE["scope"]["original_full_corpus_acceptance"],
                         "0/5 per arm; preserved status not changed by recount")
        self.assertTrue(EVIDENCE["scope"]["post_run_scope_is_not_original_scoring"])
        self.assertFalse(EVIDENCE["scope"]["line_endings_explicit_in_frozen_spec"])

    def test_trial_inventory_and_provider_hashes_are_complete(self):
        self.assertEqual(len(EVIDENCE["trials"]), 10)
        for trial in EVIDENCE["trials"]:
            self.assertFalse(trial["original_full_corpus_accepted"])
            self.assertEqual(trial["explicit_spec_check_status"], "passed")
            for key in ("prompt_sha256", "transcript_sha256", "candidate_archive_inventory_sha256",
                        "candidate_manifest_sha256"):
                self.assertRegex(trial[key], r"^[0-9a-f]{64}$")

    def test_arm_totals_reconcile_to_trial_usage_and_authored_proxy(self):
        for arm, summary in EVIDENCE["arms"].items():
            rows = [row for row in EVIDENCE["trials"] if row["arm"] == arm]
            self.assertEqual(len(rows), 5)
            self.assertEqual(summary["deduplicated_assistant_messages_with_usage_total"],
                             sum(row["deduplicated_assistant_messages_with_usage"] for row in rows))
            self.assertEqual(summary["provider_num_turns_total"], sum(row["provider_num_turns"] for row in rows))
            self.assertEqual(summary["raw_input_plus_cache_total"],
                             sum(row["provider_input_plus_cache_tokens_raw"] for row in rows))
            self.assertEqual(summary["provider_output_tokens_total"],
                             sum(row["usage"]["output_tokens"] for row in rows))
            self.assertEqual(summary["final_authored_source_proxy_tokens_total"],
                             sum(row["final_authored_source_legacy_tokenizer_proxy_tokens"] for row in rows))
            self.assertIsNone(summary["cost_per_accepted_task_usd"])

    def test_live_results_add_round_three_without_replacing_prior_rounds(self):
        runs = LIVE_RESULTS["runs"]
        self.assertEqual([run["round"] for run in runs], [1, 2, 3])
        round3 = runs[-1]
        self.assertEqual(round3["original_full_corpus_acceptance"],
                         "0/5 per arm; accepted-task cost is null")
        self.assertIsNone(round3["semaprax"]["cost_per_accepted_task_usd"])
        self.assertIsNone(round3["typescript"]["cost_per_accepted_task_usd"])
        self.assertEqual(len(round3["semaprax"]["trials"]), 5)
        self.assertEqual(len(round3["typescript"]["trials"]), 5)
        self.assertTrue((BASE / round3["evidence_file"]).is_file())

    def test_report_leads_with_unqualified_result_and_retry_evidence(self):
        report = (BASE / "ROUND-3-REPORT.md").read_text()
        self.assertIn("No qualified winner", report)
        self.assertIn("66-provider-turn outlier", report)
        for marker in ("SPX-O101", "SPX-O107", "SPX-U105", "SPX-H006", "SPX-T205", "SPX-T252"):
            self.assertIn(marker, report)


if __name__ == "__main__":
    unittest.main()
