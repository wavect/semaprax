#!/usr/bin/env python3
import importlib.util
import json
import pathlib
import tempfile
import unittest

ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("law16_boolean_negation_pair", ROOT / "law16_boolean_negation_pair.py")
PAIR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PAIR)


class BooleanNegationPairTests(unittest.TestCase):
    def test_prepared_pair_binds_two_value_truth_table_and_project_sources(self):
        result = PAIR.review(ROOT / "fixtures/boolean-negation-pair-v1.json")
        self.assertEqual(result["status"], "prepared_not_executed")
        self.assertEqual(result["semantic_contract"]["truth_table"], [{"input": False, "output": True}, {"input": True, "output": False}])
        self.assertEqual(result["routes"]["bend_verdict"]["samples_per_state"], 30)
        self.assertEqual(result["routes"]["semaprax_z3"]["samples_per_state"], 30)
        self.assertEqual(result["comparability"]["status"], "not_validated")

    def test_rejects_truth_table_drift(self):
        plan = json.loads((ROOT / "fixtures/boolean-negation-pair-v1.json").read_text())
        plan["semantic_contract"]["truth_table"][1]["output"] = True
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "plan.json"
            path.write_text(json.dumps(plan))
            with self.assertRaisesRegex(ValueError, "truth table drifted"):
                PAIR.review(path)


if __name__ == "__main__":
    unittest.main()
