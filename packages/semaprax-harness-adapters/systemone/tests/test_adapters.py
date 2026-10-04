"""Contract and refusal fixtures for the laya-local and jev-hosted adapters.

Run: python3 -m unittest discover -s packages/semaprax-harness-adapters/systemone/tests
Both adapters face the same fixtures; neither test reaches the real internet.
"""

import json
import os
import subprocess
import sys
import threading
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
sys.path.insert(0, ROOT)
import fake_servers as fs  # noqa: E402
import systemone_codec as codec  # noqa: E402

PROJECT = {"id": "p" * 64, "worktree": "w" * 64, "revision": "r" * 64}
OPTIONS = ["m-cheap", "m-mid", "m-strong"]
FEATURES = {
    "task_family": "localized_debug", "estimated_context_tokens": 12000,
    "requires_structured_output": False, "requires_tools": True,
    "confidentiality": "project", "latency_class": "interactive",
}
FLAVORS = {"laya": "laya-local", "jev": "jev-hosted"}


def request(features=None, options=None, inv="inv-000001", deadline_ms=5000, max_bytes=65536):
    return {
        "schema": "semaprax.harness-request.v1", "invocation_id": inv, "project": PROJECT,
        "lock_digest": "l" * 64, "capability": {"kind": "decision.evaluate", "version": 1},
        "operation": "evaluate", "deadline_ms": deadline_ms,
        "budget": {"max_result_bytes": max_bytes, "remaining_calls": 1}, "lineage": [],
        "payload": {"task": "model-route/v1", "features": FEATURES if features is None else features,
                    "options": OPTIONS if options is None else options},
    }


