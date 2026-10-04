"""Fast contract tests for the runner; these never launch Cargo or time a host."""
import copy, importlib.util, json, pathlib, unittest

SPEC = importlib.util.spec_from_file_location("hot_reload_run", pathlib.Path(__file__).with_name("run.py"))
RUN = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(RUN)

class Contract(unittest.TestCase):
    def test_committed_acceptance_manifest_and_fixture_are_exact(self):
        manifest = json.loads(RUN.MANIFEST.read_text())
        RUN.validate(manifest)
        self.assertEqual(manifest["scope"], "interpreter-only")
        self.assertIn("source-Agent journey", manifest["nonclaims"])

    def test_malformed_manifest_and_quantile_are_refused_or_deterministic(self):
        manifest = json.loads(RUN.MANIFEST.read_text())
        broken = copy.deepcopy(manifest); broken["unexpected"] = True
        with self.assertRaises(ValueError): RUN.validate(broken)
        self.assertEqual(RUN.summary([1, 2, 3, 4, 5]), {"samples": 5, "median_ms": 3, "p95_ms": 5, "values_ms": [1, 2, 3, 4, 5]})

if __name__ == "__main__": unittest.main()
