"""Clef-local adapter: shared conformance + joint-head, identity, readiness and refusal tests.

Run: python3 -m unittest discover -s packages/semaprax-harness-adapters/clef-local/tests
Uses an instrumented fake worker model dir (no torch, no network, no download).
"""

import json
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
ADAPTERS = os.path.abspath(os.path.join(ROOT, ".."))
sys.path.insert(0, HERE)
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ADAPTERS, "sdk", "python"))
import clef_identity as ident  # noqa: E402
import decision_conformance as dc  # noqa: E402
import decision_fixtures as fx  # noqa: E402
import fake_model  # noqa: E402

SECRET = "clef-fake-secret-0123456789"
ADAPTER = os.path.join(ROOT, "adapter.py")
WORKER = os.path.join(ROOT, "worker.py")


class WorkerProc:
    def __init__(self, model_dir, lock, device="cuda", profile="clef-flash", **env):
        e = {"PATH": os.environ.get("PATH", ""), "FAKE_SECRET": SECRET, **{k: str(v) for k, v in env.items()}}
        self.p = subprocess.Popen([sys.executable, WORKER, "--model-dir", model_dir, "--lock", lock, "--device", device, "--profile", profile],
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=e)
        self.port = int(re.search(rb":(\d+)", self.p.stdout.readline()).group(1))
        self.endpoint = f"http://127.0.0.1:{self.port}"

    def readyz(self):
        try:
            return json.loads(urllib.request.urlopen(self.endpoint + "/readyz", timeout=5).read())
        except urllib.error.HTTPError as err:
            return json.loads(err.read())

    def wait_settled(self, timeout=10):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            doc = self.readyz()
            if doc["state"] in ("ready", "failed"):
                return doc
            time.sleep(0.05)
        raise AssertionError("worker never settled")

    def stop(self):
        self.p.kill()
        self.p.communicate()


class Running:
    secret = SECRET

    def __init__(self, worker, lock, env, tmp):
        self.argv = [sys.executable, ADAPTER]
        self.worker, self.tmp = worker, tmp
        self.env = {"SEMAPRAX_HARNESS_ENDPOINT": worker.endpoint, ident.LOCK_ENV: lock, **env}
        self.mutate = None

    def stop(self):
        try:
            self.final = self.worker.readyz()["calls"]["requests"]
        except Exception:
            self.final = 0
        self.worker.stop()
        shutil.rmtree(self.tmp, ignore_errors=True)

    def posts(self):
        if hasattr(self, "final"):
            return [None] * self.final
        return [None] * self.worker.readyz()["calls"]["requests"]

    def wait_in_flight(self, timeout):
        end = time.monotonic() + timeout
        while time.monotonic() < end and not self.posts():
            time.sleep(0.02)


def launch(fault="ok", worker_lock_revision=None, wait=True, **kw):
    tmp = tempfile.mkdtemp(prefix="clef-fake-")
    mdir, lock = fake_model.build(tmp)
    wlock = lock
    if worker_lock_revision:
        _, wlock = fake_model.build(os.path.join(tmp, "other"), revision=worker_lock_revision)
    mode = {"wrong_question": "binding", "wrong_candidate": "unknown_option", "leak": "http500"}.get(fault, fault)
    env = {"FAKE_MODE": mode if mode in ("ok", "slow", "crash", "oversize", "binding", "unknown_option", "http500") else "ok"}
    env.update({k: v for k, v in kw.items() if k.startswith("FAKE_")})
    dev = kw.get("device", "cuda")
    w = WorkerProc(mdir, wlock, device=dev, **env)
    if wait:
        w.wait_settled()
    return Running(w, lock, {}, tmp), mdir, lock


class ClefTarget:
    name, crash_kind, scoreless, supports_v1 = "clef_local", "upstream", False, True

    def start(self, fault="ok", **env_extra):
        running, _, _ = launch("ok" if fault in ("abstain", "wrong_identity") else fault,
                               worker_lock_revision="a" * 40 if fault == "wrong_identity" else None)
        running.env.update(env_extra)
        if fault == "abstain":
            running.env["SEMAPRAX_HARNESS_MIN_SCORE"] = "0.99"
        return running


Conformance_clef_local = dc.conformance_case(ClefTarget())
Conformance_clef_local.__name__ = "Conformance_clef_local"


class Base(unittest.TestCase):
    def call(self, running, **req_kw):
        real_stop = running.stop
        running.stop = lambda: None  # keep the worker alive so the test can inspect it
        self.addCleanup(real_stop)
        s = dc.Session(running)
        try:
            return s.invoke(fx.v2_request(**req_kw))
        finally:
            s.close()


