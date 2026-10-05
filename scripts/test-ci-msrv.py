#!/usr/bin/env python3
"""Checks for the checked test-selector controller in ``scripts/ci-msrv.py``.

Two evidence levels are kept apart and selected by the first argument:

``fake``  A fake Cargo/libtest executable replays scripted listings and
          results, so every failure path of the controller is exercised
          without compiling anything.
``real``  The controller drives the real toolchain against the small
          standalone target in ``scripts/tests/checked-selector-fixture``,
          proving that the listing, ignore and execution behaviour it relies
          on matches the installed stable Cargo and libtest.

Run ``python3 scripts/test-ci-msrv.py fake`` or ``... real``.
"""

import io
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
router = runpy.run_path(str(ROOT / "scripts" / "ci-msrv.py"))
FIXTURE = ROOT / "scripts" / "tests" / "checked-selector-fixture"

FAKE_CARGO = r'''
import json, os, sys
scenario = json.loads(open(os.environ["FAKE_CARGO_SCENARIO"]).read())
with open(os.environ["FAKE_CARGO_LOG"], "a") as log:
    log.write(json.dumps(sys.argv[1:]) + "\n")
harness = sys.argv[sys.argv.index("--") + 1:]
if "--list" in harness:
    key = "ignored_listing" if "--ignored" in harness else "listing"
    sys.stdout.write(scenario[key])
    sys.exit(scenario.get("list_code", 0))
sys.stdout.write(scenario["run"])
sys.exit(scenario.get("run_code", 0))
'''


def listing(names):
    body = "".join(f"{name}: test\n" for name in names)
    count = len(names)
    return body + ("\n" if names else "") + f"{count} test{'' if count == 1 else 's'}, 0 benchmarks\n"


def run_output(records, passed, failed, ignored, filtered, status="ok", running=None):
    running = len(records) if running is None else running
    lines = [f"\nrunning {running} test{'' if running == 1 else 's'}\n"]
    lines += [f"test {name} ... {result}\n" for name, result in records]
    lines.append(
        f"\ntest result: {status}. {passed} passed; {failed} failed; {ignored} ignored; "
        f"0 measured; {filtered} filtered out; finished in 0.00s\n\n"
    )
    return "".join(lines)


INVENTORY = [
    "family::a",
    "family::b",
    "family::skipped",
    "family_lookalike::c",
    "other::exact",
    "other::panics",
    "other::slow",
]
IGNORED = ["family::skipped", "other::slow"]


