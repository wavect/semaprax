#!/usr/bin/env python3
"""Regression for the committed bounded LAW-16 Boolean evidence capsule."""
import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("law16_boolean_capsule", ROOT / "law16_boolean_capsule.py")
CAPSULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPSULE)


class Law16BooleanCapsuleTests(unittest.TestCase):
    def test_committed_capsule_replays_all_ten_boolean_ordinals_and_exposes_check_gap(self):
        result = CAPSULE.review(ROOT / "evidence/law16-boolean-v1")
        self.assertEqual(result["status"], "local_boolean_replay_authenticated")
        self.assertEqual(result["observations"]["bend_exact_attack_check_and_verdict_rejections"], 10)
        self.assertEqual(result["observations"]["semaprax_check_exact_attack_successes"], 10)
        self.assertEqual(result["proof_phase"]["status"], "unavailable")


if __name__ == "__main__":
    unittest.main()
