#!/usr/bin/env python3
"""Focused regression tests for the Bend 2 law benchmark runner."""
import importlib.util
import contextlib
import io
import json
import pathlib
import tempfile
import unittest
from unittest import mock

MODULE = pathlib.Path(__file__).with_name("run.py")
SPEC = importlib.util.spec_from_file_location("bend2_run", MODULE)
RUN = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(RUN)
RECEIPT_MODULE = pathlib.Path(__file__).with_name("fixture_receipt.py")
RECEIPT_SPEC = importlib.util.spec_from_file_location("bend2_fixture_receipt", RECEIPT_MODULE)
RECEIPT = importlib.util.module_from_spec(RECEIPT_SPEC); RECEIPT_SPEC.loader.exec_module(RECEIPT)


class RunnerTests(unittest.TestCase):
    def commands(self, domains=None):
        return {"schema": RUN.COMMANDS_SCHEMA, "bend": {"root": "/x", "commit": "b"}, "semaprax": {"root": "/y", "commit": "s"}, "environment": {field: field for field in RUN.ENVIRONMENT_FIELDS}, "numeric_domains": domains or {path: ["u32 checked"] for path in RUN.PATHS}, "commands": {path: ["tool", "{fixture}", "{cell}", "{case}"] for path in RUN.PATHS}}

    def manifest(self):
        return {"schema": RUN.MANIFEST_SCHEMA, "bend": {"commit": "b", "telemetry_environment": {"BEND_NO_TELEMETRY": "1"}}, "required_paths": list(RUN.PATHS), "cells": [{"id": "sort-v1", "fixture": "fixture.json", "numeric_domain": "u32 checked", "laws": ["sortedness", "permutation-and-multiplicity"], "attacks": ["empty-sort"]}]}

    def fixture(self, root):
        (root / "fixture.json").write_text(json.dumps({"schema": "semaprax.bend2-law-benchmark.fixture.v1", "id": "sort-v1", "numeric_domain": "u32 checked", "success": [{}], "attacks": {"empty-sort": [{}]}}))

    def test_manifest_requires_equal_laws_and_all_execution_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); self.fixture(root)
            manifest = self.manifest(); RUN.require_manifest(manifest, root)
            manifest["cells"][0]["laws"] = []
            with self.assertRaisesRegex(ValueError, "equal-semantics"):
                RUN.require_manifest(manifest, root)

    def test_commands_require_distinct_bend_paths_and_pinned_subjects(self):
        commands = self.commands()
        RUN.require_commands(commands, "b")
        del commands["commands"]["bend_verdict"]
        with self.assertRaisesRegex(ValueError, "every execution path"):
            RUN.require_commands(commands, "b")
        commands["commands"]["bend_verdict"] = ["tool", "{fixture}", "{cell}", "{case}"]
        commands["commands"]["bend_normal"] = ["tool", "{cell}", "{case}"]
        with self.assertRaisesRegex(ValueError, "exact fixture"):
            RUN.require_commands(commands, "b")
        commands = self.commands({path: ["u128 checked"] for path in RUN.PATHS})
        with self.assertRaisesRegex(ValueError, "numeric domain declaration"):
            RUN.require_commands(commands, "b")

    def test_drifted_identity_writes_unavailable_not_a_win(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); manifest = root / "manifest.json"; commands = root / "commands.json"; output = root / "out.json"
            self.fixture(root); manifest.write_text(json.dumps(self.manifest()))
            commands.write_text(json.dumps({"schema": RUN.COMMANDS_SCHEMA, "bend": {"root": "/no-bend", "commit": "b"}, "semaprax": {"root": "/no-spx", "commit": "s"}, "environment": {field: field for field in RUN.ENVIRONMENT_FIELDS}, "numeric_domains": {path: ["u32 checked"] for path in RUN.PATHS}, "commands": {path: ["missing", "{fixture}", "{cell}", "{case}"] for path in RUN.PATHS}}))
            self.assertEqual(RUN.main(["--manifest", str(manifest), "--commands", str(commands), "--output", str(output)]), 1)
            self.assertEqual(json.loads(output.read_text())["status"], "unavailable")

    def test_small_runs_need_explicit_pilot_label(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            RUN.main(["--commands", "/missing", "--output", "/tmp/out", "--samples", "29"])

    def test_committed_fixture_receipt_is_deterministic_and_binds_bool_cell(self):
        manifest = MODULE.with_name("manifest.json")
        first = RECEIPT.receipt(manifest)
        self.assertEqual(first, RECEIPT.receipt(manifest))
        self.assertEqual(first["schema"], RECEIPT.SCHEMA)
        scalar = next(cell for cell in first["cells"] if cell["id"] == "scalar-contract-bug-v1")
        self.assertEqual(scalar["numeric_domain"], "bool exact")
        self.assertTrue(scalar["fixture_sha256"].startswith("sha256:"))

    def test_accepted_law_gaming_fails_only_the_affected_distinct_path(self):
        manifest = self.manifest()
        commands = {"commands": {path: [path, "{fixture}"] for path in RUN.PATHS}, "numeric_domains": {path: ["u32 checked"] for path in RUN.PATHS}}

        def fake(template, cell, case, fixture, environment, timeout):
            if template[0] == "bend_normal" and case == "empty-sort":
                return {"status": "accepted", "wall_ms": 1.0, "exit_code": 0}
            if case == "success":
                return {"status": "accepted", "wall_ms": 1.0, "exit_code": 0}
            return {"status": "rejected", "wall_ms": 1.0, "exit_code": 1}

        with mock.patch.object(RUN, "invoke", side_effect=fake):
            paths = RUN.execute(manifest, commands, 30, 1)[0]["paths"]
        self.assertEqual(paths["bend_normal"]["status"], "failed")
        self.assertEqual(paths["bend_verdict"]["status"], "ok")
        self.assertEqual(paths["bend_verdict"]["warm"]["samples_ms"], [1.0] * 30)

    def test_i32_declaration_refuses_u32_cell_before_invoking_a_route(self):
        manifest = self.manifest()
        commands = {"commands": {path: [path, "{fixture}"] for path in RUN.PATHS}, "numeric_domains": {path: ["i32 checked"] for path in RUN.PATHS}}
        with mock.patch.object(RUN, "invoke") as invoke:
            paths = RUN.execute(manifest, commands, 30, 1)[0]["paths"]
        self.assertFalse(invoke.called)
        for path in RUN.PATHS:
            self.assertEqual(paths[path]["status"], "unavailable")
            self.assertEqual(paths[path]["declared_numeric_domains"], ["i32 checked"])
            self.assertIn("exact numeric domain", paths[path]["reason"])


if __name__ == "__main__":
    unittest.main()