class JointHeadPath(Base):
    def test_joint_head_path_and_zero_generate(self):
        running, _, _ = launch()
        res = self.call(running)
        # Re-launch is not needed: read counts before the adapter session closes the worker.
        self.assertEqual(res["status"], "complete")
        self.assertEqual(res["payload"]["call"]["billing"], "local")

    def test_counts(self):
        running, _, _ = launch()
        s = dc.Session(running)
        res = s.invoke(fx.v2_request())
        counts = running.worker.readyz()["calls"]
        s.close()
        self.assertEqual(res["status"], "complete")
        self.assertEqual(counts["load_release_model"], 1)
        self.assertEqual(counts["systemone"], 1)
        self.assertEqual(counts["generate"], 0)

    def test_generate_attempt_is_refused_and_counted(self):
        running, _, _ = launch(FAKE_TRY_GENERATE=1)
        s = dc.Session(running)
        res = s.invoke(fx.v2_request())
        counts = running.worker.readyz()["calls"]
        s.close()
        self.assertNotEqual(res["status"], "complete")
        self.assertEqual(counts["generate"], 1)


class Readiness(Base):
    def test_missing_head_file(self):
        tmp = tempfile.mkdtemp()
        mdir, lock = fake_model.build(tmp)
        os.remove(os.path.join(mdir, "joint_head.safetensors"))
        w = WorkerProc(mdir, lock)
        self.assertEqual(w.wait_settled()["reason"], "missing_file:joint_head.safetensors")
        res = self.call(Running(w, lock, {}, tmp))
        self.assertEqual(res["status"], "unavailable")
        self.assertIsNone(res["payload"])

    def test_mismatched_head_file(self):
        tmp = tempfile.mkdtemp()
        mdir, lock = fake_model.build(tmp)
        with open(os.path.join(mdir, "joint_head.safetensors"), "wb") as f:
            f.write(b"tampered-head")
        w = WorkerProc(mdir, lock)
        self.assertTrue(w.wait_settled()["reason"].startswith("digest_mismatch:joint_head"))
        self.assertEqual(w.readyz()["calls"]["load_release_model"], 0, "no load before verification")
        self.assertEqual(self.call(Running(w, lock, {}, tmp))["status"], "unavailable")

    def test_patched_code_is_not_imported(self):
        tmp = tempfile.mkdtemp()
        mdir, lock = fake_model.build(tmp)
        with open(os.path.join(mdir, "joint_schema_model.py"), "a") as f:
            f.write("\nraise SystemExit('patched')\n")
        w = WorkerProc(mdir, lock)
        self.assertTrue(w.wait_settled()["reason"].startswith("digest_mismatch:joint_schema_model.py"))
        w.stop()
        shutil.rmtree(tmp, ignore_errors=True)

    def test_model_identity_mismatch(self):
        running, _, _ = launch(worker_lock_revision="b" * 40)
        res = self.call(running)
        self.assertEqual(res["status"], "refused")
        self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPK008")
        self.assertEqual(running.worker.readyz()["calls"]["requests"], 0)

    def test_unsupported_device(self):
        running, _, _ = launch(device="mps")
        self.assertTrue(running.worker.readyz()["reason"].startswith("unsupported_device"))
        res = self.call(running)
        self.assertEqual(res["status"], "unsupported")
        self.assertEqual(running.worker.readyz()["calls"]["load_release_model"], 0)

    def test_worker_down_is_unavailable_never_hosted(self):
        s = socket.socket()
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
        s.close()
        tmp = tempfile.mkdtemp()
        _, lock = fake_model.build(tmp)
        w = type("W", (), {"endpoint": f"http://127.0.0.1:{port}", "stop": lambda self: None})()
        res = self.call(Running(w, lock, {}, tmp))
        self.assertEqual(res["status"], "unavailable")
        self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPK016")

    def test_non_loopback_endpoint_is_refused(self):
        tmp = tempfile.mkdtemp()
        _, lock = fake_model.build(tmp)
        w = type("W", (), {"endpoint": "https://api.cloudflare.com", "stop": lambda self: None})()
        res = self.call(Running(w, lock, {}, tmp))
        self.assertEqual(res["status"], "refused")
        self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPK002")

    def test_cold_start_readiness(self):
        running, _, _ = launch(wait=False, FAKE_LOAD_SLEEP=1.5)
        end = time.monotonic() + 5
        while running.worker.readyz()["state"] != "verifying" and time.monotonic() < end:
            time.sleep(0.02)
        self.assertEqual(running.worker.readyz()["state"], "verifying")
        s = dc.Session(running)
        cold = s.invoke(fx.v2_request())
        self.assertEqual(cold["status"], "unavailable")
        self.assertIn("cold-starting", cold["diagnostics"][0]["message"])
        self.assertEqual(running.worker.wait_settled()["state"], "ready")
        warm = s.invoke(fx.v2_request())
        s.close()
        self.assertEqual(warm["status"], "complete")

    def test_oversized_request_to_worker(self):
        running, _, _ = launch()
        req = urllib.request.Request(running.worker.endpoint + "/v1/systemone", data=b"{" + b" " * 70000 + b"}", method="POST")
        with self.assertRaises(urllib.error.HTTPError) as cm:
            urllib.request.urlopen(req, timeout=5)
        self.assertEqual(cm.exception.code, 413)
        running.stop()

    def test_larger_clef_unavailable_unless_provisioned(self):
        w = type("W", (), {"endpoint": "http://127.0.0.1:9", "stop": lambda self: None})()
        s = dc.Session(Running(w, ident.DEFAULT_LOCK, {"SEMAPRAX_HARNESS_MODEL": "clef"}, tempfile.mkdtemp()))
        res = s.invoke(fx.v2_request())
        s.close()
        self.assertEqual(res["status"], "unsupported")
        self.assertIn("unavailable unless provisioned and verified", res["diagnostics"][0]["message"])


