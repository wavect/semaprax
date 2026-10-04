#!/usr/bin/env python3
import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("law16_closure_audit", ROOT / "law16_closure_audit.py")
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)


class Law16ClosureAuditTests(unittest.TestCase):
    def test_audit_binds_exact_issue_text_and_assesses_every_acceptance_item(self):
        value = AUDIT.render()
        self.assertEqual(value["schema"], AUDIT.SCHEMA)
        self.assertEqual(value["closure"], "not_satisfied")
        self.assertEqual(value["report_status"], "incomplete")
        self.assertEqual(value["issue"]["number"], 392)
        self.assertEqual(len(value["issue"]["api_body_sha256"]), 64)
        self.assertEqual(len(value["issue"]["acceptance_criteria"]), 7)
        self.assertEqual(len(value["acceptance_assessment"]), 7)
        for source, assessment in zip(
            value["issue"]["acceptance_criteria"], value["acceptance_assessment"], strict=True
        ):
            self.assertEqual(source["text"], assessment["text"])
            self.assertFalse(source["checked_at_capture"])

    def test_audit_separates_open_requirements_from_declared_unsupported_cells(self):
        value = AUDIT.render()
        status = {row["id"]: row["status"] for row in value["acceptance_assessment"]}
        self.assertEqual(status["AC3"], "met")
        self.assertEqual(status["AC4"], "met")
        self.assertEqual(status["AC7"], "met")
        self.assertEqual(status["AC1"], "partial")
        self.assertEqual(value["unified_fresh_capture"]["status"], "fresh_capture_authenticated")
        self.assertEqual(value["unified_fresh_capture"]["fresh_route_count"], 6)
        self.assertEqual(value["fresh_guarded_i64_source_proof"]["positive_smt_discharges"], 7)
        self.assertEqual(value["fresh_guarded_i64_source_proof"]["no_op_negative"], "proof_tool_refused_no_solver_status_claimed")
        self.assertEqual(status["AC5"], "partial")
        self.assertEqual(status["AC6"], "partial")
        required = {row["id"]: row["status"] for row in value["required_implementation_assessment"]}
        self.assertEqual(len(required), 7)
        self.assertEqual(required["R2"], "partial")
        self.assertEqual(required["R4"], "partial")
        self.assertEqual(required["R5"], "partial")
        self.assertEqual(required["R6"], "met")
        self.assertEqual(required["R7"], "met")
        cells = {row["id"]: row for row in value["declared_unsupported_or_unavailable_cells"]}
        self.assertEqual(cells["checked_u32_source_syntax"]["classification"], "unsupported")
        self.assertIn("unsupported_by_pinned_parser", cells["checked_u32_source_syntax"]["status"])
        self.assertEqual(cells["cold_cache_isolation"]["classification"], "unavailable")
        self.assertEqual(cells["supported_list_theorem"]["classification"], "supplemental_profile_only")
        self.assertEqual(cells["law16_list_source_theorem"]["classification"], "unavailable_for_law16_identity")
        self.assertEqual(cells["external_lean_export_kernel"]["classification"], "supplemental_route_available_but_not_law16_cell")
        self.assertEqual(len(value["unmet_requirements"]), 3)
        valid_ids = set(status) | set(required)
        self.assertTrue(all(
            set(row["blocking_requirements"]) <= valid_ids
            for row in value["declared_unsupported_or_unavailable_cells"]
        ))

    def test_supplemental_evidence_does_not_reclassify_noop_refusal_or_close_issue(self):
        value = AUDIT.render()
        report = AUDIT.report_data()
        proof = report["supplemental_guarded_i64_balance_source_proof"]
        self.assertEqual(proof["positive_smt_discharges"], 7)
        self.assertEqual(proof["no_op_negative"]["status"], "proof_tool_refused_no_solver_status_claimed")
        self.assertEqual(value["closure"], "not_satisfied")
        self.assertEqual(value["issue_state_at_capture"], "open")

    def test_law15_i64_lean_capsule_is_reported_as_supplemental_scope(self):
        value = AUDIT.render()
        proof = value["supplemental_i64_list_proof"]
        self.assertEqual(proof["status"], "retained_test_output_and_source_hashes_verified")
        self.assertEqual(proof["coverage"]["declarations"], ["law15.collection.insert", "law15.collection.sort"])
        self.assertEqual(proof["coverage"]["laws"], ["sort_sorted", "sort_permutation", "sort_multiplicity"])
        self.assertIn("unchanged", proof["original_law16_cell"])
        self.assertIn("not a build attestation", proof["test_binary_association"])
        self.assertFalse(proof["exit_code_retained"])
        self.assertIn("includes the LAW15", value["current_report_reconciliation"]["audit_update"])
        self.assertIn("does not close", value["current_report_reconciliation"]["audit_update"])

    def test_guarded_i64_v2_controls_keep_theorem_and_admission_limits_explicit(self):
        value = AUDIT.render()["supplemental_guarded_i64_profile_v2"]
        self.assertEqual(value["status"], "supplemental_controls_and_matched_sort_law_route_no_timing")
        self.assertTrue(value["paired_controls"]["balance"]["expected_outcomes_observed"])
        self.assertEqual(value["paired_controls"]["balance"]["candidate_attack_pairs_per_route"], 1)
        self.assertIn("full-domain", value["representation_model"]["claim"])
        self.assertIn("four-element", value["bounded_sort_model"]["claim"])
        self.assertEqual(value["original_manifest_status"], "unsupported_by_pinned_parser")
        self.assertIn("no cross-source aliasing", value["law15_lean_route"]["identity_boundary"])
        comparison = value["supplemental_bend_lean_comparison"]
        self.assertEqual(comparison["disposition"], "matched_universal_semantic_laws_under_u32_embedding_no_timing")
        self.assertIn("no timing", comparison["scope_limit"])
        self.assertIn("not attested", value["build_association"])
        self.assertEqual(AUDIT.render()["closure"], "not_satisfied")

    def test_evidence_references_are_digest_bound(self):
        value = AUDIT.render()
        self.assertEqual(len(value["audited_repository_commit"]), 40)
        for row in value["evidence_basis"]:
            path = ROOT / row["path"]
            self.assertTrue(path.is_file())
            self.assertEqual(row["bytes"], path.stat().st_size)
            self.assertEqual(row["sha256"], "sha256:" + AUDIT.digest(path))


if __name__ == "__main__":
    unittest.main()
