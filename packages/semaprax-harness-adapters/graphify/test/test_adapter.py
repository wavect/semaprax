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
UPSTREAM_NEW = os.environ.get("SEMAPRAX_HARNESS_UPSTREAM_NEW", "")  # a graphifyy 0.9.75 venv executable
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
    def __init__(self, root, cache, upstream=None, extra_env=None, wrap=()):
        env = dict(os.environ, SEMAPRAX_HARNESS_UPSTREAM=upstream or UPSTREAM, SEMAPRAX_HARNESS_PROJECT_ROOT=root,
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
    UP, VERSION = UPSTREAM, "0.9.25"

    def index_dir(self):
        return os.path.join(self.cache, "graphify-index", self.VERSION)

    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="hp07a-")
        self.root, self.cache = os.path.join(self.tmp, "proj"), os.path.join(self.tmp, "cache")
        os.makedirs(self.root)
        os.makedirs(self.cache)
        make_project(self.root)
        self.before = tree(self.root)
        self.client = Client(self.root, self.cache, upstream=self.UP)
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.addCleanup(self.client.close)

    def labels(self, res):
        return [i["text"] for i in res["payload"]["items"]]


class RealGraphify:
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
        self.assertEqual(res["provenance"]["upstream_version"], self.VERSION)
        self.assertEqual(tree(self.root), self.before)  # nothing written into the project
        self.assertTrue(os.path.isfile(os.path.join(self.index_dir(), "graphify-out", "graph.json")))

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
        # 0.9.75 parses Cargo.toml as a code file (one node); 0.9.25 skips it. Coverage follows what upstream indexed.
        self.assertEqual("Cargo.toml" in skipped, self.VERSION == "0.9.25")
        self.assertIn("compiler-owned", skipped["spx/m.spx"])
        self.assertEqual(cov["indexed_files"], 3 if self.VERSION == "0.9.25" else 4)
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
        graph = os.path.join(self.index_dir(), "graphify-out", "graph.json")
        stamp = os.stat(graph).st_mtime_ns
        again = Client(self.root, self.cache, upstream=self.UP)
        self.addCleanup(again.close)
        again.init()
        again.call("orient")
        self.assertEqual(os.stat(graph).st_mtime_ns, stamp)  # reused, not rebuilt
        again.close()
        with open(os.path.join(self.root, "web/app.ts"), "a") as fh:
            fh.write("export function added() { return 1 }\n")
        third = Client(self.root, self.cache, upstream=self.UP)
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
        shim = make_shim(self.tmp, self.VERSION, envdump, self.UP)
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
        c = Client(self.root, self.cache, upstream=self.UP, wrap=[SANDBOX_EXEC, "-p", DENY_NET])
        self.addCleanup(c.close)
        c.init()
        res = c.call("search", {"query": "render"})
        self.assertTrue(res["payload"]["items"])
        self.assertEqual(res["payload"]["items"][0]["path"], "web/app.ts")


class RealGraphify025(RealGraphify, AdapterCase):
    pass


@unittest.skipUnless(UPSTREAM_NEW, "needs SEMAPRAX_HARNESS_UPSTREAM_NEW (graphifyy 0.9.75 venv)")
class RealGraphify075(RealGraphify, AdapterCase):
    UP, VERSION = UPSTREAM_NEW, "0.9.75"


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


COLLISION = {
    "a.py": "class Base:\n    def run(self):\n        return 1\n    def go(self):\n        return self.run()\n\n"
            "class Child(Base):\n    def run(self):\n        return super().run() + 2\n    def other(self):\n        return self.run()\n",
    "b.py": "class Other:\n    def run(self):\n        return 3\n\ndef use(o):\n    return o.run()\n\n"
            "def dyn(name, o):\n    return getattr(o, name)()\n\nclass L:\n    def ping(self): return 1\nclass R:\n    def ping(self): return 2\n"
            "class Both(L, R):\n    def call(self):\n        return self.ping()\n",
    "c.ts": "export class A { run() { return 1 } go() { return this.run() } }\nexport class B extends A { run() { return super.run() + 1 } }\n"
            "export function use(x: A) { return x.run() }\n",
}


