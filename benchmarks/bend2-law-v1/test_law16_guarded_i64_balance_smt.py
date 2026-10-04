import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location(
    "law16_guarded_i64_balance_smt", ROOT / "law16_guarded_i64_balance_smt.py"
)
ROUTE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ROUTE)
CAPSULE = ROOT / "evidence/law16-guarded-i64-balance-smt-v1"
FRESH_CAPSULE = ROOT / "evidence/law16-unified-fresh-guarded-i64-v1"


class GuardedI64BalanceSourceProofTests(unittest.TestCase):
    def test_exact_selected_source_postconditions_have_installed_z3_discharges(self):
        result = ROUTE.verify_capsule(CAPSULE)
        self.assertEqual(result["status"], "bounded_source_postconditions_proved_full_u32_route_incomplete")
        self.assertEqual(result["positive_smt_discharges"], 7)
        self.assertEqual(result["full_u32_original"], "unsupported_by_this_source_profile")

    def test_noop_transfer_is_refused_without_inventing_solver_status(self):
        result = ROUTE.verify_capsule(CAPSULE)
        self.assertEqual(
            result["no_op_negative"], "proof_tool_refused_no_solver_status_claimed"
        )
        self.assertEqual(result["overall_law16"], "incomplete")

    def test_fresh_capture_refuses_a_mismatched_caller_pin_before_execution(self):
        executable = pathlib.Path(sys.executable)
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "fresh"
            with self.assertRaisesRegex(ValueError, "digest"):
                ROUTE.run_fresh(
                    executable,
                    executable,
                    output,
                    "sha256:" + "0" * 64,
                    "sha256:" + ROUTE.file_digest(executable),
                    "0" * 40,
                )
            self.assertFalse(output.exists())

    def test_retained_fresh_capsule_authenticates_without_rerunning_tools(self):
        result = json.loads((FRESH_CAPSULE / "result.json").read_text())
        review = ROUTE.verify_fresh_capsule(
            FRESH_CAPSULE,
            "sha256:" + result["semaprax"]["sha256"],
            "sha256:" + result["z3"]["sha256"],
            result["semaprax"]["build_commit"],
        )
        self.assertEqual(review["positive_smt_discharges"], 7)
        self.assertEqual(review["raw_streams"], 16)
        self.assertEqual(review["no_op_negative"], "proof_tool_refused_no_solver_status_claimed")


if __name__ == "__main__":
    unittest.main()
