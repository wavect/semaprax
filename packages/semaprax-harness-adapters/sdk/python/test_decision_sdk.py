"""Negative controls for the decision fixtures/validators and the cancellable serve loop.

Run: python3 -m unittest discover -s packages/semaprax-harness-adapters/sdk/python
"""

import copy
import io
import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import decision_fixtures as fx  # noqa: E402
import semaprax_harness_adapter as sdk  # noqa: E402


def good_result(req, scoreless=False):
    p = req["payload"]
    n = len(p["options"])
    return {
        "choice": "m1", "abstain": False, "abstention_reason": "none",
        "scores": None if scoreless else {o: (0.8 if o == "m1" else 0.2 / (n - 1)) for o in p["options"]},
        "score_kind": "none" if scoreless else "option_distribution", "native_confidence": None, "native_confidence_kind": None,
        "calibration_id": None,
        "call": {"adapter": "x@1", "requested_model": "m", "answering_model": None, "checkpoint": None, "identity_kind": "unknown",
                 "rendered_digest": p["rendered"]["digest"], "wire_bytes": 100,
                 "usage": {"input_tokens": None, "output_tokens": None, "basis": "unknown"}, "billing": "unknown"},
    }


class Validators(unittest.TestCase):
    def test_good_result_passes_and_digest_is_stable(self):
        req = fx.v2_payload()
        req = {"payload": req}
        fx.validate_v2_result(good_result(req), req["payload"])
        fx.validate_v2_result(good_result(req, True), req["payload"], scoreless=True)
        self.assertEqual(fx.v2_payload()["rendered"]["digest"], fx.v2_payload()["rendered"]["digest"])

    def test_bad_results_are_rejected(self):
        req = {"payload": fx.v2_payload()}
        muts = [
            lambda r: r["scores"].__setitem__("m0", float("nan")), lambda r: r["scores"].__setitem__("ghost", 0.0),
            lambda r: r.__setitem__("choice", "m0"), lambda r: r["call"].__setitem__("rendered_digest", "sha256:0"),
            lambda r: r["call"].__setitem__("wire_bytes", 10**9), lambda r: r.__setitem__("native_confidence", 0.5),
            lambda r: r.__setitem__("scores", None), lambda r: r.__setitem__("abstain", True),
        ]
        for m in muts:
            r = copy.deepcopy(good_result(req))
            m(r)
            with self.assertRaises(AssertionError):
                fx.validate_v2_result(r, req["payload"])
        with self.assertRaises(AssertionError):  # a scoreless adapter must not emit scores
            fx.validate_v2_result(good_result(req), req["payload"], scoreless=True)


class Serve(unittest.TestCase):
    def drive(self, handler, req, env):
        frames = [
            {"jsonrpc": "2.0", "id": 1, "method": "harness/initialize", "params": {"protocol": sdk.PROTOCOL, "offered": [{"kind": "decision.evaluate", "version": 2}]}},
            {"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": req},
            {"jsonrpc": "2.0", "id": 9, "method": "harness/shutdown"},
        ]
        stdin = io.BytesIO(b"".join(json.dumps(f).encode() + b"\n" for f in frames))
        stdout = io.BytesIO()
        sdk.serve_cancellable([{"kind": "decision.evaluate", "version": 2, "operations": ["evaluate"]}],
                              {("decision.evaluate", "evaluate"): handler}, {"provider_id": "x", "adapter_version": "1", "upstream_version": "1"},
                              secret_env=("S",), env=env, stdin=stdin, stdout=stdout)
        return [json.loads(x) for x in stdout.getvalue().splitlines()]

    def test_secrets_scrubbed_and_exceptions_typed(self):
        req = fx.v2_request()
        out = self.drive(lambda r, c: ("complete", {"a": 1}, [{"code": "c", "message": "token=hunter2"}]), req, {"S": "hunter2"})
        self.assertNotIn("hunter2", json.dumps(out))
        out = self.drive(lambda r, c: (_ for _ in ()).throw(RuntimeError("hunter2")), req, {"S": "hunter2"})
        res = out[1]["result"]
        self.assertEqual((res["status"], res["diagnostics"][0]["code"]), ("failed", "SPX-HPK099"))
        self.assertNotIn("hunter2", json.dumps(out))

    def test_oversize_payload_is_refused_not_truncated(self):
        req = fx.v2_request(max_bytes=100)
        res = self.drive(lambda r, c: ("complete", {"pad": "x" * 500}, []), req, {})[1]["result"]
        self.assertEqual((res["status"], res["payload"], res["diagnostics"][0]["code"]), ("refused", None, "SPX-HPK007"))


if __name__ == "__main__":
    unittest.main()
