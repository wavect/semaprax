"""Mini Jev adapter: contract tests against a counting fake engine (no torch, no model, no network).

Run: python3 -m unittest discover -s packages/semaprax-harness-adapters/minijev-local/tests
"""

import copy
import json
import math
import os
import socket
import sys
import threading
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..")
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "..", "sdk", "python"))
sys.path.insert(0, os.path.join(ROOT, "..", "systemone"))
import adapter  # noqa: E402
import decision_conformance as dc  # noqa: E402
import decision_fixtures as fx  # noqa: E402
import minijev_wire as w  # noqa: E402
from fake_engine import FakeEngine, finish  # noqa: E402
from semaprax_harness_adapter import AdapterError  # noqa: E402
from worker import Worker  # noqa: E402

def _read(name):
    with open(os.path.join(ROOT, name)) as f:
        return f.read()


TOKEN = "minijev-test-token-0123456789"


class Crash(BaseException):
    """Escapes the worker's engine-fault handler: simulates the worker process dying mid-call."""


class Rig:
    """An in-process worker around a fake engine plus the adapter environment for it."""

    def __init__(self, engine=None, max_queue=4, token=TOKEN, **env):
        self.engine = engine or FakeEngine()
        self.worker = Worker(self.engine, token, max_queue=max_queue)
        self.thread = self.worker.start_background()
        host, port = self.worker.address
        self.env = {"SEMAPRAX_HARNESS_MINIJEV_ADDR": f"{host}:{port}", adapter.SECRET_ENV: TOKEN,
                    "SEMAPRAX_HARNESS_MINIJEV_ALLOW_FAKE": "1", **env}

    def stop(self):
        self.engine.release.set()
        self.worker.stop()
        self.thread.join(timeout=6)

    def call(self, req=None, cancelled=None, **kw):
        return adapter.handle(req or fx.v2_request(**kw), cancelled or threading.Event(), self.env)


class Base(unittest.TestCase):
    def rig(self, **kw):
        r = Rig(**kw)
        self.addCleanup(r.stop)
        return r

    def _try(self, r):
        try:
            r.call()
        except AdapterError:
            pass

    def refused(self, fn, status, code=None):
        with self.assertRaises(AdapterError) as cm:
            fn()
        self.assertEqual(cm.exception.status, status)
        if code:
            self.assertEqual(cm.exception.code, code)
        return cm.exception


class Decision(Base):
    def test_exactly_one_scoring_call_and_no_generation(self):
        r = self.rig()
        req = fx.v2_request()
        status, out, diags = r.call(req)
        self.assertEqual(status, "complete")
        fx.validate_v2_result(out, req["payload"])
        self.assertEqual(r.engine.counts["score"], 1)
        self.assertTrue(all(v == 0 for k, v in r.engine.counts.items() if k != "score"), r.engine.counts)
        self.assertEqual(out["score_kind"], "candidate_relative")
        self.assertEqual(out["call"]["billing"], "local")
        self.assertEqual(out["call"]["identity_kind"], "local_declared")
        self.assertEqual(out["call"]["usage"]["basis"], "local_measured")
        self.assertEqual(diags[0]["code"], "SPX-HPK100")
        self.assertIn("experimental=true", diags[0]["message"])

    def test_forbidden_engine_calls_are_counted_and_fail(self):
        e = FakeEngine()
        with self.assertRaises(AssertionError):
            e.generate("x")
        self.assertEqual(e.counts["generate"], 1)

    def test_selects_between_two_models_and_maps_letters_back(self):
        r = self.rig(engine=FakeEngine(scripted=lambda u, k: finish([0.1, 3.0], 9)))
        req = fx.v2_request(n=2)
        _, out, _ = r.call(req)
        self.assertEqual(out["choice"], "m1")
        self.assertEqual(set(out["scores"]), {"m0", "m1"})
        user, k = r.engine.users[0]
        self.assertEqual(k, 2)
        self.assertIn("\nA = m0: ", user)
        self.assertIn("\nB = m1: ", user)
        self.assertTrue(user.endswith("\n\nANSWER:"))
        self.assertIn(req["payload"]["rendered"]["state"], user)

    def test_wire_bytes_is_the_sent_score_line(self):
        r = self.rig()
        req = fx.v2_request()
        _, out, _ = r.call(req)
        self.assertEqual(out["call"]["wire_bytes"], len(w.dumps({"v": 1, "op": "score", "k": 3, "user": w.render_user(req["payload"]["rendered"])})))

    def test_tie_is_native_abstention(self):
        r = self.rig(engine=FakeEngine(scripted=lambda u, k: finish([2.0, 2.0, 1.0], 5)))
        _, out, _ = r.call()
        self.assertTrue(out["abstain"])
        self.assertIsNone(out["choice"])
        self.assertEqual(out["abstention_reason"], "native")
        fx.validate_v2_result(out, fx.v2_payload())

    def test_native_min_confidence_abstains(self):
        r = self.rig(engine=FakeEngine(scripted=lambda u, k: finish([1.0, 0.9, 0.8], 5)), SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE="0.5")
        _, out, _ = r.call()
        self.assertTrue(out["abstain"])
        self.assertEqual(out["native_confidence_kind"], "minijev.choice_confidence")
        r2 = self.rig(engine=FakeEngine(scripted=lambda u, k: finish([6.0, 0.0, 0.0], 5)), SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE="0.5")
        self.assertFalse(r2.call()[1]["abstain"])


