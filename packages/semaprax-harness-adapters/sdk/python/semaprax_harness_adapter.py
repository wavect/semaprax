"""Minimal adapter-side helper for semaprax.harness-rpc.v1 (docs/HARNESS-PROVIDER-V1.md).

Standard library only. An adapter supplies its accepted capabilities and one
handler per (kind, operation); the helper owns framing, the initialize/
shutdown handshake, envelope echo and cancellation bookkeeping.
"""

import json
import sys

PROTOCOL = "semaprax.harness-rpc.v1"
RESULT_SCHEMA = "semaprax.harness-result.v1"
STATUSES = ("complete", "partial", "stale", "unavailable", "unsupported", "refused", "failed")


class AdapterError(Exception):
    """Raised by a handler to produce a non-complete status."""

    def __init__(self, status, code, message):
        super().__init__(message)
        if status not in STATUSES:
            raise ValueError(status)
        self.status, self.code, self.message = status, code, message


def result(request, status, payload, provenance, diagnostics=()):
    """Result envelope echoing the request's binding fields."""
    return {
        "schema": RESULT_SCHEMA,
        "invocation_id": request["invocation_id"],
        "project": request["project"],
        "capability": request["capability"],
        "status": status,
        "payload": payload,
        "diagnostics": list(diagnostics),
        "provenance": provenance,
    }


