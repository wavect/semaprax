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

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "sdk", "python"))
from semaprax_harness_adapter import PROTOCOL, result  # noqa: E402

import model_profile  # noqa: E402
import systemone_codec as codec  # noqa: E402

ADAPTER_VERSION = "0.2.0"
DEFAULT_MAX_RESPONSE = 65536
MODELS_MAX = 65536
WARN_BOTH_THRESHOLDS = "SPX-HPK101"


def _float_env(env, name):
    raw = env.get(name)
    if not raw:
        return None
    try:
        return float(raw)
    except ValueError:
        return float("nan")


class Config:
    """Adapter configuration: environment + one backend + the model profile (data).

    Credentials and endpoints come from the environment only; the profile
    carries model facts only. `SEMAPRAX_HARNESS_MIN_SCORE` is a deprecated alias
    for the host-side minimum chosen-option mass and is never forwarded
    upstream; `SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE` is the backend-native
    abstention threshold (MR-02).
    """

    def __init__(self, env, backend):
        self.env = env
        self.backend = backend
        self.secret = backend.secret
        self.endpoint = env.get("SEMAPRAX_HARNESS_ENDPOINT") or backend.default_endpoint
        self.approved = env.get("SEMAPRAX_HARNESS_REMOTE_APPROVED") == "1"
        self.min_score = _float_env(env, "SEMAPRAX_HARNESS_MIN_SCORE")
        self.native_min = _float_env(env, "SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE")
        self.profile = None
        self.profile_error = None
        try:
            self.profile = model_profile.load(env, lambda: backend.default_profile(env))
        except codec.CodecError as err:
            self.profile_error = err  # raised at invocation time, before any network use

    def scrub(self, text):
        text = str(text)
        if self.secret:
            text = text.replace(self.secret, "[redacted]")
        return text[:300]

    def target(self):
        """(scheme, host, port, remote) after the backend's endpoint policy; raises CodecError."""
        return self.backend.endpoint_policy(self)

    def validate(self):
        """Config rules shared by every backend, then the backend's own."""
        self.target()
        if self.profile_error:
            raise self.profile_error
        self.backend.validate_config(self)
        for name, v in (("SEMAPRAX_HARNESS_MIN_SCORE", self.min_score), ("SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE", self.native_min)):
            if v is not None and not 0.0 <= v <= 1.0:
                raise codec.CodecError("refused", "SPX-HPK006", f"{name} must be in [0,1]")
        if self.native_min is not None and self.backend.native_threshold_field is None:
            raise codec.CodecError("refused", "SPX-HPK006", "this backend has no native confidence threshold")
        if self.profile["scoreless"] and self.min_score is not None:
            raise codec.CodecError("refused", "SPX-HPK006", "a scoreless profile cannot apply a minimum option mass")

    def warnings(self):
        if self.min_score is not None and self.native_min is not None:
            return [{"code": WARN_BOTH_THRESHOLDS, "message": "warning: SEMAPRAX_HARNESS_MIN_SCORE (deprecated alias, host-side min option mass) "
                     "and SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE (forwarded upstream) are both set; they are independent thresholds"}]
        return []


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


def encode_body(body):
    """The one serialization of an upstream body; its length is the wire size."""
    return json.dumps(body, separators=(",", ":"), sort_keys=True).encode()


def http_call(cfg, inv, method, path, data=None, limit=None):
    """One bounded HTTP exchange -> (status_code, bytes). Never follows redirects.

    `data` is the already-serialized request body (bytes) or None.
    """
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
    if data is not None:
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


class Context:
    """What a backend may use: bounded exchanges bound to one config + invocation."""

    def __init__(self, cfg, inv):
        self.cfg, self.inv = cfg, inv

    def get(self, path, limit=None):
        return http_call(self.cfg, self.inv, "GET", path, None, limit)

    def post(self, path, data, limit=None):
        return http_call(self.cfg, self.inv, "POST", path, data, limit)

    @staticmethod
    def status_error(code):
        return status_error(code)


def _adapter_ref(provenance):
    return f"{provenance['provider_id']}@{provenance['adapter_version']}"