class Identity(Base):
    def test_identity_is_pinned_in_the_call_record(self):
        r = self.rig()
        _, out, _ = r.call()
        ident = r.engine.identity
        self.assertEqual(out["call"]["checkpoint"], w.composite_checkpoint(ident))
        for part in (ident["revision"], ident["tokenizer_sha"][:12], ident["code_commit"][:12], w.renderer_pin(ident)):
            self.assertIn(part, out["call"]["checkpoint"])
        self.assertLessEqual(len(out["call"]["checkpoint"]), 128)
        self.assertIn(w.PINNED_CODE_COMMIT, adapter.PROVENANCE["upstream_version"])

    def test_every_identity_component_changes_the_checkpoint(self):
        seen = set()
        for key, val in (("revision", "9" * 40), ("tokenizer_sha", "8" * 64), ("system_sha", "7" * 64)):
            e = FakeEngine()
            e.identity[key] = val
            r = self.rig(engine=e)
            seen.add(r.call()[1]["call"]["checkpoint"])
        seen.add(self.rig().call()[1]["call"]["checkpoint"])
        self.assertEqual(len(seen), 4)

    def test_pinned_profile_refuses_changed_identity_before_scoring(self):
        base = self.rig()
        ident = base.engine.identity
        profile = {"profile_id": "pin", "model": ident["model"], "checkpoint": w.composite_checkpoint(ident), "identity_kind": "local_declared",
                   "score_kind": "candidate_relative", "scoreless": False, "max_options": 16, "max_state_bytes": 4096, "modalities": ["text"]}
        self.assertEqual(self.rig(**{"SEMAPRAX_HARNESS_MODEL_PROFILE": json.dumps(profile)}).call()[0], "complete")
        for key, val in (("revision", "9" * 40), ("tokenizer_sha", "8" * 64), ("system_sha", "7" * 64), ("model", "other-model")):
            e = FakeEngine()
            e.identity = dict(ident, **{key: val})
            r = self.rig(engine=e, **{"SEMAPRAX_HARNESS_MODEL_PROFILE": json.dumps(profile)})
            self.refused(r.call, "refused", "SPX-HPK009")
            self.assertEqual(e.counts["score"], 0, "identity is checked before any scoring")

    def test_foreign_code_commit_and_incomplete_identity_refuse(self):
        for key, val in (("code_commit", "0" * 40), ("revision", "main"), ("tokenizer_sha", "")):
            e = FakeEngine()
            e.identity[key] = val
            r = self.rig(engine=e)
            self.refused(r.call, "refused", "SPX-HPK009")
            self.assertEqual(e.counts["score"], 0)

    def test_fake_engine_is_refused_unless_allowed(self):
        r = self.rig()
        env = {k: v for k, v in r.env.items() if k != "SEMAPRAX_HARNESS_MINIJEV_ALLOW_FAKE"}
        self.refused(lambda: adapter.handle(fx.v2_request(), threading.Event(), env), "unsupported", "SPX-HPK004")
        self.assertEqual(r.engine.counts["score"], 0)

    def test_identity_change_during_call_is_refused(self):
        e = FakeEngine()
        def flip(u, k):
            e.identity = dict(e.identity, revision="5" * 40)
            return finish([1.0, 2.0, 3.0], 3)
        e.scripted = flip
        self.refused(self.rig(engine=e).call, "refused", "SPX-HPK009")

    def test_profile_must_be_candidate_relative_scoring(self):
        r = self.rig()
        i = r.engine.identity
        bad = {"profile_id": "p", "model": i["model"], "checkpoint": "x", "identity_kind": "immutable_checkpoint", "score_kind": "option_distribution",
               "scoreless": False, "max_options": 16, "max_state_bytes": 4096, "modalities": ["text"]}
        r.env["SEMAPRAX_HARNESS_MODEL_PROFILE"] = json.dumps(bad)
        self.refused(r.call, "refused", "SPX-HPK006")


