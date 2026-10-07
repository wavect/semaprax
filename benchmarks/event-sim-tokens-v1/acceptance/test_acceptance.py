import json
import subprocess
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CORPUS = json.loads((HERE / "corpus.json").read_text(encoding="utf-8"))
sys.path.insert(0, str(HERE))
import run as acceptance_runner
sys.path.insert(0, str(ROOT))
import oracle


class AcceptanceTests(unittest.TestCase):
    def test_one_diagnostic_line_does_not_require_a_line_terminator(self):
        for stderr in (b"error", b"error\n", b"error\r\n"):
            with self.subTest(stderr=stderr):
                self.assertTrue(acceptance_runner._one_diagnostic_line(stderr))

        for stderr in (b"", b"\n", b" \r\n", b"error\nsecond", b"error\n\n"):
            with self.subTest(stderr=stderr):
                self.assertFalse(acceptance_runner._one_diagnostic_line(stderr))

    def test_corpus_is_accepted_by_reference_oracle(self):
        result = subprocess.run(
            [sys.executable, str(ROOT / "oracle.py")],
            input="{}\n",
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)

        for case in CORPUS["valid"]:
            actual = oracle.simulate(case["input"])
            self.assertEqual(actual, case["expected"], case["name"])

    def test_runner_accepts_the_reference_oracle(self):
        result = subprocess.run(
            [
                sys.executable,
                str(HERE / "run.py"),
                "--command-json",
                json.dumps([sys.executable, str(ROOT / "oracle.py")]),
            ],
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("acceptance passed", result.stdout)

    def test_priority_queue_and_completion_boundary_are_pinned(self):
        cases = {case["name"]: case["expected"] for case in CORPUS["valid"]}
        rows = cases["queued-priority-tie"]["assignments"]
        self.assertEqual([row["id"] for row in rows], ["A", "C", "D", "B"])
        same_time = cases["completion-and-arrival-same-time"]["assignments"]
        self.assertEqual([(row["id"], row["start"]) for row in same_time], [("A", 0), ("B", 4)])

    def test_unbounded_json_whitespace_preserves_bounded_request_semantics(self):
        case = next(case for case in CORPUS["valid"]
                    if case["name"] == "large-leading-whitespace")
        raw = acceptance_runner.request_text(case)
        compact = acceptance_runner.request_text({"input": case["input"]})
        self.assertGreater(len(raw.encode("utf-8")), 65_536)
        self.assertLessEqual(len(compact.encode("utf-8")), 65_536)
        self.assertEqual(len(case["input"]["patients"]), 1)
        self.assertEqual(len(case["input"]["servers"]), 1)

        outputs = []
        for request in (compact, raw):
            result = subprocess.run(
                [sys.executable, str(ROOT / "oracle.py")],
                input=request,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            outputs.append(result.stdout)
        expected = json.dumps(case["expected"], ensure_ascii=False, separators=(",", ":")) + "\n"
        self.assertEqual(outputs, [expected, expected])

    def test_max_cardinality_escaped_json_is_over_limit_without_whitespace(self):
        cases = {case["name"]: case for case in CORPUS["valid"]}
        compact = cases["max-cardinality-compact"]
        escaped = cases["max-cardinality-escaped-keys-and-ids"]
        compact_text = acceptance_runner.request_text(compact)
        escaped_text = acceptance_runner.request_text(escaped)
        self.assertEqual(compact["input"], escaped["input"])
        self.assertEqual(len(compact["input"]["servers"]), 8)
        self.assertEqual(len(compact["input"]["patients"]), 256)
        self.assertLessEqual(len(compact_text.encode("utf-8")), 65_536)
        self.assertGreater(len(escaped_text.encode("utf-8")), 65_536)
        self.assertEqual(json.loads(escaped_text), escaped["input"])

        outputs = []
        for request in (compact_text, escaped_text):
            result = subprocess.run(
                [sys.executable, str(ROOT / "oracle.py")], input=request,
                text=True, capture_output=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            outputs.append(result.stdout)
        expected = json.dumps(compact["expected"], ensure_ascii=False, separators=(",", ":")) + "\n"
        self.assertEqual(outputs, [expected, expected])

    def test_capacity_invalids_have_exact_status_empty_stdout_and_one_line(self):
        cases = {case["name"]: case for case in CORPUS["invalid"]}
        for name in ("nine-servers-exceeds-capacity", "257-patients-exceeds-capacity"):
            with self.subTest(name=name):
                result = subprocess.run(
                    [sys.executable, str(ROOT / "oracle.py")],
                    input=acceptance_runner.render_request(cases[name], "invalid"),
                    capture_output=True, check=False,
                )
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, b"")
                self.assertEqual(len(result.stderr.splitlines()), 1)
                self.assertTrue(result.stderr.splitlines()[0].strip())


if __name__ == "__main__":
    unittest.main()