class Provenance(Base):
    def test_receipt_binds_identity(self):
        running, _, lock = launch()
        res = self.call(running)
        call = res["payload"]["call"]
        rel = ident.release(ident.load_lock(lock), "clef-flash")
        self.assertEqual(call["identity_kind"], "immutable_checkpoint")
        self.assertEqual(call["checkpoint"], ident.identity_digest(rel))
        self.assertEqual(call["billing"], "local")
        self.assertEqual(call["adapter"], "ai.cloudflare/clef-local-decision@0.2.0")
        self.assertEqual(res["payload"]["score_kind"], "option_distribution")

    def test_readiness_exposes_identity(self):
        running, _, lock = launch()
        doc = running.worker.readyz()
        rel = ident.release(ident.load_lock(lock), "clef-flash")
        self.assertEqual(doc["identity"], ident.identity_digest(rel))
        self.assertTrue(doc["digests_verified"])
        self.assertEqual(set(doc["digests"]), set(ident.GROUPS))
        running.stop()

    def test_each_component_changes_identity(self):
        _, lock = fake_model.build(tempfile.mkdtemp())
        rel = ident.release(ident.load_lock(lock), "clef-flash")
        base = ident.identity_digest(rel)
        for name in ("model-00001-of-00001.safetensors", "joint_head.safetensors", "joint_schema_model.py", "tokenizer.json"):
            alt = json.loads(json.dumps(rel))
            alt["files"][name]["sha256"] = "0" * 64
            self.assertNotEqual(ident.identity_digest(alt), base, name)
        alt = dict(rel, revision="1" * 40)
        self.assertNotEqual(ident.identity_digest(alt), base)

    def test_committed_lock(self):
        lock = ident.load_lock()
        flash, big = lock["releases"]["clef-flash"], lock["releases"]["clef"]
        self.assertEqual(flash["status"], "supported")
        self.assertEqual(big["status"], "unprovisioned")
        self.assertEqual(flash["revision"], "17f0b0ad64efb65d273590632833508766b2aae6")
        self.assertEqual(flash["total_bytes"], sum(m["bytes"] for m in flash["files"].values()))
        for rel in lock["releases"].values():
            self.assertTrue(all(len(m["sha256"]) == 64 for m in rel["files"].values()))
            self.assertEqual({m["group"] for m in rel["files"].values()}, set(ident.GROUPS) | {"license"})

    def test_adapter_has_no_install_or_fetch_paths(self):
        for name in ("adapter.py", "clef_backend.py", "worker.py", "clef_identity.py"):
            text = open(os.path.join(ROOT, name)).read()
            for bad in ("pip install", "urlopen", "urlretrieve", "snapshot_download(", "cloudflare.com", "api.typesafe"):
                self.assertNotIn(bad, text, f"{name}: {bad}")
        self.assertIn("urlopen", open(os.path.join(ROOT, "provision.py")).read())


if __name__ == "__main__":
    unittest.main()
