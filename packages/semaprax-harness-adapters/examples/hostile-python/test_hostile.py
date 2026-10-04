"""Each HOSTILE_MODE misbehaves observably on the wire. A conformance suite must reject these."""
import json
import os
import signal
import subprocess
import sys
import tempfile
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "tools"))
from drive import drive  # noqa: E402

ADAPTER = [sys.executable, os.path.join(HERE, "adapter.py")]


def run(mode, kind="context.repository", op="search", payload=None, env=(), timeout=30):
    return drive(ADAPTER, kind, op, payload or {"query": "x"}, env_extra=[f"HOSTILE_MODE={mode}", *env], timeout=timeout)


def invoke_result(mode, **kw):
    replies, err, code = run(mode, **kw)
    return next(r for r in replies if r.get("id") == 2)["result"], replies, err, code


class HostileTest(unittest.TestCase):
    def test_baseline_is_well_behaved(self):
        r, replies, _, code = invoke_result("")
        self.assertEqual((r["invocation_id"], code, len(replies)), ("inv-000001", 0, 3))
        self.assertEqual(r["project"]["revision"], "r" * 64)

    def test_spoof_invocation(self):
        self.assertNotEqual(invoke_result("spoof_invocation")[0]["invocation_id"], "inv-000001")

    def test_spoof_project(self):
        self.assertNotEqual(invoke_result("spoof_project")[0]["project"]["id"], "p" * 64)

    def test_fake_revision(self):
        self.assertNotEqual(invoke_result("fake_revision")[0]["project"]["revision"], "r" * 64)

    def test_wrong_protocol(self):
        replies, _, _ = run("wrong_protocol")[:3]
        self.assertNotEqual(replies[0]["result"]["protocol"], "semaprax.harness-rpc.v1")

    def test_flood(self):
        replies, _, _ = run("flood")[:3]
        self.assertGreater(len(replies), 10000)

    def test_oversized_frame(self):
        r = invoke_result("oversized_frame")[0]
        self.assertGreater(len(json.dumps(r)), 4 * 1024 * 1024)

    def test_malformed_frame(self):
        proc = subprocess.run(ADAPTER, input=self._frames(), capture_output=True,
                              env={**os.environ, "HOSTILE_MODE": "malformed_frame"})
        self.assertIn(b"{this is not json\n", proc.stdout)

    def test_unsolicited_request(self):
        _, replies, _, _ = invoke_result("unsolicited_request")
        self.assertTrue(any(r.get("method") == "host/readFile" for r in replies))

    def test_sampling_request(self):
        _, replies, _, _ = invoke_result("sampling_request")
        self.assertTrue(any(r.get("method") == "sampling/createMessage" for r in replies))

    def test_path_escape_and_absolute(self):
        self.assertTrue(invoke_result("path_escape")[0]["payload"]["items"][0]["path"].startswith("../.."))
        self.assertTrue(invoke_result("absolute_path")[0]["payload"]["items"][0]["path"].startswith("/"))

    def test_drop_critical_error(self):
        r = invoke_result("drop_critical_error", kind="command.view", op="view",
                          payload={"stdout": "ok\nerror: boom\nFAILED\npanic here"})[0]
        text = r["payload"]["view"]["text"].lower()
        self.assertNotIn("error", text)
        self.assertNotIn("panic", text)

    def test_forbidden_model(self):
        r = invoke_result("forbidden_model", kind="decision.evaluate", op="evaluate",
                          payload={"task": "model-route/v1", "features": {}, "options": ["a", "b"]})[0]
        self.assertNotIn(r["payload"]["choice"], ["a", "b"])

    def test_stderr_flood(self):
        _, _, err, _ = invoke_result("stderr_flood")
        self.assertGreater(len(err), 8 * 1024 * 1024)

    def test_secret_probe(self):
        with tempfile.NamedTemporaryFile("w", delete=False) as fh:
            fh.write("TOP-SECRET-VALUE")
        try:
            r = invoke_result("secret_probe", env=[f"SECRET_PATH={fh.name}"])[0]
            self.assertEqual(r["payload"]["items"][0]["text"], "TOP-SECRET-VALUE")
        finally:
            os.unlink(fh.name)

    def test_crash_on_invoke(self):
        replies, _, code = run("crash_on_invoke")
        self.assertEqual(code, 3)
        self.assertEqual([r["id"] for r in replies], [1])

    def test_hang_on_initialize(self):
        with self.assertRaises(subprocess.TimeoutExpired):
            run("hang_on_initialize", timeout=2)

    @staticmethod
    def _read(path):
        with open(path) as fh:
            return fh.read()

    @staticmethod
    def _frames():
        project = {"id": "p", "worktree": "w", "revision": "r"}
        cap = {"kind": "context.repository", "version": 1}
        msgs = [{"jsonrpc": "2.0", "id": 1, "method": "harness/initialize", "params": {"protocol": "semaprax.harness-rpc.v1", "offered": [cap]}},
                {"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": {"invocation_id": "i", "project": project, "capability": cap, "operation": "search", "payload": {}}},
                {"jsonrpc": "2.0", "id": 3, "method": "harness/shutdown"}]
        return b"".join(json.dumps(m).encode() + b"\n" for m in msgs)

    def test_ignore_cancel_survives_and_leaves_grandchild(self):
        pidfile = tempfile.mktemp(prefix="hp16a-pid-")
        proc = subprocess.Popen(ADAPTER, stdin=subprocess.PIPE, stdout=subprocess.PIPE, start_new_session=True,
                                env={**os.environ, "HOSTILE_MODE": "ignore_cancel", "HOSTILE_PIDFILE": pidfile})
        try:
            frames = self._frames().splitlines(keepends=True)
            proc.stdin.write(frames[0] + frames[1])
            proc.stdin.flush()
            for _ in range(100):
                if os.path.exists(pidfile) and self._read(pidfile).endswith("\n"):
                    break
                time.sleep(0.05)
            adapter_pid, child_pid = map(int, self._read(pidfile).split())
            proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "harness/cancel", "params": {"invocation_id": "i"}}).encode() + b"\n")
            proc.stdin.flush()
            time.sleep(0.5)
            self.assertIsNone(proc.poll(), "adapter ignored cancel and kept running")
            os.kill(child_pid, 0)  # grandchild alive
        finally:
            os.killpg(proc.pid, signal.SIGKILL)  # what a conformant host must do
            proc.wait()
            proc.stdin.close()
            proc.stdout.close()
            if os.path.exists(pidfile):
                os.unlink(pidfile)
        time.sleep(0.2)
        with self.assertRaises(ProcessLookupError):
            os.kill(child_pid, 0)


if __name__ == "__main__":
    unittest.main()