class CollisionBase:
    """Same-name collisions: no falsely exact callers, on either schema profile."""

    def setUp(self):
        super().setUp()
        for rel, text in COLLISION.items():
            with open(os.path.join(self.root, rel), "w") as fh:
                fh.write(text)
        self.client.init()

    def refs(self, symbol):
        res = self.client.call("references", {"symbol": symbol})
        out = []
        for i in res["payload"]["items"]:
            rel = i["edges"][0]["relation"]
            out.append((i["path"], i["span"]["start_line"], rel.split(":")[1], i["provenance"]))
        return res, out

    def test_run_collisions_never_structural_unless_resolved(self):
        res, out = self.refs("run")
        self.assertTrue(out)
        for path, line, status, prov in out:
            self.assertEqual(prov == "structural", status == "resolved", (path, line, status, prov))
        # `o.run()` (untyped receiver) and the TS `x.run()` collide with several same-named methods: never resolved
        by_site = {(p, l): st for p, l, st, _ in out}
        self.assertNotEqual(by_site.get(("b.py", 6), "ambiguous"), "resolved")
        self.assertNotEqual(by_site.get(("c.ts", 3), "ambiguous"), "resolved")
        self.assertFalse(res["payload"]["coverage"]["exhaustive"])
        self.assertTrue(any(d["code"] == "SPX-HPG012" for d in res["diagnostics"]))
        self.assertEqual(sum(res["payload"]["metadata"]["resolution"].values()), len(out))

    def test_multiple_inheritance_ping_is_never_resolved(self):
        _, out = self.refs("ping")
        self.assertTrue(all(st != "resolved" for _, _, st, _ in out))  # L.ping vs R.ping: undecidable statically

    def test_dynamic_dispatch_leaves_no_edge_and_no_exhaustive_claim(self):
        res, out = self.refs("dyn")
        self.assertEqual(out, [])
        self.assertFalse(res["payload"]["coverage"]["exhaustive"])
        self.assertNotIn("no_references", res["payload"])

    def test_every_reference_site_is_one_line_call_site(self):
        res, _ = self.refs("run")
        for i in res["payload"]["items"]:
            self.assertEqual(i["span"]["start_line"], i["span"]["end_line"])
            self.assertTrue(i["text"].startswith("[call-site] "))


class Collision025(CollisionBase, AdapterCase):
    def test_no_intra_class_trust(self):
        _, out = self.refs("run")
        # 0.9.25 profile has no documented intra-class binding: every colliding call is ambiguous
        self.assertTrue(all(st == "ambiguous" for _, _, st, _ in out))


@unittest.skipUnless(UPSTREAM_NEW, "needs SEMAPRAX_HARNESS_UPSTREAM_NEW")
class Collision075(CollisionBase, AdapterCase):
    UP, VERSION = UPSTREAM_NEW, "0.9.75"

    def test_intra_class_super_and_override(self):
        _, out = self.refs("run")
        st = {(p, l): s for p, l, s, _ in out}
        self.assertEqual(st.get(("a.py", 11)), "resolved")    # Child.other -> self.run(), no subclass override
        self.assertEqual(st.get(("a.py", 9)), "resolved")     # super().run() -> the only defining ancestor
        self.assertEqual(st.get(("a.py", 5)), "ambiguous")    # Base.go -> self.run(): Child overrides it


