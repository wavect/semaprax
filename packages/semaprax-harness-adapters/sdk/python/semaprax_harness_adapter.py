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


class _Slot:
    """One invocation's single terminal decision.

    The supervisor is the only sender. A worker hands its outcome to the slot;
    the first decision (success, cancellation or deadline) wins and later ones
    are discarded, so a late success can never replace a selected failure.
    """

    def __init__(self, req, deadline):
        import threading

        self.req = req
        self.deadline = deadline
        self.cancel = threading.Event()
        self.done = threading.Event()  # the handler thread has really stopped
        self.lock = threading.Lock()
        self.started = False
        self.decision = None

    def request_cancel(self):
        with self.lock:
            self.cancel.set()

    def try_start(self):
        """Claim handler dispatch; refused once a cancel was requested."""
        with self.lock:
            if self.cancel.is_set() or self.decision is not None:
                return False
            self.started = True
            return True

    def decide(self, envelope):
        with self.lock:
            if self.decision is None:
                self.decision = envelope
            return self.decision

    def accept(self, envelope, now, cancelled_env, timeout_env):
        """A worker's outcome: accepted only if no earlier decision stands."""
        with self.lock:
            if self.decision is None:
                if self.cancel.is_set():
                    self.decision = cancelled_env
                elif now >= self.deadline:
                    self.decision = timeout_env
                else:
                    self.decision = envelope


def serve_cancellable(accepted, handlers, provenance, secret_env=(), env=None, stdin=None, stdout=None,
                      cancel_grace=2.0, stuck_exit=None):
    """Like `serve`, but a worker thread runs each invocation so cancel and deadline are honored.

    handlers: {(kind, operation): fn(request, cancelled) -> (status, payload, diagnostics)}
    where `cancelled` is a threading.Event the handler should poll or wait on.
    The loop itself enforces: a capability version outside the negotiated set ->
    unsupported/SPX-HPK004 before the handler runs, `harness/cancel` -> refused/SPX-HPK013,
    the request deadline (captured when the invoke is admitted) -> failed/SPX-HPK011, a payload larger than
    budget.max_result_bytes -> refused/SPX-HPK007 (never truncated), and any unexpected exception ->
    failed/SPX-HPK099 naming only the exception type. Values of the environment
    variables in `secret_env` are scrubbed from every diagnostic.

    Terminal ownership: each invocation has one supervisor and one decision. A cancel
    that arrives before the handler starts dispatches no handler work. A cancel or
    deadline sets the handler's event and the terminal reply is sent only after the
    handler thread has actually stopped, so a reply never frees the caller's concurrency
    slot while the handler still runs. A handler that does not stop within `cancel_grace`
    seconds cannot be killed safely from inside Python; the adapter then sends no
    receipt and calls `stuck_exit` (default: exit the process), handing enforcement to the
    host's process-level supervisor.
    """
    import os
    import threading
    import time

    stdin = stdin or sys.stdin.buffer
    stdout = stdout or sys.stdout.buffer
    env = os.environ if env is None else env
    secrets = [env[n] for n in secret_env if env.get(n)]
    out_lock = threading.Lock()
    state_lock = threading.Lock()
    live = {}  # invocation_id -> _Slot, only while its supervisor runs
    pending = set()  # supervisor threads still running

    def stuck():
        if stuck_exit is not None:
            stuck_exit()
        else:
            os._exit(70)

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

    def cancelled_env(req, message="cancelled"):
        return fail(req, "refused", "SPX-HPK013", message)

    def timeout_env(req):
        return fail(req, "failed", "SPX-HPK011", "timeout")

    def work(req, slot):
        try:
            fn = handlers.get((req["capability"]["kind"], req["operation"]))
            if fn is None:
                env_ = fail(req, "unsupported", "unsupported", "operation not implemented")
            else:
                try:
                    status, payload, diags = fn(req, slot.cancel)
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
            slot.accept(env_, time.monotonic(), cancelled_env(req), timeout_env(req))
        finally:
            slot.done.set()

    def supervise(mid, req, slot):
        try:
            if not slot.try_start():
                # Cancelled before dispatch: no handler work ran at all.
                slot.decide(cancelled_env(req, "cancelled before the handler started; no handler work ran"))
            else:
                threading.Thread(target=work, args=(req, slot), daemon=True).start()
                while not slot.done.wait(0.01):
                    if slot.cancel.is_set():
                        slot.decide(cancelled_env(req))
                        break
                    if time.monotonic() >= slot.deadline:
                        slot.cancel.set()
                        slot.decide(timeout_env(req))
                        break
                if not slot.done.is_set() and not slot.done.wait(cancel_grace):
                    stuck()  # the handler outlived its cleanup allowance; no receipt is honest
                    return
            send({"jsonrpc": "2.0", "id": mid, "result": slot.decision})
        finally:
            with state_lock:
                if live.get(req["invocation_id"]) is slot:
                    del live[req["invocation_id"]]
                pending.discard(threading.current_thread())

    cancelled_early = {}  # insertion-ordered, bounded: ids cancelled before their invoke
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
            with state_lock:
                slot = live.get(iid)
                if slot is None:
                    cancelled_early[iid] = True
                    while len(cancelled_early) > 1024:
                        del cancelled_early[next(iter(cancelled_early))]
            if slot is not None:
                slot.request_cancel()
        elif method == "harness/shutdown":
            with state_lock:
                threads = list(pending)
            for t in threads:
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
            slot = _Slot(req, time.monotonic() + max(req.get("deadline_ms", 1000), 1) / 1000.0)
            t = threading.Thread(target=supervise, args=(mid, req, slot), daemon=True)
            with state_lock:
                if cancelled_early.pop(req["invocation_id"], None):
                    slot.cancel.set()
                live[req["invocation_id"]] = slot
                pending.add(t)
            t.start()
        else:
            send({"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "method not found"}})
    with state_lock:
        slots, threads = list(live.values()), list(pending)
    for slot in slots:
        slot.request_cancel()
    for t in threads:
        t.join(timeout=5)
