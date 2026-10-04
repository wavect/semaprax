#!/usr/bin/env python3
"""Fake local Caveman 3.1.0 middleware runtime (test fixture). Usage: fake_runtime.py <dir>

Emulates the loopback wire of middleware protocol 1.1 at upstream commit
8af1f1b9b1346bca0722a1556f119b4e6675cc96 of JuliusBrussee/caveman, written against the raw files:
  docs/technical/middleware-protocol.md   (§2 bearer auth on every route and Origin refusal, §3 feature
                                           header, §6 errors and "decisions are 200", §7 plan constraints,
                                           §12 originals and sessions/delete)
  packages/sdk/python/caveman_cloud/middleware/runtime.py   (request body, headers)
  packages/sdk/python/caveman_cloud/middleware/validate.py  (plan shape the client accepts)
  packages/sdk/python/caveman_cloud/middleware/{protocol,types}.py (capabilities, marker, features)
Shape fields were aligned with a real v3.1.0 runtime on 2026-10-04 (recorded/exchange.json). Mode
"replay" serves that recording verbatim (patching only the per-request ids and digests).
Behaviour comes from <dir>/mode.txt; each request appends "METHOD route" to <dir>/calls.log (also
for refused ones) and the last optimize body and headers are saved in <dir>/last_optimize.json.
"""
import hashlib
import json
import os
import sys
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

D = sys.argv[1]
P = "/caveman/v1/middleware/"


def mode():
    f = os.path.join(D, "mode.txt")
    return open(f).read().strip() if os.path.exists(f) else "compress"


def collapse(t):
    out, prev, n = [], None, 0
    for line in t.split("\n") + [None]:
        if line == prev:
            n += 1
            continue
        if prev is not None:
            out.append(prev if n == 1 else f"{prev} (x{n})")
        prev, n = line, 1
    return "\n".join(out)


TOKEN = "tok-1"
REC = os.path.join(os.path.dirname(os.path.abspath(__file__)), "recorded", "exchange.json")


