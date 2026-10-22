"""Regressions for exact inventory transport and compiled include ownership."""
import ast
import hashlib
import json
from pathlib import Path
import re
import tempfile
import unittest


REPO = Path(__file__).resolve().parents[2]
GENERATOR = REPO / "docs/evidence/canonical-byte-source-20261022/current-source-authority.py"
PARSER = REPO / "docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py"


def definitions(path, names, values=None):
    tree = ast.parse(path.read_text())
    selected = [
        node for node in tree.body
        if isinstance(node, ast.FunctionDef) and node.name in names
        or isinstance(node, ast.Assign)
        and any(isinstance(target, ast.Name) and target.id in names for target in node.targets)
    ]
    result = dict(values or {})
    exec(compile(ast.Module(body=selected, type_ignores=[]), str(path), "exec"), result)
    return result


class InventoryTransportTests(unittest.TestCase):
    def test_original_tools_stay_byte_identical_to_their_sealed_archives(self):
        bindings = json.loads((REPO / "docs/evidence/source-boundary-domains-20261016/original-tool-bindings.json").read_text())
        for row in bindings["tools"]:
            with self.subTest(source=row["source"]):
                actual = (REPO / row["source"]).read_bytes()
                self.assertEqual(actual, (REPO / row["archive"]).read_bytes())
                self.assertEqual(hashlib.sha256(actual).hexdigest(), row["sha256"])

    def test_exact_transport_preserves_a_preceding_legacy_grammar(self):
        ns = definitions(GENERATOR, {"JOIN", "inventory_transport"})
        prefix = '// legacy\nmacro_rules! legacy { () => { "α" }; }\n'
        tail = '\naudit_source_join!("α", "β")\n'
        expected = tail.replace("audit_source_join!", "concat!")
        self.assertEqual(ns["inventory_transport"](ns["JOIN"] + tail), expected)
        self.assertEqual(ns["inventory_transport"](prefix + ns["JOIN"] + tail), prefix + expected)

    def test_missing_duplicate_or_changed_transport_fails_closed(self):
        ns = definitions(GENERATOR, {"JOIN", "inventory_transport"})
        for source in ["", ns["JOIN"] * 2, ns["JOIN"].replace("pieces.concat()", "pieces.clear()")]:
            with self.subTest(source=source), self.assertRaises(AssertionError):
                ns["inventory_transport"](source)


