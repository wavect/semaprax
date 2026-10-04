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
