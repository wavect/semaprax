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
        process = value["matched_boolean"]["process_provenance"]
        self.assertEqual(process["command_count"], 240)
        self.assertEqual(process["cold_cache"]["status"], "unavailable")
        effort = value["proof_effort"]["boolean_agent_synthesis"]
        self.assertEqual(effort["matched_pairs"], 10)
        self.assertEqual(effort["per_language"]["bend2"]["agent_turns"], 10)
        self.assertEqual(effort["per_language"]["semaprax-scalar-v1"]["agent_turns"], 10)
        annotations = value["annotations_and_changed_bytes"]
        self.assertEqual(annotations["matched_boolean"]["status"], "unavailable")
        self.assertEqual(annotations["historical_bounded_balance_v2"]["status"], "retained_source_evidence_only")


if __name__ == "__main__":
    unittest.main()