class Inputs(Base):
    def assert_no_scoring(self, r):
        self.assertEqual(r.engine.counts["score"], 0)

    def test_duplicate_and_foreign_options_refused(self):
        r = self.rig()
        for mutate in (lambda p: p.__setitem__("options", ["m0", "m0", "m2"]),
                       lambda p: p["candidates"][2].__setitem__("id", "m1"),
                       lambda p: p.__setitem__("options", ["m0", "m1", "gpt-x"]),
                       lambda p: p["candidates"].pop()):
            req = fx.v2_request()
            mutate(req["payload"])
            self.refused(lambda: r.call(req), "refused")
        labels = fx.v2_request()
        labels["payload"]["rendered"]["option_labels"]["m9"] = labels["payload"]["rendered"]["option_labels"].pop("m2")
        fx.retag_digest(labels["payload"])
        self.refused(lambda: r.call(labels), "refused")
        self.assert_no_scoring(r)

    def test_too_many_options_single_option_and_image(self):
        r = self.rig()
        self.refused(lambda: r.call(n=17), "refused", "SPX-HPK005")
        self.refused(lambda: r.call(n=1), "unsupported", "SPX-HPK004")
        self.refused(lambda: r.call(features=dict(fx.V2_FEATURES, input_modalities=["image", "text"])), "unsupported", "SPX-HPK004")
        self.assert_no_scoring(r)

    def test_excessive_context_refused_before_the_worker(self):
        r = self.rig()
        req = fx.v2_request()
        req["payload"]["rendered"]["state"] = "x" * 4097
        fx.retag_digest(req["payload"])
        self.refused(lambda: r.call(req), "refused", "SPX-HPK005")
        self.refused(lambda: r.call(max_wire_bytes=100), "refused", "SPX-HPK005")
        self.assert_no_scoring(r)

    def test_v1_and_wrong_task_unsupported(self):
        r = self.rig()
        self.refused(lambda: r.call(fx.v1_request()), "unsupported", "SPX-HPK004")
        self.assert_no_scoring(r)

    def test_wrong_renderer_unsupported(self):
        r = self.rig()
        req = fx.v2_request()
        req["payload"]["rendered"]["renderer"] = "other.renderer"
        fx.retag_digest(req["payload"])
        self.refused(lambda: r.call(req), "unsupported", "SPX-HPK004")
        self.assert_no_scoring(r)


