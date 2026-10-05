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
sys.path.insert(0, os.path.join(ROOT, "..", "sdk", "python"))
import decision_fixtures as fx  # noqa: E402

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
            "protocol": "semaprax.harness-rpc.v1", "offered": [{"kind": "decision.evaluate", "version": 1}, {"kind": "decision.evaluate", "version": 2}]}})
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


def post_wire(srv):
    return [x for x in srv.log if x[0] == "POST"][0][2]


class V2(unittest.TestCase):
    """model-route/v2, MR-02 threshold semantics and MR-15 profiles; same fakes as v1."""

    def go(self, flavor, req=None, mode="ok", **env):
        srv = fs.start(flavor, mode)
        self.addCleanup(srv.stop)
        a = Adapter(flavor, env_for(flavor, srv, **env))
        res = a.invoke(req or fx.v2_request())
        out, err = a.close()
        self.assertNotIn(fs.KEY.encode(), out + err)
        return res, srv

    def test_v2_uses_rendered_content_verbatim_and_types_the_call(self):
        req = fx.v2_request()
        r = req["payload"]["rendered"]
        for flavor in ("jev", "laya"):
            res, srv = self.go(flavor, req)
            self.assertEqual(res["status"], "complete")
            fx.validate_v2_result(res["payload"], req["payload"])
            post = post_wire(srv)
            qid = next(iter(post["questions"]))
            q = post["questions"][qid]
            self.assertEqual((post["state"], q["instructions"], q["criteria"]), (r["state"], r["instructions"], r["option_labels"]))
            self.assertEqual(qid, codec.question_id_v2("inv-000001", req["payload"]["options"], r["digest"]))
            sent = json.dumps(post, separators=(",", ":"), sort_keys=True).encode()
            call = res["payload"]["call"]
            self.assertEqual(call["wire_bytes"], len(sent))
            self.assertEqual(call["rendered_digest"], r["digest"])
            self.assertEqual(call["usage"], {"input_tokens": 61, "output_tokens": 0, "basis": "provider_reported"})
            self.assertEqual(res["payload"]["score_kind"], "option_distribution")
        jev, _ = self.go("jev", req)
        self.assertEqual((jev["payload"]["call"]["identity_kind"], jev["payload"]["call"]["billing"], jev["payload"]["call"]["checkpoint"]),
                         ("mutable_service", "api", None))
        self.assertEqual(jev["payload"]["call"]["adapter"], "ai.typesafe/jev-decision@0.2.0")
        self.assertEqual((jev["payload"]["call"]["requested_model"], jev["payload"]["call"]["answering_model"]), ("jev-test-1", "jev-test-1"))
        laya, _ = self.go("laya", req)
        c = laya["payload"]["call"]
        self.assertEqual((c["identity_kind"], c["checkpoint"], c["billing"], c["answering_model"]), ("local_declared", "multilingual", "local", "laya-rl-agent"))
        self.assertEqual((laya["payload"]["native_confidence"], laya["payload"]["native_confidence_kind"]), (0.1, "laya.confidence"))
        self.assertEqual(jev["payload"]["native_confidence_kind"], "jev.confidence")

    def test_latest_alias_is_mutable_and_wrong_checkpoint_refused(self):
        res, _ = self.go("jev", SEMAPRAX_HARNESS_MODEL="jev-latest")
        self.assertEqual(res["payload"]["call"]["identity_kind"], "mutable_service")
        res, _ = self.go("laya", mode="wrong_checkpoint")
        self.assertIsNone(res["payload"])

    def test_v2_refusals_send_nothing(self):
        def tamper_digest(p):
            p["rendered"]["state"] += "x"

        def foreign_option(p):
            p["options"][0] = "opaque-model-id"
        for mutate in (tamper_digest, foreign_option, lambda p: p.__setitem__("excerpt", "x"),
                       lambda p: p["features"].__setitem__("phase", "dream"), lambda p: p.__setitem__("max_wire_bytes", 100),
                       lambda p: p["features"].__setitem__("input_modalities", ["text", "image"])):
            req = fx.v2_request()
            mutate(req["payload"])
            for flavor in ("jev", "laya"):
                res, srv = self.go(flavor, req)
                self.assertIsNone(res["payload"], mutate)
                self.assertIn(res["status"], ("refused", "unsupported"))
                self.assertFalse([x for x in srv.log if x[0] == "POST"])

    def test_min_score_is_host_side_alias_never_forwarded_as_laya_min_confidence(self):
        res, srv = self.go("laya", SEMAPRAX_HARNESS_MIN_SCORE="0.2")
        self.assertNotIn("min_confidence", post_wire(srv))
        self.assertFalse(res["payload"]["abstain"])
        self.assertEqual(res["payload"]["abstention_reason"], "none")
        res, srv = self.go("laya", SEMAPRAX_HARNESS_MIN_SCORE="0.99")
        self.assertEqual((res["payload"]["abstain"], res["payload"]["abstention_reason"], res["payload"]["choice"]), (True, "host_threshold", None))
        self.assertNotIn("min_confidence", post_wire(srv))
        res, _ = self.go("laya", req=request(), SEMAPRAX_HARNESS_MIN_SCORE="0.99")  # v1 keeps its documented behavior
        self.assertTrue(res["payload"]["abstain"])

    def test_native_threshold_is_forwarded_independently_and_both_warn(self):
        res, srv = self.go("laya", SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE="0.4")
        self.assertEqual(post_wire(srv)["min_confidence"], 0.4)
        self.assertEqual([d["code"] for d in res["diagnostics"]], ["SPX-HPK100"])
        self.assertFalse(res["payload"]["abstain"])  # mass is high; only the native threshold changed
        res, srv = self.go("laya", SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE="0.4", SEMAPRAX_HARNESS_MIN_SCORE="0.2")
        self.assertEqual(post_wire(srv)["min_confidence"], 0.4)
        self.assertEqual([d["code"] for d in res["diagnostics"]], ["SPX-HPK100", "SPX-HPK101"])
        self.assertEqual(res["status"], "complete")
        for bad in ("2", "nan", "x"):
            res, srv = self.go("laya", SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE=bad)
            self.assertEqual((res["status"], res["diagnostics"][0]["code"]), ("refused", "SPX-HPK006"))
            self.assertFalse(srv.log)
        res, srv = self.go("jev", SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE="0.4")  # Jev has no native threshold: no silent drop
        self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPK006")

    def test_native_abstention_stays_authoritative(self):
        res, _ = self.go("laya", mode="abstain", SEMAPRAX_HARNESS_MIN_SCORE="0.99")
        self.assertEqual((res["payload"]["abstain"], res["payload"]["abstention_reason"]), (True, "native"))

    def test_native_confidence_metadata_is_validated_not_derived(self):
        self.assertEqual(codec.parse_response({"answers": {"q": {"type": "choice", "choice": "a", "probabilities": {"a": 0.9, "b": 0.1}}}}, "q", ["a", "b"])[1].get("native_confidence"), None)
        doc = {"answers": {"q": {"type": "choice", "choice": "a", "confidence": 7, "probabilities": {"a": 0.9, "b": 0.1}}}}
        payload, info = codec.parse_response(doc, "q", ["a", "b"])
        self.assertEqual(info["native_confidence"], "invalid")


