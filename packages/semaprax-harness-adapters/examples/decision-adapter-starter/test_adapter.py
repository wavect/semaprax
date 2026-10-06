"""Starter adapter: determinism plus the shared conformance matrix (scoring and scoreless).

Run: python3 -m unittest discover -s packages/semaprax-harness-adapters/examples/decision-adapter-starter
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, "..", "..", "sdk", "python"))
import adapter  # noqa: E402
import decision_conformance as dc  # noqa: E402
from conformance_target import StarterTarget  # noqa: E402

for _t in (StarterTarget(), StarterTarget(scoreless=True)):
    _cls = dc.conformance_case(_t)
    globals()[_cls.__name__] = _cls


class Scorer(unittest.TestCase):
    def test_deterministic_and_keyword_driven(self):
        labels = {"m0": "m0: economy tier", "m1": "m1: frontier tools"}
        a = adapter.score_options(labels, {"frontier", "tools"}, False)
        self.assertEqual(a, adapter.score_options(labels, {"frontier", "tools"}, False))
        self.assertEqual(a[0], "m1")
        self.assertAlmostEqual(sum(a[1].values()), 1.0, places=5)

    def test_ties_pick_the_first_and_scoreless_has_no_scores(self):
        choice, scores = adapter.score_options({"m0": "a", "m1": "b"}, set(), True)
        self.assertEqual((choice, scores), ("m0", None))


if __name__ == "__main__":
    unittest.main()
