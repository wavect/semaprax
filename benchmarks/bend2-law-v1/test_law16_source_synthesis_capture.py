import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location(
    "law16_source_synthesis_capture", ROOT / "law16_source_synthesis_capture.py"
)
CAPTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPTURE)


class Law16SourceSynthesisCaptureTests(unittest.TestCase):
    def test_exact_renderer_suffix_removal_matches_solver_input_hash(self):
        renderer = b"(set-logic ALL)\n(check-sat)\n(get-model)\n"
        solver_input = CAPTURE.normalize_renderer_output(renderer)
        self.assertEqual(solver_input, b"(set-logic ALL)\n(check-sat)\n")
        self.assertEqual(
            CAPTURE.digest_bytes(solver_input),
            "sha256:e2f83d2f28ea744e462a2740beec2223538074be8107f170d9a4a3be96f2c175",
        )

    def test_renderer_suffix_must_be_exact(self):
        with self.assertRaisesRegex(ValueError, "exact model-query suffix"):
            CAPTURE.normalize_renderer_output(b"(check-sat)\n")
        with self.assertRaisesRegex(ValueError, "exact model-query suffix"):
            CAPTURE.normalize_renderer_output(b"(check-sat)\n(get-model) \n")

    def test_summary_uses_nearest_rank_p95_and_retains_all_samples(self):
        self.assertEqual(
            CAPTURE.summarize(list(range(1, 31))),
            {"count": 30, "p50_ns": 15.5, "p95_ns": 29},
        )


if __name__ == "__main__":
    unittest.main()
