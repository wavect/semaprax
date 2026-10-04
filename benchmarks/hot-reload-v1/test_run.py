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
        self.assertEqual(
            {row["id"] for row in manifest["scenario_matrix"]},
            {"cold-small-a-to-b", "warm-repeated-a-b-a", "no-op", "multi-module-import-closure", "failed-edit-repair"},
        )

    def test_dry_run_retains_explicit_sample_and_warmup_counts(self):
        import sys, tempfile
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "plan.json"
            previous = sys.argv
            try:
                sys.argv = ["run.py", "--dry-run", "--samples", "11", "--warmups", "3", "--output", str(output)]
                RUN.main()
            finally:
                sys.argv = previous
            plan = json.loads(output.read_text())
        self.assertEqual((plan["samples"], plan["warmups"]), (11, 3))

    def test_malformed_manifest_and_quantile_are_refused_or_deterministic(self):
        manifest = json.loads(RUN.MANIFEST.read_text())
        broken = copy.deepcopy(manifest); broken["unexpected"] = True
        with self.assertRaises(ValueError): RUN.validate(broken)
        self.assertEqual(RUN.summary([1, 2, 3, 4, 5]), {"samples": 5, "median_ms": 3, "p95_ms": 5, "values_ms": [1, 2, 3, 4, 5]})

    def test_compiler_subject_refuses_a_missing_or_mismatched_embedded_commit(self):
        with self.assertRaises(RuntimeError):
            RUN.compiler_subject("/usr/bin/true", None)

    def test_scenario_summary_uses_actual_phase_metric_or_noop_plan_round_trip(self):
        records = [{"phases": [{"save_to_ack_ms": 4.0, "plan_control_round_trip_ms": 3.0}]}]
        self.assertEqual(RUN.scenario_summary(records)["values_ms"], [4.0])
        no_op = [{"phases": [{"plan_control_round_trip_ms": 2.0}]}]
        self.assertEqual(RUN.scenario_summary(no_op)["values_ms"], [2.0])

    def test_phase_timings_require_nonnegative_integer_nanoseconds(self):
        response = {"phase_timings_ns": {"source_admission_check_ns": 1234567}}
        self.assertEqual(RUN.phase_ms(response, "source_admission_check_ns", required=True), 1.234567)
        self.assertIsNone(RUN.phase_ms({}, "candidate_preparation_ns", required=False))
        for invalid in (-1, 1.5, True, "5"):
            with self.subTest(invalid=invalid), self.assertRaises(RuntimeError):
                RUN.phase_ms({"phase_timings_ns": {"candidate_preparation_ns": invalid}}, "candidate_preparation_ns", required=True)
        with self.assertRaises(RuntimeError):
            RUN.phase_ms({}, "source_admission_check_ns", required=True)

if __name__ == "__main__": unittest.main()
