"""Reusable conformance runner for decision.evaluate adapters (v1 and v2).

Standard library only. A `Target` describes how to start one adapter backend
with an injected behavior ("fault"); `conformance_case(target)` returns a
`unittest.TestCase` class that exercises, for that backend:

  wrong question / candidate / identity   (request-side and, where the backend
                                           has an upstream, upstream-side)
  abstention, timeout, cancellation, crash, excessive output, secret redaction
  over-limit request, unsupported modality, scoreless honesty, v1 compatibility
  choice-select/v1 (MR-11): round trip over decision.evaluate v3, refusal before
                                           inference when v3 was not negotiated or
                                           the task rides another version

Target contract (duck-typed):

  name: str            scoreless: bool          supports_v1: bool
  supports_choice: bool (optional, default True): advertises choice-select/v1
  crash_kind: "process" | "upstream"
  start(fault, **env) -> Running   fault in FAULTS; raises NotImplementedError never:
                                   every fault must be mappable (see FAULTS)

  Running: argv (list), env (dict), secret (str), stop(), posts() -> list of upstream
           requests seen so far (empty for a backend with no upstream),
           wait_in_flight(timeout) -> None, mutate(payload) -> payload (optional
           request corruption for faults that corrupt the request itself).
           Optional terminal-ownership probes (DV-04): handler_dispatches() -> int
           (handler invocations begun so far; falls back to len(posts())) and
           handler_running() -> bool (a handler is executing right now). A target
           without an upstream and without these probes cannot have the dispatch
           count or the reply-while-running rule observed; the reply-shape rules
           (one terminal reply, cancel/deadline status never replaced) still run.

Terminal ownership (DV-04, #564): an early cancel dispatches zero handler work,
each invocation gets exactly one terminal reply, no terminal reply is sent while
the handler still runs, and a late success never replaces a cancel/deadline failure.
"""

import json
import os
import subprocess
import sys
import threading
import time
import unittest

import decision_fixtures as fx

FAULTS = ("ok", "wrong_question", "wrong_candidate", "wrong_identity", "abstain", "slow", "crash", "oversize", "leak")
REFUSALS = ("refused", "failed", "unsupported", "unavailable")


