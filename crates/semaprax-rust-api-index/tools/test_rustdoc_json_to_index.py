"""Boundary tests for the production bounded type-closure expander."""

import argparse
import hashlib
import json
import importlib.util
import pathlib
import unittest
from unittest import mock


CONVERTER_PATH = pathlib.Path(__file__).with_name("rustdoc_json_to_index.py")
SPEC = importlib.util.spec_from_file_location("rustdoc_json_to_index", CONVERTER_PATH)
converter = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(converter)


class TypeClosureBoundsTests(unittest.TestCase):
    def expand(self, graph, roots):
        records = [{"_type_root_ids": list(root_set), "support": "supported"} for root_set in roots]

        def record_for(item_id, *_args):
            return (
                {
                    "path": f"fixture::{item_id}",
                    "kind": "struct",
                    "visibility": "public",
                    "references": [],
                },
                set(graph.get(item_id, ())),
            )

        with (
            mock.patch.object(
                converter,
                "path_for_id",
                side_effect=lambda item_id, *_args: f"fixture::{item_id}",
            ),
            mock.patch.object(converter, "type_record_for_id", side_effect=record_for),
        ):
            types = converter.build_type_closures(records, {}, {}, {}, None)
        return records, types

    def test_cycles_are_visited_once_and_depth_boundary_is_exact(self):
        records, types = self.expand({"a": ["b"], "b": ["a"]}, [["a"]])
        self.assertEqual(records[0]["reachable_types"], ["fixture::a", "fixture::b"])
        self.assertEqual(records[0]["type_closure_depth"], 2)
        self.assertEqual(len(types), 2)

        at_depth_limit = {str(i): [str(i + 1)] for i in range(converter.MAX_TYPE_DEPTH - 1)}
        records, types = self.expand(at_depth_limit, [["0"]])
        self.assertEqual(records[0]["type_closure_depth"], converter.MAX_TYPE_DEPTH)
        self.assertEqual(len(types), converter.MAX_TYPE_DEPTH)

        over_depth = {str(i): [str(i + 1)] for i in range(converter.MAX_TYPE_DEPTH)}
        with self.assertRaisesRegex(converter.InputError, "exceeds the configured depth"):
            self.expand(over_depth, [["0"]])

    def test_per_item_demand_expansion_accepts_limit_and_rejects_one_over(self):
        at_limit = {"root": [f"leaf{i:03}" for i in range(converter.MAX_TYPE_REFERENCES - 1)]}
        records, types = self.expand(at_limit, [["root"]])
        self.assertEqual(len(records[0]["reachable_types"]), converter.MAX_TYPE_REFERENCES)
        self.assertEqual(len(types), converter.MAX_TYPE_REFERENCES)

        over_limit = {"root": [f"leaf{i:03}" for i in range(converter.MAX_TYPE_REFERENCES)]}
        with self.assertRaisesRegex(converter.InputError, "per-item bound"):
            self.expand(over_limit, [["root"]])

    def test_selected_api_type_union_accepts_limit_and_rejects_one_over(self):
        at_limit = [[f"type{i:03}"] for i in range(converter.MAX_TYPES)]
        records, types = self.expand({}, at_limit)
        self.assertEqual(len(records), converter.MAX_TYPES)
        self.assertEqual(len(types), converter.MAX_TYPES)

        over_limit = [[f"type{i:03}"] for i in range(converter.MAX_TYPES + 1)]
        with self.assertRaisesRegex(converter.InputError, "selected APIs reach too many types"):
            self.expand({}, over_limit)

    def test_private_method_is_emitted_only_as_an_explicit_rejection(self):
        item = {
            "name": "hidden",
            "visibility": "default",
            "docs": None,
            "span": None,
            "inner": {
                "function": {
                    "sig": {
                        "inputs": [],
                        "output": {"primitive": "bool"},
                        "is_c_variadic": False,
                    },
                    "generics": {"params": [], "where_predicates": []},
                }
            },
        }
        row = converter.function_record(
            ("fixture", "PublicApi", "hidden"),
            item,
            "inherent_method",
            {},
            {},
            None,
            owner_id="public_owner",
        )
        self.assertEqual(row["visibility"], "private")
        self.assertEqual(row["support"], "rejected")
        self.assertEqual(row["reason"], "private")