class WorkerRefusals(Base):
    def test_unsupported_encoding_and_engine_length_refusals(self):
        def boom(kind):
            def f(u, k):
                raise ValueError(kind)
            return f
        self.refused(self.rig(engine=FakeEngine(scripted=boom("unsupported"))).call, "unsupported", "SPX-HPK004")
        self.refused(self.rig(engine=FakeEngine(scripted=boom("too_long"))).call, "refused", "SPX-HPK005")
        r = self.rig(engine=FakeEngine(scripted=boom("x")))
        self.refused(r.call, "unsupported")

    def test_engine_fault_is_one_failed_call_and_worker_survives(self):
        n = {"i": 0}
        def f(u, k):
            n["i"] += 1
            if n["i"] == 1:
                raise RuntimeError(TOKEN)
            return finish([1.0, 2.0, 3.0], 3)
        r = self.rig(engine=FakeEngine(scripted=f))
        e = self.refused(r.call, "failed", "SPX-HPK012")
        self.assertNotIn(TOKEN, e.message)
        self.assertEqual(r.call()[0], "complete")

    def test_malformed_scores_never_become_routes(self):
        good = finish([1.0, 2.0, 3.0], 3)
        cases = {
            "nan": dict(good, logits=[math.nan, 1.0, 2.0]),
            "inf": dict(good, logits=[math.inf, 1.0, 2.0]),
            "short": dict(good, logits=[1.0, 2.0], p_cand=[0.4, 0.6]),
            "long": finish([1.0, 2.0, 3.0, 4.0], 3),
            "prob_range": dict(good, p_cand=[0.1, 0.2, 1.5]),
            "prob_vs_logits": dict(good, p_cand=[0.8, 0.1, 0.1]),
            "wrong_argmax": dict(good, pred_pos=0),
            "bad_pos_type": dict(good, pred_pos="2"),
            "lying_tie": dict(good, tie=True),
            "missing": {k: v for k, v in good.items() if k != "logits"},
            "strings": dict(good, logits=["1", "2", "3"]),
        }
        for name, doc in cases.items():
            r = self.rig(engine=FakeEngine(scripted=lambda u, k, d=doc: d))
            e = self.refused(r.call, "refused", "SPX-HPK009")
            self.assertEqual(e.status, "refused", name)

    def test_oversize_worker_response_refused(self):
        r = self.rig(engine=FakeEngine(scripted=lambda u, k: dict(finish([1.0, 2.0, 3.0], 3), pad="x" * 20000)))
        self.refused(r.call, "refused", "SPX-HPK007")

    def test_worker_busy_is_unavailable(self):
        e = FakeEngine(delay=20)
        r = self.rig(engine=e, max_queue=0)
        t = threading.Thread(target=lambda: self._try(r), daemon=True)
        t.start()
        for _ in range(100):
            if e.counts["score"]:
                break
            time.sleep(0.02)
        self.refused(r.call, "unavailable", "SPX-HPK016")
        e.release.set()
        t.join(timeout=5)

    def test_wrong_token_refused_and_non_loopback_address_refused(self):
        r = self.rig()
        r.env[adapter.SECRET_ENV] = "wrong-token-0123456789"
        self.refused(r.call, "refused", "SPX-HPK012")
        r.env["SEMAPRAX_HARNESS_MINIJEV_ADDR"] = "10.0.0.5:9"
        self.refused(r.call, "refused", "SPX-HPK002")
        del r.env[adapter.SECRET_ENV]
        self.refused(r.call, "refused", "SPX-HPK003")
        self.assertEqual(r.engine.counts["score"], 0)

    def test_worker_rejects_oversize_user_before_tokenization_and_bad_k(self):
        r = self.rig()
        host, port = r.worker.address
        def ask(msg):
            s = socket.create_connection((host, port), timeout=3)
            s.sendall(w.dumps({"auth": TOKEN}) + b"\n" + w.dumps(msg) + b"\n")
            data = s.makefile().readline()
            s.close()
            return json.loads(data)
        self.assertEqual(ask({"v": 1, "op": "score", "k": 2, "user": "x" * 9000})["code"], "too_long")
        self.assertEqual(ask({"v": 1, "op": "score", "k": 17, "user": "x"})["code"], "unsupported")
        self.assertEqual(ask({"v": 1, "op": "score", "k": 1, "user": "x"})["code"], "unsupported")
        self.assertEqual(ask({"v": 2, "op": "ping"})["code"], "bad_request")
        self.assertEqual(r.engine.counts["score"], 0)

    def test_worker_refuses_non_loopback_bind_and_weak_token(self):
        with self.assertRaises(ValueError):
            Worker(FakeEngine(), TOKEN, host="0.0.0.0")
        with self.assertRaises(ValueError):
            Worker(FakeEngine(), "short")


