"""Transport, configuration and frame loop shared by the SystemOne adapters.

Standard library only. Configuration comes from host-provided environment
variables only (never files, never $HOME). A secret is read once, sent only in
the Authorization header, and scrubbed from anything this process emits.
"""

import http.client
import json
import os
import socket
import ssl
import sys
import threading
import time
import urllib.parse

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "sdk", "python"))
from semaprax_harness_adapter import PROTOCOL, result  # noqa: E402

import systemone_codec as codec  # noqa: E402

ADAPTER_VERSION = "0.1.0"
LOOPBACK = ("127.0.0.1", "localhost", "::1")
DEFAULT_MAX_RESPONSE = 65536
MODELS_MAX = 65536


def is_loopback(host):
    return host in LOOPBACK


class Config:
    """Adapter configuration resolved from an environment mapping."""

    def __init__(self, env, kind):
        self.kind = kind
        self.env = env
        self.secret = env.get("SEMAPRAX_HARNESS_SECRET_JEV" if kind == "jev" else "SEMAPRAX_HARNESS_SECRET_LAYA") or None
        self.model = env.get("SEMAPRAX_HARNESS_MODEL") or ("multilingual" if kind == "laya" else None)
        self.endpoint = env.get("SEMAPRAX_HARNESS_ENDPOINT") or ("https://api.typesafe.ai" if kind == "jev" else None)
        self.approved = env.get("SEMAPRAX_HARNESS_REMOTE_APPROVED") == "1"
        raw = env.get("SEMAPRAX_HARNESS_MIN_SCORE")
        self.min_score = None
        if raw:
            try:
                self.min_score = float(raw)
            except ValueError:
                self.min_score = float("nan")

    def scrub(self, text):
        text = str(text)
        if self.secret:
            text = text.replace(self.secret, "[redacted]")
        return text[:300]

    def target(self):
        """(scheme, host, port, remote) after the endpoint policy; raises CodecError."""
        if not self.endpoint:
            raise codec.CodecError("refused", "SPX-HPK002", "SEMAPRAX_HARNESS_ENDPOINT is required (a user-selected existing server)")
        try:
            u = urllib.parse.urlsplit(self.endpoint)
            port = u.port
        except ValueError:
            raise codec.CodecError("refused", "SPX-HPK002", "endpoint is not a valid URL")
        if u.scheme not in ("http", "https") or not u.hostname or u.username or u.password or u.query or u.fragment or u.path not in ("", "/"):
            raise codec.CodecError("refused", "SPX-HPK002", "endpoint must be scheme://host[:port] without credentials or path")
        loop = is_loopback(u.hostname)
        if self.kind == "laya":
            if not loop or u.scheme != "http":
                raise codec.CodecError("refused", "SPX-HPK002", "laya-local only talks to an http loopback server")
            return "http", u.hostname, port or 80, False
        if not loop:
            if not self.approved:
                raise codec.CodecError("refused", "SPX-HPK001", "remote use needs host approval (SEMAPRAX_HARNESS_REMOTE_APPROVED=1)")
            if u.scheme != "https":
                raise codec.CodecError("refused", "SPX-HPK002", "a remote endpoint must use https")
        return u.scheme, u.hostname, port or (443 if u.scheme == "https" else 80), not loop


class Invocation:
    """One in-flight call: deadline, cancellation and the live connection."""

    def __init__(self, deadline_ms, max_bytes):
        self.deadline = time.monotonic() + max(deadline_ms, 1) / 1000.0
        self.max_bytes = max_bytes
        self.cancelled = threading.Event()
        self.conn = None
        self.lock = threading.Lock()

    def remaining(self):
        return self.deadline - time.monotonic()

    def cancel(self):
        self.cancelled.set()
        with self.lock:
            conn = self.conn
        if conn is not None and conn.sock is not None:
            try:
                conn.sock.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            conn.close()


