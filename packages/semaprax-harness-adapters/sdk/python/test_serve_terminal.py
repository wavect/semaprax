"""Terminal ownership of `serve_cancellable` (DV-04, #564): one decision per invocation,
no handler dispatch after an early cancel, no reply while the handler still runs.

Run: python3 -m unittest discover -s packages/semaprax-harness-adapters/sdk/python
"""

import json
import os
import sys
import threading
import time
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import decision_fixtures as fx  # noqa: E402
import semaprax_harness_adapter as sdk  # noqa: E402

WAIT = 20.0
KINDS = [{"kind": "decision.evaluate", "version": 2, "operations": ["evaluate"]}]
PROV = {"provider_id": "x", "adapter_version": "1", "upstream_version": "1"}


class Out:
    """Thread-safe stdout collector with bounded waits."""

    def __init__(self):
        self.cv = threading.Condition()
        self.buf = b""

    def write(self, data):
        with self.cv:
            self.buf += data
            self.cv.notify_all()

    def flush(self):
        pass

    def frames(self):
        with self.cv:
            return [json.loads(x) for x in self.buf.splitlines() if x]

    def reply(self, mid):
        end = time.monotonic() + WAIT
        with self.cv:
            while True:
                hits = [f for f in map(json.loads, filter(None, self.buf.splitlines())) if f.get("id") == mid]
                if hits:
                    return hits[0]
                left = end - time.monotonic()
                if left <= 0:
                    raise AssertionError(f"no reply for id {mid}")
                self.cv.wait(left)


class Session:
    def __init__(self, handler, **kw):
        r, w = os.pipe()
        self.w = os.fdopen(w, "wb")
        self.out = Out()
        self.thread = threading.Thread(
            target=lambda: sdk.serve_cancellable(
                KINDS, {("decision.evaluate", "evaluate"): handler}, PROV,
                stdin=os.fdopen(r, "rb"), stdout=self.out, **kw),
            daemon=True)
        self.thread.start()
        self.send({"jsonrpc": "2.0", "id": 1, "method": "harness/initialize",
                   "params": {"protocol": sdk.PROTOCOL, "offered": [{"kind": "decision.evaluate", "version": 2}]}})
        self.out.reply(1)

    def send(self, obj):
        self.w.write(json.dumps(obj).encode() + b"\n")
        self.w.flush()

    def invoke(self, mid, iid, deadline_ms=60000):
        req = fx.v2_request()
        req["invocation_id"] = iid
        req["deadline_ms"] = deadline_ms
        self.send({"jsonrpc": "2.0", "id": mid, "method": "harness/invoke", "params": req})

    def cancel(self, iid):
        self.send({"jsonrpc": "2.0", "id": 0, "method": "harness/cancel", "params": {"invocation_id": iid}})

    def close(self):
        self.send({"jsonrpc": "2.0", "id": 99, "method": "harness/shutdown"})
        self.out.reply(99)
        self.w.close()
        self.thread.join(WAIT)


def ok(req, cancelled):
    return "complete", {"a": 1}, []


class Terminal(unittest.TestCase):
    def test_positive_control_normal_call_runs_the_handler_once(self):
        calls = []
        s = Session(lambda r, c: calls.append(1) or ok(r, c))
        s.invoke(2, "i1")
        res = s.out.reply(2)["result"]
        s.close()
        self.assertEqual((res["status"], len(calls)), ("complete", 1))

    def test_early_cancel_dispatches_no_handler_work(self):
        calls = []
        s = Session(lambda r, c: calls.append(c.is_set()) or ok(r, c))
        s.cancel("i1")
        s.invoke(2, "i1")
        res = s.out.reply(2)["result"]
        s.close()
        self.assertEqual(calls, [], "handler must not run after an early cancel")
        self.assertEqual((res["status"], res["diagnostics"][0]["code"]), ("refused", "SPX-HPK013"))
        self.assertIsNone(res["payload"])

    def test_late_success_cannot_replace_a_selected_deadline_failure(self):
        saw_cancel, release = threading.Event(), threading.Event()

        def handler(r, c):
            c.wait(WAIT)
            saw_cancel.set()
            release.wait(WAIT)
            return "complete", {"late": True}, []

        s = Session(handler)
        s.invoke(2, "i1", deadline_ms=50)
        self.assertTrue(saw_cancel.wait(WAIT))
        # The deadline decision is made, but the handler is still running: no receipt yet.
        self.assertEqual([f for f in s.out.frames() if f.get("id") == 2], [])
        release.set()
        res = s.out.reply(2)["result"]
        s.close()
        self.assertEqual((res["status"], res["diagnostics"][0]["code"], res["payload"]), ("failed", "SPX-HPK011", None))
        self.assertEqual(len([f for f in s.out.frames() if f.get("id") == 2]), 1)

    def test_cancel_receipt_waits_for_real_handler_cleanup_and_calls_do_not_overlap(self):
        running, peak, lock = [0], [0], threading.Lock()
        saw_cancel, release, entered = threading.Event(), threading.Event(), threading.Event()

        def handler(r, c):
            entered.set()
            with lock:
                running[0] += 1
                peak[0] = max(peak[0], running[0])
            try:
                saw_cancel.clear()
                c.wait(WAIT)
                saw_cancel.set()
                release.wait(WAIT)  # deliberately delayed cleanup
                return "complete", {}, []
            finally:
                with lock:
                    running[0] -= 1

        s = Session(handler)
        for n, iid in ((2, "i1"), (3, "i2")):
            release.clear()
            entered.clear()
            s.invoke(n, iid)
            self.assertTrue(entered.wait(WAIT))
            s.cancel(iid)
            self.assertTrue(saw_cancel.wait(WAIT))
            self.assertEqual([f for f in s.out.frames() if f.get("id") == n], [], "refused while handler live")
            release.set()
            res = s.out.reply(n)["result"]
            self.assertEqual((res["status"], res["diagnostics"][0]["code"]), ("refused", "SPX-HPK013"))
            with lock:
                self.assertEqual(running[0], 0, "reply only after the handler stopped")
        s.close()
        self.assertEqual(peak[0], 1)

    def test_non_cooperating_handler_reaches_process_level_enforcement(self):
        release, stuck, entered = threading.Event(), threading.Event(), threading.Event()
        s = Session(lambda r, c: (entered.set(), release.wait(WAIT), ok(r, c))[2], cancel_grace=0.1, stuck_exit=stuck.set)
        s.invoke(2, "i1")
        self.assertTrue(entered.wait(WAIT))
        s.cancel("i1")
        self.assertTrue(stuck.wait(WAIT), "stuck handler must trigger process-level enforcement")
        self.assertEqual([f for f in s.out.frames() if f.get("id") == 2], [], "no receipt for unsettled work")
        release.set()
        s.w.close()
        s.thread.join(WAIT)

    def test_completion_cancel_race_emits_exactly_one_reply(self):
        s = Session(ok)
        for n in range(2, 42):
            iid = f"r{n}"
            s.invoke(n, iid)
            s.cancel(iid)
        for n in range(2, 42):
            s.out.reply(n)
        s.close()
        for n in range(2, 42):
            self.assertEqual(len([f for f in s.out.frames() if f.get("id") == n]), 1, n)

    def test_sequential_calls_leave_bounded_threads(self):
        s = Session(ok)
        base = threading.active_count()
        for n in range(2, 62):
            s.invoke(n, f"s{n}")
            self.assertEqual(s.out.reply(n)["result"]["status"], "complete")
        time.sleep(0.2)
        self.assertLessEqual(threading.active_count(), base + 2)
        s.close()


if __name__ == "__main__":
    unittest.main()
