import json
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "tools"))
from drive import drive  # noqa: E402

ADAPTER = [sys.executable, os.path.join(HERE, "adapter.py")]


def make_tree(extra=None):
    d = tempfile.mkdtemp(prefix="hp16a-idx-")
    os.makedirs(os.path.join(d, "src"))
    os.makedirs(os.path.join(d, ".git"))
    os.makedirs(os.path.join(d, "node_modules"))
    for rel, text in {"src/a.py": "def greet(name):\n    return name\n\ngreet('x')\n",
                      "src/b.rs": "pub fn greet() {}\n",
                      ".git/c.py": "def greet(): pass\n", "node_modules/d.js": "function greet(){}\n"}.items():
        with open(os.path.join(d, rel), "w") as fh:
            fh.write(text)
    for rel, text in (extra or {}).items():
        with open(os.path.join(d, rel), "w") as fh:
            fh.write(text)
    return d


def call(root, op, payload):
    replies, _, code = drive(ADAPTER, "context.repository", op, payload, root=root)
    assert code == 0 and len(replies) == 3, replies
    return replies[1]["result"]


class SourceIndexTest(unittest.TestCase):
    def test_handshake_accepts_only_offered(self):
        replies, _, _ = drive(ADAPTER, "decision.evaluate", "evaluate", {}, root=make_tree())
        self.assertEqual(replies[0]["result"]["accepted"], [])

    def test_orient_counts(self):
        r = call(make_tree(), "orient", {})
        self.assertEqual(r["status"], "complete")
        self.assertEqual(r["payload"]["files"], 2)
        self.assertEqual(r["payload"]["languages"], {"python": 1, "rust": 1})

    def test_search_items_and_hidden_dirs_skipped(self):
        r = call(make_tree(), "search", {"query": "greet"})
        paths = {i["path"] for i in r["payload"]["items"]}
        self.assertEqual(paths, {"src/a.py", "src/b.rs"})
        top = r["payload"]["items"][0]
        self.assertEqual(top["path"], "src/a.py")
        self.assertEqual(top["span"], {"start_line": 1, "end_line": 1})
        self.assertEqual(top["provenance"], "structural")
        import hashlib
        self.assertEqual(top["digest"], hashlib.sha256(b"def greet(name):").hexdigest())
        self.assertTrue(r["payload"]["coverage"]["exhaustive"])
        for item in r["payload"]["items"]:
            self.assertFalse(os.path.isabs(item["path"]) or ".." in item["path"])

    def test_echo_binding(self):
        r = call(make_tree(), "orient", {})
        self.assertEqual(r["invocation_id"], "inv-000001")
        self.assertEqual(r["capability"], {"kind": "context.repository", "version": 1})

    def test_skeleton(self):
        r = call(make_tree(), "skeleton", {"path": "src/a.py"})
        self.assertEqual([i["span"]["start_line"] for i in r["payload"]["items"]], [1])

    def test_references_exhaustive(self):
        r = call(make_tree(), "references", {"symbol": "greet"})
        self.assertEqual(len(r["payload"]["items"]), 3)
        self.assertTrue(r["payload"]["coverage"]["exhaustive"])

    def test_skipped_file_makes_result_partial_and_not_exhaustive(self):
        root = make_tree({"src/big.py": "x = 1\n" * 100000, "src/bin.py": ""})
        with open(os.path.join(root, "src/bin.py"), "wb") as fh:
            fh.write(b"\xff\xfe\x00")
        r = call(root, "references", {"symbol": "greet"})
        cov = r["payload"]["coverage"]
        self.assertEqual(r["status"], "partial")
        self.assertFalse(cov["exhaustive"])
        self.assertEqual({(s["path"], s["reason"]) for s in cov["skipped"]},
                         {("src/big.py", "file-too-large"), ("src/bin.py", "not-utf8")})

    def test_missing_root_is_unavailable(self):
        replies, _, _ = drive(ADAPTER, "context.repository", "orient", {}, env_extra=["SEMAPRAX_HARNESS_PROJECT_ROOT=/nonexistent-hp16a"])
        self.assertEqual(replies[1]["result"]["status"], "unavailable")

    def test_unknown_op_unsupported(self):
        self.assertEqual(call(make_tree(), "rewrite", {})["status"], "unsupported")

    def test_standalone_cli(self):
        out = subprocess.run([sys.executable, os.path.join(HERE, "index.py"), "search", make_tree(), "greet"],
                             capture_output=True, text=True, check=True).stdout
        doc = json.loads(out)
        self.assertEqual(len(doc["items"]), 3)  # a.py def + call, b.rs
        self.assertIn("coverage", doc)


if __name__ == "__main__":
    unittest.main()