def http_call(cfg, inv, method, path, body=None, limit=None):
    """One bounded HTTP exchange -> (status_code, bytes). Never follows redirects."""
    scheme, host, port, _ = cfg.target()
    limit = limit or inv.max_bytes
    if inv.cancelled.is_set():
        raise codec.CodecError("refused", "SPX-HPK013", "cancelled")
    if inv.remaining() <= 0:
        raise codec.CodecError("failed", "SPX-HPK011", "deadline exhausted before the call")
    if scheme == "https":
        conn = http.client.HTTPSConnection(host, port, timeout=inv.remaining(), context=ssl.create_default_context())
    else:
        conn = http.client.HTTPConnection(host, port, timeout=inv.remaining())
    headers = {"Accept": "application/json", "User-Agent": "semaprax-harness-systemone/" + ADAPTER_VERSION}
    if cfg.secret:
        headers["Authorization"] = "Bearer " + cfg.secret
    data = None
    if body is not None:
        data = json.dumps(body, separators=(",", ":"), sort_keys=True).encode()
        headers["Content-Type"] = "application/json"
    with inv.lock:
        inv.conn = conn
    try:
        conn.request(method, path, body=data, headers=headers)
        resp = conn.getresponse()
        sock = conn.sock
        chunks, size = [], 0
        while True:
            if inv.remaining() <= 0:
                raise socket.timeout()
            if sock is not None:
                sock.settimeout(max(inv.remaining(), 0.001))
            chunk = resp.read(8192)
            if not chunk:
                break
            size += len(chunk)
            if size > limit:
                raise codec.CodecError("refused", "SPX-HPK007", f"response exceeds {limit} bytes")
            chunks.append(chunk)
        return resp.status, b"".join(chunks)
    except codec.CodecError:
        raise
    except (socket.timeout, TimeoutError):
        if inv.cancelled.is_set():
            raise codec.CodecError("refused", "SPX-HPK013", "cancelled")
        raise codec.CodecError("failed", "SPX-HPK011", "timeout")
    except ConnectionRefusedError:
        raise codec.CodecError("unavailable", "SPX-HPK016", "endpoint is not accepting connections (no server is started or downloaded)")
    except (OSError, http.client.HTTPException, ValueError, AttributeError):
        if inv.cancelled.is_set():
            raise codec.CodecError("refused", "SPX-HPK013", "cancelled")
        raise codec.CodecError("failed", "SPX-HPK012", "transport error")
    finally:
        with inv.lock:
            inv.conn = None
        conn.close()


def status_error(code):
    """Map a non-200 HTTP status to a refusal without echoing the body."""
    if code in (401, 403):
        return codec.CodecError("refused", "SPX-HPK012", f"authentication or entitlement refused (HTTP {code})")
    if code in (400, 404, 413, 422):
        return codec.CodecError("refused", "SPX-HPK012", f"request refused by the server (HTTP {code})")
    return codec.CodecError("failed", "SPX-HPK012", f"server error (HTTP {code})")


def entitled(cfg, inv):
    """Jev: confirm the configured model name is listed by GET /v1/models."""
    code, raw = http_call(cfg, inv, "GET", "/v1/models", limit=MODELS_MAX)
    if code != 200:
        raise status_error(code)
    doc = codec.parse_json(raw, MODELS_MAX)
    names = [m.get("name") for m in (doc.get("models") if isinstance(doc, dict) else None) or [] if isinstance(m, dict)]
    if cfg.model not in names:
        raise codec.CodecError("refused", "SPX-HPK014", "configured model is not listed for this account")


