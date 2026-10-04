#!/usr/bin/env python3
"""Fake local Caveman 3.1.0 middleware runtime (test fixture). Usage: fake_runtime.py <dir>

Emulates only the loopback wire the Semaprax adapter uses. Mirrors, at upstream commit
8af1f1b9b1346bca0722a1556f119b4e6675cc96 of JuliusBrussee/caveman:
  docs/technical/middleware-protocol.md   (GET capabilities, POST optimize, plan/error schemas, the
                                           "[caveman: shortened; exact original via caveman_retrieve
                                           handle=cmw_<48 hex>]" header, record-mode semantics)
  packages/sdk/python/caveman_cloud/middleware/{runtime,types,validate}.py (request body keys)
It was written from a read-only summary of those files, not from a recorded real exchange.
Behaviour comes from <dir>/mode.txt; each request appends "METHOD route" to <dir>/calls.log and the
last optimize body/auth header are saved in <dir>/last_optimize.json.
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

    def do_GET(self):
        open(os.path.join(D, "calls.log"), "a").write("GET " + self.path[len(P):] + "\n")
        if self.path != P + "capabilities":
            return self.send(404, {"schema_version": 1, "error": {"code": "invalid_request"}})
        self.send(200, {
            "schema_version": 1, "protocol": {"min": 1, "max": 1},
            "features": ["http_status_v2", "revision_tolerant", "tolerant_reader"],
            "policy_revision": "pr1", "runtime_build": "fake-3.1.0",
            "transforms": [{"transform_id": "ccr-text", "implementation_version": "1", "deterministic": True,
                            "recovery": "exact_ccr", "eligible_segment_kinds": ["tool_result"]}],
            "limits": {"deadline_ms": 500, "request_bytes": 2097152, "segment_bytes": 524288, "page_bytes": 262144},
            "max_retention_seconds": 604800, "persistent": True, "recovery": True, "retention_seconds": 604800})

    def do_POST(self):
        route = self.path[len(P):]
        open(os.path.join(D, "calls.log"), "a").write("POST " + route + "\n")
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        if route != "optimize":
            return self.send(404, {"schema_version": 1, "error": {"code": "invalid_request"}})
        json.dump({"auth": self.headers.get("Authorization"), "body": body}, open(os.path.join(D, "last_optimize.json"), "w"))
        m = mode()
        if m == "crash":
            return self.send(503, {"schema_version": 1, "error": {"code": "runtime_unavailable"}})
        if m == "hang":
            time.sleep(8)
        if m == "garbage":
            return self.send(200, "this is not json", raw=True)
        seg = body["segments"][0]
        text = seg["content"]
        plan = {"schema_version": 1, "status": "applied", "mode": "compress", "policy_revision": "pr1",
                "runtime_build": "fake-3.1.0", "replacements": [], "skipped": [], "counts": {}}
        if m == "record" or body["mode"] != "compress":
            plan.update(mode="record", status="bypassed")
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
        plan["replacements"].append({
            "segment_id": seg["id"], "source_id": seg["source_id"], "original_sha256": seg["sha256"],
            "transform_id": "ccr-text", "transform_version": "1", "sha256": sha, "text": out, "recovery_handle": handle})
        self.send(200, plan)


srv = HTTPServer(("127.0.0.1", 0), H)
open(os.path.join(D, "port"), "w").write(str(srv.server_port))
srv.serve_forever()
