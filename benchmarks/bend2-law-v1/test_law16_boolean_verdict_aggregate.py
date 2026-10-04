#!/usr/bin/env python3
import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location(
    "law16_boolean_verdict_aggregate", ROOT / "law16_boolean_verdict_aggregate.py"
)
AGGREGATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AGGREGATE)


class BooleanVerdictAggregateTests(unittest.TestCase):
    def test_retained_routes_are_authenticated_but_not_a_matched_semantic_pair(self):
        result = AGGREGATE.review(
            ROOT / "evidence/law16-process-state-bend-bool-verdict-v1",
            ROOT / "evidence/law16-process-state-semaprax-bool-z3-v3",
        )
        self.assertEqual(result["status"], "not_matched")
        self.assertEqual(result["routes"]["bend_verdict"]["raw_streams"], 120)
        self.assertEqual(result["routes"]["semaprax_z3"]["raw_streams"], 120)
        self.assertEqual(result["comparability"]["semantic_contract"], "unavailable: score-to-u32 and Boolean negation differ")


if __name__ == "__main__":
    unittest.main()