class Availability(Base):
    def test_dead_worker_is_unavailable_and_nothing_is_started(self):
        s = socket.socket()
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
        s.close()
        env = {"SEMAPRAX_HARNESS_MINIJEV_ADDR": f"127.0.0.1:{port}", adapter.SECRET_ENV: TOKEN}
        self.refused(lambda: adapter.handle(fx.v2_request(), threading.Event(), env), "unavailable", "SPX-HPK016")

    def test_worker_dying_mid_call_is_unavailable(self):
        def die(u, k):
            raise Crash()
        r = self.rig(engine=FakeEngine(scripted=die))
        self.refused(r.call, "unavailable", "SPX-HPK016")

    def test_cancellation_returns_promptly(self):
        e = FakeEngine(delay=30)
        r = self.rig(engine=e)
        ev = threading.Event()
        threading.Timer(0.3, ev.set).start()
        t = time.monotonic()
        self.refused(lambda: r.call(cancelled=ev), "refused", "SPX-HPK013")
        self.assertLess(time.monotonic() - t, 2.0)
        self.assertEqual(e.counts["score"], 1, "no second hidden call")

    def test_cancelled_before_start_never_connects(self):
        r = self.rig()
        ev = threading.Event()
        ev.set()
        self.refused(lambda: r.call(cancelled=ev), "refused", "SPX-HPK013")
        self.assertEqual(r.engine.counts["score"], 0)

    def test_exhausted_deadline(self):
        e = FakeEngine(delay=30)
        r = self.rig(engine=e)
        t = time.monotonic()
        self.refused(lambda: r.call(deadline_ms=300), "failed", "SPX-HPK011")
        self.assertLess(time.monotonic() - t, 2.0)
        self.assertEqual(e.counts["score"], 1)

    def test_queued_cancelled_request_does_not_spend_a_forward_pass(self):
        e = FakeEngine(delay=20)
        r = self.rig(engine=e, max_queue=2)
        t = threading.Thread(target=lambda: self._try(r), daemon=True)
        t.start()
        for _ in range(100):
            if e.counts["score"]:
                break
            time.sleep(0.02)
        ev = threading.Event()
        threading.Timer(0.3, ev.set).start()
        self.refused(lambda: r.call(cancelled=ev), "refused", "SPX-HPK013")
        e.release.set()
        t.join(timeout=5)
        time.sleep(0.3)
        self.assertEqual(e.counts["score"], 1, "the queued, cancelled call never reached the engine")


class DemoCommand(Base):
    def test_decide_once_runs_through_the_frame_protocol(self):
        import subprocess
        r = self.rig(engine=FakeEngine(scripted=lambda u, k: finish([0.2, 2.2], 7)))
        env = {"PATH": os.environ["PATH"], **r.env}
        out = subprocess.run([sys.executable, os.path.join(ROOT, "decide_once.py")], env=env, capture_output=True, text=True, timeout=60)
        self.assertEqual(out.returncode, 0, out.stderr)
        res = json.loads(out.stdout)
        self.assertEqual((res["status"], res["payload"]["choice"]), ("complete", "m1"))
        self.assertEqual(r.engine.counts["score"], 1)


