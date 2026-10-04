"""Real-graphify tests for the Graphify adapter. Run: python3 -m unittest discover -s test"""

import glob
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ADAPTER = os.path.join(HERE, "..", "adapter.py")
UPSTREAM = os.environ.get("SEMAPRAX_HARNESS_UPSTREAM", os.path.expanduser("~/.local/bin/graphify"))
SANDBOX_EXEC = "/usr/bin/sandbox-exec"
DENY_NET = "(version 1)(allow default)(deny network*)"

FILES = {
    "py/util.py": "def helper(x):\n    return x + 1\n\nclass Greeter:\n    def greet(self, name):\n        return helper(len(name))\n",
    "py/main.py": "from util import Greeter, helper\n\ndef run():\n    g = Greeter()\n    print(g.greet('a'), helper(2))\n",
    "web/app.ts": "export function render(n: number): number { return n * 2 }\nexport class Widget { draw() { return render(3) } }\n",
    "spx/m.spx": "fn main() -> i32 { 0 }\n",
    "spx/x.spatch": "patch\n",
    "Cargo.toml": "[package]\nname = \"x\"\n",
}


def make_project(root):
    for rel, text in FILES.items():
        path = os.path.join(root, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w") as fh:
            fh.write(text)


def tree(root):
    return sorted(os.path.relpath(os.path.join(b, f), root) for b, _, fs in os.walk(root) for f in fs)


class Client:
    def __init__(self, root, cache, upstream=UPSTREAM, extra_env=None, wrap=()):
        env = dict(os.environ, SEMAPRAX_HARNESS_UPSTREAM=upstream, SEMAPRAX_HARNESS_PROJECT_ROOT=root,
                   SEMAPRAX_HARNESS_CACHE_DIR=cache)
        env.update(extra_env or {})
        self.root, self.n = root, 0
        self.proc = subprocess.Popen(list(wrap) + [sys.executable, ADAPTER], stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)

    def rpc(self, method, params):
        self.n += 1
        frame = {"jsonrpc": "2.0", "id": self.n, "method": method, "params": params}
        self.proc.stdin.write(json.dumps(frame).encode() + b"\n")
        self.proc.stdin.flush()
        return json.loads(self.proc.stdout.readline())

    def init(self):
        return self.rpc("harness/initialize", {"protocol": "semaprax.harness-rpc.v1", "host_version": "t", "descriptor_digest": "d",
                                               "offered": [{"kind": "context.repository", "version": 1}], "project": {}})

    def call(self, op, payload=None, budget=65536):
        req = {"schema": "semaprax.harness-request.v1", "invocation_id": f"inv-{self.n}",
               "project": {"id": "p", "worktree": "w", "revision": "r"}, "lock_digest": "l",
               "capability": {"kind": "context.repository", "version": 1}, "operation": op, "deadline_ms": 120000,
               "budget": {"max_result_bytes": budget, "remaining_calls": 8}, "lineage": [], "payload": payload or {}}
        resp = self.rpc("harness/invoke", req)
        self.last_request = req
        return resp["result"]

    def close(self):
        if self.proc.stdin.closed:
            return
        try:
            self.rpc("harness/shutdown", {})
        except Exception:
            pass
        self.proc.stdin.close()
        self.proc.wait(timeout=20)
        self.stderr = self.proc.stderr.read().decode()
        self.proc.stdout.close()
        self.proc.stderr.close()


class AdapterCase(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="hp07a-")
        self.root, self.cache = os.path.join(self.tmp, "proj"), os.path.join(self.tmp, "cache")
        os.makedirs(self.root)
        os.makedirs(self.cache)
        make_project(self.root)
        self.before = tree(self.root)
        self.client = Client(self.root, self.cache)
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.addCleanup(self.client.close)

    def labels(self, res):
        return [i["text"] for i in res["payload"]["items"]]


class RealGraphify(AdapterCase):
    def test_initialize_accepts_context_repository(self):
        resp = self.client.init()["result"]
        self.assertEqual(resp["protocol"], "semaprax.harness-rpc.v1")
        self.assertEqual(resp["accepted"][0]["operations"], ["orient", "search", "skeleton", "references"])

    def test_orient_search_spans_and_no_repo_pollution(self):
        self.client.init()
        res = self.client.call("search", {"query": "helper"})
        self.assertEqual(res["status"], "partial")  # .spx/.spatch/Cargo.toml are not covered
        first = res["payload"]["items"][0]
        self.assertEqual((first["path"], first["span"]["start_line"], first["provenance"], first["language"]),
                         ("py/util.py", 1, "structural", "python"))
        self.assertRegex(first["digest"], r"^sha256:[0-9a-f]{64}$")
        self.assertEqual(first["rank"], 1)
        orient = self.client.call("orient")
        self.assertTrue(orient["payload"]["items"])
        self.assertEqual(res["provenance"]["upstream_version"], "0.9.25")
        self.assertEqual(tree(self.root), self.before)  # nothing written into the project
        self.assertTrue(os.path.isfile(os.path.join(self.cache, "graphify-index", "graphify-out", "graph.json")))

    def test_skeleton_ordered_by_line(self):
        self.client.init()
        res = self.client.call("skeleton", {"path": "py/util.py"})
        lines = [i["span"]["start_line"] for i in res["payload"]["items"]]
        self.assertEqual(lines, sorted(lines))
        self.assertTrue(any("Greeter" in t for t in self.labels(res)))
        self.assertTrue(res["payload"]["coverage"]["exhaustive"])

    def test_references_structural_and_not_exhaustive_with_skips(self):
        self.client.init()
        res = self.client.call("references", {"symbol": "helper"})
        items = res["payload"]["items"]
        self.assertTrue(any(i["path"] == "py/main.py" for i in items))
        self.assertTrue(all(i["provenance"] in ("structural", "inferred") for i in items))
        cov = res["payload"]["coverage"]
        self.assertFalse(cov["complete"])
        self.assertFalse(cov["exhaustive"])
        none = self.client.call("references", {"symbol": "nothing_called_this"})
        self.assertEqual(none["payload"]["items"], [])
        self.assertFalse(none["payload"]["coverage"]["exhaustive"])
        self.assertEqual(none["status"], "partial")
        self.assertIn("not evidence", none["diagnostics"][0]["message"])

    def test_spx_skipped_with_reasons(self):
        self.client.init()
        cov = self.client.call("orient")["payload"]["coverage"]
        skipped = {s["path"]: s["reason"] for s in cov["skipped"]}
        self.assertIn("spx/m.spx", skipped)
        self.assertIn("spx/x.spatch", skipped)
        self.assertIn("Cargo.toml", skipped)
        self.assertIn("compiler-owned", skipped["spx/m.spx"])
        self.assertEqual(cov["indexed_files"], 3)
        unsupported = self.client.call("skeleton", {"path": "spx/m.spx"})
        self.assertEqual(unsupported["status"], "unsupported")

    def test_stale_after_edit_then_rebuild(self):
        self.client.init()
        self.client.call("search", {"query": "helper"})
        with open(os.path.join(self.root, "py/util.py"), "a") as fh:
            fh.write("\ndef brand_new():\n    return 1\n")
        stale = self.client.call("search", {"query": "brand_new", "refresh": "never"})
        self.assertEqual(stale["status"], "stale")
        self.assertEqual(stale["payload"]["items"], [])
        self.assertEqual(stale["diagnostics"][0]["code"], "SPX-HPG002")
        fresh = self.client.call("search", {"query": "brand_new"})  # default refresh=auto rebuilds
        self.assertEqual(fresh["payload"]["items"][0]["path"], "py/util.py")
        self.assertEqual(fresh["payload"]["metadata"]["refresh"], "refresh")

    def test_restart_reuses_verified_index_and_detects_edit(self):
        self.client.init()
        self.client.call("orient")
        self.client.close()
        graph = os.path.join(self.cache, "graphify-index", "graphify-out", "graph.json")
        stamp = os.stat(graph).st_mtime_ns
        again = Client(self.root, self.cache)
        self.addCleanup(again.close)
        again.init()
        again.call("orient")
        self.assertEqual(os.stat(graph).st_mtime_ns, stamp)  # reused, not rebuilt
        again.close()
        with open(os.path.join(self.root, "web/app.ts"), "a") as fh:
            fh.write("export function added() { return 1 }\n")
        third = Client(self.root, self.cache)
        self.addCleanup(third.close)
        third.init()
        self.assertTrue(third.call("search", {"query": "added"})["payload"]["items"])
        self.assertNotEqual(os.stat(graph).st_mtime_ns, stamp)  # a new session rebuilds a stale index

    def test_unsupported_version_refused(self):
        shim = make_shim(self.tmp, "9.9.9", None)
        c = Client(self.root, self.cache, upstream=shim)
        self.addCleanup(c.close)
        c.init()
        res = c.call("orient")
        self.assertEqual(res["status"], "unsupported")
        self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPG001")

    def test_name_alone_is_not_identity(self):
        fake = os.path.join(self.tmp, "fake", "bin")
        os.makedirs(fake)
        path = os.path.join(fake, "graphify")
        with open(path, "w") as fh:
            fh.write("#!/bin/sh\necho graphify 0.9.25\n")
        os.chmod(path, 0o755)
        c = Client(self.root, self.cache, upstream=path)
        self.addCleanup(c.close)
        c.init()
        self.assertEqual(c.call("orient")["status"], "unsupported")

    def test_planted_env_not_inherited(self):
        envdump = os.path.join(self.tmp, "env.txt")
        shim = make_shim(self.tmp, "0.9.25", envdump)
        planted = {"OPENAI_API_KEY": "sk-planted", "ANTHROPIC_API_KEY": "sk-planted", "OPENAI_BASE_URL": "http://127.0.0.1:9/v1",
                   "GEMINI_API_KEY": "planted", "GRAPHIFY_API_KEY": "planted", "GITHUB_TOKEN": "planted",
                   "GRAPHIFY_TRIAGE_BACKEND": "openai", "OLLAMA_BASE_URL": "http://example.invalid", "GRAPHIFY_OUT": "/tmp/elsewhere"}
        c = Client(self.root, self.cache, upstream=shim, extra_env=planted)
        self.addCleanup(c.close)
        c.init()
        self.assertEqual(c.call("orient")["status"], "partial")
        with open(envdump) as fh:
            dumped = fh.read()
        for key in planted:
            self.assertNotIn(key + "=", dumped)
        self.assertNotIn("sk-planted", dumped)
        self.assertIn("HOME=" + os.path.join(self.cache, "home"), dumped)

    @unittest.skipUnless(os.path.exists(SANDBOX_EXEC), "needs macOS sandbox-exec")
    def test_network_blocked_run(self):
        probe = subprocess.run([SANDBOX_EXEC, "-p", DENY_NET, sys.executable, "-c",
                                "import socket,sys\ntry:\n socket.create_connection(('1.1.1.1',53),2)\nexcept OSError: sys.exit(0)\nsys.exit(1)"])
        self.assertEqual(probe.returncode, 0, "sandbox does not block network")
        c = Client(self.root, self.cache, wrap=[SANDBOX_EXEC, "-p", DENY_NET])
        self.addCleanup(c.close)
        c.init()
        res = c.call("search", {"query": "render"})
        self.assertTrue(res["payload"]["items"])
        self.assertEqual(res["payload"]["items"][0]["path"], "web/app.ts")


class Schema(unittest.TestCase):
    def test_edge_confidence_mapping(self):
        sys.path.insert(0, os.path.join(HERE, ".."))
        import adapter  # noqa: E402
        base = {"directed": False, "nodes": [
            {"id": "a", "label": "a()", "file_type": "code", "source_file": "x.py", "source_location": "L1"},
            {"id": "b", "label": "b()", "file_type": "code", "source_file": "x.py", "source_location": "L2"}], "links": []}
        edge = {"source": "a", "target": "b", "relation": "calls", "confidence": "INFERRED", "source_file": "x.py", "source_location": "L1"}
        base["links"] = [edge]
        self.assertEqual(len(adapter.Graph(base).edges), 1)
        base["nodes"].append({"id": "ext", "label": "Vec", "file_type": "code", "source_file": "", "source_location": ""})
        self.assertEqual(len(adapter.Graph(dict(base, links=[dict(edge, target="ext")])).edges), 1)  # stub kept as unresolved target
        base["links"] = [dict(edge, confidence="CERTAIN")]
        with self.assertRaises(adapter.AdapterError):
            adapter.Graph(base)
        with self.assertRaises(adapter.AdapterError):
            adapter.Graph({"nodes": [], "edges": []})  # a different schema is refused


def make_shim(tmp, version, envdump):
    """A fake venv: real-looking distribution metadata, and a script that records its env then runs real graphify."""
    venv = os.path.join(tmp, "shimvenv" + version)
    dist = os.path.join(venv, "lib", "python3.12", "site-packages", f"graphifyy-{version}.dist-info")
    os.makedirs(dist)
    os.makedirs(os.path.join(venv, "bin"))
    with open(os.path.join(dist, "METADATA"), "w") as fh:
        fh.write(f"Metadata-Version: 2.4\nName: graphifyy\nVersion: {version}\n"
                 "Project-URL: Repository, https://github.com/Graphify-Labs/graphify\n\n")
    path = os.path.join(venv, "bin", "graphify")
    dump = f"env > '{envdump}'\n" if envdump else ""
    with open(path, "w") as fh:
        fh.write(f"#!/bin/sh\n{dump}exec '{os.path.realpath(UPSTREAM)}' \"$@\"\n")
    os.chmod(path, 0o755)
    return path


if __name__ == "__main__":
    unittest.main()