def evaluate(cfg, req, inv):
    """decision.evaluate/v1 -> (status, payload, diagnostics)."""
    started = time.monotonic()
    cfg.target()
    if cfg.kind == "jev":
        if not cfg.secret:
            raise codec.CodecError("refused", "SPX-HPK003", "SEMAPRAX_HARNESS_SECRET_JEV is not provided by the host")
        if not cfg.model:
            raise codec.CodecError("refused", "SPX-HPK014", "SEMAPRAX_HARNESS_MODEL must name an entitled model")
    if cfg.min_score is not None and not 0.0 <= cfg.min_score <= 1.0:
        raise codec.CodecError("refused", "SPX-HPK006", "SEMAPRAX_HARNESS_MIN_SCORE must be in [0,1]")
    features, options = codec.validate_request_payload(req.get("payload"))
    extra = {}
    if cfg.kind == "laya":
        extra["lang"] = codec.SUPPORTED_LANGUAGE
        if cfg.min_score is not None:
            extra["min_confidence"] = cfg.min_score
    qid, body = codec.build_body(req["invocation_id"], features, options, cfg.model, extra)
    if cfg.kind == "jev":
        entitled(cfg, inv)
    code, raw = http_call(cfg, inv, "POST", "/v1/systemone", body)
    if code != 200:
        raise status_error(code)
    doc = codec.parse_json(raw, inv.max_bytes)
    payload, info = codec.parse_response(doc, qid, options, cfg.min_score)
    if cfg.kind == "laya" and info.get("checkpoint") not in (None, cfg.model):
        raise codec.CodecError("refused", "SPX-HPK008", "server answered with a different checkpoint than requested")
    elapsed = int((time.monotonic() - started) * 1000)
    parts = [f"latency_ms={elapsed}", f"requested_model={cfg.model}"]
    if info.get("model"):
        parts.append(f"answering_model={info['model']}")
    if info.get("checkpoint"):
        parts.append(f"checkpoint={info['checkpoint']}")
    if "input_tokens" in info:
        parts.append(f"input_tokens={info['input_tokens']}")
    if cfg.kind == "jev" and cfg.model.endswith("latest"):
        parts.append("model_is_mutable_alias=true")
    return "complete", payload, [{"code": "SPX-HPK100", "message": cfg.scrub("; ".join(parts))}]


def run(cfg, accepted, provenance, stdin=None, stdout=None):
    """Frame loop with a worker thread so harness/cancel can interrupt a call."""
    stdin = stdin or sys.stdin.buffer
    stdout = stdout or sys.stdout.buffer
    out_lock = threading.Lock()
    active = {}
    cancelled_early = set()
    worker = {"t": None}

    def send(obj):
        line = json.dumps(obj, separators=(",", ":"), sort_keys=True).encode() + b"\n"
        with out_lock:
            stdout.write(line)
            stdout.flush()

    def invoke(mid, req):
        inv = Invocation(req.get("deadline_ms", 1000), min(req["budget"]["max_result_bytes"], DEFAULT_MAX_RESPONSE))
        active[req["invocation_id"]] = inv
        if req["invocation_id"] in cancelled_early:
            inv.cancel()
        try:
            if (req["capability"]["kind"], req["operation"]) != ("decision.evaluate", "evaluate"):
                env = result(req, "unsupported", None, provenance, [{"code": "unsupported", "message": "operation not implemented"}])
            else:
                try:
                    status, payload, diags = evaluate(cfg, req, inv)
                    env = result(req, status, payload, provenance, diags)
                except codec.CodecError as err:
                    env = result(req, err.status, None, provenance, [{"code": err.code, "message": cfg.scrub(err.message)}])
                except Exception as err:  # never leak a message that could hold a secret
                    env = result(req, "failed", None, provenance, [{"code": "SPX-HPK099", "message": "internal adapter error: " + type(err).__name__}])
        finally:
            active.pop(req["invocation_id"], None)
        send({"jsonrpc": "2.0", "id": mid, "result": env})

    for raw in stdin:
        line = raw.rstrip(b"\n")
        if not line:
            continue
        msg = json.loads(line)
        method, mid = msg.get("method"), msg.get("id")
        if method == "harness/initialize":
            params = msg.get("params", {})
            if params.get("protocol") != PROTOCOL:
                send({"jsonrpc": "2.0", "id": mid, "error": {"code": -32600, "message": "unsupported protocol"}})
                continue
            offered = {(c["kind"], c["version"]) for c in params.get("offered", [])}
            send({"jsonrpc": "2.0", "id": mid, "result": {"protocol": PROTOCOL, "accepted": [c for c in accepted if (c["kind"], c["version"]) in offered]}})
        elif method == "harness/cancel":
            iid = msg.get("params", {}).get("invocation_id")
            cancelled_early.add(iid)
            inv = active.get(iid)
            if inv:
                inv.cancel()
        elif method == "harness/shutdown":
            if worker["t"]:
                worker["t"].join(timeout=5)
            send({"jsonrpc": "2.0", "id": mid, "result": {}})
            return
        elif method == "harness/invoke":
            t = threading.Thread(target=invoke, args=(mid, msg["params"]), daemon=True)
            worker["t"] = t
            t.start()
        else:
            send({"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "method not found"}})
    for inv in list(active.values()):
        inv.cancel()
    if worker["t"]:
        worker["t"].join(timeout=5)
