import importlib.util
import pathlib
import tempfile
import unittest

ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("law16_annotation_summary", ROOT / "law16_annotation_summary.py")
SUMMARY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SUMMARY)


class AnnotationSummaryTests(unittest.TestCase):
    def test_retained_ten_pair_summary_binds_source_bytes_and_seeds(self):
        result = SUMMARY.summarize()
        self.assertEqual(result["matched_pairs"], 10)
        self.assertEqual(len(result["rows"]), 20)
        bend = result["rows"][0]
        self.assertEqual(bend["explicit_annotation_and_proof_counts"]["law_declarations"]["count"], 1)
        self.assertEqual(bend["explicit_annotation_and_proof_counts"]["proof_terms"]["count"], 1)
        sem = result["rows"][-1]
        self.assertEqual(sem["final_source"]["sha256"], sem["fixed_seed"]["sha256"])
        self.assertEqual(sem["explicit_annotation_and_proof_counts"]["ensures"]["count"], 1)
        self.assertEqual(sem["explicit_annotation_and_proof_counts"]["explicit_proof_bodies"]["count"], 0)

    def test_safe_read_rejects_traversal_and_symlinks(self):
        with self.assertRaises(ValueError):
            SUMMARY.safe_read(ROOT, "../AGENTS.md")
        with tempfile.TemporaryDirectory() as temporary:
            base = pathlib.Path(temporary)
            (base / "target").write_text("source")
            (base / "link").symlink_to(base / "target")
            with self.assertRaises(ValueError):
                SUMMARY.safe_read(base, "link")


if __name__ == "__main__":
    unittest.main()
