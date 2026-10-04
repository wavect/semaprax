#!/usr/bin/env python3
"""Hostile fixture checks for every LAW-16 equal-spec control."""
import copy
import importlib.util
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parent
MODULE = ROOT / "equal_spec_controls.py"
SPEC = importlib.util.spec_from_file_location("bend2_equal_spec_controls", MODULE)
CONTROLS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CONTROLS)


class EqualSpecControlTests(unittest.TestCase):
    def cell(self, identifier):
        manifest = json.loads((ROOT / "manifest.json").read_text())
        return next(cell for cell in manifest["cells"] if cell["id"] == identifier)

    def fixture(self, cell):
        return json.loads((ROOT / cell["fixture"]).read_text())

    def test_committed_corpus_satisfies_every_equal_spec_control(self):
        for identifier in CONTROLS.EXPECTED:
            cell = self.cell(identifier)
            CONTROLS.validate(cell, self.fixture(cell))

    def test_each_seeded_loophole_is_rejected_if_promoted_to_success(self):
        for identifier in CONTROLS.EXPECTED:
            cell = self.cell(identifier)
            fixture = self.fixture(cell)
            hostile = copy.deepcopy(fixture)
            hostile["success"] = next(iter(fixture["attacks"].values()))
            with self.assertRaisesRegex(ValueError, "equal-spec control"):
                CONTROLS.validate(cell, hostile)

    def test_wrong_u32_domain_cannot_be_relabelled_as_i32(self):
        cell = self.cell("structured-balance-transfer-v1")
        hostile = copy.deepcopy(cell)
        hostile["numeric_domain"] = "i32 checked"
        with self.assertRaisesRegex(ValueError, "numeric domain"):
            CONTROLS.validate(hostile, self.fixture(cell))


if __name__ == "__main__":
    unittest.main()
