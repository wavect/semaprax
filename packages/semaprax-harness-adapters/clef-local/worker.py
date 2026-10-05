#!/usr/bin/env python3
"""Clef warm worker: serves the pinned release over loopback (explicit start only).

    python3 worker.py --model-dir DIR --profile clef-flash --device cuda [--port N]

Model libraries (torch/transformers/safetensors/huggingface_hub) are imported
only here, only inside the provisioned environment. The worker never installs,
downloads or fetches code: offline mode is forced, every pinned file is
sha256-verified against clef.lock.json before the release's own
`joint_schema_model.py` is imported, and inference goes through that module's
`load_release_model` + `systemone` (the joint head), never `generate`.

Endpoints (127.0.0.1 only):
  GET  /readyz         readiness + identity (state loading|verifying|ready|failed)
  POST /v1/systemone   SystemOne body -> SystemOne response (+ routing.model = identity)
"""

import argparse
import importlib.util
import json
import os
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import clef_identity as ident  # noqa: E402

MAX_BODY = 65536


class Worker:
    def __init__(self, model_dir, profile, device, lock_path=None):
        self.model_dir, self.profile, self.device = model_dir, profile, device
        self.lock_path = lock_path
        self.state, self.reason = "loading", None
        self.rel = None
        self.engine = None
        self.counts = {"load_release_model": 0, "requests": 0, "systemone": 0, "generate": 0}
        self.infer_lock = threading.Lock()
        self.count_lock = threading.Lock()

    def _fail(self, reason):
        self.state, self.reason = "failed", reason

    def start(self):
        try:
            self.rel = ident.release(ident.load_lock(self.lock_path), self.profile)
        except ident.LockError as err:
            return self._fail("lock:" + str(err)[:80])
        if self.rel.get("status") != "supported":
            return self._fail("unprovisioned_release")
        if self.device not in self.rel["devices"]:
            return self._fail("unsupported_device:" + self.device[:32])
        self.state = "verifying"
        try:
            ident.verify_dir(self.model_dir, self.rel)
        except ident.LockError as err:
            return self._fail(str(err)[:120])
        os.environ["HF_HUB_OFFLINE"] = "1"
        os.environ["TRANSFORMERS_OFFLINE"] = "1"
        try:
            self.engine = self._load()
        except Exception as err:
            return self._fail("load_error:" + type(err).__name__)
        self.state = "ready"

    def _load(self):
        path = os.path.join(self.model_dir, ident.CODE_FILE)  # digest-verified above
        spec = importlib.util.spec_from_file_location("joint_schema_model", path)
        mod = importlib.util.module_from_spec(spec)
        sys.modules["joint_schema_model"] = mod
        spec.loader.exec_module(mod)
        with self.count_lock:
            self.counts["load_release_model"] += 1
        model, processor = mod.load_release_model(self.model_dir, device=self.device)
        self._guard_generate(model)
        return model, processor, mod.systemone

    def _guard_generate(self, model):
        def refuse(*a, **k):
            with self.count_lock:
                self.counts["generate"] += 1
            raise RuntimeError("free-form generation is not a Clef decision path")
        mods = list(model.modules()) if hasattr(model, "modules") else [model]
        for m in mods:
            if hasattr(m, "generate"):
                m.generate = refuse

    def readiness(self):
        doc = {"state": self.state, "reason": self.reason, "profile": self.profile, "device": self.device,
               "joint_head": True, "calls": dict(self.counts)}
        if self.rel and self.state == "ready":
            doc.update(revision=self.rel["revision"], identity=ident.identity_digest(self.rel),
                       digests=ident.group_digests(self.rel), digests_verified=True,
                       license=self.rel["license"])
        return doc

    def answer(self, body):
        """-> (http status, document)."""
        if self.state != "ready":
            return 503, {"error": "not ready", "state": self.state}
        if not isinstance(body, dict) or body.get("model") != self.profile:
            return 400, {"error": "model does not match the worker's pinned profile"}
        if "images" in body or "videos" in body:
            return 400, {"error": "text-only worker"}
        model, processor, systemone = self.engine
        with self.count_lock:
            self.counts["requests"] += 1
        with self.infer_lock:
            try:
                out = systemone(model, processor, body)
            except ValueError as err:
                return 400, {"error": str(err)[:120]}
            with self.count_lock:
                self.counts["systemone"] += 1
        out["routing"] = {"model": ident.identity_digest(self.rel)}
        return 200, out


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, code, doc):
        raw = json.dumps(doc, separators=(",", ":"), sort_keys=True).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        try:
            self.wfile.write(raw)
        except OSError:
            pass

    def do_GET(self):
        if self.path == "/readyz":
            w = self.server.worker
            return self._send(200 if w.state == "ready" else 503, w.readiness())
        self._send(404, {})

    def do_POST(self):
        if self.path != "/v1/systemone":
            return self._send(404, {})
        try:
            n = int(self.headers.get("Content-Length", ""))
        except ValueError:
            return self._send(400, {"error": "length required"})
        if n < 0 or n > MAX_BODY:
            return self._send(413, {"error": f"body exceeds {MAX_BODY} bytes"})
        try:
            body = json.loads(self.rfile.read(n))
        except ValueError:
            return self._send(400, {"error": "invalid JSON"})
        self._send(*self.server.worker.answer(body))


def serve(worker, port=0):
    srv = ThreadingHTTPServer(("127.0.0.1", port), Handler)  # loopback only, never configurable
    srv.daemon_threads = True
    srv.worker = worker
    return srv


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--model-dir", required=True)
    ap.add_argument("--profile", default="clef-flash")
    ap.add_argument("--device", default="cuda")
    ap.add_argument("--port", type=int, default=0)
    ap.add_argument("--lock", default=os.environ.get(ident.LOCK_ENV))
    a = ap.parse_args(argv)
    w = Worker(a.model_dir, a.profile, a.device, a.lock)
    srv = serve(w, a.port)
    print(f"clef-worker listening on http://127.0.0.1:{srv.server_port}", flush=True)
    threading.Thread(target=w.start, daemon=True).start()  # cold start behind a live /readyz
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
