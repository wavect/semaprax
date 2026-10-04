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


class RunnerTests(unittest.TestCase):
    def manifest(self):
        return {"schema": RUN.MANIFEST_SCHEMA, "bend": {"commit": "b", "telemetry_environment": {"BEND_NO_TELEMETRY": "1"}}, "required_paths": list(RUN.PATHS), "cells": [{"id": "sort-v1", "numeric_domain": "u32 checked", "laws": ["sortedness", "permutation-and-multiplicity"], "attacks": ["empty-sort"]}]}

    def test_manifest_requires_equal_laws_and_all_execution_paths(self):
        manifest = self.manifest(); RUN.require_manifest(manifest)
        manifest["cells"][0]["laws"] = []
        with self.assertRaisesRegex(ValueError, "equal-semantics"):
            RUN.require_manifest(manifest)

    def test_commands_require_distinct_bend_paths_and_pinned_subjects(self):
        commands = {"schema": RUN.COMMANDS_SCHEMA, "bend": {"root": "/x", "commit": "b"}, "semaprax": {"root": "/y", "commit": "s"}, "environment": {field: field for field in RUN.ENVIRONMENT_FIELDS}, "commands": {path: ["tool", "{cell}", "{case}"] for path in RUN.PATHS}}
        RUN.require_commands(commands, "b")
        del commands["commands"]["bend_verdict"]
        with self.assertRaisesRegex(ValueError, "every execution path"):
            RUN.require_commands(commands, "b")

    def test_drifted_identity_writes_unavailable_not_a_win(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); manifest = root / "manifest.json"; commands = root / "commands.json"; output = root / "out.json"
            manifest.write_text(json.dumps(self.manifest()))
            commands.write_text(json.dumps({"schema": RUN.COMMANDS_SCHEMA, "bend": {"root": "/no-bend", "commit": "b"}, "semaprax": {"root": "/no-spx", "commit": "s"}, "environment": {field: field for field in RUN.ENVIRONMENT_FIELDS}, "commands": {path: ["missing", "{cell}", "{case}"] for path in RUN.PATHS}}))
            self.assertEqual(RUN.main(["--manifest", str(manifest), "--commands", str(commands), "--output", str(output)]), 1)
            self.assertEqual(json.loads(output.read_text())["status"], "unavailable")

    def test_small_runs_need_explicit_pilot_label(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            RUN.main(["--commands", "/missing", "--output", "/tmp/out", "--samples", "29"])

    def test_accepted_law_gaming_fails_only_the_affected_distinct_path(self):
        manifest = self.manifest()
        commands = {"commands": {path: [path] for path in RUN.PATHS}}

        def fake(template, cell, case, environment, timeout):
            if template == ["bend_normal"] and case == "empty-sort":
                return {"status": "accepted", "wall_ms": 1.0, "exit_code": 0}
            if case == "success":
                return {"status": "accepted", "wall_ms": 1.0, "exit_code": 0}
            return {"status": "rejected", "wall_ms": 1.0, "exit_code": 1}

        with mock.patch.object(RUN, "invoke", side_effect=fake):
            paths = RUN.execute(manifest, commands, 30, 1)[0]["paths"]
        self.assertEqual(paths["bend_normal"]["status"], "failed")
        self.assertEqual(paths["bend_verdict"]["status"], "ok")
        self.assertEqual(paths["bend_verdict"]["warm"]["samples_ms"], [1.0] * 30)


if __name__ == "__main__":
    unittest.main()
