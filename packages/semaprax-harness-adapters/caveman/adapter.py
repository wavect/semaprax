#!/usr/bin/env python3
"""command.view/v1 adapter for Caveman input compression (pinned 3.1.0, commit 8af1f1b9).

Wire contract (read at the pin; see README.md for the files and what is still unverified):
the host-started local runtime (`caveman start`, 127.0.0.1:8787, started by the user with
DO_NOT_TRACK=1 and CAVEMAN_WORK_TAGS=0) serves the middleware protocol 1.1 under
`/caveman/v1/middleware/`. This adapter sends already-captured output to
`POST optimize` as one `tool_result` segment with `mode:"compress"`, after `GET capabilities`.
It never re-runs the command, never starts or installs the runtime, never posts receipts, and
refuses any endpoint that is not loopback. The Caveman recovery header and handle are dropped:
the Semaprax retention store is the only authoritative raw recovery.

Endpoint override: `<retention>/caveman-endpoint` (one line `host:port`, loopback only); optional
bearer token in `<retention>/caveman-token` (0600). Both are explicit host provisioning.
"""
import base64
import hashlib
import http.client
import ipaddress
import json
import os
import re
import sys
import uuid

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "sdk", "python"))
from semaprax_harness_adapter import AdapterError, serve  # noqa: E402

PROVIDER = "ai.caveman/caveman-command-view"
KIND = "command.view"
PINNED_VERSION = "3.1.0"
DEFAULT_MIN_BYTES = 1024
MAX_RAW = 8 * 1024 * 1024
CALL_TIMEOUT = 20
CRITICAL = re.compile(r"(?i)\b(error|fail(ed|ure|ures)?|panic(ked)?|fatal|critical|exception|traceback)\b")
BENIGN = re.compile(r"\.\.\. (ok|ignored)\s*$")
PATCH = re.compile(r"^(diff --git |--- a/|\+\+\+ b/|@@ -\d)", re.M)


def retention_dir():
    d = os.environ.get("SEMAPRAX_HARNESS_RETENTION_DIR", "")
    if not os.path.isabs(d) or not os.path.isdir(d):
        raise AdapterError("unavailable", "retention-missing", "SEMAPRAX_HARNESS_RETENTION_DIR is not an absolute directory")
    return d


PREFIX = "/caveman/v1/middleware/"
DEFAULT_ENDPOINT = "127.0.0.1:8787"
HEADER = re.compile(r"\A\[caveman: shortened; exact original via caveman_retrieve handle=cmw_[a-f0-9]{48}\]\n")
OK_STATUS = ("applied", "reused", "optimized")


def endpoint(ret):
    path = os.path.join(ret, "caveman-endpoint")
    text = open(path).read().strip() if os.path.isfile(path) else DEFAULT_ENDPOINT
    host, _, port = text.rpartition(":")
    if not host or not port.isdigit() or not _loopback(host):
        raise AdapterError("refused", "caveman-egress", "endpoint is not a loopback host:port")
    return host, int(port)


def call(ret, method, path, body=None):
    """One loopback HTTP call; http.client never consults proxy environment variables."""
    host, port = endpoint(ret)
    headers = {"Accept": "application/json"}
    tok = os.path.join(ret, "caveman-token")
    if os.path.isfile(tok):
        headers["Authorization"] = "Bearer " + open(tok).read().strip()
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    conn = http.client.HTTPConnection(host, port, timeout=CALL_TIMEOUT)
    try:
        conn.request(method, PREFIX + path, body=data, headers=headers)
        resp = conn.getresponse()
        raw = resp.read(4 << 20)
    except (OSError, http.client.HTTPException):
        raise AdapterError("unavailable", "caveman-unreachable", "local Caveman runtime is not reachable")
    finally:
        conn.close()
    try:
        doc = json.loads(raw.decode("utf-8"))
    except ValueError:
        raise AdapterError("failed", "caveman-invalid-output", "runtime reply is not JSON")
    if resp.status != 200:
        code = (doc.get("error") or {}).get("code", "error") if isinstance(doc, dict) else "error"
        raise AdapterError("failed", "caveman-http", f"runtime answered {resp.status} {code}")
    if not isinstance(doc, dict) or doc.get("schema_version") != 1:
        raise AdapterError("failed", "caveman-invalid-output", "runtime reply has no schema_version 1")
    return doc


def plan(req):
    p = req.get("payload") or {}
    if p.get("external_hooks") or "caveman" in (p.get("lineage") or []):
        return "complete", {"form": "post-execution", "route": "bypass", "reason": "external-owner-holds-the-view"}, []
    cfg = p.get("config") or {}
    est = p.get("estimated_output_bytes")
    if isinstance(est, int) and est < int(cfg.get("min_bytes", DEFAULT_MIN_BYTES)):
        return "complete", {"form": "post-execution", "route": "bypass", "reason": "small-output"}, []
    return "complete", {"form": "post-execution", "route": "post-execution", "operation": "view",
                        "raw_recovery": "host-retained-streams"}, []


