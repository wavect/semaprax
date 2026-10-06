"""Negative controls for the terminal-ownership conformance rules (DV-04, #564).

The embedded suite must pass the real SDK loop and reject an adapter that breaks
any one rule: dispatch after an early cancel, a second terminal reply, a reply
while the handler still runs, or a late success replacing a cancel/deadline failure.

Run: python3 -m unittest discover -s packages/semaprax-harness-adapters/sdk/python
"""

import io
import os
import shutil
import sys
import tempfile
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import decision_conformance as dc  # noqa: E402

RULES = (
    "test_early_cancel_dispatches_no_handler_work",
    "test_cancel_sends_exactly_one_terminal_reply_after_handler_stops",
    "test_deadline_sends_exactly_one_terminal_reply_after_handler_stops",
)


class Running:
    def __init__(self, mode):
        self.state = tempfile.mkdtemp()
        self.argv = [sys.executable, os.path.join(HERE, "fake_terminal_adapter.py")]
        self.env = {"TERM_MODE": mode, "TERM_STATE": self.state}
        self.secret = "neg-secret-0123456789"

    def stop(self):
        shutil.rmtree(self.state, ignore_errors=True)

    def posts(self):
        return []

    def handler_dispatches(self):
        try:
            with open(os.path.join(self.state, "dispatches")) as f:
                return len(f.read().split())
        except FileNotFoundError:
            return 0

    def handler_running(self):
        return os.path.exists(os.path.join(self.state, "running"))

    def wait_in_flight(self, timeout):
        end = time.monotonic() + timeout
        while self.handler_dispatches() == 0 and time.monotonic() < end:
            time.sleep(0.02)


class Target:
    name = "terminal-negative"
    scoreless, supports_v1, crash_kind = True, False, "process"

    def __init__(self, mode):
        self.mode = mode

    def start(self, fault="ok", **env):
        return Running(self.mode)


def failed_rules(mode):
    case = dc.conformance_case(Target(mode))
    result = unittest.TextTestRunner(stream=io.StringIO()).run(unittest.TestSuite(case(n) for n in RULES))
    return {t.id().rsplit(".", 1)[1] for t, _ in result.failures + result.errors}


class TerminalOwnershipNegativeControls(unittest.TestCase):
    def test_conforming_adapter_passes_every_rule(self):
        self.assertEqual(failed_rules("good"), set())

    def test_dispatch_after_early_cancel_is_rejected(self):
        self.assertIn(RULES[0], failed_rules("dispatch_after_cancel"))

    def test_second_terminal_reply_is_rejected(self):
        self.assertEqual(failed_rules("double_reply"), set(RULES))

    def test_reply_while_handler_runs_is_rejected(self):
        self.assertIn(RULES[1], failed_rules("reply_while_running"))

    def test_late_success_replacing_cancel_or_deadline_is_rejected(self):
        self.assertEqual(failed_rules("late_success"), set(RULES[1:]))


if __name__ == "__main__":
    unittest.main()