class Profiles(unittest.TestCase):
    """The model profile is data: validated, credential-free, and selects the model."""

    def go(self, flavor, profile, req=None, **env):
        srv = fs.start(flavor)
        self.addCleanup(srv.stop)
        env = env_for(flavor, srv, **env)
        env["SEMAPRAX_HARNESS_MODEL_PROFILE"] = profile if isinstance(profile, str) else json.dumps(profile)
        a = Adapter(flavor, env)
        res = a.invoke(req or fx.v2_request())
        a.close()
        return res, srv

    def profile(self, **kw):
        p = {"profile_id": "p1", "model": "jev-test-1", "checkpoint": None, "identity_kind": "mutable_service",
             "score_kind": "option_distribution", "scoreless": False, "max_options": 16, "max_state_bytes": 4096,
             "modalities": ["text"]}
        p.update(kw)
        return p

    def test_two_profiles_one_adapter_select_different_models(self):
        seen = []
        for model in ("jev-test-1", "jev-test-2"):
            res, srv = self.go("jev", self.profile(model=model, profile_id="id-" + model))
            self.assertEqual(res["status"], "complete")
            self.assertEqual(res["payload"]["call"]["requested_model"], model)
            seen.append(post_wire(srv)["model"])
            self.assertIn("profile=id-" + model, res["diagnostics"][0]["message"])
        self.assertEqual(seen, ["jev-test-1", "jev-test-2"])
        seen = []
        for model in ("multilingual", "english"):
            res, srv = self.go("laya", self.profile(model=model, checkpoint=model, identity_kind="local_declared"))
            self.assertEqual(res["payload"]["call"]["checkpoint"], model)
            seen.append(post_wire(srv)["model"])
        self.assertEqual(seen, ["multilingual", "english"])

    def test_malformed_profiles_are_refused_before_any_request(self):
        bad = ["{not json", "[]", self.profile(endpoint="http://x"), self.profile(api_key="k"), self.profile(scoreless=True),
               self.profile(score_kind="none"), self.profile(identity_kind="immutable_checkpoint"), self.profile(modalities=[]),
               self.profile(modalities=["video"]), self.profile(max_options=0), self.profile(max_options=17),
               self.profile(max_state_bytes=5000), self.profile(model=""), self.profile(identity_kind="trust-me")]
        missing = self.profile()
        del missing["model"]
        bad.append(missing)
        for p in bad:
            res, srv = self.go("jev", p)
            self.assertEqual((res["status"], res["diagnostics"][0]["code"], res["payload"]), ("refused", "SPX-HPK006", None), p)
            self.assertFalse(srv.log, p)

    def test_profile_limits_apply_before_inference(self):
        res, srv = self.go("jev", self.profile(max_options=2))
        self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPK005")
        res, srv = self.go("jev", self.profile(max_state_bytes=10))
        self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPK005")
        res, srv = self.go("jev", self.profile(renderer="other.render.v9"))
        self.assertEqual(res["status"], "unsupported")
        self.assertFalse([x for x in srv.log if x[0] == "POST"])

    def test_scoreless_profile_never_fabricates_scores(self):
        res, _ = self.go("jev", self.profile(score_kind="none", scoreless=True))
        p = res["payload"]
        self.assertEqual((p["scores"], p["score_kind"]), (None, "none"))
        fx.validate_v2_result(p, fx.v2_payload(), scoreless=True)
        res, _ = self.go("jev", self.profile(score_kind="none", scoreless=True), SEMAPRAX_HARNESS_MIN_SCORE="0.5")
        self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPK006")

    def test_pinned_jev_profile_is_immutable_only_when_echoed(self):
        res, _ = self.go("jev", self.profile(identity_kind="immutable_checkpoint", checkpoint="jev-test-1"))
        self.assertEqual((res["payload"]["call"]["identity_kind"], res["payload"]["call"]["checkpoint"]), ("immutable_checkpoint", "jev-test-1"))
        res, _ = self.go("jev", self.profile(identity_kind="immutable_checkpoint", checkpoint="jev-2026-01"))
        self.assertEqual(res["payload"]["call"]["identity_kind"], "mutable_service")

    def test_credentials_stay_out_of_the_profile(self):
        res, srv = self.go("jev", self.profile(), SEMAPRAX_HARNESS_SECRET_JEV=fs.KEY)
        self.assertEqual(res["status"], "complete")
        self.assertNotIn(fs.KEY, json.dumps(self.profile()))


class Codec(unittest.TestCase):
    def test_render_is_deterministic_and_bounded(self):
        a = codec.render_features(FEATURES)
        self.assertEqual(a, codec.render_features(dict(reversed(list(FEATURES.items())))))
        self.assertLessEqual(len(a), codec.MAX_STATE_BYTES)

    def test_v2_digest_canonicalization_is_compact_sorted_utf8(self):
        r = {"renderer": "r", "instructions": "i", "state": "caf\u00e9", "option_labels": {"m0": "m0: a"}}
        text = '{"instructions":"i","option_labels":{"m0":"m0: a"},"renderer":"r","state":"caf\u00e9"}'
        self.assertEqual(codec.canonical_json(r), text.encode("utf-8"))
        self.assertEqual(codec.rendered_digest(r), fx.rendered_digest(r))
        self.assertTrue(codec.rendered_digest(r).startswith("sha256:"))

    def test_question_id_binds_invocation_and_options(self):
        self.assertNotEqual(codec.question_id("a", OPTIONS), codec.question_id("b", OPTIONS))
        self.assertNotEqual(codec.question_id("a", OPTIONS), codec.question_id("a", OPTIONS[:2]))


if __name__ == "__main__":
    unittest.main()
