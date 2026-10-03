"""Boundary tests for the production bounded type-closure expander."""

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


if __name__ == "__main__":
    unittest.main()