class FakeCargoTests(unittest.TestCase):
    """Evidence level 1: controller failure paths against a fake Cargo."""

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        (self.root / "cargo.py").write_text(FAKE_CARGO)
        self.log = self.root / "log"
        self.base = [sys.executable, str(self.root / "cargo.py"), "test", "--locked", "--lib"]

    def tearDown(self):
        self.directory.cleanup()

    def scenario(self, run, run_code=0, names=INVENTORY, ignored=IGNORED, **extra):
        path = self.root / "scenario.json"
        path.write_text(json.dumps(dict(
            listing=listing(names), ignored_listing=listing(ignored),
            run=run, run_code=run_code, **extra,
        )))
        environment = dict(os.environ, FAKE_CARGO_SCENARIO=str(path), FAKE_CARGO_LOG=str(self.log))
        return environment

    def invocations(self):
        if not self.log.exists():
            return []
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def checked(self, environment, **selection):
        out = io.StringIO()
        code = router["run_checked_selection"](self.base, environment, out=out, **selection)
        return code, out.getvalue()

    def refused(self, message, environment, **selection):
        with self.assertRaises(ValueError) as caught:
            self.checked(environment, **selection)
        self.assertIn(message, str(caught.exception))

    def test_valid_exact_test_runs_once_and_reports_counts(self):
        environment = self.scenario(run_output([("other::exact", "ok")], 1, 0, 0, 6))
        code, out = self.checked(environment, exact=("other::exact",), harness=["--nocapture"])
        self.assertEqual(code, 0)
        self.assertIn("discovered 7, selected 1, executed 1, passed 1, ignored 0", out)
        calls = self.invocations()
        self.assertEqual(calls[0][-2:], ["--", "--list"])
        self.assertEqual(calls[1][-3:], ["--", "--list", "--ignored"])
        self.assertEqual(calls[2][3:], ["--", "--exact", "other::exact", "--nocapture"])
        self.assertEqual(len(calls), 3)

    def test_missing_exact_test_fails_before_execution(self):
        environment = self.scenario(run_output([], 0, 0, 0, 7))
        self.refused("is not in the listed target", environment, exact=("other::renamed",))
        self.assertEqual(len(self.invocations()), 2)

    def test_malformed_selectors_and_shapes_fail_before_execution(self):
        environment = self.scenario("")
        for selection, message in (
            (dict(exact=("other::exact other::panics",)), "malformed exact test selector"),
            (dict(exact=("--exact",)), "malformed exact test selector"),
            (dict(exact=("other::exact", "other::exact")), "selected twice"),
            (dict(prefix="family"), "malformed test family selector"),
            (dict(prefix="family::a"), "malformed test family selector"),
            (dict(), "not both or neither"),
            (dict(exact=("other::exact",), prefix="family::"), "not both or neither"),
            (dict(exact=("other::exact",), policy="all"), "unknown ignore policy"),
        ):
            self.refused(message, environment, **selection)
        self.assertTrue(all("--exact" not in call[3:] for call in self.invocations()))

    def test_prefix_family_is_fully_inventoried_and_excludes_lookalikes(self):
        records = [("family::a", "ok"), ("family::b", "ok"), ("family::skipped", "ignored")]
        environment = self.scenario(run_output(records, 2, 0, 1, 4))
        code, out = self.checked(environment, prefix="family::")
        self.assertEqual(code, 0)
        self.assertIn("discovered 7, selected 3, executed 2, passed 2, ignored 1", out)
        self.assertEqual(self.invocations()[2][3:], ["--", "family::"])

    def test_family_member_missing_from_results_fails(self):
        records = [("family::a", "ok"), ("family::skipped", "ignored")]
        environment = self.scenario(run_output(records, 1, 0, 1, 4, running=3))
        self.refused("missing completion record for family::b", environment, prefix="family::")

    def test_similar_name_outside_prefix_cannot_satisfy_or_join_family(self):
        names = INVENTORY + ["nested::family::c"]
        environment = self.scenario("", names=names)
        self.refused("escapes its declared scope: nested::family::c", environment, prefix="family::")
        environment = self.scenario(run_output([("family_lookalike::c", "ok")], 1, 0, 0, 6))
        self.refused("selects no runnable case", environment, prefix="family::x::")
        records = [("family::a", "ok"), ("family::b", "ok"), ("family::skipped", "ignored"),
                   ("family_lookalike::c", "ok")]
        environment = self.scenario(run_output(records, 3, 0, 1, 3))
        self.refused("unselected test family_lookalike::c", environment, prefix="family::")

    def test_empty_family_is_an_error_not_an_unfiltered_run(self):
        environment = self.scenario("")
        self.refused("selects no runnable case", environment, prefix="absent::")
        self.assertEqual(len(self.invocations()), 2)

    def test_required_but_ignored_and_ignored_only_selections_fail(self):
        environment = self.scenario("")
        self.refused("required test other::slow is ignored", environment, exact=("other::slow",))
        self.refused("is not ignored, so --ignored skips it", environment,
                     exact=("other::exact",), policy="ignored")
        environment = self.scenario("", ignored=IGNORED + ["family::a", "family::b"])
        self.refused("selects no runnable case", environment, prefix="family::")

    def test_deliberately_selected_ignored_tests_run(self):
        environment = self.scenario(run_output([("other::slow", "ok")], 1, 0, 0, 6))
        code, out = self.checked(environment, exact=("other::slow",), policy="ignored")
        self.assertEqual(code, 0)
        self.assertEqual(self.invocations()[2][3:], ["--", "--exact", "other::slow", "--ignored"])
        records = [("family::a", "ok"), ("family::b", "ok"), ("family::skipped", "ok")]
        environment = self.scenario(run_output(records, 3, 0, 0, 4))
        code, out = self.checked(environment, prefix="family::", policy="include-ignored")
        self.assertEqual(code, 0)
        self.assertIn("selected 3, executed 3, passed 3, ignored 0", out)
        self.assertEqual(self.invocations()[-1][3:], ["--", "family::", "--include-ignored"])

    def test_ignored_result_for_a_required_case_fails(self):
        environment = self.scenario(run_output([("other::exact", "ignored")], 0, 0, 1, 6))
        self.refused("other::exact reported ignored, expected ok", environment, exact=("other::exact",))

    def test_should_panic_success_is_a_pass(self):
        run = run_output([], 1, 0, 0, 6, running=1).replace(
            "\n\ntest result", "\ntest other::panics - should panic ... ok\n\ntest result")
        environment = self.scenario(run)
        code, out = self.checked(environment, exact=("other::panics",))
        self.assertEqual(code, 0)
        self.assertIn("executed 1, passed 1", out)

    def test_failing_test_propagates_its_exit_status(self):
        run = run_output([("other::exact", "FAILED")], 0, 1, 0, 6, status="FAILED")
        environment = self.scenario(run, run_code=101)
        code, out = self.checked(environment, exact=("other::exact",))
        self.assertEqual(code, 101)
        self.assertIn("libtest exited with status 101", out)

    def test_failure_hidden_behind_zero_exit_is_refused(self):
        run = run_output([("other::exact", "FAILED")], 0, 1, 0, 6, status="FAILED")
        environment = self.scenario(run)
        self.refused("reported FAILED, expected ok", environment, exact=("other::exact",))

    def test_aborted_and_incomplete_runs_fail(self):
        environment = self.scenario("\nrunning 1 test\n", run_code=134)
        code, _ = self.checked(environment, exact=("other::exact",))
        self.assertEqual(code, 134)
        environment = self.scenario("\nrunning 1 test\n")
        self.refused("missing completion record", environment, exact=("other::exact",))
        environment = self.scenario("\nrunning 1 test\ntest other::exact ... ok\n")
        self.refused("expected one libtest summary, saw 0", environment, exact=("other::exact",))
        environment = self.scenario(run_output([("other::exact", "ok")], 1, 0, 0, 6).replace(
            "running 1 test", "running 0 tests"))
        self.refused("expected one libtest run of 1 tests", environment, exact=("other::exact",))

    def test_summary_is_bound_to_the_listed_target(self):
        # A familiar summary from another target size cannot be accepted.
        environment = self.scenario(run_output([("other::exact", "ok")], 1, 0, 0, 99))
        self.refused("expected (1, 0, 0, 0, 6)", environment, exact=("other::exact",))
        doubled = run_output([("other::exact", "ok")], 1, 0, 0, 6)
        environment = self.scenario(doubled + doubled)
        self.refused("duplicate completion record", environment, exact=("other::exact",))

    def test_spoofed_record_from_test_output_fails_closed(self):
        run = run_output([("other::exact", "ok"), ("other::exact", "ok")], 1, 0, 0, 6, running=1)
        environment = self.scenario(run)
        self.refused("duplicate completion record for other::exact", environment,
                     exact=("other::exact",))

    def test_single_threaded_uncaptured_output_before_the_result(self):
        run = ("\nrunning 1 test\ntest other::exact ... test other::panics ... ok\nok\n"
               "\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; "
               "finished in 0.00s\n")
        environment = self.scenario(run)
        code, out = self.checked(environment, exact=("other::exact",),
                                 harness=["--test-threads=1", "--nocapture"])
        self.assertEqual(code, 0)

    def test_malformed_inventory_and_extra_targets_fail(self):
        for names_listing, message in (
            ("other::exact: test\n", "exactly one libtest target, found 0"),
            (listing(INVENTORY) + listing(["doc::x"]), "exactly one libtest target, found 2"),
            ("garbage\n\n1 test, 0 benchmarks\n", "malformed test inventory line"),
            ("a: test\n\n2 tests, 0 benchmarks\n", "summary does not match"),
            ("a: test\na: test\n\n2 tests, 0 benchmarks\n", "duplicate test"),
            ("a: benchmark\n\n0 tests, 1 benchmark\n", "malformed test inventory line"),
        ):
            path = self.root / "scenario.json"
            path.write_text(json.dumps(dict(listing=names_listing, ignored_listing=listing([]), run="")))
            environment = dict(os.environ, FAKE_CARGO_SCENARIO=str(path), FAKE_CARGO_LOG=str(self.log))
            self.refused(message, environment, exact=("other::exact",))
        environment = self.scenario("", ignored=["not::listed"])
        self.refused("escaped the full inventory", environment, exact=("other::exact",))

    def test_listing_failure_and_oversized_listing_fail(self):
        environment = self.scenario("", list_code=101)
        with self.assertRaises(subprocess.CalledProcessError):
            self.checked(environment, exact=("other::exact",))
        environment = self.scenario("")
        with self.assertRaises(ValueError) as caught:
            router["capture_listing"](
                [*self.base, "--", "--list"], environment, limit=16)
        self.assertIn("exceeded 16 bytes", str(caught.exception))

    def test_unsupported_harness_argument_is_refused(self):
        environment = self.scenario(run_output([("other::exact", "ok")], 1, 0, 0, 6))
        self.refused("unsupported libtest argument", environment,
                     exact=("other::exact",), harness=["--skip", "x"])

    def test_repair_shards_partition_inventory_and_use_the_checked_seam(self):
        prefix = router["REPAIR_FILTER"]
        cases = [prefix + name for name in "edcba"]
        names = cases + ["other::exact"]
        union = []
        for index in range(2):
            selected = router["repair_shard_names"](names, index, 2)
            union += selected
            records = [(name, "ok") for name in selected]
            environment = self.scenario(
                run_output(records, len(selected), 0, 0, len(names) - len(selected)),
                names=names, ignored=[])
            out = io.StringIO()
            base = [sys.executable, str(self.root / "cargo.py"), *router["REPAIR_TEST"][1:]]
            self.assertEqual(router["run_repair_shard"]("repair", f"{index}/2", environment, base, out), 0)
            self.assertIn(f"repair {index}/2: {len(selected)} source-repair cases", out.getvalue())
            last = self.invocations()[-1]
            self.assertEqual(last[last.index("--"):], ["--", "--exact", *selected, "--test-threads=1"])
        self.assertEqual(sorted(union), sorted(cases))
        self.assertEqual(len(union), len(set(union)))
        with self.assertRaises(ValueError) as caught:
            router["repair_shard_names"](names, 5, 6)
        self.assertIn("no case", str(caught.exception))
        environment = self.scenario("", names=names, ignored=[cases[0]])
        base = [sys.executable, str(self.root / "cargo.py"), *router["REPAIR_TEST"][1:]]
        with self.assertRaises(ValueError) as caught:
            router["run_repair_shard"]("repair", "4/5", environment, base, io.StringIO())
        self.assertIn("only ignored cases", str(caught.exception))