def serve(accepted, handlers, provenance, stdin=None, stdout=None):
    """Serve frames until shutdown or EOF.

    accepted: [{"kind","version","operations"}]
    handlers: {(kind, operation): fn(request) -> (status, payload, diagnostics)}
    provenance: {"provider_id","adapter_version","upstream_version"}
    """
    stdin = stdin or sys.stdin.buffer
    stdout = stdout or sys.stdout.buffer
    cancelled = set()

    def send(obj):
        stdout.write(json.dumps(obj, separators=(",", ":"), sort_keys=True).encode() + b"\n")
        stdout.flush()

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
            send({"jsonrpc": "2.0", "id": mid, "result": {
                "protocol": PROTOCOL,
                "accepted": [c for c in accepted if (c["kind"], c["version"]) in offered],
            }})
        elif method == "harness/cancel":
            cancelled.add(msg.get("params", {}).get("invocation_id"))
        elif method == "harness/shutdown":
            send({"jsonrpc": "2.0", "id": mid, "result": {}})
            return
        elif method == "harness/invoke":
            req = msg["params"]
            if req["invocation_id"] in cancelled:
                envelope = result(req, "refused", None, provenance, [{"code": "cancelled", "message": "cancelled before start"}])
            else:
                key = (req["capability"]["kind"], req["operation"])
                fn = handlers.get(key)
                if fn is None:
                    envelope = result(req, "unsupported", None, provenance, [{"code": "unsupported", "message": f"operation {key} not implemented"}])
                else:
                    try:
                        status, payload, diags = fn(req)
                        envelope = result(req, status, payload, provenance, diags)
                    except AdapterError as err:
                        envelope = result(req, err.status, None, provenance, [{"code": err.code, "message": err.message}])
            send({"jsonrpc": "2.0", "id": mid, "result": envelope})
        else:
            send({"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "method not found"}})


def serve_cancellable(accepted, handlers, provenance, secret_env=(), env=None, stdin=None, stdout=None):
    """Like `serve`, but a worker thread runs each invocation so cancel and deadline are honored.

    handlers: {(kind, operation): fn(request, cancelled) -> (status, payload, diagnostics)}
    where `cancelled` is a threading.Event the handler should poll or wait on.
    The loop itself enforces: a capability version outside the negotiated set ->
    unsupported/SPX-HPK004 before the handler runs, `harness/cancel` -> refused/cancelled, the request
    deadline -> failed/timeout, a payload larger than budget.max_result_bytes ->
    refused/SPX-HPK007 (never truncated), and any unexpected exception ->
    failed/SPX-HPK099 naming only the exception type. Values of the environment
    variables in `secret_env` are scrubbed from every diagnostic.
    """
    import os
    import threading
    import time

    stdin = stdin or sys.stdin.buffer
    stdout = stdout or sys.stdout.buffer
    env = os.environ if env is None else env
    secrets = [env[n] for n in secret_env if env.get(n)]
    out_lock = threading.Lock()
    live = {}  # invocation_id -> {"cancel": Event, "done": Event, "env": result envelope or None}

    def scrub(text):
        text = str(text)
        for s in secrets:
            text = text.replace(s, "[redacted]")
        return text[:300]

    def send(obj):
        with out_lock:
            stdout.write(json.dumps(obj, separators=(",", ":"), sort_keys=True).encode() + b"\n")
            stdout.flush()

    def fail(req, status, code, message):
        return result(req, status, None, provenance, [{"code": code, "message": scrub(message)}])

    def work(mid, req, slot):
        cancel = slot["cancel"]
        fn = handlers.get((req["capability"]["kind"], req["operation"]))
        if fn is None:
            env_ = fail(req, "unsupported", "unsupported", "operation not implemented")
        else:
            try:
                status, payload, diags = fn(req, cancel)
                limit = req.get("budget", {}).get("max_result_bytes")
                size = len(json.dumps(payload, separators=(",", ":"), sort_keys=True).encode()) if payload is not None else 0
                if limit is not None and size > limit:
                    env_ = fail(req, "refused", "SPX-HPK007", f"result exceeds {limit} bytes")
                else:
                    diags = [dict(d, message=scrub(d.get("message", ""))) for d in diags]
                    env_ = result(req, status, payload, provenance, diags)
            except AdapterError as err:
                env_ = fail(req, err.status, err.code, err.message)
            except Exception as err:  # never leak a message that could hold a secret
                env_ = fail(req, "failed", "SPX-HPK099", "internal adapter error: " + type(err).__name__)
        slot["env"] = env_
        slot["done"].set()

    def supervise(mid, req, slot):
        deadline = time.monotonic() + max(req.get("deadline_ms", 1000), 1) / 1000.0
        while not slot["done"].wait(0.01):
            if slot["cancel"].is_set():
                slot["env"] = fail(req, "refused", "SPX-HPK013", "cancelled")
                break
            if time.monotonic() >= deadline:
                slot["cancel"].set()
                slot["env"] = fail(req, "failed", "SPX-HPK011", "timeout")
                break
        live.pop(req["invocation_id"], None)
        send({"jsonrpc": "2.0", "id": mid, "result": slot["env"]})

    pending = []
    cancelled_early = set()
    negotiated = None  # set of (kind, version) once initialized
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
            negotiated = {(c["kind"], c["version"]) for c in acc}
            send({"jsonrpc": "2.0", "id": mid, "result": {"protocol": PROTOCOL, "accepted": acc}})
        elif method == "harness/cancel":
            iid = msg.get("params", {}).get("invocation_id")
            cancelled_early.add(iid)
            if iid in live:
                live[iid]["cancel"].set()
        elif method == "harness/shutdown":
            for t in pending:
                t.join(timeout=5)
            send({"jsonrpc": "2.0", "id": mid, "result": {}})
            return
        elif method == "harness/invoke":
            req = msg["params"]
            cap = (req["capability"]["kind"], req["capability"].get("version"))
            if negotiated is not None and cap not in negotiated:
                # Never run a handler for a capability version this session did
                # not negotiate (MR-11: choice-select/v1 needs decision.evaluate v3).
                send({"jsonrpc": "2.0", "id": mid, "result": fail(req, "unsupported", "SPX-HPK004", "capability version was not negotiated")})
                continue
            slot = {"cancel": threading.Event(), "done": threading.Event(), "env": None}
            if req["invocation_id"] in cancelled_early:
                slot["cancel"].set()
            live[req["invocation_id"]] = slot
            threading.Thread(target=work, args=(mid, req, slot), daemon=True).start()
            t = threading.Thread(target=supervise, args=(mid, req, slot), daemon=True)
            pending.append(t)
            t.start()
        else:
            send({"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "method not found"}})
    for slot in list(live.values()):
        slot["cancel"].set()
    for t in pending:
        t.join(timeout=5)