class Session:
    """One adapter subprocess driven over the stdio protocol."""

    def __init__(self, running, versions=(1, 2)):
        self.running = running
        env = {"PATH": os.environ.get("PATH", ""), **running.env}
        self.p = subprocess.Popen(running.argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
        self.send({"jsonrpc": "2.0", "id": 1, "method": "harness/initialize", "params": {
            "protocol": "semaprax.harness-rpc.v1", "offered": [{"kind": "decision.evaluate", "version": v} for v in versions]}})
        self.accepted = self.read()["result"]["accepted"]
        self._dog = threading.Timer(60.0, self.p.kill)  # a hung adapter fails the test, not the run
        self._dog.daemon = True
        self._dog.start()

    def send(self, obj):
        self.p.stdin.write(json.dumps(obj).encode() + b"\n")
        self.p.stdin.flush()

    def read(self):
        line = self.p.stdout.readline()
        return json.loads(line) if line else None

    def invoke(self, req):
        """Result envelope, or None when the process died without answering."""
        self.send({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": req})
        frame = self.read()
        return frame["result"] if frame else None

    def cancel(self, inv):
        self.send({"jsonrpc": "2.0", "method": "harness/cancel", "params": {"invocation_id": inv}})

    def drain(self):
        """Shut down and return every frame still pending, shutdown reply included."""
        self.send({"jsonrpc": "2.0", "id": 9, "method": "harness/shutdown"})
        frames = []
        while True:
            frame = self.read()
            if frame is None:
                return frames
            frames.append(frame)
            if frame.get("id") == 9:
                return frames

    def close(self):
        self._dog.cancel()
        out = err = b""
        try:
            self.send({"jsonrpc": "2.0", "id": 9, "method": "harness/shutdown"})
            self.read()
        except (OSError, ValueError):
            pass
        try:
            out, err = self.p.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            self.p.kill()
            out, err = self.p.communicate()
        self.running.stop()
        return out, err


def conformance_case(target):
    """A TestCase class running the full matrix against `target`."""

    class Conformance(unittest.TestCase):
        maxDiff = None

        def run_fault(self, fault, req, versions=(1, 2), **env):
            running = target.start(fault, **env)
            if hasattr(running, "mutate") and running.mutate:
                req = dict(req, payload=running.mutate(json.loads(json.dumps(req["payload"]))))
            s = Session(running, versions)
            res = s.invoke(req)
            out, err = s.close()
            self.assertNotIn(running.secret.encode(), out + err, "secret leaked on the wire or stderr")
            self.assertNotIn(running.secret, json.dumps(res), "secret leaked in the result")
            return res, running

        def refused(self, res, code=None, statuses=REFUSALS):
            self.assertIsNotNone(res, "adapter produced no result")
            self.assertIn(res["status"], statuses)
            self.assertIsNone(res["payload"], "a refusal must not carry a decision")
            if code:
                self.assertEqual(res["diagnostics"][0]["code"], code)

        def posts(self, running):
            return list(running.posts())

        # -- success shapes ----------------------------------------------------
        def test_valid_v2_result(self):
            req = fx.v2_request()
            res, _ = self.run_fault("ok", req)
            self.assertEqual(res["status"], "complete")
            fx.validate_v2_result(res["payload"], req["payload"], scoreless=target.scoreless)
            self.assertFalse(res["payload"]["abstain"])
            self.assertEqual(res["payload"]["score_kind"] == "none", target.scoreless)

        def test_v1_compatibility(self):
            req = fx.v1_request()
            res, _ = self.run_fault("ok", req)
            if not target.supports_v1:
                return self.refused(res, statuses=("unsupported", "refused"))
            self.assertEqual(res["status"], "complete")
            fx.validate_v1_result(res["payload"], req["payload"]["options"])

        def test_descriptor_negotiates_both_versions(self):
            s = Session(target.start("ok"))
            self.addCleanup(s.close)
            self.assertEqual(sorted(c["version"] for c in s.accepted), [1, 2])

        # -- choice-select/v1 over decision.evaluate v3 (MR-11) -------------
        def test_choice_round_trip_over_v3(self):
            s = Session(target.start("ok"), (1, 2, 3))
            self.addCleanup(s.close)
            advertised = 3 in [c["version"] for c in s.accepted]
            self.assertEqual(advertised, getattr(target, "supports_choice", True))
            if not advertised:
                return
            req = fx.choice_request()
            res = s.invoke(req)
            self.assertEqual(res["status"], "complete", res["diagnostics"])
            fx.validate_choice_result(res["payload"], req["payload"], scoreless=target.scoreless)
            self.assertFalse(res["payload"]["abstain"])

        def test_choice_without_negotiated_v3_is_refused_before_inference(self):
            res, running = self.run_fault("ok", fx.choice_request(), versions=(1, 2))
            self.refused(res, "SPX-HPK004", ("unsupported",))
            self.assertFalse(self.posts(running), "nothing is dispatched for an unnegotiated task")

        def test_choice_task_on_another_version_is_refused(self):
            res, running = self.run_fault("ok", fx.choice_request(version=2), versions=(1, 2, 3))
            self.refused(res, "SPX-HPK004", ("unsupported",))
            self.assertFalse(self.posts(running))

        def test_choice_tampered_options_are_refused(self):
            if not getattr(target, "supports_choice", True):
                self.skipTest("target does not advertise choice-select/v1")
            req = fx.choice_request()
            req["payload"]["options"] = ["c0", "rm -rf /"]
            res, running = self.run_fault("ok", req, versions=(1, 2, 3))
            self.refused(res)
            self.assertFalse(self.posts(running))

        # -- wrong question / candidate / identity ---------------------------
        def test_wrong_question_request(self):
            for task in ("model-route/v9", "tool-select/v1"):
                req = fx.v2_request()
                req["payload"]["task"] = task
                res, running = self.run_fault("ok", req)
                self.refused(res, "SPX-HPK004", ("unsupported",))
                self.assertFalse(self.posts(running), "nothing is dispatched for a refused request")

        def test_wrong_candidate_request(self):
            for mutate in (lambda p: p.__setitem__("options", ["m1", "m0", "m2"]),
                           lambda p: p["candidates"][1].__setitem__("id", "x1"),
                           lambda p: p["candidates"].pop()):
                req = fx.v2_request()
                mutate(req["payload"])
                res, running = self.run_fault("ok", req)
                self.refused(res)
                self.assertFalse(self.posts(running))

        def test_wrong_identity_request_digest(self):
            req = fx.v2_request()
            req["payload"]["rendered"]["state"] += "\ntampered"
            res, running = self.run_fault("ok", req)
            self.refused(res)
            self.assertFalse(self.posts(running))

        def test_wrong_question_candidate_identity_from_backend(self):
            for fault in ("wrong_question", "wrong_candidate", "wrong_identity"):
                res, _ = self.run_fault(fault, fx.v2_request())
                self.refused(res)

        # -- limits and honesty ------------------------------------------------
        def test_over_limit_request_is_refused_before_dispatch(self):
            res, running = self.run_fault("ok", fx.v2_request(max_wire_bytes=10))
            self.refused(res, "SPX-HPK005")
            self.assertFalse(self.posts(running))

        def test_unsupported_modality_is_refused_before_dispatch(self):
            req = fx.v2_request(features=dict(fx.V2_FEATURES, input_modalities=["image", "text"]))
            res, running = self.run_fault("ok", req)
            self.refused(res, statuses=("unsupported",))
            self.assertFalse(self.posts(running))

        def test_scoreless_never_fabricates_scores(self):
            if not target.scoreless:
                self.skipTest("target advertises scores")
            res, _ = self.run_fault("ok", fx.v2_request())
            self.assertIsNone(res["payload"]["scores"])
            self.assertEqual(res["payload"]["score_kind"], "none")

        # -- behaviors ----------------------------------------------------------
        def test_abstention(self):
            res, _ = self.run_fault("abstain", fx.v2_request())
            self.assertEqual(res["status"], "complete")
            p = res["payload"]
            self.assertTrue(p["abstain"])
            self.assertIsNone(p["choice"])
            self.assertIn(p["abstention_reason"], ("native", "host_threshold"))
            fx.validate_v2_result(p, fx.v2_payload(), scoreless=target.scoreless)

        def test_timeout(self):
            t = time.monotonic()
            res, _ = self.run_fault("slow", fx.v2_request(deadline_ms=500))
            self.refused(res, "SPX-HPK011", ("failed",))
            self.assertLess(time.monotonic() - t, 3.0)

        def test_cancellation(self):
            running = target.start("slow")
            s = Session(running)
            s.send({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": fx.v2_request(deadline_ms=20000)})
            running.wait_in_flight(3.0)
            t = time.monotonic()
            s.cancel("inv-000001")
            res = s.read()["result"]
            self.assertLess(time.monotonic() - t, 1.5)
            self.refused(res, "SPX-HPK013")
            s.close()

        # -- terminal ownership (DV-04) -------------------------------------------
        def dispatches(self, running):
            probe = getattr(running, "handler_dispatches", None)
            return probe() if probe else len(self.posts(running))

        def assert_handler_stopped(self, running):
            probe = getattr(running, "handler_running", None)
            if probe:
                self.assertFalse(probe(), "terminal reply sent while the handler still runs")

        def test_early_cancel_dispatches_no_handler_work(self):
            running = target.start("slow")
            s = Session(running)
            s.cancel("inv-000001")  # cancel first: the invoke arrives already cancelled
            s.send({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": fx.v2_request(deadline_ms=20000)})
            t = time.monotonic()
            res = s.read()["result"]
            self.assertLess(time.monotonic() - t, 1.5)
            self.refused(res, "SPX-HPK013")
            self.assertEqual(self.dispatches(running), 0, "an early cancel must dispatch no handler work")
            self.assert_handler_stopped(running)
            frames = s.drain()
            self.assertEqual([f for f in frames if f.get("id") == 2], [], "a second terminal reply for one invocation")
            s.close()

        def test_cancel_sends_exactly_one_terminal_reply_after_handler_stops(self):
            running = target.start("slow")
            s = Session(running)
            s.send({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": fx.v2_request(deadline_ms=20000)})
            running.wait_in_flight(3.0)
            s.cancel("inv-000001")
            s.cancel("inv-000001")  # a repeated cancel must not mint a second reply
            res = s.read()["result"]
            self.refused(res, "SPX-HPK013")  # a late success must not replace the cancel failure
            self.assert_handler_stopped(running)
            time.sleep(0.5)  # a late success would arrive as a second frame
            frames = s.drain()
            self.assertEqual([f for f in frames if f.get("id") == 2], [], "a second terminal reply for one invocation")
            s.close()

        def test_deadline_sends_exactly_one_terminal_reply_after_handler_stops(self):
            running = target.start("slow")
            s = Session(running)
            s.send({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": fx.v2_request(deadline_ms=500)})
            res = s.read()["result"]
            self.refused(res, "SPX-HPK011", ("failed",))  # a late success must not replace the deadline failure
            self.assert_handler_stopped(running)
            s.cancel("inv-000001")  # a cancel after the terminal decision changes nothing
            time.sleep(0.5)
            frames = s.drain()
            self.assertEqual([f for f in frames if f.get("id") == 2], [], "a second terminal reply for one invocation")
            s.close()

        def test_crash(self):
            res, _ = self.run_fault("crash", fx.v2_request())
            if target.crash_kind == "process":
                self.assertIsNone(res, "a crashed adapter must not produce a result frame")
            else:
                self.refused(res)

        def test_excessive_output(self):
            res, _ = self.run_fault("oversize", fx.v2_request())
            self.refused(res, "SPX-HPK007")

        def test_secret_redaction(self):
            res, running = self.run_fault("leak", fx.v2_request())  # run_fault asserts the secret is absent everywhere
            self.assertTrue(running.secret, "a target must configure a secret for this check")
            self.assertIsNotNone(res)

    Conformance.__name__ = Conformance.__qualname__ = "Conformance_" + target.name.replace("-", "_")
    return Conformance