class NoDownloads(unittest.TestCase):
    def test_adapter_has_no_network_process_or_installer_code(self):
        for name in ("adapter.py", "minijev_wire.py"):
            src = _read(name)
            for needle in ("urllib", "http.client", "requests", "subprocess", "pip ", "huggingface", "from_pretrained", "os.system", "Popen"):
                self.assertNotIn(needle, src, f"{name} must not contain {needle}")

    def test_real_engine_is_offline_and_never_generates(self):
        src = _read("worker.py")
        self.assertIn('os.environ["HF_HUB_OFFLINE"] = "1"', src)
        body = src.split("class RealEngine")[1].split("def main")[0]
        for needle in (".generate(", "run_json", "run_split", "run_labels", "sequence_logprobs", "full_logits_from_hidden", "snapshot_download"):
            self.assertNotIn(needle, body)
        self.assertIn("candidate_logits_fp32", body)
        self.assertIn("L.score(", body)

    def test_letter_table_bounds(self):
        self.assertEqual(w.letter(0), "A")
        self.assertEqual(w.letter(15), "P")
        with self.assertRaises(ValueError):
            w.letter(26)

    def test_descriptor_is_v2_local_loopback_and_untested(self):
        d = json.loads(_read("harness-provider.json"))
        self.assertEqual([c["version"] for c in d["capabilities"]], [2])
        self.assertEqual(d["permissions"]["network"], ["loopback:user-selected-worker"])
        self.assertEqual(d["permissions"]["process"], [])
        self.assertEqual(d["support"]["tested"], [])
        self.assertEqual(d["provider"]["id"], adapter.PROVIDER_ID)
        self.assertEqual(d["adapter"]["version"], adapter.ADAPTER_VERSION)
        self.assertIn(w.PINNED_CODE_COMMIT, d["upstream"]["package"])


# -- shared SDK conformance matrix -------------------------------------------------
class _Running:
    def __init__(self, rig, mutate):
        self.rig, self.mutate = rig, mutate
        self.argv = [sys.executable, os.path.join(ROOT, "adapter.py")]
        self.env, self.secret = rig.env if rig else {}, TOKEN

    def stop(self):
        if self.rig:
            self.rig.stop()

    def posts(self):
        return [] if not self.rig else list(self.rig.engine.users)

    def wait_in_flight(self, timeout):
        end = time.monotonic() + timeout
        while time.monotonic() < end and self.rig and not self.rig.engine.counts["score"]:
            time.sleep(0.02)


class MiniJevTarget:
    name, scoreless, supports_v1, crash_kind = "minijev", False, False, "upstream"

    def start(self, fault="ok", **env):
        mut = {"wrong_question": lambda p: dict(p, task="model-route/v9"),
               "wrong_candidate": lambda p: dict(p, options=list(reversed(p["options"]))),
               "wrong_identity": lambda p: dict(p, rendered=dict(p["rendered"], digest="sha256:" + "0" * 64))}.get(fault)
        if fault == "crash":
            def die(u, k):
                raise Crash()
            return _Running(Rig(FakeEngine(scripted=die)), None)
        engine = FakeEngine()
        if fault == "slow":
            engine = FakeEngine(delay=30)
        elif fault == "oversize":
            engine = FakeEngine(scripted=lambda u, k: dict(finish([1.0, 2.0, 3.0], 3), pad="x" * 20000))
        elif fault == "abstain":
            engine = FakeEngine(scripted=lambda u, k: finish([1.0, 1.0, 0.0], 3))
        elif fault == "leak":
            def leak(u, k):
                raise ValueError(TOKEN)
            engine = FakeEngine(scripted=leak)
        rig = Rig(engine)
        rig.env.update(env)
        return _Running(rig, mut)


class Conformance_minijev(dc.conformance_case(MiniJevTarget())):
    def test_descriptor_negotiates_both_versions(self):
        s = dc.Session(MiniJevTarget().start("ok"))
        self.addCleanup(s.close)
        self.assertEqual([c["version"] for c in s.accepted], [2])

    def test_leak_is_refused_not_accepted(self):
        res, _ = self.run_fault("leak", fx.v2_request())
        self.assertNotEqual(res["status"], "complete")


Conformance_minijev.__name__ = Conformance_minijev.__qualname__ = "Conformance_minijev"

if __name__ == "__main__":
    unittest.main()