class ActualFormat61Tests(unittest.TestCase):
    def test_actual_pinned_rustdoc_alias_and_payload_closure(self):
        fixture = CONVERTER_PATH.parent.parent / "fixtures" / "sg-format61"
        capture = json.loads((fixture / "capture-results.json").read_text())
        original = next(row["args"] for row in capture if row["name"] == "full")
        values = dict(zip(original[2::2], original[3::2]))
        args = argparse.Namespace(**{key[2:].replace("-", "_"): value for key, value in values.items()})
        args.source_root = pathlib.Path(args.source_root)
        args.rustdoc_format_version = int(args.rustdoc_format_version)
        args.renamed_from = None
        args.select = []
        document = json.loads((fixture / "review_index.json").read_text())
        extractor_digest = "sha256:" + hashlib.sha256(CONVERTER_PATH.read_bytes()).hexdigest()
        self.assertEqual("sha256:" + hashlib.sha256((fixture / "rust-index-fixture.rs").read_bytes()).hexdigest(), args.source_sha256)
        envelope = converter.extract(document, args, extractor_digest)
        index = envelope["index"]
        items = {row["path"]: row for row in index["items"]}
        self.assertIn("review_index::public_api::increment", items)
        increment = items["review_index::public_api::increment"]
        self.assertEqual(increment["visibility"], "public")
        self.assertEqual(increment["support"], "supported")
        self.assertFalse(any("::internal::" in path for path in items))
        event = items["review_index::make_event"]
        self.assertEqual(event["reachable_types"], ["review_index::Event", "review_index::Payload"])
        self.assertTrue(event["closure_complete"])
        types = {row["path"]: row for row in index["types"]}
        self.assertEqual(types["review_index::Event"]["references"], ["review_index::Payload"])
        args.select = ["review_index::public_api::increment", "review_index::make_event"]
        self.assertEqual(converter.extract(document, args, extractor_digest), envelope)


class Format61RegressionTests(unittest.TestCase):
    """Synthetic format-61 decoder boundaries; these are not rustdoc captures."""

    def test_glob_named_module_alias_traverses_callable_contents(self):
        def module(children, visibility="default", name=None):
            return {"name": name, "visibility": visibility, "inner": {"module": {"items": children}}}
        document = {"root": "root", "index": {
            "root": module(["internal", "bridge", "root_glob"]),
            "internal": module(["nested"]),
            "nested": module(["fn"], "public", "nested"),
            "fn": {"name": "increment", "visibility": "public", "inner": {"function": {}}},
            "bridge": module(["alias"]),
            "alias": {"visibility": "public", "inner": {"use": {"id": "nested", "name": "public_api", "is_glob": False}}},
            "root_glob": {"visibility": "public", "inner": {"use": {"id": "bridge", "is_glob": True}}},
        }}
        exports = converter.module_exports(document, "fixture")
        self.assertEqual(exports["fn"], {("fixture", "public_api", "increment")})
        self.assertNotIn("internal", exports)
        document["index"]["nested"]["inner"]["module"]["items"].append("cycle")
        document["index"]["cycle"] = {"visibility": "public", "inner": {"use": {"id": "root", "is_glob": True}}}
        self.assertEqual(converter.module_exports(document, "fixture")["fn"], exports["fn"])

    def test_enum_tuple_named_and_plain_variants_preserve_payload_edges(self):
        index = {
            "event": {"name": "Event", "visibility": "public", "inner": {"enum": {"variants": ["tuple", "named", "plain"]}}},
            "tuple": {"inner": {"variant": {"kind": {"tuple": ["field"]}}}},
            "named": {"inner": {"variant": {"kind": {"struct": {"fields": ["field"], "has_stripped_fields": False}}}}},
            "plain": {"inner": {"variant": {"kind": "plain"}}},
            "field": {"visibility": "default", "inner": {"struct_field": {"resolved_path": {"id": "payload", "path": "Payload", "args": None}}}},
        }
        document = {"index": index, "paths": {}}
        exports = {"event": {("fixture", "Event")}}
        _, refs = converter.type_record_for_id("event", document, exports, {}, None)
        self.assertEqual(refs, {"payload"})
        index["plain"]["inner"]["variant"]["kind"] = {"unknown": []}
        with self.assertRaisesRegex(converter.InputError, "variant kind"):
            converter.type_record_for_id("event", document, exports, {}, None)


if __name__ == "__main__":
    unittest.main()