class SpansAndCache(AdapterCase):
    def test_definition_span_is_independently_rehashed(self):
        with open(os.path.join(self.root, "py/util.py"), "a") as fh:
            fh.write("\ndef multi(a):\n    b = a\n    return b\n")
        self.client.init()
        res = self.client.call("search", {"query": "multi"})
        first = res["payload"]["items"][0]
        self.assertEqual((first["path"], first["span"]), ("py/util.py", {"start_line": 8, "end_line": 10}))
        self.assertTrue(first["text"].startswith("[definition] "))
        import hashlib
        with open(os.path.join(self.root, "py/util.py"), "rb") as fh:
            body = b"\n".join(fh.read().split(b"\n")[7:10])
        self.assertEqual(first["digest"], "sha256:" + hashlib.sha256(body).hexdigest())
        self.assertEqual(res["payload"]["metadata"]["span_kinds"].get("definition"), 1)
        ts = self.client.call("search", {"query": "render"})["payload"]["items"][0]
        self.assertEqual(ts["span"]["start_line"], ts["span"]["end_line"])  # no TS resolver: labelled start-line
        self.assertTrue(ts["text"].startswith("[start-line] "))

    def test_old_cache_is_never_read_as_new_schema(self):
        # a 0.9.25 index sitting where another version looks, or with a foreign profile, is refused and rebuilt
        self.client.init()
        self.client.call("orient")
        graph = os.path.join(self.index_dir(), "graphify-out", "graph.json")
        meta = os.path.join(self.index_dir(), "adapter-meta.json")
        with open(meta) as fh:
            m = json.load(fh)
        self.assertEqual((m["schema"], m["upstream"], m["profile"]), ("semaprax.graphify-adapter-meta.v2", "0.9.25", "graphify-0.9.25-node-link"))
        self.client.close()
        for mutate in (lambda d: d.update(upstream="0.9.75"), lambda d: d.update(profile="graphify-0.9.75-node-link"),
                       lambda d: d.update(schema="semaprax.graphify-adapter-meta.v1")):
            d = dict(m)
            mutate(d)
            with open(meta, "w") as fh:
                json.dump(d, fh)
            before = os.stat(graph).st_mtime_ns
            c = Client(self.root, self.cache, upstream=self.UP)
            c.init()
            c.call("orient")
            c.close()
            self.assertNotEqual(os.stat(graph).st_mtime_ns, before)  # rebuilt, not reused
        with open(meta) as fh:
            m = json.load(fh)
        with open(graph, "a") as fh:
            fh.write(" ")  # tampered graph bytes: checksum mismatch
        before = os.stat(graph).st_mtime_ns
        c = Client(self.root, self.cache, upstream=self.UP)
        c.init()
        c.call("orient")
        c.close()
        self.assertNotEqual(os.stat(graph).st_mtime_ns, before)

    def test_legacy_unversioned_cache_dir_is_ignored(self):
        legacy = os.path.join(self.cache, "graphify-index", "graphify-out")
        os.makedirs(legacy)
        with open(os.path.join(legacy, "graph.json"), "w") as fh:
            fh.write('{"nodes": [], "links": []}')
        self.client.init()
        res = self.client.call("search", {"query": "helper"})
        self.assertTrue(res["payload"]["items"])
        self.assertEqual(res["payload"]["metadata"]["refresh"], "build")

    def test_two_processes_one_cache_dir(self):
        a = Client(self.root, self.cache, upstream=self.UP)
        b = Client(self.root, self.cache, upstream=self.UP)
        self.addCleanup(a.close)
        self.addCleanup(b.close)
        a.init()
        b.init()
        # interleave first-use of both processes against one cold cache
        ra = a.call("search", {"query": "helper"})
        rb = b.call("search", {"query": "helper"})
        self.assertEqual(ra["payload"]["items"], rb["payload"]["items"])
        self.assertEqual(rb["payload"]["metadata"]["source_digest"], ra["payload"]["metadata"]["source_digest"])
        leftovers = [n for n in os.listdir(os.path.join(self.cache, "graphify-index")) if ".stage-" in n]
        self.assertEqual(leftovers, [])
        c = Client(self.root, self.cache, upstream=self.UP)
        self.addCleanup(c.close)
        c.init()
        self.assertEqual(c.call("search", {"query": "helper"})["payload"]["metadata"]["refresh"], "reuse")

    def test_concurrent_cold_builds_race(self):
        import threading
        results = []
        def go():
            c = Client(self.root, self.cache, upstream=self.UP)
            c.init()
            results.append(c.call("search", {"query": "helper"}))
            c.close()
        threads = [threading.Thread(target=go) for _ in range(3)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        self.assertEqual(len(results), 3)
        self.assertEqual(len({json.dumps(r["payload"]["items"], sort_keys=True) for r in results}), 1)
        self.assertEqual([n for n in os.listdir(os.path.join(self.cache, "graphify-index")) if ".stage-" in n], [])


@unittest.skipUnless(UPSTREAM_NEW, "needs SEMAPRAX_HARNESS_UPSTREAM_NEW")
class SpansAndCache075(SpansAndCache):
    UP, VERSION = UPSTREAM_NEW, "0.9.75"

    def test_old_cache_is_never_read_as_new_schema(self):
        # a real 0.9.25 meta and graph copied under the 0.9.75 directory name must not be reused
        old = Client(self.root, self.cache, upstream=UPSTREAM)
        old.init()
        old.call("orient")
        old.close()
        shutil.copytree(os.path.join(self.cache, "graphify-index", "0.9.25"), self.index_dir())
        c = Client(self.root, self.cache, upstream=self.UP)
        c.init()
        res = c.call("orient")
        c.close()
        self.assertEqual(res["payload"]["metadata"]["refresh"], "build")
        with open(os.path.join(self.index_dir(), "adapter-meta.json")) as fh:
            self.assertEqual(json.load(fh)["upstream"], "0.9.75")


def graph_with(version, nodes, links, **extra):
    return dict({"directed": False, "nodes": nodes, "links": links}, **extra)


class HostileSchema(unittest.TestCase):
    def setUp(self):
        sys.path.insert(0, os.path.join(HERE, ".."))
        import adapter
        self.adapter = adapter
        self.n = {"id": "a", "label": "a()", "file_type": "code", "source_file": "x.py", "source_location": "L1"}
        self.e = {"source": "a", "target": "a", "relation": "calls", "confidence": "EXTRACTED", "source_file": "x.py", "source_location": "L1"}

    def refuse(self, version, raw):
        with self.assertRaises(self.adapter.AdapterError) as cm:
            self.adapter.Graph(raw, version)
        self.assertEqual(cm.exception.code, "SPX-HPG003")

    def test_each_profile_refuses_hostile_layouts(self):
        for v in ("0.9.25", "0.9.75"):
            self.refuse(v, {"nodes": [], "edges": []})                                   # raw --no-cluster layout
            self.refuse(v, {"nodes": [dict(self.n, source_file="/etc/passwd")], "links": []})
            self.refuse(v, {"nodes": [dict(self.n, source_file="../x.py")], "links": []})
            self.refuse(v, {"nodes": [dict(self.n, source_location="line 3")], "links": []})
            self.refuse(v, {"nodes": [{k: x for k, x in self.n.items() if k != "label"}], "links": []})
            self.refuse(v, {"nodes": [self.n], "links": [dict(self.e, confidence="CERTAIN")]})
            self.refuse(v, {"nodes": [self.n], "links": [{k: x for k, x in self.e.items() if k != "relation"}]})
            self.refuse(v, [])
        self.refuse("0.9.75", {"nodes": [dict(self.n, _callable="yes")], "links": []})   # optional key present but mistyped
        self.refuse("9.9.9", {"nodes": [], "links": []})                                  # no profile, no guess

    def test_optional_fields_never_fabricate_relations(self):
        g = self.adapter.Graph({"nodes": [self.n, dict(self.n, id="b", label="b()", _callable=True)], "links": []}, "0.9.75")
        self.assertEqual(g.edges, [])
        self.assertEqual(self.adapter.Graph({"nodes": [self.n], "links": [self.e]}, "0.9.25").edges[0]["relation"], "calls")


def make_shim(tmp, version, envdump, real=UPSTREAM):
    """A fake venv: real-looking distribution metadata, and a script that records its env then runs real graphify."""
    venv = os.path.join(tmp, "shimvenv" + version + str(abs(hash(real))))
    dist = os.path.join(venv, "lib", "python3.12", "site-packages", f"graphifyy-{version}.dist-info")
    os.makedirs(dist)
    os.makedirs(os.path.join(venv, "bin"))
    with open(os.path.join(dist, "METADATA"), "w") as fh:
        fh.write(f"Metadata-Version: 2.4\nName: graphifyy\nVersion: {version}\n"
                 "Project-URL: Repository, https://github.com/Graphify-Labs/graphify\n\n")
    path = os.path.join(venv, "bin", "graphify")
    dump = f"env > '{envdump}'\n" if envdump else ""
    with open(path, "w") as fh:
        fh.write(f"#!/bin/sh\n{dump}exec '{os.path.realpath(real)}' \"$@\"\n")
    os.chmod(path, 0o755)
    return path



# ---- MC-02 / MC-05 / MC-06: adoption snapshots, forced rebuild, complete-result byte bound -----------------

sys.path.insert(0, os.path.join(HERE, ".."))
sys.path.insert(0, os.path.join(HERE, "..", "..", "sdk", "python"))
import io  # noqa: E402
from unittest import mock  # noqa: E402

import adapter as gadapter  # noqa: E402
import semaprax_harness_adapter as sdk  # noqa: E402


def raw_graph(files, tag="a"):
    nodes = [{"id": f"{tag}{i}", "label": f"{tag}_fn{i}()", "file_type": "code", "source_file": rel, "source_location": "L1"}
             for i, rel in enumerate(files)]
    return {"directed": False, "nodes": nodes, "links": []}


def make_user_index(root, version="0.9.25", files=("py/util.py", "py/main.py"), tag="a", rel="graphify-out"):
    """A hand-built, adoptable user index bound to the project's files (the real 0.9.25 CLI binds by absolute key)."""
    gdir = os.path.join(root, rel)
    os.makedirs(os.path.join(gdir, "cache", "ast", f"v{version}-t"), exist_ok=True)
    write_graph(gdir, files, tag)
    with open(os.path.join(gdir, ".graphify_root"), "w") as fh:
        fh.write(os.path.realpath(root))
    stat = {}
    for rel_file in files:
        with open(os.path.join(root, rel_file), "rb") as fh:
            stat[rel_file] = {"hashes": {rel_file: gadapter.graphify_file_hash(fh.read(), rel_file)}}
    with open(os.path.join(gdir, "cache", "stat-index.json"), "w") as fh:
        json.dump(stat, fh)
    return gdir


def write_graph(gdir, files, tag):
    path = os.path.join(gdir, "graph.json")
    if os.path.exists(path):
        os.chmod(path, 0o644)
    with open(path, "w") as fh:
        json.dump(raw_graph(files, tag), fh)


def index_snapshot(gdir):
    out = {}
    for base, _, names in os.walk(gdir):
        for n in names:
            p = os.path.join(base, n)
            with open(p, "rb") as fh:
                out[os.path.relpath(p, gdir)] = (fh.read(), os.stat(p).st_mode)
    return out


class SnapshotAdoption(unittest.TestCase):
    """MC-02: copied-snapshot adoption binds the validated bytes, the identity and the loaded graph together."""

    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="mc02-")
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.root, self.cache = os.path.realpath(os.path.join(self.tmp, "proj")), os.path.join(self.tmp, "cache")
        os.makedirs(self.root)
        os.makedirs(self.cache)
        make_project(self.root)
        self.gdir = make_user_index(self.root)
        self.ix = gadapter.Index(self.root, self.cache)
        self.ix.identity = "0.9.25"
        self.ac = {"mode": "copied-snapshot", "rel": "graphify-out"}
        self.files = gadapter.walk_files(self.root)
        self.digest = gadapter.source_digest(self.root, self.files)

    def adopt(self):
        return self.ix.adopt(self.ac, self.digest, self.files)

    def adopted_dirs(self):
        base = os.path.join(self.cache, "adopted")
        return sorted(d for d in os.listdir(base) if not d.startswith(".")) if os.path.isdir(base) else []

    def labels(self, st):
        return sorted(n["label"] for n in st["graph"].nodes)

    def test_unchanged_index_is_adopted_and_identity_names_the_staged_bytes(self):
        before = index_snapshot(self.gdir)
        st, note = self.adopt()
        self.assertIsNone(note)
        self.assertEqual(st["served_by"], "adopted-snapshot")
        (name,) = self.adopted_dirs()
        dest = os.path.join(self.cache, "adopted", name, "graphify-out")
        self.assertEqual(gadapter.Index.user_digest(dest), name)
        self.assertEqual(st["snapshot_digest"], name)
        self.assertEqual(self.labels(st), ["a_fn0()", "a_fn1()"])
        self.assertEqual(index_snapshot(self.gdir), before)

    def test_edit_after_verification_is_never_served_as_the_verified_graph(self):
        orig, fired = self.ix.verify_user_index, []

        def verify_then_swap(ac, files):
            out = orig(ac, files)
            if not fired:
                fired.append(1)
                write_graph(self.gdir, ["web/app.ts"], "b")  # schema-valid, but its file has no recorded signature
            return out

        self.ix.verify_user_index = verify_then_swap
        st, note = self.adopt()
        self.assertIsNone(st)
        self.assertIn("incompatible", note)
        self.assertEqual(self.adopted_dirs(), [])
        self.assertEqual(self.ix.state, None)

    def test_edit_after_verification_that_stays_valid_is_served_as_itself(self):
        orig, fired = self.ix.verify_user_index, []

        def verify_then_swap(ac, files):
            out = orig(ac, files)
            if not fired:
                fired.append(1)
                write_graph(self.gdir, ["py/util.py", "py/main.py"], "b")
            return out

        self.ix.verify_user_index = verify_then_swap
        st, note = self.adopt()
        self.assertIsNone(note)
        self.assertEqual(self.labels(st), ["b_fn0()", "b_fn1()"])
        self.assertEqual(st["snapshot_digest"], gadapter.Index.user_digest(self.gdir))
        (name,) = self.adopted_dirs()
        self.assertEqual(name, st["snapshot_digest"])

    def test_binding_metadata_drift_during_the_copy_is_retried(self):
        orig, calls = self.ix.stage_copy, []

        def drifting_copy(src, dst):
            orig(src, dst)
            calls.append(src)
            if len(calls) == 1:  # between the files of the first attempt, the binding metadata changes
                path = os.path.join(self.gdir, "cache", "stat-index.json")
                with open(path) as fh:
                    stat = json.load(fh)
                stat["py/util.py"]["hashes"]["py/util.py"] = "0" * 64
                with open(path, "w") as fh:
                    json.dump(stat, fh)

        self.ix.stage_copy = drifting_copy
        st, note = self.adopt()
        # The retry sees the drifted (now content-mismatching) binding and refuses it rather than serving stale bytes.
        self.assertIsNone(st)
        self.assertIn("differs from its indexed content", note)
        self.assertEqual(self.adopted_dirs(), [])

    def test_a_perpetually_moving_source_falls_back_within_a_fixed_bound(self):
        orig, copies = self.ix.stage_copy, []

        def moving_copy(src, dst):
            orig(src, dst)
            copies.append(src)
            write_graph(self.gdir, ["py/util.py", "py/main.py"], f"m{len(copies)}")

        self.ix.stage_copy = moving_copy
        st, note = self.adopt()
        self.assertIsNone(st)
        self.assertIn("kept changing", note)
        self.assertLessEqual(len(copies), gadapter.ADOPT_ATTEMPTS * 5)
        self.assertEqual(self.adopted_dirs(), [])

    def test_reused_destination_is_verified_not_trusted_by_name(self):
        st, _ = self.adopt()
        (name,) = self.adopted_dirs()
        dest = os.path.join(self.cache, "adopted", name, "graphify-out")
        os.chmod(os.path.join(dest, "graph.json"), 0o644)
        with open(os.path.join(dest, "graph.json"), "w") as fh:
            json.dump(raw_graph(["py/util.py"], "evil"), fh)
        st, note = self.adopt()
        self.assertIsNone(note)
        self.assertEqual(self.labels(st), ["a_fn0()", "a_fn1()"])
        self.assertEqual(gadapter.Index.user_digest(dest), name)

    def test_existing_destination_from_another_publisher_is_accepted_only_by_identity(self):
        real_rename = os.rename
        for same in (True, False):
            shutil.rmtree(os.path.join(self.cache, "adopted"), ignore_errors=True)
            self.ix.state = None

            def racing_rename(src, dst, same=same):
                if os.path.basename(src).startswith(".stage-"):
                    if same:
                        shutil.copytree(src, dst)
                    else:
                        os.makedirs(dst)
                        with open(os.path.join(dst, "graph.json"), "w") as fh:
                            fh.write("{}")
                    raise OSError("destination exists")
                return real_rename(src, dst)

            with mock.patch.object(gadapter.os, "rename", racing_rename):
                st, note = self.adopt()
            if same:
                self.assertIsNone(note)
                self.assertEqual(self.labels(st), ["a_fn0()", "a_fn1()"])
            else:
                self.assertIsNone(st)
                self.assertIn("different bytes", note)
            self.assertEqual([d for d in os.listdir(os.path.join(self.cache, "adopted")) if d.startswith(".stage")], [])

    def test_failure_leaves_the_user_index_and_last_good_cache_intact(self):
        self.adopt()
        (name,) = self.adopted_dirs()
        dest = os.path.join(self.cache, "adopted", name, "graphify-out")
        good = index_snapshot(dest)
        before = index_snapshot(self.gdir)
        with mock.patch.object(self.ix, "stage_copy", side_effect=OSError("disk full")):
            st, note = self.adopt()
        self.assertIsNone(st)
        self.assertIn("incompatible", note)
        self.assertEqual(index_snapshot(dest), good)
        self.assertEqual(index_snapshot(self.gdir), before)

    def test_read_only_mode_still_refuses_an_index_that_moves_while_read(self):
        orig = self.ix.load_from

        def load_then_edit(*a, **k):
            out = orig(*a, **k)
            write_graph(self.gdir, ["py/util.py", "py/main.py"], "z")
            return out

        self.ix.load_from = load_then_edit
        with self.assertRaises(gadapter.AdapterError) as cm:
            self.ix.adopt({"mode": "read-only", "rel": "graphify-out"}, self.digest, self.files)
        self.assertEqual(cm.exception.code, "SPX-HPG013")