class RealToolchainTests(unittest.TestCase):
    """Evidence level 2: real Cargo and libtest against a tiny fixture target."""

    @classmethod
    def setUpClass(cls):
        cls.directory = tempfile.TemporaryDirectory()
        cls.environment = dict(os.environ, CARGO_TARGET_DIR=cls.directory.name)
        for variable in ("CHECKED_FIXTURE_FAIL", "CHECKED_FIXTURE_ABORT", "RUSTFLAGS"):
            cls.environment.pop(variable, None)
        cargo = os.environ.get("CARGO") or shutil.which("cargo")
        if cargo is None:
            raise unittest.SkipTest("cargo is unavailable")
        cls.base = [cargo, "test", "--locked", "--offline", "--manifest-path",
                    str(FIXTURE / "Cargo.toml"), "-p", "checked-selector-fixture", "--lib"]
        version = subprocess.run([cargo, "--version"], capture_output=True, text=True, check=True)
        print(f"real toolchain: {version.stdout.strip()}", file=sys.stderr)

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def checked(self, environment=None, **selection):
        out = io.StringIO()
        code = router["run_checked_selection"](
            self.base, environment or self.environment, out=out, **selection)
        return code, out.getvalue()

    def test_listing_reports_every_case_and_the_ignored_subset(self):
        names, ignored = router["discover"](self.base, self.environment)
        self.assertEqual(len(names), 10)
        self.assertEqual(ignored, {"family::ignored_member", "only_ignored::alone"})

    def test_exact_case_runs_once(self):
        code, out = self.checked(exact=("escape::inside",))
        self.assertEqual(code, 0, out)
        self.assertIn("discovered 10, selected 1, executed 1, passed 1, ignored 0", out)

    def test_family_counts_ignored_and_should_panic_members(self):
        code, out = self.checked(prefix="family::")
        self.assertEqual(code, 0, out)
        self.assertIn("selected 4, executed 3, passed 3, ignored 1", out)
        code, out = self.checked(prefix="family::", harness=["--test-threads=1", "--nocapture"])
        self.assertEqual(code, 0, out)
        code, out = self.checked(prefix="family::", policy="include-ignored")
        self.assertEqual(code, 0, out)
        self.assertIn("selected 4, executed 4, passed 4, ignored 0", out)

    def test_ignored_policies_match_libtest(self):
        code, out = self.checked(exact=("only_ignored::alone",), policy="ignored")
        self.assertEqual(code, 0, out)
        self.assertIn("executed 1, passed 1", out)
        with self.assertRaises(ValueError):
            self.checked(exact=("only_ignored::alone",))
        with self.assertRaises(ValueError):
            self.checked(prefix="only_ignored::")

    def test_scope_escape_and_missing_names_fail(self):
        with self.assertRaises(ValueError) as caught:
            self.checked(prefix="escape::")
        self.assertIn("nested::escape::outside", str(caught.exception))
        with self.assertRaises(ValueError):
            self.checked(exact=("family::renamed",))

    def test_failing_and_aborting_cases_fail(self):
        code, _ = self.checked(dict(self.environment, CHECKED_FIXTURE_FAIL="1"),
                               exact=("gated::fails_on_request",))
        self.assertEqual(code, 101)
        code, _ = self.checked(dict(self.environment, CHECKED_FIXTURE_ABORT="1"),
                               exact=("gated::aborts_on_request",))
        self.assertNotEqual(code, 0)


def main():
    level = sys.argv[1] if len(sys.argv) > 1 else ""
    cases = {"fake": FakeCargoTests, "real": RealToolchainTests}
    if level not in cases:
        print("usage: test-ci-msrv.py fake|real", file=sys.stderr)
        return 2
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(cases[level])
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    if not result.wasSuccessful():
        return 1
    print(f"{level} level: {result.testsRun} run, {len(result.skipped)} skipped")
    return 0 if result.testsRun and not result.skipped else 1


if __name__ == "__main__":
    sys.exit(main())