class Adapter:
    """Drives one adapter subprocess over the stdio protocol."""

    def __init__(self, flavor, env):
        full = {"PATH": os.environ.get("PATH", ""), **env}
        self.p = subprocess.Popen(
            [sys.executable, os.path.join(ROOT, FLAVORS[flavor], "adapter.py")],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=full)
        self.send({"jsonrpc": "2.0", "id": 1, "method": "harness/initialize", "params": {
            "protocol": "semaprax.harness-rpc.v1", "offered": [{"kind": "decision.evaluate", "version": 1}]}})
        self.read()

    def send(self, obj):
        self.p.stdin.write(json.dumps(obj).encode() + b"\n")
        self.p.stdin.flush()

    def read(self):
        return json.loads(self.p.stdout.readline())

    def invoke(self, req):
        self.send({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": req})
        return self.read()["result"]

    def close(self):
        self.send({"jsonrpc": "2.0", "id": 9, "method": "harness/shutdown"})
        self.read()
        out, err = self.p.communicate(timeout=5)
        return out, err


def env_for(flavor, srv, **extra):
    base = {"SEMAPRAX_HARNESS_ENDPOINT": f"http://127.0.0.1:{srv.server_port}"}
    if flavor == "jev":
        base.update({"SEMAPRAX_HARNESS_SECRET_JEV": fs.KEY, "SEMAPRAX_HARNESS_MODEL": "jev-test-1"})
    base.update(extra)
    return base


class Contract(unittest.TestCase):
    flavors = ("laya", "jev")

    def run_case(self, flavor, mode, req=None, env_extra=None, drop=()):
        srv = fs.start(flavor, mode)
        self.addCleanup(srv.stop)
        env = env_for(flavor, srv, **(env_extra or {}))
        for k in drop:
            env.pop(k, None)
        a = Adapter(flavor, env)
        res = a.invoke(req or request())
        out, err = a.close()
        self.assertNotIn(fs.KEY.encode(), out + err)
        self.assertNotIn(fs.KEY, json.dumps(res))
        return res, srv

    def refused(self, res, code, statuses=("refused", "failed", "unsupported", "unavailable")):
        self.assertIn(res["status"], statuses)
        self.assertIsNone(res["payload"], "a refusal must not carry a route")
        self.assertEqual(res["diagnostics"][0]["code"], code)

    def test_accepts_valid_decision_and_sends_only_features(self):
        for f in self.flavors:
            res, srv = self.run_case(f, "ok")
            self.assertEqual(res["status"], "complete")
            p = res["payload"]
            self.assertIn(p["choice"], OPTIONS)
            self.assertFalse(p["abstain"])
            self.assertEqual(set(p["scores"]), set(OPTIONS))
            self.assertEqual(res["invocation_id"], "inv-000001")
            post = [x for x in srv.log if x[0] == "POST"][0][2]
            self.assertEqual(set(post["questions"]), {codec.question_id("inv-000001", OPTIONS)})
            self.assertEqual(post["state"], codec.render_features(FEATURES))
            self.assertLessEqual(len(post["state"].encode()), codec.MAX_STATE_BYTES)
            self.assertIn("latency_ms=", res["diagnostics"][0]["message"])
            allowed = {"state", "model", "questions"} | ({"lang"} if f == "laya" else set())
            self.assertEqual(set(post), allowed)

    def test_nan_scores(self):
        for f in self.flavors:
            res, _ = self.run_case(f, "nan")
            self.refused(res, "SPX-HPK009")

    def test_out_of_range_scores(self):
        for f in self.flavors:
            res, _ = self.run_case(f, "range")
            self.refused(res, "SPX-HPK009")

    def test_unknown_option_choice(self):
        for f in self.flavors:
            res, _ = self.run_case(f, "unknown_option")
            self.refused(res, "SPX-HPK010")

    def test_unknown_probability_key(self):
        for f in self.flavors:
            res, _ = self.run_case(f, "unknown_prob_key")
            self.refused(res, "SPX-HPK010")

    def test_mismatched_request_binding(self):
        for f in self.flavors:
            res, _ = self.run_case(f, "binding")
            self.refused(res, "SPX-HPK008")

    def test_wrong_answer_type_and_bad_json(self):
        for f in self.flavors:
            self.refused(self.run_case(f, "wrong_type")[0], "SPX-HPK008")
            self.refused(self.run_case(f, "bad_json")[0], "SPX-HPK008")

    def test_oversize_response(self):
        for f in self.flavors:
            res, _ = self.run_case(f, "oversize")
            self.refused(res, "SPX-HPK007")

    def test_timeout(self):
        for f in self.flavors:
            t = time.monotonic()
            res, _ = self.run_case(f, "slow", req=request(deadline_ms=500))
            self.refused(res, "SPX-HPK011", ("failed",))
            self.assertLess(time.monotonic() - t, 2.5)

    def test_cancellation(self):
        for f in self.flavors:
            srv = fs.start(f, "slow")
            self.addCleanup(srv.stop)
            a = Adapter(f, env_for(f, srv))
            a.send({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": request(deadline_ms=20000)})
            # Jev first lists models, so wait until the slow POST is in flight.
            for _ in range(100):
                if any(x[0] == "POST" for x in srv.log):
                    break
                time.sleep(0.02)
            t = time.monotonic()
            a.send({"jsonrpc": "2.0", "method": "harness/cancel", "params": {"invocation_id": "inv-000001"}})
            res = a.read()["result"]
            self.assertLess(time.monotonic() - t, 1.5)
            self.refused(res, "SPX-HPK013")
            a.close()

    def test_unsupported_language_and_profile(self):
        for f in self.flavors:
            res, srv = self.run_case(f, "ok", req=request(features={**FEATURES, "language": "de"}))
            self.refused(res, "SPX-HPK004")
            res, _ = self.run_case(f, "ok", req=request(features={**FEATURES, "profile": "tool-select/v1"}))
            self.refused(res, "SPX-HPK004")
            res, _ = self.run_case(f, "ok", req=request(features={**FEATURES, "project_text": "secret source"}))
            self.refused(res, "SPX-HPK004")
            self.assertFalse([x for x in srv.log if x[0] == "POST"])
            bad = dict(FEATURES, task_family="x" * 200)
            self.refused(self.run_case(f, "ok", req=request(features=bad))[0], "SPX-HPK005")

    def test_overlong_features_are_refused_not_truncated(self):
        for f in self.flavors:
            many = [f"model-{i}" for i in range(codec.MAX_OPTIONS + 1)]
            res, srv = self.run_case(f, "ok", req=request(options=many))
            self.refused(res, "SPX-HPK005")
            long_ids = ["a" * 100] * 7
            long_ids = [c * 100 for c in "abcdefg"]
            res, srv = self.run_case(f, "ok", req=request(options=long_ids))
            self.refused(res, "SPX-HPK005")
            self.assertFalse([x for x in srv.log if x[0] == "POST"], "nothing is sent for a refused input")

    def test_missing_mandatory_feature(self):
        for f in self.flavors:
            feats = dict(FEATURES)
            del feats["requires_tools"]
            self.refused(self.run_case(f, "ok", req=request(features=feats))[0], "SPX-HPK006")

    def test_unregistered_task(self):
        for f in self.flavors:
            req = request()
            req["payload"]["task"] = "tool-select/v1"
            self.refused(self.run_case(f, "ok", req=req)[0], "SPX-HPK004")

    def test_abstention_is_not_an_accepted_route(self):
        res, _ = self.run_case("laya", "abstain")
        self.assertEqual(res["status"], "complete")
        self.assertTrue(res["payload"]["abstain"])
        self.assertIsNone(res["payload"]["choice"])
        res, _ = self.run_case("jev", "ok", env_extra={"SEMAPRAX_HARNESS_MIN_SCORE": "0.99"})
        self.assertTrue(res["payload"]["abstain"])

    def test_wrong_checkpoint_refused(self):
        res, _ = self.run_case("laya", "wrong_checkpoint")
        self.refused(res, "SPX-HPK008")

    def test_server_error_status_does_not_echo_body(self):
        for f in self.flavors:
            res, _ = self.run_case(f, "http500")
            self.refused(res, "SPX-HPK012", ("failed",))

    def test_server_absent_is_unavailable_never_started(self):
        for f in self.flavors:
            srv = fs.start(f)
            env = env_for(f, srv)
            srv.shutdown()
            srv.server_close()
            a = Adapter(f, env)
            res = a.invoke(request())
            a.close()
            self.refused(res, "SPX-HPK016", ("unavailable",))


class LayaOnly(unittest.TestCase):
    def test_non_loopback_and_missing_endpoint_refused(self):
        for ep in ("http://10.1.2.3:8000", "https://127.0.0.1:8000", "http://127.0.0.1:8000/x", None):
            env = {} if ep is None else {"SEMAPRAX_HARNESS_ENDPOINT": ep}
            a = Adapter("laya", env)
            res = a.invoke(request())
            a.close()
            self.assertEqual(res["status"], "refused")
            self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPK002")


class JevOnly(unittest.TestCase):
    def test_remote_requires_host_approval(self):
        a = Adapter("jev", {"SEMAPRAX_HARNESS_SECRET_JEV": fs.KEY, "SEMAPRAX_HARNESS_MODEL": "jev-test-1"})
        res = a.invoke(request())
        out, err = a.close()
        self.assertEqual((res["status"], res["diagnostics"][0]["code"]), ("refused", "SPX-HPK001"))
        self.assertNotIn(fs.KEY.encode(), out + err)

    def test_remote_http_refused_even_when_approved(self):
        a = Adapter("jev", {"SEMAPRAX_HARNESS_SECRET_JEV": fs.KEY, "SEMAPRAX_HARNESS_MODEL": "m",
                            "SEMAPRAX_HARNESS_REMOTE_APPROVED": "1", "SEMAPRAX_HARNESS_ENDPOINT": "http://example.invalid"})
        res = a.invoke(request())
        a.close()
        self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPK002")

    def test_missing_secret_and_model(self):
        srv = fs.start("jev")
        self.addCleanup(srv.stop)
        for drop, code in (("SEMAPRAX_HARNESS_SECRET_JEV", "SPX-HPK003"), ("SEMAPRAX_HARNESS_MODEL", "SPX-HPK014")):
            env = env_for("jev", srv)
            del env[drop]
            a = Adapter("jev", env)
            res = a.invoke(request())
            a.close()
            self.assertEqual((res["status"], res["diagnostics"][0]["code"]), ("refused", code))
        self.assertFalse(srv.log)

    def test_wrong_key_and_unentitled_model(self):
        srv = fs.start("jev")
        self.addCleanup(srv.stop)
        a = Adapter("jev", env_for("jev", srv, SEMAPRAX_HARNESS_SECRET_JEV="wrong-key-value"))
        res = a.invoke(request())
        out, err = a.close()
        self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPK012")
        self.assertNotIn(b"wrong-key-value", out + err)
        self.assertNotIn(fs.KEY, json.dumps(res))
        a = Adapter("jev", env_for("jev", srv, SEMAPRAX_HARNESS_MODEL="jev-not-mine"))
        res = a.invoke(request())
        a.close()
        self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPK014")

    def test_key_only_in_authorization_header(self):
        srv = fs.start("jev")
        self.addCleanup(srv.stop)
        a = Adapter("jev", env_for("jev", srv))
        a.invoke(request())
        a.close()
        for method, path, body, auth in srv.log:
            self.assertEqual(auth, "Bearer " + fs.KEY)
            self.assertNotIn(fs.KEY, json.dumps(body))


class Codec(unittest.TestCase):
    def test_render_is_deterministic_and_bounded(self):
        a = codec.render_features(FEATURES)
        self.assertEqual(a, codec.render_features(dict(reversed(list(FEATURES.items())))))
        self.assertLessEqual(len(a), codec.MAX_STATE_BYTES)

    def test_question_id_binds_invocation_and_options(self):
        self.assertNotEqual(codec.question_id("a", OPTIONS), codec.question_id("b", OPTIONS))
        self.assertNotEqual(codec.question_id("a", OPTIONS), codec.question_id("a", OPTIONS[:2]))


if __name__ == "__main__":
    unittest.main()
