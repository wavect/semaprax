import importlib.util
import pathlib
import unittest

ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("report", ROOT / "law16_current_report.py")
REPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORT)


class CurrentReportTests(unittest.TestCase):
    def test_report_preserves_ten_pairs_separate_timing_and_open_closure(self):
        value = REPORT.render()
        self.assertEqual(value["status"], "incomplete")
        self.assertEqual(value["matched_boolean"]["agent_pairs"]["total_pairs"], 10)
        self.assertTrue(any("no cross-route timing ratio" in row for row in value["nonclaims"]))
        self.assertTrue(value["closure"].startswith("no:"))
        self.assertNotIn("current_head", value["pins_and_trust"])
        self.assertEqual(
            value["matched_boolean"]["ordinary_and_nonproof_process_routes"]["bend-ordinary"][
                "fresh_process"
            ]["count"],
            30,
        )
        self.assertEqual(
            value["matched_boolean"]["ordinary_and_nonproof_process_routes"]["semaprax-check"][
                "repeat_process"
            ]["count"],
            30,
        )
        self.assertEqual(
            value["matched_boolean"]["bounded_proof_and_verdict_process_routes"]["semaprax_z3"][
                "fresh_process"
            ]["count"],
            30,
        )
        self.assertEqual(value["matched_boolean"]["proof_path_nonresult"]["samples"], 60)

    def test_report_uses_v2_provenance_only_for_its_own_bound_samples(self):
        value = REPORT.render()
        provenance = value["measurement_provenance"]
        self.assertIn("process-v2 raw samples", provenance["identity_scope"])
        self.assertEqual(provenance["hardware"]["status"], "observed")
        self.assertEqual(provenance["operating_system"]["status"], "observed")
        self.assertEqual(provenance["backend"]["status"], "observed")
        self.assertEqual(provenance["flags"]["compiler_optimization"]["status"], "unavailable")
        cache_probe = value["cache_isolation_probe"]
        self.assertEqual(cache_probe["status"], "unavailable")
        self.assertEqual(cache_probe["checking_measurements"]["samples"], 0)
        self.assertEqual(cache_probe["container_state"]["after_running"], 0)
        guest = value["guest_file_cache_cold_warm"]
        self.assertEqual(guest["status"], "thirty_guest_cache_pairs_authenticated")
        self.assertEqual(guest["summary"]["bend_ordinary"]["cold"]["count"], 30)
        self.assertEqual(guest["summary"]["semaprax_check"]["warm"]["count"], 30)
        self.assertIn("host and Rosetta caches unknown", guest["scope"])
        process = value["matched_boolean"]["process_provenance"]
        self.assertEqual(process["command_count"], 240)
        self.assertEqual(process["cold_cache"]["status"], "unavailable")
        candidate = value["matched_boolean"]["process_routes"]["bend_candidate_fresh_process"]
        self.assertEqual(candidate["count"], 30)
        self.assertEqual(candidate["p50_ns"], 106149771.0)
        rss = value["matched_boolean"]["peak_rss"]
        self.assertEqual(rss["bend_verdict"]["samples"], 30)
        self.assertEqual(rss["semaprax_z3"]["samples"], 30)
        effort = value["proof_effort"]["boolean_agent_synthesis"]
        self.assertEqual(effort["matched_pairs"], 10)
        self.assertEqual(effort["per_language"]["bend2"]["agent_turns"], 10)
        self.assertEqual(effort["per_language"]["semaprax-scalar-v1"]["agent_turns"], 10)
        annotations = value["annotations_and_changed_bytes"]
        self.assertEqual(annotations["matched_boolean"]["status"], "retained_source_evidence_only")
        self.assertEqual(annotations["matched_boolean"]["matched_pairs"], 10)
        self.assertEqual(len(annotations["matched_boolean"]["rows"]), 20)
        self.assertEqual(annotations["historical_bounded_balance_v2"]["status"], "retained_source_evidence_only")

    def test_report_includes_full_u32_controls_as_supplemental_only(self):
        value = REPORT.render()
        controls = value["supplemental_full_u32_encoding_controls"]
        self.assertEqual(controls["status"], "supplemental_controls_pass")
        self.assertTrue(controls["original_manifest_unchanged"])
        self.assertEqual(controls["candidate_and_attack_routes"], 12)
        self.assertEqual(controls["domain_boundary_controls"], 4)
        self.assertEqual(value["status"], "incomplete")
        self.assertTrue(value["closure"].startswith("no:"))
        self.assertTrue(any("original checked-u32 cells remain unadmitted" in row for row in controls["nonclaims"]))

    def test_report_names_bounded_model_level_sort_check_without_source_theorem_claim(self):
        value = REPORT.render()
        controls = value["supplemental_full_u32_encoding_controls"]["equal_spec_profile"]
        self.assertEqual(controls["source"], "full_u32_equal_spec.py")
        sort = controls["universal_model_checks"]["sort"]
        self.assertEqual(sort["expected"], ["unsat", "sat"])
        self.assertIn("length four", sort["claim"])
        result = controls["sort_result_interpretation"]
        self.assertIn("exactly four elements", result["scope"])
        self.assertIn("not an unbounded-list theorem", result["claim_boundary"])
        self.assertIn("not a source-translation", result["claim_boundary"])
        self.assertEqual(value["status"], "incomplete")

    def test_report_exposes_authenticated_per_cell_mad_without_confidence_claim(self):
        value = REPORT.render()
        summary = value["matched_boolean"]["timing_variation"]
        self.assertIn("median absolute deviation", summary["method"])
        self.assertIn("not a confidence interval", summary["interpretation"])
        self.assertIn("historical_process_v1", summary["cells"])
        self.assertIn("process_v2", summary["cells"])
        self.assertIn("ordinary_check_v1", summary["cells"])
        self.assertIn("proof_verdict_v1", summary["cells"])
        bend_candidate = summary["cells"]["process_v2"]["bend_candidate"]
        self.assertEqual(bend_candidate["fresh_process"]["count"], 30)
        self.assertEqual(bend_candidate["fresh_process"]["mad_ns"], 14070520.5)
        self.assertEqual(bend_candidate["repeat_process"]["count"], 30)

    def test_report_authenticates_bounded_balance_source_proofs_without_reclassifying_refusal(self):
        value = REPORT.render()
        proof = value["supplemental_guarded_i64_balance_source_proof"]
        self.assertEqual(proof["positive_smt_discharges"], 7)
        self.assertTrue(all(row["status"] == "smt_proved" for row in proof["selected_postconditions"]))
        self.assertEqual(proof["no_op_negative"]["status"], "proof_tool_refused_no_solver_status_claimed")
        self.assertIn("unclaimed", proof["no_op_negative"]["solver_outcome_classification"])
        self.assertEqual(proof["full_u32_original"], "unsupported_by_this_source_profile")
        self.assertEqual(proof["overall_law16"], "incomplete")
        self.assertEqual(value["status"], "incomplete")
        self.assertTrue(any("does not prove source lowering" in row for row in proof["nonclaims"]))

    def test_report_keeps_source_distinct_universal_sort_and_cost_provenance_incomplete(self):
        value = REPORT.render()
        routes = value["supplemental_guarded_i64_profile_v2"]["routes"]
        self.assertEqual(len(routes), 4)
        self.assertEqual(
            routes[3]["disposition"],
            "matched_universal_semantic_laws_under_u32_embedding_no_timing",
        )
        theorems = value["supplemental_universal_list_theorems"]
        self.assertEqual(theorems["bend"]["status"], "supplemental_bend_u32_sort_source_proved")
        self.assertEqual(theorems["semaprax"]["status"], "supplemental_i64_list_profile_proved")
        self.assertIn("distinct source algorithms", theorems["comparison_scope"])
        cost = value["matched_boolean"]["cost_provenance"]
        self.assertEqual(cost["trials"], 20)
        self.assertEqual(cost["aggregate_token_usage"]["total_tokens"], 333999)
        self.assertEqual(cost["cost_usage"]["status"], "unavailable")
        probe = value["matched_boolean"]["claude_cost_probe"]
        self.assertEqual(probe["provider_cost_usd"], 0.023741)
        self.assertEqual(probe["result"]["subtype"], "error_max_budget_usd")
        self.assertFalse(probe["campaign_admission"])
        self.assertFalse(probe["raw_provider_stream_retained"])
        refactor = value["supplemental_boolean_refactor_cell"]
        self.assertEqual(refactor["raw_streams"], 16)
        self.assertEqual(refactor["checked_u32"], "not_admitted_by_this_boolean_cell")
        self.assertFalse(value["claude_boolean_pilot"]["campaign_admission"])
        self.assertEqual(value["status"], "incomplete")


if __name__ == "__main__":
    unittest.main()
