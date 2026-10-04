#!/usr/bin/env python3
"""command.view/v1 adapter for Caveman input compression (pinned 3.1.0, commit 8af1f1b9).

Mirrors the middleware protocol 1.1 client of the pinned SDK (docs/technical/middleware-protocol.md
and packages/sdk/python/caveman_cloud/middleware/{runtime,validate,protocol,types}.py). It sends
already-captured output to a user-started loopback runtime (`caveman start`, run with DO_NOT_TRACK=1
and CAVEMAN_WORK_TAGS=0) as one `tool_result` segment with `mode:"compress"`:
GET capabilities -> POST optimize -> full plan validation (validate.plan, §7) -> POST sessions/delete.
Auth (§2): every route needs `Authorization: Bearer <runtime credential>` from the host-provisioned
`<retention>/caveman-token`; without it NO request is made. No Origin header, no provider API key, no
receipts, no redirects, no proxy. The Caveman marker and handle are stripped; the Semaprax retention
store is the only authoritative raw recovery, and the upstream copy is revoked right after the call (§12).
Endpoint: `<retention>/caveman-endpoint` (`host:port`, loopback only), default 127.0.0.1:8787.
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
FEATURES = "http_status_v2, revision_tolerant"  # types.py CLIENT_FEATURES_HEADER_VALUE (mw.md §3)
CLIENT = "semaprax-command-view/0.1.0"
MARKER = "[caveman: shortened; exact original via caveman_retrieve handle="  # types.py RECOVERY_MARKER_PREFIX
TOKEN = re.compile(r"[a-zA-Z0-9._:/-]{1,256}")
HANDLE = re.compile(r"cmw_[a-f0-9]{48}")
DIGEST = re.compile(r"[a-f0-9]{64}")
OVERHEAD = "Exact output is recoverable through the Semaprax recovery handle."


class Invalid(Exception):
    pass


def endpoint(ret):
    path = os.path.join(ret, "caveman-endpoint")
    text = open(path).read().strip() if os.path.isfile(path) else DEFAULT_ENDPOINT
    host, _, port = text.rpartition(":")
    if not host or not port.isdigit() or not _loopback(host):
        raise AdapterError("refused", "caveman-egress", "endpoint is not a loopback host:port")
    return host, int(port)


def credential(ret):
    """mw.md §2: a runtime credential is mandatory on every route, loopback included."""
    tok = os.path.join(ret, "caveman-token")
    value = open(tok).read().strip() if os.path.isfile(tok) else ""
    if not value or "\n" in value or "\r" in value:
        raise AdapterError("unavailable", "caveman-credential-missing", "caveman runtime credential not provisioned")
    return value


def call(ret, token, method, path, body=None):
    """One loopback HTTP call; http.client never consults proxies and never follows redirects."""
    host, port = endpoint(ret)
    headers = {"Authorization": "Bearer " + token, "Caveman-Middleware-Features": FEATURES,
               "Caveman-Middleware-Client": CLIENT}
    data = None
    if body is not None:
        data = body.encode("utf-8")
        headers["Content-Type"] = "application/json"
    conn = http.client.HTTPConnection(host, port, timeout=CALL_TIMEOUT)
    try:
        conn.request(method, PREFIX + path, body=data, headers=headers)
        resp = conn.getresponse()
        raw = resp.read((4 << 20) + 1)
    except (OSError, http.client.HTTPException):
        raise AdapterError("unavailable", "caveman-unreachable", "local Caveman runtime is not reachable")
    finally:
        conn.close()
    if len(raw) > 4 << 20:
        raise AdapterError("failed", "caveman-payload-limit", "runtime reply exceeds 4 MiB")
    try:
        doc = json.loads(raw.decode("utf-8"))
    except ValueError:
        doc = None
    if resp.status != 200:  # 3xx is refused (redirect_refused), never followed
        code = ((doc or {}).get("error") or {}).get("code", "error") if isinstance(doc, dict) else "error"
        raise AdapterError("failed", "caveman-http", f"runtime answered {resp.status} {code}")
    if doc is None:
        raise AdapterError("failed", "caveman-invalid-output", "runtime reply is not JSON")
    return doc


def _tok(v):
    return isinstance(v, str) and TOKEN.fullmatch(v) is not None


def _int(v):
    return type(v) is int and 0 <= v <= 9007199254740991


def _pos(v):
    return type(v) is int and 0 < v <= 9007199254740991


def parse_capabilities(v):
    """protocol.py parse_capabilities (mw.md §4): (usable transforms, mode, segment_bytes, policy_revision)."""
    if not isinstance(v, dict) or v.get("schema_version") != 1 or type(v.get("schema_version")) is not int:
        raise Invalid("schema_version")
    lim, proto = v.get("limits"), v.get("protocol")
    if isinstance(proto, dict) and _int(proto.get("min")) and _int(proto.get("max")) and not proto["min"] <= 1 <= proto["max"]:
        raise Invalid("protocol")
    if (not _tok(v.get("policy_revision")) or not isinstance(v.get("runtime_build"), str)
            or not isinstance(v.get("transforms"), list) or not isinstance(lim, dict)
            or not all(_pos(lim.get(k)) for k in ("deadline_ms", "request_bytes", "segment_bytes", "page_bytes"))
            or type(v.get("persistent")) is not bool or type(v.get("recovery")) is not bool or not _int(v.get("retention_seconds"))):
        raise Invalid("capabilities")
    usable, seen = [], set()
    for t in v["transforms"]:
        if (isinstance(t, dict) and _tok(t.get("transform_id")) and _tok(t.get("implementation_version"))
                and isinstance(t.get("eligible_segment_kinds"), list) and t.get("deterministic") is True
                and t.get("recovery") == "exact_ccr" and t["transform_id"] not in seen):
            seen.add(t["transform_id"])
            usable.append(t)
    mode = v["mode"] if v.get("mode") in ("record", "compress") else "record"
    return usable, mode, lim["segment_bytes"], v["policy_revision"]


def validate_plan(p, request, input_digest, transforms, caps_mode, segment_bytes):
    """validate.py plan() (mw.md §7): the whole plan is checked before anything is used."""
    try:
        if (p["schema_version"] != 1 or type(p["schema_version"]) is not int or p["request_id"] != request["request_id"]
                or p["input_digest"] != input_digest or not _tok(p["policy_revision"])
                or not (isinstance(p["replacement_set_id"], str) and DIGEST.fullmatch(p["replacement_set_id"]))
                or p["status"] not in ("optimized", "bypassed", "record") or not isinstance(p["reason"], str)
                or not isinstance(p["replacements"], list) or not isinstance(p["skipped"], list)):
            raise ValueError
        m, rec = p["measurement"], p["recovery"]
        if (m["basis"] != "inferred" or m["scope"] != "segment" or m["verified_saved_usd"] != 0
                or not isinstance(m["tokenizer"], str)
                or not all(_int(m[k]) for k in ("tokens_before", "tokens_after", "unique_tokens_reduced", "recovery_overhead_tokens"))
                or m["tokens_after"] > m["tokens_before"] or p["stability"]["provider_bytes"] != "unobserved"
                or p["stability"]["provider_cache_hits"] != "unobserved"
                or p["stability"]["native"] not in ("persistent_choices", "unavailable")
                or type(rec["available"]) is not bool or type(rec["persistent"]) is not bool or not _int(rec["expires_at"])):
            raise ValueError
        segs = {s["id"]: s for s in request["segments"]}
        by_id = {t["transform_id"]: t for t in transforms}
        seen, credited, reduction, unique = set(), set(), 0, 0
        for r in p["replacements"]:
            s, t = segs[r["segment_id"]], by_id[r["transform_id"]]
            size = len(r["text"].encode("utf-8"))
            if (r["segment_id"] in seen or r["original_sha256"] != s["sha256"] or r["source_id"] != s["source_id"]
                    or r["transform_id"] not in request["policy"]["transforms"] or not _tok(r["transform_version"])
                    or s["kind"] not in t["eligible_segment_kinds"] or request["mode"] == "record" or caps_mode == "record"
                    or not isinstance(r["text"], str) or size > segment_bytes or size >= len(s["content"].encode("utf-8"))
                    or not DIGEST.fullmatch(r["sha256"]) or r["sha256"] != hashlib.sha256(r["text"].encode("utf-8")).hexdigest()
                    or not _int(r["tokens_before"]) or not _int(r["tokens_after"]) or r["tokens_after"] >= r["tokens_before"]
                    or type(r["reused"]) is not bool or type(r["unique_original"]) is not bool
                    or (r["unique_original"] and (r["reused"] or r["original_sha256"] in credited))):
                raise ValueError
            if (not request["recovery_binding"] or rec["binding_id"] != request["recovery_binding"]["id"]
                    or not rec["available"] or not rec["persistent"] or not HANDLE.fullmatch(r["recovery_handle"])
                    or not r["text"].startswith(f"{MARKER}{r['recovery_handle']}]\n")):
                raise ValueError
            seen.add(r["segment_id"])
            reduction += r["tokens_before"] - r["tokens_after"]
            if r["unique_original"]:
                unique += r["tokens_before"] - r["tokens_after"]
                credited.add(r["original_sha256"])
        for k in p["skipped"]:
            if k["segment_id"] not in segs or k["segment_id"] in seen or not isinstance(k["reason"], str):
                raise ValueError
            seen.add(k["segment_id"])
        if (len(seen) != len(segs) or m["tokens_before"] - m["tokens_after"] != reduction or m["unique_tokens_reduced"] != unique
                or (p["replacements"] and (p["status"] != "optimized" or reduction <= m["recovery_overhead_tokens"]))
                or (p["status"] == "optimized" and not p["replacements"])):
            raise ValueError
    except (KeyError, TypeError, ValueError, UnicodeError):
        raise AdapterError("failed", "caveman-invalid-plan", "runtime plan failed protocol 1.1 validation")


def plan(req):
    p = req.get("payload") or {}
    if p.get("external_hooks") or "caveman" in (p.get("lineage") or []):
        return "complete", {"form": "post-execution", "route": "bypass", "reason": "external-owner-holds-the-view"}, []
    try:
        credential(retention_dir())
    except AdapterError as e:
        if e.code == "caveman-credential-missing":
            return "complete", {"form": "post-execution", "route": "bypass", "reason": "caveman runtime credential not provisioned"}, []
        raise
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


def _revoke(ret, token, scope):
    """mw.md §12 sessions/delete: drop the upstream copy of the original; best effort, never changes the view."""
    try:
        call(ret, token, "POST", "sessions/delete",
             json.dumps({"schema_version": 1, "scope": scope}, separators=(",", ":")))
    except AdapterError:
        pass


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
    token = credential(ret)  # no credential: no request at all (mw.md §2)
    try:
        transforms, caps_mode, segment_bytes, revision = parse_capabilities(call(ret, token, "GET", "capabilities"))
    except Invalid:
        raise AdapterError("failed", "caveman-invalid-output", "capabilities failed the section 4 reader")
    if caps_mode != "compress":
        return _unsupported("runtime-mode-record")  # record is the upstream default and changes no payload
    usable = [t for t in transforms if "tool_result" in t["eligible_segment_kinds"]]
    if not usable or len(text.encode("utf-8")) > segment_bytes:
        return _unsupported("no-eligible-transform")
    rid = str(uuid.uuid4())
    sid = hashlib.sha256(text.encode("utf-8")).hexdigest()[:24]
    scope = {"namespace": "semaprax", "session_id": "sx-" + uuid.uuid4().hex, "branch_id": "main", "cache_epoch": "0"}
    seg = {"id": "cv-" + sid, "source_id": "cv-" + sid, "kind": "tool_result", "cache_region": "live_zone",
           "content": text, "sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(), "protected": False, "opaque": False}
    request = {"schema_version": 1, "request_id": rid, "logical_call_id": rid, "attempt_id": str(uuid.uuid4()),
               "idempotency_key": rid, "scope": scope, "sequence": 0,
               "adapter": {"id": "semaprax-command-view", "version": "0.1.0", "framework_version": "none", "serialization_revision": "1"},
               "model": None, "mode": "compress",
               "policy": {"revision": revision, "transforms": [t["transform_id"] for t in transforms]},
               "segments": [seg], "context_manifest": [],
               # runtime.py: compress needs an owned binding; declared only to satisfy it, the marker is stripped below.
               "recovery_binding": {"id": str(uuid.uuid4()), "kind": "host_tool", "tool_name": "caveman_retrieve", "overhead_text": OVERHEAD}}
    body = json.dumps(request, ensure_ascii=False, separators=(",", ":"), allow_nan=False)
    try:
        plan_doc = call(ret, token, "POST", "optimize", body)
        validate_plan(plan_doc, request, hashlib.sha256(body.encode("utf-8")).hexdigest(), transforms, caps_mode, segment_bytes)
    finally:
        _revoke(ret, token, scope)
    if plan_doc["status"] != "optimized":
        return _unsupported(f"runtime-{plan_doc['status']}-{plan_doc['reason']}")  # decisions are 200 + bypassed (mw.md §6)
    rep = plan_doc["replacements"][0]
    out = rep["text"][len(f"{MARKER}{rep['recovery_handle']}]\n"):]  # Semaprax's own recovery reference replaces Caveman's
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
