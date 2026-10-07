import json
import subprocess
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CORPUS = json.loads((HERE / "corpus.json").read_text(encoding="utf-8"))
sys.path.insert(0, str(ROOT))
import oracle


class AcceptanceTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
