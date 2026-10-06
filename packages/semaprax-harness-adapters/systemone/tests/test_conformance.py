"""Shared decision-adapter conformance for every implemented backend.

Run: python3 -m unittest discover -s packages/semaprax-harness-adapters/systemone/tests
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ADAPTERS = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(ADAPTERS, "sdk", "python"))
sys.path.insert(0, os.path.join(ADAPTERS, "examples", "decision-adapter-starter"))
import decision_conformance as dc  # noqa: E402
from conformance_target import StarterTarget  # noqa: E402
from conformance_targets import SystemOneTarget  # noqa: E402

TARGETS = [SystemOneTarget("jev"), SystemOneTarget("laya"), StarterTarget(), StarterTarget(scoreless=True)]
for _t in TARGETS:
    _cls = dc.conformance_case(_t)
    globals()[_cls.__name__] = _cls

if __name__ == "__main__":
    unittest.main()
