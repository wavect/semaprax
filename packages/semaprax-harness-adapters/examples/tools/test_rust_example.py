"""Spawns the cargo-built Rust example adapter. Skipped when it is not built:
  cargo build --offline -p semaprax-harness --example context_adapter_rust
"""
import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from drive import drive  # noqa: E402

REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
BIN = os.environ.get("CONTEXT_ADAPTER_RUST") or os.path.join(REPO, "target", "debug", "examples", "context_adapter_rust")


@unittest.skipUnless(os.path.exists(BIN), "context_adapter_rust not built")
class RustAdapterTest(unittest.TestCase):
    def test_fixed_corpus_search(self):
        replies, _, code = drive([BIN], "context.repository", "search", {"query": "add"}, env_extra=["SEMAPRAX_HARNESS_PROJECT_ROOT"])
        r = replies[1]["result"]
        self.assertEqual(code, 0)
        self.assertEqual(r["status"], "complete")
        self.assertEqual(sorted(i["path"] for i in r["payload"]["items"]), ["src/lib.rs", "src/main.rs"])

    def test_project_root_search_matches_python_shape(self):
        d = tempfile.mkdtemp(prefix="hp16a-rs-")
        with open(os.path.join(d, "x.rs"), "w") as fh:
            fh.write("fn needle() {}\n")
        replies, _, _ = drive([BIN], "context.repository", "search", {"query": "needle"}, root=d)
        r = replies[1]["result"]
        item = r["payload"]["items"][0]
        self.assertEqual((item["path"], item["span"], item["provenance"]), ("x.rs", {"start_line": 1, "end_line": 1}, "structural"))
        self.assertEqual(r["invocation_id"], "inv-000001")
        self.assertTrue(r["payload"]["coverage"]["exhaustive"])

    def test_unsupported_op_and_unaccepted_capability(self):
        r = drive([BIN], "context.repository", "references", {})[0][1]["result"]
        self.assertEqual(r["status"], "unsupported")
        self.assertEqual(drive([BIN], "decision.evaluate", "evaluate", {})[0][0]["result"]["accepted"], [])


if __name__ == "__main__":
    unittest.main()
