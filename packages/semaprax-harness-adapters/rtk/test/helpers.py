"""Shared fixtures: adapter driver, real-binary lookup, scratch repo, side-effect scripts."""
import base64
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ADAPTER_DIR = os.path.dirname(HERE)
ADAPTER = [sys.executable, os.path.join(ADAPTER_DIR, "adapter.py")]
RTK = os.environ.get("RTK_BIN", "/private/tmp/claude-501/hp-tools/rtk-0.51.0/rtk")
PINNED_SHA256 = "02866e65968c0359b19495a5dc2d3ec643a1553fdd1c83c0ad33a8ad33bac3c9"
MEASUREMENTS = os.environ.get("RTK_MEASUREMENTS")  # optional JSONL sink


def need_rtk():
    if not os.path.isfile(RTK):
        raise unittest.SkipTest(f"pinned rtk not prepared at {RTK} (see RESEARCH.md step 1)")


def measure(label, **fields):
    row = dict(label=label, **fields)
    if MEASUREMENTS:
        with open(MEASUREMENTS, "a") as f:
            f.write(json.dumps(row, sort_keys=True) + "\n")


def drive(requests, retention, upstream=RTK, extra_env=None):
    """Run the adapter once: initialize, one invoke per request, shutdown. Returns result envelopes."""
    env = {"PATH": "/usr/bin:/bin", "SEMAPRAX_HARNESS_UPSTREAM": upstream, "SEMAPRAX_HARNESS_RETENTION_DIR": retention}
    env.update(extra_env or {})
    frames = [{"jsonrpc": "2.0", "id": 0, "method": "harness/initialize", "params": {
        "protocol": "semaprax.harness-rpc.v1", "host_version": "t", "descriptor_digest": "d",
        "offered": [{"kind": "command.view", "version": 1}], "project": {}}}]
    for i, (op, payload) in enumerate(requests, 1):
        frames.append({"jsonrpc": "2.0", "id": i, "method": "harness/invoke", "params": {
            "schema": "semaprax.harness-request.v1", "invocation_id": f"inv-{i:06d}",
            "project": {"id": "p", "worktree": "w", "revision": "r"}, "lock_digest": "l",
            "capability": {"kind": "command.view", "version": 1}, "operation": op,
            "deadline_ms": 30000, "budget": {"max_result_bytes": 65536, "remaining_calls": 8},
            "lineage": [], "payload": payload}})
    frames.append({"jsonrpc": "2.0", "id": 99, "method": "harness/shutdown", "params": {}})
    data = b"".join(json.dumps(f).encode() + b"\n" for f in frames)
    proc = subprocess.run(ADAPTER, input=data, capture_output=True, env=env, timeout=60)
    assert proc.returncode == 0, proc.stderr.decode()
    replies = [json.loads(line) for line in proc.stdout.splitlines()]
    return [r["result"] for r in replies[1:-1]]


def one(op, payload, retention, **kw):
    return drive([(op, payload)], retention, **kw)[0]


def b64(b):
    return base64.b64encode(b).decode()


def write_exec(path, text):
    with open(path, "w") as f:
        f.write(text)
    os.chmod(path, 0o755)


FAKE_CARGO = r"""#!/bin/sh
# fake cargo: libtest-formatted output, one counter line per run. FAKE_CARGO_MODE: ok|fail|sleep|badutf8|bigerr
echo x >> "$FAKE_CARGO_COUNTER"
echo "   Compiling demo v0.1.0 (/x)" >&2
echo "     Running unittests src/lib.rs (target/debug/deps/demo-abc)"
echo
echo "running 300 tests"
i=0; while [ $i -lt 300 ]; do echo "test tests::case_$i ... ok"; i=$((i+1)); done
case "$FAKE_CARGO_MODE" in
sleep) echo $$ > "$FAKE_CARGO_COUNTER.pid"; sleep 30; exit 0;;
badutf8) printf 'note: \377\376 invalid bytes\n' ;;
bigerr) i=0; while [ $i -lt 4000 ]; do echo "warning: unused variable number $i in some module path here" >&2; i=$((i+1)); done ;;
esac
case "$FAKE_CARGO_MODE" in
fail|badutf8|bigerr)
  echo "test tests::planted_failure ... FAILED"; echo; echo "failures:"; echo
  echo "---- tests::planted_failure stdout ----"
  echo "thread 'tests::planted_failure' panicked at src/lib.rs:9:5:"
  echo "assertion failed: CRITICAL-PLANTED-7731"; echo
  echo "failures:"; echo "    tests::planted_failure"; echo
  echo "test result: FAILED. 300 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s"
  echo "error: test failed, to rerun pass \`--lib\`" >&2
  exit 101;;
esac
echo; echo "test result: ok. 300 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s"
exit 0
"""


class Scratch(unittest.TestCase):
    """Temp dir with retention/, bin/ (fake cargo) and a counter file."""

    def setUp(self):
        need_rtk()
        self.tmp = tempfile.mkdtemp(prefix="hp09a-")
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.ret = os.path.join(self.tmp, "retention")
        os.makedirs(self.ret)
        self.bin = os.path.join(self.tmp, "bin")
        os.makedirs(self.bin)
        write_exec(os.path.join(self.bin, "cargo"), FAKE_CARGO)
        self.counter = os.path.join(self.tmp, "counter")
        open(self.counter, "w").close()

    def runs(self):
        with open(self.counter) as f:
            return len(f.read().splitlines())

    def env(self, **extra):
        e = {"PATH": f"{self.bin}:/usr/bin:/bin:/opt/homebrew/bin", "HOME": self.tmp, "LC_ALL": "C",
             "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_SYSTEM": "/dev/null",
             "FAKE_CARGO_COUNTER": self.counter}
        e.update(extra)
        return e

    def make_repo(self, files=40, commits=30):
        repo = os.path.join(self.tmp, "repo")
        os.makedirs(os.path.join(repo, "src"))
        g = lambda *a: subprocess.run(["git", *a], cwd=repo, env=self.env(), check=True, capture_output=True)
        g("init", "-q", ".")
        g("config", "user.email", "a@b")
        g("config", "user.name", "n")
        for i in range(files):
            with open(os.path.join(repo, "src", f"f{i:03d}.txt"), "w") as f:
                f.write("".join(f"line {i}-{j} alpha beta gamma TODO\n" for j in range(40)))
        g("add", ".")
        g("commit", "-qm", "base")
        for k in range(commits):
            g("commit", "-qm", f"commit number {k}", "--allow-empty")
        for i in range(files):
            with open(os.path.join(repo, "src", f"f{i:03d}.txt"), "a") as f:
                f.write("".join(f"changed {i}-{j} delta\n" for j in range(30)))
        for i in range(20):
            with open(os.path.join(repo, f"u{i}.tmp"), "w") as f:
                f.write("x")
        return repo

    def run_cmd(self, argv, cwd=None, env=None, timeout=60):
        return subprocess.run(argv, cwd=cwd, env=env or self.env(), capture_output=True, stdin=subprocess.DEVNULL, timeout=timeout)
