import importlib.util
import pathlib
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("nonproof_capsule", ROOT / "law16_boolean_negation_nonproof_capsule.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class NonproofCapsuleTests(unittest.TestCase):
    def test_committed_capsule_validates_from_its_repository_copy(self):
        review = MODULE.review(ROOT / "evidence/law16-boolean-negation-nonproof-process-v1")
        self.assertEqual(review["status"], "local_matched_nonproof_routes_authenticated")
        self.assertEqual(review["routes"]["bend-ordinary"]["fresh_process"]["count"], 30)
        self.assertEqual(review["routes"]["semaprax-check"]["repeat_process"]["count"], 30)

    def test_review_does_not_open_recorded_external_input_paths(self):
        original = pathlib.Path.is_file

        def repository_only(path):
            text = str(path)
            if text.startswith("/tmp/law16-boolean-nonproof") or text.startswith("/private/tmp/law16-boolean-nonproof"):
                raise AssertionError("validator accessed an external captured-input path")
            return original(path)

        with mock.patch.object(pathlib.Path, "is_file", repository_only):
            review = MODULE.review(ROOT / "evidence/law16-boolean-negation-nonproof-process-v1")
        self.assertEqual(review["status"], "local_matched_nonproof_routes_authenticated")


if __name__ == "__main__":
    unittest.main()