def recorded(method, path):
    return next(e for e in json.load(open(REC))["exchanges"] if (e["method"], e["path"]) == (method, path))


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def send(self, status, body, raw=False):
        data = body.encode() if raw else json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def err(self, status, code):
        self.send(status, {"schema_version": 1, "error": {"code": code}})

    def gate(self):
        route = self.path[len(P):]
        open(os.path.join(D, "calls.log"), "a").write(f"{self.command} {route}\n")
        if self.headers.get("Origin") or self.headers.get("Sec-Fetch-Site") == "cross-site":
            self.err(403, "forbidden_origin")
        elif self.headers.get("Authorization") != "Bearer " + TOKEN:
            self.err(401, "unauthorized")
        elif "http_status_v2" not in (self.headers.get("Caveman-Middleware-Features") or ""):
            self.err(400, "invalid_request")
        else:
            return route
        return None

    def do_GET(self):
        if self.gate() != "capabilities":
            return
        if mode() == "replay":
            return self.send(200, recorded("GET", P + "capabilities")["response_body"])
        self.send(200, {
            "schema_version": 1, "protocol": {"min": 1, "max": 1},
            "features": ["http_status_v2", "originals_lifecycle", "revision_tolerant", "tolerant_reader"],
            "mode": "record" if mode() == "record" else "compress",
            "policy_revision": "pr1", "runtime_build": "fake-3.1.0",
            "transforms": [{"transform_id": "caveman.engine.log.v1", "implementation_version": "1", "safety_classes": ["S4"],
                            "deterministic": True, "recovery": "exact_ccr", "eligible_segment_kinds": ["tool_result"],
                            "requires_eval": True, "not_smaller_fallback": "original",
                            "provenance": {"kind": "native", "source_manifest_sha256": None}, "conformance_digest": "0" * 64}],
            "limits": {"deadline_ms": 500, "request_bytes": 2097152, "segment_bytes": 524288, "page_bytes": 262144,
                       "retrieve_deadline_ms": 5000, "queue_depth": 16, "retrieve_queue_depth": 16, "max_segments": 256,
                       "max_manifest_items": 4096, "receipt_bytes": 16384},
            "max_retention_seconds": 604800, "persistent": True, "recovery": True, "retention_seconds": 604800})

    def do_POST(self):
        route = self.gate()
        if route is None:
            return
        raw = self.rfile.read(int(self.headers["Content-Length"]))
        body = json.loads(raw)
        if route == "sessions/delete":
            if mode() == "replay":
                return self.send(200, recorded("POST", P + route)["response_body"])
            return self.send(200, {"schema_version": 1, "status": "revoked", "originals_deleted": True,
                                   "deleted": {"scopes": 1, "choices": 1, "grants": 1, "originals": 1}})
        if route != "optimize":
            return self.err(404, "not_found")
        json.dump({"headers": dict(self.headers.items()), "body": body}, open(os.path.join(D, "last_optimize.json"), "w"))
        m = mode()
        if m == "replay":
            plan = recorded("POST", P + "optimize")["response_body"]
            plan["request_id"] = body["request_id"]
            plan["input_digest"] = hashlib.sha256(raw).hexdigest()
            plan["recovery"]["binding_id"] = body["recovery_binding"]["id"]
            return self.send(200, plan)
        if m == "crash":
            return self.err(503, "runtime_unavailable")
        if m == "hang":
            time.sleep(8)
        if m == "garbage":
            return self.send(200, "this is not json", raw=True)
        seg = body["segments"][0]
        text = seg["content"]
        plan = {"schema_version": 1, "request_id": body["request_id"], "input_digest": hashlib.sha256(raw).hexdigest(),
                "policy_revision": "pr1", "replacement_set_id": hashlib.sha256(raw + b"set").hexdigest(),
                "status": "optimized", "reason": "eligible", "runtime_build": "fake-3.1.0",
                "replacements": [], "skipped": [],
                "stability": {"provider_bytes": "unobserved", "provider_cache_hits": "unobserved", "native": "persistent_choices"}}
        n = len(text.split())
        plan["measurement"] = {"basis": "inferred", "scope": "segment", "verified_saved_usd": 0, "tokenizer": "fake-words", "overhead_coverage": "segment_and_declared_recovery_tool",
                               "tokens_before": n, "tokens_after": n, "unique_tokens_reduced": 0, "recovery_overhead_tokens": 2}
        plan["recovery"] = {"available": True, "persistent": True, "expires_at": 4102444800,
                            "binding_id": body["recovery_binding"]["id"]}
        if m == "bypassed":
            plan.update(status="bypassed", reason="not_smaller")
            plan["skipped"] = [{"segment_id": seg["id"], "source_id": seg["source_id"], "sha256": seg["sha256"], "reason": "not_smaller"}]
            return self.send(200, plan)
        out = collapse(text)
        if m == "grow":
            out = text + "\n" + "padding " * 400
        if m == "drop_error":
            out = "\n".join(l for l in out.split("\n") if "ERROR" not in l)
        handle = "cmw_" + hashlib.sha256(text.encode()).hexdigest()[:48]
        out = f"[caveman: shortened; exact original via caveman_retrieve handle={handle}]\n" + out
        sha = hashlib.sha256(out.encode()).hexdigest()
        if m == "bad_sha":
            sha = "0" * 64
        after = len(out.split())
        plan["measurement"].update(tokens_after=after, unique_tokens_reduced=max(0, n - after))
        plan["replacements"].append({
            "segment_id": seg["id"], "source_id": seg["source_id"], "original_sha256": seg["sha256"],
            "transform_id": "caveman.engine.log.v1", "transform_version": "1", "sha256": sha, "text": out,
            "tokens_before": n, "tokens_after": after, "reused": False, "unique_original": True, "recovery_handle": handle})
        self.send(200, plan)


srv = HTTPServer(("127.0.0.1", 0), H)
open(os.path.join(D, "port"), "w").write(str(srv.server_port))
srv.serve_forever()
