import importlib.util
import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("incremental_admission", ROOT / "law16_project_incremental_admission.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ProjectIncrementalAdmissionTests(unittest.TestCase):
    def test_exact_checked_u32_control_and_stale_cache_negative_are_bound(self):
        document = MODULE.review()
        self.assertEqual(document["status"], "unsupported")
        self.assertEqual(document["semantic_contract"]["success_witness"]["rechecked"], ["core"])
        self.assertEqual(document["semantic_contract"]["success_witness"]["reused"], ["api"])
        self.assertEqual(document["semantic_contract"]["negative_control"]["witness"]["rechecked"], [])
        self.assertEqual(document["semantic_contract"]["negative_control"]["witness"]["reused"], ["core", "api"])

    def test_committed_review_is_reproducible(self):
        expected = json.loads((ROOT / "evidence/law16-project-incremental-admission-v1.json").read_text())
        self.assertEqual(MODULE.review(), expected)


if __name__ == "__main__":
    unittest.main()