class CompiledIncludeTests(unittest.TestCase):
    def fixture(self, directory):
        root = Path(directory)
        (root / "src").mkdir()
        (root / "tests").mkdir()
        (root / "Cargo.toml").write_text("")
        (root / "src/lib.rs").write_text('include!("tail.rs");\n')
        (root / "src/tail.rs").write_text("pub fn retained() {}\n")
        literal = 'include_str!("../src/tail.rs")'
        (root / "tests/harness.rs").write_text(f"const DATA: &str = {literal};\n")
        edge = {"parent": "tests/harness.rs", "start": (root / "tests/harness.rs").read_text().index(literal), "text": literal,
                "attributes": [], "child": "src/tail.rs"}
        values = {
            "R": root, "O": root / "out", "re": re, "json": json,
            "registered": [("src/lib.rs", [("lib", [], "semaprax", None)]),
                           ("tests/harness.rs", [("test:harness", [], "semaprax", None)])],
            "sha": lambda data: hashlib.sha256(data).hexdigest(),
            "files": {"src/lib.rs": [("lib", [], "semaprax", None)],
                      "src/tail.rs": [("test:harness", [], "semaprax", edge)]},
            "roots": {("semaprax", "lib"): "src/lib.rs",
                      ("semaprax", "test:harness"): "tests/harness.rs"},
            "parser": PARSER,
            "mask": definitions(PARSER, {"mask"}, {"re": re})["mask"],
        }
        values["O"].mkdir()
        ns = definitions(GENERATOR, {"closed", "manifest", "literal_inventory_acquisitions", "compiled_acquisition_prefix",
                                     "DATA_PROJECTIONS", "data_projection_records", "projection_fragment_edge", "projection_fragment_edges"}, dict(values, Path=Path))
        return root, ns, [{"path": "src/lib.rs"}, {"path": "src/tail.rs"}]

    def test_test_data_cannot_hide_the_same_files_production_include(self):
        with tempfile.TemporaryDirectory() as directory:
            _, ns, chunks = self.fixture(directory)
            first = ns["manifest"](chunks)
            tail = first["classes"]["src/tail.rs"]
            self.assertEqual(tail["role"], "production")
            self.assertEqual([chain["kind"] for chain in tail["chains"]], ["test", "lib"])
            self.assertEqual(tail["chains"][0]["edges"][0]["parent"], "tests/harness.rs")
            self.assertEqual(tail["chains"][1]["edges"][0]["text"], 'include!("tail.rs")')
            self.assertEqual(first, ns["manifest"](chunks), "replay must not duplicate a chain")

    def test_legacy_tail_data_projection_requires_both_exact_readers(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ns, _ = self.fixture(directory)
            parent = "src/source_reflows_merge/main67107_inputs_tail.rs"
            target = "tests/workspace/semantic_graph/source_inventory.rs"
            path = root / parent
            path.parent.mkdir(parents=True)
            reader = 'include_str!("../../tests/workspace/semantic_graph/source_inventory.rs")'
            path.write_text(reader + "\n" + reader + "\n")
            edges = ns["projection_fragment_edges"](parent, target)
            self.assertEqual(len(edges), 2)
            self.assertEqual([edge["start"] for edge in edges], [0, len(reader) + 1])
            record = ("test:harness", [], "semaprax", {
                "parent": "tests/harness.rs", "start": 0,
                "text": 'include_str!("../' + parent + '")',
                "attributes": [], "child": parent,
            })
            projected = ns["data_projection_records"](parent, target, edges[0], [record])
            self.assertEqual(len(projected), 1)
            self.assertEqual(projected[0][1]["authority"], "none; closed read-only text projection")
            path.write_text(reader + "\n")
            with self.assertRaisesRegex(AssertionError, "both exact"):
                ns["projection_fragment_edges"](parent, target)

    def test_module_ancestor_is_classified_without_executing_its_uninvoked_macros(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ns, _ = self.fixture(directory)
            (root / "src/lib.rs").write_text(
                'mod leaf;\nmacro_rules! unused { () => { include!(concat!("x", ".rs")); } }\n'
            )
            (root / "src/leaf.rs").write_text('pub fn actual() {}\n')
            ns["files"] = {
                "src/lib.rs": [("lib", [], "semaprax", None)],
                "src/leaf.rs": [("lib", [{"source_path": "src/lib.rs", "line": 1,
                                         "module": "leaf", "attributes": []}], "semaprax", None)],
            }
            result = ns["manifest"]([{"path": "src/leaf.rs"}])
            self.assertEqual(result["classes"]["src/lib.rs"]["role"], "production")
            self.assertIn("src/lib.rs", result["closure_only"])
            self.assertEqual(result["classes"]["src/leaf.rs"]["chains"][0]["edges"][0]["child"], "src/leaf.rs")
            self.assertNotIn("src/x.rs", result["sources"])

    def test_nonliteral_compiled_include_still_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ns, chunks = self.fixture(directory)
            (root / "src/lib.rs").write_text('include!(concat!("tail", ".rs"));\n')
            with self.assertRaisesRegex(AssertionError, "nonliteral compiled include"):
                ns["manifest"](chunks)

    def test_include_tokens_inside_test_data_cannot_supply_a_compiled_chain(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ns, chunks = self.fixture(directory)
            (root / "src/data.rs").write_text('include!("tail.rs");\n')
            literal = 'include_str!("../src/data.rs")'
            harness = root / "tests/harness.rs"
            harness.write_text(harness.read_text() + f"const RAW: &str = {literal};\n")
            edge = {"parent": "tests/harness.rs", "start": harness.read_text().index(literal),
                    "text": literal, "attributes": [], "child": "src/data.rs"}
            ns["files"]["src/data.rs"] = [("test:harness", [], "semaprax", edge)]
            result = ns["manifest"](chunks + [{"path": "src/data.rs"}])
            self.assertEqual(result["classes"]["src/data.rs"]["role"], "test-only")
            tail = result["classes"]["src/tail.rs"]
            self.assertEqual(len(tail["chains"]), 2)
            self.assertFalse(any(edge["parent"] == "src/data.rs"
                                 for chain in tail["chains"] for edge in chain["edges"]))

    def test_literal_index_keeps_utf8_offsets_order_and_both_real_occurrences(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ns, _ = self.fixture(directory)
            literal = 'include_str!("../src/tail.rs")'
            prefix = f'// α {literal}\nconst RAW: &str = r#"{literal}"#;\n'
            first = f'const FIRST: &str = {literal};\n'
            second = f'const SECOND: &str = {literal};\n'
            (root / "tests/harness.rs").write_text(prefix + first + second)
            ns = definitions(GENERATOR, {"literal_inventory_acquisitions", "compiled_acquisition_prefix"}, dict(ns, Path=Path))
            records = [("test:harness", [], "semaprax", None)]
            actual = ns["literal_inventory_acquisitions"]({"src/tail.rs"}, [("tests/harness.rs", records)])
            edges = [row[3] for row in actual["src/tail.rs"]]
            expected = [len((prefix + "const FIRST: &str = ").encode()),
                        len((prefix + first + "const SECOND: &str = ").encode())]
            self.assertEqual([edge["start"] for edge in edges], expected)
            for edge in edges:
                raw = (root / edge["parent"]).read_bytes()
                self.assertEqual(raw[edge["start"]:edge["start"] + len(literal.encode())], literal.encode())

    def test_literal_index_does_not_accept_a_foreign_source(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ns, _ = self.fixture(directory)
            (root / "tests/harness.rs").write_text('include_str!("/outside/tail.rs");\n')
            ns = definitions(GENERATOR, {"literal_inventory_acquisitions", "compiled_acquisition_prefix"}, dict(ns, Path=Path))
            with self.assertRaises(ValueError):
                ns["literal_inventory_acquisitions"]({"src/tail.rs"}, [
                    ("tests/harness.rs", [("test:harness", [], "semaprax", None)])])

    def test_legacy_text_projection_is_closed_finite_and_keeps_its_actual_data_edge(self):
        ns = definitions(GENERATOR, {"closed", "DATA_PROJECTIONS", "data_projection_records"}, {"re": re})
        parent, target = sorted(ns["DATA_PROJECTIONS"])[0]
        data = {"parent": "tests/harness.rs", "child": parent, "text": 'include_str!("source.rs")'}
        fragment = {"parent": parent, "child": target, "text": 'include!("fragment.rs")'}
        record = ("test:harness", [], "semaprax", data)
        [(folded, witness)] = ns["data_projection_records"](parent, target, fragment, [record])
        self.assertEqual(record[3], data, "the actual data acquisition remains intact")
        self.assertEqual(folded[3], fragment)
        self.assertEqual(witness["data_acquisition"], data)
        self.assertIn("read-only text", witness["authority"])
        self.assertEqual(ns["data_projection_records"]("unknown.rs", target, fragment, [record]), [])
        with self.assertRaisesRegex(AssertionError, "cannot supply production authority"):
            ns["data_projection_records"](parent, target, fragment, [("lib", [], "semaprax", data)])

    def test_text_fragment_is_exact_even_beside_uninterpreted_macro_text(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ns, _ = self.fixture(directory)
            parent, target = sorted(ns["DATA_PROJECTIONS"])[0]
            owner = root / parent
            owner.parent.mkdir(parents=True)
            literal = f'include!("{Path(target).name}");\n'
            other = 'include!(concat!("uninterpreted", ".rs"));\n'
            owner.write_text(other + literal)
            edge = ns["projection_fragment_edge"](parent, target)
            self.assertEqual(edge["start"], len(other.encode()))
            self.assertEqual(edge["child"], target)
            for invalid in [other, other + literal + literal]:
                owner.write_text(invalid)
                with self.assertRaisesRegex(AssertionError, "one exact literal"):
                    ns["projection_fragment_edge"](parent, target)

    def test_compiled_fragment_data_keeps_the_full_bridge_and_legacy_view(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ns, _ = self.fixture(directory)
            (root / "tests/fragment.rs").write_text('include_str!("../src/tail.rs");\n')
            bridge = {"parent": "tests/harness.rs", "child": "tests/fragment.rs",
                      "text": 'include!("fragment.rs")', "start": 0, "attributes": []}
            record = ("test:harness", [], "semaprax", bridge)
            rows = ns["literal_inventory_acquisitions"]({"src/tail.rs"}, [
                ("tests/fragment.rs", [record])])["src/tail.rs"]
            self.assertEqual(len(rows), 1, "an empty module chain needs the actual root bridge")
            self.assertEqual(rows[0][3][0], bridge)
            self.assertEqual(rows[0][3][1]["parent"], "tests/fragment.rs")
            data_only = dict(bridge, text='include_str!("fragment.rs")')
            self.assertEqual(ns["literal_inventory_acquisitions"]({"src/tail.rs"}, [
                ("tests/fragment.rs", [("test:harness", [], "semaprax", data_only)])])["src/tail.rs"], [])

    def test_nested_literal_compilation_retains_each_actual_bridge(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ns, chunks = self.fixture(directory)
            (root / "src/lib.rs").write_text('include!("outer.rs");\n')
            (root / "src/outer.rs").write_text('include!("inner.rs");\n')
            (root / "src/inner.rs").write_text("pub fn nested() {}\n")
            result = ns["manifest"](chunks)
            inner = result["classes"]["src/inner.rs"]
            self.assertEqual(inner["role"], "production")
            self.assertEqual([edge["parent"] for edge in inner["chains"][0]["edges"]],
                             ["src/lib.rs", "src/outer.rs"])
            self.assertEqual([edge["child"] for edge in inner["chains"][0]["edges"]],
                             ["src/outer.rs", "src/inner.rs"])


if __name__ == "__main__":
    unittest.main()