def _stream(p, key, ret):
    if p.get(key + "_path"):
        path = os.path.realpath(os.path.join(ret, p[key + "_path"]))
        if os.path.commonpath([path, os.path.realpath(ret)]) != os.path.realpath(ret):
            raise AdapterError("refused", "path-outside-retention", f"{key}_path escapes the retention directory")
        with open(path, "rb") as f:
            return f.read(MAX_RAW + 1)
    if p.get(key + "_b64") is not None:
        return base64.b64decode(p[key + "_b64"])
    return str(p.get(key, "")).encode("utf-8")


def _critical_missing(raw_text, view_text):
    return [s for s in (l.strip() for l in raw_text.splitlines())
            if s and CRITICAL.search(s) and not BENIGN.search(s) and s not in view_text]


def _loopback(bind):
    try:
        return ipaddress.ip_address(bind).is_loopback
    except ValueError:
        return bind == "localhost"


def _unsupported(code):
    return "unsupported", None, [{"code": "caveman.bypass", "message": code}]


def view(req):
    p = req.get("payload") or {}
    ret = retention_dir()
    out_b, err_b = _stream(p, "stdout", ret), _stream(p, "stderr", ret)
    total = len(out_b) + len(err_b)
    if total > MAX_RAW:
        return _unsupported("output-too-large")
    min_bytes = int(p.get("min_bytes", DEFAULT_MIN_BYTES))
    if total < min_bytes:
        return _unsupported("small-output")
    try:
        text = out_b.decode("utf-8") + ("\n[stderr]\n" + err_b.decode("utf-8") if err_b else "")
    except UnicodeDecodeError:
        return _unsupported("not-utf8")
    if "\x00" in text or text.lstrip()[:1] in ("{", "[") or PATCH.search(text):
        return _unsupported("unsupported-format")
    caps = call(ret, "GET", "capabilities")
    transforms = [t["transform_id"] for t in caps.get("transforms", [])
                  if isinstance(t, dict) and t.get("deterministic") is True and t.get("recovery") == "exact_ccr"
                  and "tool_result" in (t.get("eligible_segment_kinds") or [])]
    if not transforms:
        return _unsupported("no-eligible-transform")
    rid = str(uuid.uuid4())
    seg = "cv-" + hashlib.sha256(text.encode()).hexdigest()[:24]
    sess = hashlib.sha256(str((req.get("project") or {}).get("id", "")).encode()).hexdigest()[:32]
    body = {
        "schema_version": 1, "request_id": rid, "idempotency_key": rid, "logical_call_id": rid, "attempt_id": 1,
        "scope": {"namespace": "semaprax", "session_id": sess, "branch_id": "main", "cache_epoch": "0"},
        "adapter": None, "model": "semaprax-command-view", "mode": "compress",
        "policy": {"revision": caps.get("policy_revision"), "transforms": transforms},
        "segments": [{"id": seg, "source_id": seg, "kind": "tool_result", "cache_region": "live_zone",
                      "content": text, "sha256": hashlib.sha256(text.encode()).hexdigest(),
                      "protected": False, "opaque": False}],
        "context_manifest": [],
        "recovery_binding": {"id": "semaprax-host-recovery", "kind": "host_tool", "tool_name": "caveman_retrieve",
                             "overhead_text": "Exact output is recoverable through the Semaprax recovery handle."},
    }
    reply = call(ret, "POST", "optimize", body)
    mode = reply.get("mode")
    if mode != "compress":
        return _unsupported(f"runtime-mode-{mode}")  # record mode changes no payload: no saving to claim
    if reply.get("status") not in OK_STATUS:
        return _unsupported(f"runtime-status-{reply.get('status')}")
    reps = [r for r in reply.get("replacements", []) if isinstance(r, dict) and r.get("segment_id") == seg]
    if len(reps) != 1 or not isinstance(reps[0].get("text"), str):
        return _unsupported("no-replacement")
    out = reps[0]["text"]
    if hashlib.sha256(out.encode()).hexdigest() != reps[0].get("sha256"):
        raise AdapterError("failed", "caveman-invalid-output", "replacement sha256 does not match its text")
    out = HEADER.sub("", out, count=1)  # Semaprax's own recovery reference replaces Caveman's
    if len(out.encode("utf-8")) >= len(text.encode("utf-8")):
        return _unsupported("not-smaller")
    missing = _critical_missing(text, out)
    if missing:
        raise AdapterError("failed", "caveman-dropped-critical", f"view dropped {len(missing)} error line(s)")
    v = {"text": out, "lossless": False, "omissions": max(0, text.count("\n") - out.count("\n"))}
    if p.get("recovery_handle"):
        v["recovery_handle"] = p["recovery_handle"]
    return "complete", {"form": "post-execution", "view": v}, [
        {"code": "caveman.compress", "message": f"raw_bytes={total} view_bytes={len(out.encode())}"}]


if __name__ == "__main__":
    serve([{"kind": KIND, "version": 1, "operations": ["plan", "view"]}],
          {(KIND, "plan"): plan, (KIND, "view"): view},
          {"provider_id": PROVIDER, "adapter_version": "0.1.0", "upstream_version": PINNED_VERSION})