def evaluate(cfg, req, inv, provenance=None):
    """decision.evaluate v1/v2/v3 -> (status, payload, diagnostics). Task selects the wire.

    v3 carries only choice-select/v1 (MR-11) and v1/v2 never carry it; the
    choice content is host-rendered and forwarded verbatim like v2.
    """
    started = time.monotonic()
    cfg.validate()
    backend = cfg.backend
    payload = req.get("payload")
    choice = isinstance(payload, dict) and payload.get("task") == codec.TASK_CHOICE
    if choice != (req.get("capability", {}).get("version") == codec.CHOICE_VERSION):
        raise codec.CodecError("unsupported", "SPX-HPK004", "choice-select/v1 travels only as decision.evaluate v3")
    v2 = choice or (isinstance(payload, dict) and payload.get("task") == codec.TASK_V2)
    digest, extra_diag = None, []
    if choice:
        v2req = codec.validate_request_choice(payload)
        options = v2req["options"]
        prof = cfg.profile
        if len(options) > prof["max_options"] or len(v2req["rendered"]["state"].encode()) > prof["max_state_bytes"]:
            raise codec.CodecError("refused", "SPX-HPK005", "request exceeds the model profile limits")
        qid, body = codec.build_body_choice(req["invocation_id"], v2req)
        digest = v2req["rendered"]["digest"]
    elif v2:
        v2req = codec.validate_request_v2(payload)
        options = v2req["options"]
        prof = cfg.profile
        if len(options) > prof["max_options"] or len(v2req["rendered"]["state"].encode()) > prof["max_state_bytes"]:
            raise codec.CodecError("refused", "SPX-HPK005", "request exceeds the model profile limits")
        if not set(v2req["features"]["input_modalities"]) <= set(prof["modalities"]):
            raise codec.CodecError("unsupported", "SPX-HPK004", "input modality is outside the model profile")
        if v2req["rendered"]["renderer"] != prof["renderer"]:
            raise codec.CodecError("unsupported", "SPX-HPK004", "renderer is outside the model profile")
        qid, body = codec.build_body_v2(req["invocation_id"], v2req)
        digest = v2req["rendered"]["digest"]
    else:
        features, options = codec.validate_request_payload(payload)
        qid, body = codec.build_body(req["invocation_id"], features, options, cfg.profile["model"])
    body = backend.envelope(body, cfg, cfg.native_min)
    wire = encode_body(body)
    if v2 and len(wire) > v2req["max_wire_bytes"]:
        raise codec.CodecError("refused", "SPX-HPK005", "upstream request exceeds max_wire_bytes")
    ctx = Context(cfg, inv)
    backend.discover(ctx)
    code, raw = backend.send(ctx, wire)
    if code != 200:
        raise status_error(code)
    doc = codec.parse_json(raw, inv.max_bytes)
    out, info = codec.parse_response(doc, qid, options, cfg.min_score)
    backend.check_response(info, cfg)
    if v2:
        out = codec.build_result_v2(out, info, cfg, _adapter_ref(provenance or {"provider_id": backend.name, "adapter_version": ADAPTER_VERSION}), digest, len(wire))
    elapsed = int((time.monotonic() - started) * 1000)
    parts = [f"latency_ms={elapsed}", f"profile={cfg.profile['profile_id']}", f"requested_model={cfg.profile['model']}"]
    if info.get("model"):
        parts.append(f"answering_model={info['model']}")
    if info.get("checkpoint"):
        parts.append(f"checkpoint={info['checkpoint']}")
    if "input_tokens" in info:
        parts.append(f"input_tokens={info['input_tokens']}")
    if backend.call_identity(info, cfg)[2] == "mutable_service":
        parts.append("model_is_mutable_alias=true")
    return "complete", out, [{"code": "SPX-HPK100", "message": cfg.scrub("; ".join(parts))}] + cfg.warnings()


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

    negotiated = {"set": None}  # (kind, version) pairs once initialized

    def invoke(mid, req):
        inv = Invocation(req.get("deadline_ms", 1000), min(req["budget"]["max_result_bytes"], DEFAULT_MAX_RESPONSE))
        active[req["invocation_id"]] = inv
        if req["invocation_id"] in cancelled_early:
            inv.cancel()
        try:
            if (req["capability"]["kind"], req["operation"]) != ("decision.evaluate", "evaluate"):
                env = result(req, "unsupported", None, provenance, [{"code": "unsupported", "message": "operation not implemented"}])
            elif negotiated["set"] is not None and (req["capability"]["kind"], req["capability"].get("version")) not in negotiated["set"]:
                # A capability version this session did not negotiate (for example
                # choice-select/v1 without v3) is refused before any inference.
                env = result(req, "unsupported", None, provenance, [{"code": "SPX-HPK004", "message": "capability version was not negotiated"}])
            else:
                try:
                    status, payload, diags = evaluate(cfg, req, inv, provenance)
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
            acc = [c for c in accepted if (c["kind"], c["version"]) in offered]
            negotiated["set"] = {(c["kind"], c["version"]) for c in acc}
            send({"jsonrpc": "2.0", "id": mid, "result": {"protocol": PROTOCOL, "accepted": acc}})
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