def counting_shim(tmp, version, counter, fail=False):
    """A qualified-looking graphify that counts every extraction it is asked to run."""
    shim_dir = tempfile.mkdtemp(dir=tmp)
    path = make_shim(shim_dir, version, None)
    with open(path) as fh:
        body = fh.read().split("\n", 1)[1]
    with open(path, "w") as fh:
        fh.write(f"#!/bin/sh\necho x >> '{counter}'\n" + ("echo boom >&2\nexit 3\n" if fail else body))
    return path


class ForcedRebuild(unittest.TestCase):
    """MC-05: refresh=rebuild outranks first-use adoption and never touches the user index."""

    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="mc05-")
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.root, self.cache = os.path.realpath(os.path.join(self.tmp, "proj")), os.path.join(self.tmp, "cache")
        os.makedirs(self.root)
        os.makedirs(self.cache)
        make_project(self.root)
        self.gdir = make_user_index(self.root)
        self.counter = os.path.join(self.tmp, "count")
        self.before = index_snapshot(self.gdir)

    def builds(self):
        if not os.path.exists(self.counter):
            return 0
        with open(self.counter) as fh:
            return len(fh.read().split())

    def client(self, mode, fail=False):
        shim = counting_shim(self.tmp, "0.9.25", self.counter, fail)
        c = Client(self.root, self.cache, upstream=shim, extra_env={"SEMAPRAX_HARNESS_CFG_ADOPT_INDEX": mode})
        self.addCleanup(c.close)
        c.init()
        return c

    def test_first_use_rebuild_runs_exactly_one_owned_extraction_in_both_modes(self):
        for mode in ("read-only", "copied-snapshot"):
            for op, payload in (("orient", {}), ("search", {"query": "fn"})):
                shutil.rmtree(self.cache, ignore_errors=True)
                os.makedirs(self.cache)
                if os.path.exists(self.counter):
                    os.remove(self.counter)
                res = self.client(mode).call(op, dict(payload, refresh="rebuild"))
                self.assertEqual(self.builds(), 1, (mode, op))
                self.assertIn(res["status"], ("complete", "partial"))
                meta = res["payload"]["metadata"]
                self.assertEqual(meta["refresh"], "build")
                self.assertNotIn("index_adoption", meta)
                self.assertEqual(index_snapshot(self.gdir), self.before)

    def test_first_use_auto_and_never_still_adopt_without_extraction(self):
        for mode, label in (("read-only", "reused-user-index"), ("copied-snapshot", "copied-validated-index")):
            for refresh in ("auto", "never"):
                shutil.rmtree(self.cache, ignore_errors=True)
                os.makedirs(self.cache)
                res = self.client(mode).call("orient", {"refresh": refresh})
                self.assertEqual(self.builds(), 0, (mode, refresh))
                self.assertEqual(res["payload"]["metadata"]["index_adoption"], label)
        self.assertEqual(index_snapshot(self.gdir), self.before)

    def test_later_rebuilds_behave_like_the_first(self):
        for mode in ("read-only", "copied-snapshot"):
            shutil.rmtree(self.cache, ignore_errors=True)
            os.makedirs(self.cache)
            if os.path.exists(self.counter):
                os.remove(self.counter)
            c = self.client(mode)
            c.call("orient", {})
            self.assertEqual(self.builds(), 0)
            c.call("orient", {"refresh": "rebuild"})
            self.assertEqual(self.builds(), 1, mode)
            c.call("orient", {"refresh": "rebuild"})
            self.assertEqual(self.builds(), 2, mode)
        self.assertEqual(index_snapshot(self.gdir), self.before)

    def test_forced_build_failure_is_reported_not_relabelled_as_adoption(self):
        for mode in ("read-only", "copied-snapshot"):
            shutil.rmtree(self.cache, ignore_errors=True)
            os.makedirs(self.cache)
            if os.path.exists(self.counter):
                os.remove(self.counter)
            res = self.client(mode, fail=True).call("orient", {"refresh": "rebuild"})
            self.assertEqual(res["status"], "failed")
            self.assertEqual(res["diagnostics"][0]["code"], "SPX-HPG004")
            self.assertIsNone(res["payload"])
            self.assertEqual(self.builds(), 1)
            self.assertEqual(index_snapshot(self.gdir), self.before)


