#!/usr/bin/env python3
import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location(
    "law16_process_state_capsule", ROOT / "law16_process_state_capsule.py"
)
CAPSULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPSULE)


class ProcessStateCapsuleTests(unittest.TestCase):
    def test_committed_boolean_process_state_has_sixty_bound_child_samples(self):
        result = CAPSULE.review(ROOT / "evidence/law16-process-state-semaprax-bool-v1")
        self.assertEqual(result["status"], "local_process_state_authenticated")
        self.assertEqual(result["raw_streams"], 120)
        self.assertEqual(result["states"]["fresh_process"]["count"], 30)
        self.assertEqual(result["states"]["repeat_process"]["count"], 30)
        self.assertEqual(result["cold_state"]["status"], "unavailable")


if __name__ == "__main__":
    unittest.main()
