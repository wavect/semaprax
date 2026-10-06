"""Negative-control adapter for the terminal-ownership conformance rules (DV-04).

TERM_MODE=good runs the real SDK loop. Every other mode is a deliberately broken
protocol loop violating exactly one rule:

  dispatch_after_cancel  runs the handler although the invoke arrived already cancelled
  double_reply           sends a second terminal frame after the cancel/deadline reply
  reply_while_running    replies on cancel/deadline while the handler keeps running
  late_success           ignores cancel and deadline and answers with the late success

TERM_STATE is a directory: `dispatches` gets one line per handler start and
`running` exists while a handler executes. The handler waits for cancel (up to
HANDLER_MAX seconds) like a slow upstream call.
"""

import json
import os
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import semaprax_harness_adapter as sdk  # noqa: E402

MODE = os.environ.get("TERM_MODE", "good")
STATE = os.environ["TERM_STATE"]
KINDS = [{"kind": "decision.evaluate", "version": 2, "operations": ["evaluate"]}]
PROV = {"provider_id": "neg", "adapter_version": "1", "upstream_version": "1"}
HANDLER_MAX = 30.0 if MODE == "good" else 1.2


def handler(req, cancelled):
    with open(os.path.join(STATE, "dispatches"), "a") as f:
        f.write("1\n")
    marker = os.path.join(STATE, "running")
    open(marker, "w").close()
    try:
        cancelled.wait(HANDLER_MAX)
        return "complete", {"late": True}, []
    finally:
        os.remove(marker)


def broken():
    lock = threading.Lock()
    live, early = {}, set()

    def send(obj):
        with lock:
            sys.stdout.buffer.write(json.dumps(obj).encode() + b"\n")
            sys.stdout.buffer.flush()

    def success(mid, req):
        send({"jsonrpc": "2.0", "id": mid, "result": sdk.result(req, "complete", {"late": True}, PROV)})

    def reply(mid, req, status, code):
        env = sdk.result(req, status, None, PROV, [{"code": code, "message": "x"}])
        send({"jsonrpc": "2.0", "id": mid, "result": env})
        if MODE == "double_reply":
            time.sleep(0.2)
            success(mid, req)

    def run(mid, req, ev, deadline):
        iid = req["invocation_id"]
        if iid in early and MODE != "dispatch_after_cancel":
            return reply(mid, req, "refused", "SPX-HPK013")
        h = threading.Thread(target=handler, args=(req, ev), daemon=True)
        h.start()
        while h.is_alive():
            if ev.is_set() or time.monotonic() >= deadline:
                if MODE == "late_success":
                    h.join()
                    return success(mid, req)
                cancelled = ev.is_set()
                if MODE != "reply_while_running":
                    ev.set()
                    h.join()
                return reply(mid, req, *(("refused", "SPX-HPK013") if cancelled else ("failed", "SPX-HPK011")))
            h.join(0.01)
        success(mid, req)  # the handler finished before any cancel or deadline

    for raw in sys.stdin.buffer:
        msg = json.loads(raw)
        m, mid = msg.get("method"), msg.get("id")
        if m == "harness/initialize":
            send({"jsonrpc": "2.0", "id": mid, "result": {"protocol": sdk.PROTOCOL, "accepted": KINDS}})
        elif m == "harness/cancel":
            iid = msg["params"]["invocation_id"]
            if iid in live:
                live[iid].set()
            else:
                early.add(iid)
        elif m == "harness/invoke":
            req = msg["params"]
            ev = threading.Event()
            live[req["invocation_id"]] = ev
            if req["invocation_id"] in early and MODE == "dispatch_after_cancel":
                ev.set()
            dl = time.monotonic() + req.get("deadline_ms", 1000) / 1000.0
            threading.Thread(target=run, args=(mid, req, ev, dl), daemon=True).start()
        elif m == "harness/shutdown":
            time.sleep(0.6)
            send({"jsonrpc": "2.0", "id": mid, "result": {}})
            return


if MODE == "good":
    sdk.serve_cancellable(KINDS, {("decision.evaluate", "evaluate"): handler}, PROV)
else:
    broken()