# ---- MC-06: the complete serialized result fits max_result_bytes ---------------------------------------------

def wire_size(request, index, out):
    """Exact bytes of the result envelope the SDK sends for a handler return value."""
    status, payload, diags = out
    prov = {"provider_id": gadapter.PROVIDER_ID, "adapter_version": gadapter.ADAPTER_VERSION, "upstream_version": index.identity}
    env = sdk.result(request, status, payload, prov, diags)
    return len(json.dumps(env, separators=(",", ":"), sort_keys=True).encode())


class ResultBudget(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="mc06-")
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.ix = gadapter.Index(self.tmp, os.path.join(self.tmp, "cache"))
        self.ix.identity = "0.9.25"
        self.handlers = gadapter.make_handlers(self.ix)

    def state(self, skipped=(), errors=(), files=("py/a.py",), nodes=0, indexed=None):
        graph = gadapter.Graph(raw_graph([files[0]] * nodes, "n") if nodes else {"nodes": [], "links": []}, "0.9.25")
        st = {"digest": "d" * 64, "graph": graph, "indexed": list(indexed if indexed is not None else files),
              "skipped": [{"path": p, "reason": "document or media: skipped by --code-only (no model-backed ingestion)"} for p in skipped],
              "errors": [{"path": p, "reason": "e" * 240} for p in errors]}
        self.ix.ensure = lambda refresh, st=st: (st, False)
        return st

    def request(self, op, payload=None, budget=65536):
        return {"schema": "semaprax.harness-request.v1", "invocation_id": "inv-1", "project": {"id": "p", "worktree": "w", "revision": "r"},
                "capability": {"kind": "context.repository", "version": 1}, "operation": op, "deadline_ms": 1000,
                "budget": {"max_result_bytes": budget, "remaining_calls": 1}, "payload": payload or {}}

    def call(self, op, payload=None, budget=65536):
        req = self.request(op, payload, budget)
        out = self.handlers[("context.repository", op)](req)
        return req, out

    def served(self, op, payload=None, budget=65536):
        """Run through the SDK's real serve loop and measure the bytes it writes for the result."""
        req = self.request(op, payload, budget)
        frame = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "harness/invoke", "params": req}).encode() + b"\n"
        out = io.BytesIO()
        sdk.serve([], self.handlers, {"provider_id": gadapter.PROVIDER_ID, "adapter_version": gadapter.ADAPTER_VERSION,
                                      "upstream_version": "0.9.25"}, stdin=io.BytesIO(frame), stdout=out)
        line = out.getvalue().rstrip(b"\n")
        result = json.loads(line)["result"]
        return result, len(json.dumps(result, separators=(",", ":"), sort_keys=True).encode())

    def assert_honest(self, status, payload):
        if payload is None:
            return
        cov = payload["coverage"]
        self.assertEqual(status == "complete", cov["complete"])
        if cov["skipped"] or cov["extraction_errors"]:
            self.assertFalse(cov["complete"])

    def docs(self, n=200):
        return [f"docs/{chr(97 + i % 26) * 200}/{chr(65 + i % 26) * 200}/file-{i:03d}.md" for i in range(n)]

    def test_zero_item_200_skipped_docs_fit_default_and_small_budgets(self):
        self.state(skipped=self.docs())
        for budget in (65536, 4096):
            result, size = self.served("orient", budget=budget)
            self.assertLessEqual(size, budget, budget)
            self.assertNotEqual(result["status"], "complete")
            self.assertEqual(result["payload"]["items"], [])
            cov = result["payload"]["coverage"]
            self.assertFalse(cov["complete"] or cov["exhaustive"])
            self.assertLess(len(cov["skipped"]), 200)
            om = result["payload"]["metadata"]["omitted"]
            self.assertEqual(om["skipped"], 200 - len(cov["skipped"]))
            self.assertIn("SPX-HPG008", [d["code"] for d in result["diagnostics"]])

    def test_long_error_lists_and_large_metadata_cannot_bypass_the_cap(self):
        st = self.state(skipped=self.docs(50), errors=[f"src/{'x' * 150}/f{i}.py" for i in range(200)])
        st.update(adoption="y" * 900, served_by="owned-cache")
        for budget in (65536, 8192, 2048):
            req, out = self.call("orient", budget=budget)
            self.assertLessEqual(wire_size(req, self.ix, out), budget, budget)
            self.assertIsNotNone(out[1], budget)
            self.assertFalse(out[1]["coverage"]["complete"])
            if len(out[1]["coverage"]["extraction_errors"]) < 200:
                self.assertIn("SPX-HPG014", [d["code"] for d in out[2]])

    def test_non_ascii_and_json_escaping_are_measured_as_wire_bytes(self):
        nasty = [f'docs/é漢字\U0001F600/"q"\\\n/f{i}.md' for i in range(120)]
        self.state(skipped=nasty)
        for budget in (4096, 9001):
            result, size = self.served("orient", budget=budget)
            self.assertLessEqual(size, budget)
            self.assertEqual(result["payload"]["items"], [])

    def test_exact_boundary_and_boundary_plus_one(self):
        self.state(files=("py/a.py",), nodes=40, skipped=self.docs(3))
        req, full = self.call("search", {"query": "fn", "limit": 40}, budget=1 << 30)
        self.assertEqual(len(full[1]["items"]), 40)
        size = wire_size(req, self.ix, full)
        req, same = self.call("search", {"query": "fn", "limit": 40}, budget=size)
        self.assertEqual(same, full)  # exactly at the boundary nothing is omitted
        req, less = self.call("search", {"query": "fn", "limit": 40}, budget=size - 1)
        self.assertLessEqual(wire_size(req, self.ix, less), size - 1)
        self.assertNotEqual(less, full)
        self.assertLess(len(less[1]["items"]) + len(less[1]["coverage"]["skipped"]), 43)
        self.assertIn("omitted", less[1]["metadata"])
        self.assert_honest(less[0], less[1])

    def test_truncation_never_reports_complete_or_exhaustive(self):
        self.state(files=("py/a.py",), nodes=60, indexed=["py/a.py"])
        req, full = self.call("search", {"query": "fn", "limit": 60}, budget=1 << 30)
        self.assertEqual(full[0], "complete")
        size = wire_size(req, self.ix, full)
        for budget in range(size, size - 1500, -97):
            req, out = self.call("search", {"query": "fn", "limit": 60}, budget=budget)
            if budget < size:
                self.assertEqual(out[0], "partial", budget)
                self.assertFalse(out[1]["coverage"]["complete"] or out[1]["coverage"]["exhaustive"])
            self.assertLessEqual(wire_size(req, self.ix, out), budget)

    def test_required_minimum_that_cannot_fit_is_a_bounded_refusal(self):
        self.state(skipped=self.docs(5), nodes=0)
        for budget in (1, 64, 300):
            req, out = self.call("orient", budget=budget)
            self.assertEqual(out[0], "refused")
            self.assertIsNone(out[1])
            self.assertEqual([d["code"] for d in out[2]], ["SPX-HPK007"])
        result, size = self.served("orient", budget=1)
        self.assertEqual(result["status"], "refused")

    def test_stale_early_return_obeys_the_bound(self):
        st = self.state(skipped=self.docs(), errors=[f"e{i}.py" for i in range(200)])
        self.ix.ensure = lambda refresh: (st, True)
        for budget in (65536, 4096):
            result, size = self.served("search", {"query": "x", "refresh": "never"}, budget=budget)
            self.assertLessEqual(size, budget)
            self.assertEqual(result["status"], "stale")
            self.assertFalse(result["payload"]["coverage"]["complete"])
            self.assertEqual(result["diagnostics"][0]["code"], "SPX-HPG002")
            self.assertEqual(result["payload"]["items"], [])

    def test_unsupported_file_early_return_obeys_the_bound(self):
        paths = self.docs(150)
        self.state(skipped=paths)
        long_path = "docs/" + "z" * 900 + ".md"
        for target, budget in ((paths[0], 65536), (paths[0], 4096), (long_path, 4096)):
            result, size = self.served("skeleton", {"path": target}, budget=budget)
            self.assertLessEqual(size, budget, (target[:20], budget))
            self.assertIn(result["status"], ("unsupported", "refused"))
            if result["payload"] is not None:
                self.assertFalse(result["payload"]["coverage"]["complete"])
        result, _ = self.served("skeleton", {"path": paths[0]}, budget=4096)
        self.assertEqual(result["status"], "unsupported")
        self.assertEqual(result["diagnostics"][0]["code"], "SPX-HPG009")

    def test_skeleton_exhaustive_only_when_no_item_was_dropped(self):
        self.state(files=("py/a.py",), nodes=50)
        req, full = self.call("skeleton", {"path": "py/a.py"}, budget=1 << 30)
        self.assertTrue(full[1]["coverage"]["exhaustive"])
        size = wire_size(req, self.ix, full)
        req, same = self.call("skeleton", {"path": "py/a.py"}, budget=size)
        self.assertEqual(same, full)
        req, less = self.call("skeleton", {"path": "py/a.py"}, budget=size - 1)
        self.assertLessEqual(wire_size(req, self.ix, less), size - 1)
        self.assertFalse(less[1]["coverage"]["exhaustive"])
        self.assertLess(len(less[1]["items"]), 50)


if __name__ == "__main__":
    unittest.main()
