"""Loopback fakes reproducing the documented SystemOne wire shapes.

Laya shape: docs/http-api.md @ v0.3.26 (full payload with `routing`).
Jev shape: https://api.typesafe.ai/openapi.json 0.2.0 (`model`, `answers`, `usage`).
The same `mode` switches drive both so both adapters face identical fixtures.
"""

import json
import socket
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

KEY = "jev-secret-KEY-0123456789"


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, code, raw, ctype="application/json"):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        try:
            self.wfile.write(raw)
        except OSError:
            pass

    def do_GET(self):
        s = self.server
        s.log.append(("GET", self.path, None, self.headers.get("Authorization")))
        if s.flavor == "jev" and self.path == "/v1/models":
            if self.headers.get("Authorization") != "Bearer " + KEY:
                return self._send(401, json.dumps({"detail": "bad key " + KEY}).encode())
            return self._send(200, json.dumps({"models": [{"name": n, "description": "x", "release_date": "2026-09-15"} for n in ("jev-test-1", "jev-test-2", "jev-latest")]}).encode())
        if s.flavor == "laya" and self.path == "/health":
            return self._send(200, b'{"status":"ok"}')
        self._send(404, b"{}")

    def do_POST(self):
        s = self.server
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        s.log.append(("POST", self.path, body, self.headers.get("Authorization")))
        if self.path != "/v1/systemone":
            return self._send(404, b"{}")
        if s.flavor == "jev" and self.headers.get("Authorization") != "Bearer " + KEY:
            return self._send(401, json.dumps({"detail": "bad key " + KEY}).encode())
        mode = s.mode
        if mode == "entitlement_revoked":
            return self._send(403, json.dumps({"detail": "revoked " + KEY}).encode())
        if mode == "crash":
            self.close_connection = True
            try:
                self.connection.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            return
        if mode == "slow":
            time.sleep(3)
        if mode == "http500":
            return self._send(500, json.dumps({"detail": "boom " + KEY}).encode())
        qid = next(iter(body["questions"]))
        opts = list(body["questions"][qid]["criteria"])
        n = len(opts)
        probs = {o: round(1.0 / n, 4) for o in opts}
        probs[opts[-1]] = round(1.0 - sum(v for o, v in probs.items() if o != opts[-1]), 4) + 0.2
        tot = sum(probs.values())
        probs = {o: round(v / tot, 4) for o, v in probs.items()}
        choice = max(probs, key=probs.get)
        ans = {"type": "choice", "choice": choice, "probabilities": probs, "confidence": 0.1}
        resp = {"model": "laya-rl-agent" if s.flavor == "laya" else "jev-test-1", "answers": {qid: ans}, "usage": {"input_tokens": 61, "output_tokens": 0}}
        if s.flavor == "laya":
            resp["routing"] = {"model": body.get("model", "multilingual"), "repo": "convaiinnovations/laya/multilingual", "reason": "explicit"}
            ans["answer_confidence"] = probs[choice]
        raw = None
        if mode == "nan":
            probs[opts[0]] = float("nan")
        elif mode == "range":
            probs[opts[0]] = 1.7
        elif mode == "unknown_option":
            ans["choice"] = "not-an-option"
        elif mode == "unknown_prob_key":
            probs["ghost"] = 0.0
        elif mode == "binding":
            resp["answers"] = {"route-deadbeefdeadbeef": ans}
        elif mode == "wrong_type":
            ans["type"] = "score"
        elif mode == "bad_json":
            raw = b"{not json"
        elif mode == "oversize":
            resp["pad"] = "x" * 200000
        elif mode == "abstain" and s.flavor == "laya":
            ans["abstention"] = "abstained"
        elif mode == "wrong_checkpoint" and s.flavor == "laya":
            resp["routing"]["model"] = "english"
        if raw is None:
            raw = json.dumps(resp).encode()
        self._send(200, raw)


class Fake(ThreadingHTTPServer):
    daemon_threads = True

    def stop(self):
        self.shutdown()
        self.server_close()


def start(flavor, mode="ok"):
    srv = Fake(("127.0.0.1", 0), Handler)
    srv.flavor, srv.mode, srv.log = flavor, mode, []
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv
