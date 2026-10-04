#!/usr/bin/env python3
"""Contracts for the isolated Codex LAW-16 raw-trial runner."""
import hashlib
import importlib.util
import json
import pathlib
import subprocess
import tempfile
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("bend2_codex_trial", ROOT / "codex_agent_trial.py")
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


def digest(body):
    return "sha256:" + hashlib.sha256(body).hexdigest()


def configuration():
    return {"schema": RUNNER.PLAN.CONFIG_SCHEMA, "model": {"provider": "openai-codex-cli", "name": "gpt-6-luna", "configuration_sha256": digest(b"config")},
            "tool_access": {"network": "provider-managed", "filesystem": "read-only", "shell": "read-only"},
            "fixed_budget": {"max_tokens": 1000, "max_cost_usd": "1.00"},
            "execution": {"runner": "codex_exec_json_v1", "sandbox": "read-only", "working_directory": "fresh-empty-directory", "repository_access": "none", "max_wall_seconds": 60}, "trials_per_cell": 10}


def plan(root):
    path = root / "plan.json"
    path.write_text(json.dumps({"schema": RUNNER.PLAN.SCHEMA, "agent_configuration": {"value": configuration()}, "cells": [{"status": "preregistered", "trials": [{
        "id": "boolean:bend2:1", "language": "bend2", "status": "not_run", "execution": {"repository_access": "none"},
        "acceptance": {"success_witnesses": [{"input": False, "output": True}], "rejected_law_gaming_attacks": {"weakened-postcondition": [{"input": False, "output": False}]}},
    }]}]}))
    return path


class CodexTrialTests(unittest.TestCase):
    def test_runner_uses_empty_read_only_ephemeral_codex_and_retains_jsonl(self):
        events = b'{"type":"turn.completed","usage":{"input_tokens":10,"cached_input_tokens":2,"output_tokens":3}}\n'
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); path = plan(root)
            with mock.patch.object(RUNNER.subprocess, "run", side_effect=[
                subprocess.CompletedProcess(["codex", "--version"], 0, b"codex 1", b""),
                subprocess.CompletedProcess(["codex", "exec"], 0, events, b""),
            ]) as invoke:
                record = RUNNER.run(path, "boolean:bend2:1", root / "evidence")
            command = invoke.call_args_list[1].args[0]
            self.assertIn("--skip-git-repo-check", command)
            self.assertIn("--ephemeral", command)
            self.assertIn("--ignore-user-config", command)
            self.assertEqual(command[command.index("-s") + 1], "read-only")
            self.assertEqual(invoke.call_args_list[1].kwargs["cwd"].name, "workspace")
            self.assertEqual(record["status"], "executed_unassessed")
            self.assertEqual(record["telemetry"]["token_usage"]["total_tokens"], 15)
            self.assertEqual(record["telemetry"]["cost_usage"]["status"], "unavailable")
            self.assertTrue((root / "evidence" / "events.jsonl").is_file())

    def test_missing_usage_or_a_token_overrun_is_ineligible_not_a_pass(self):
        for events in (b'{"type":"turn.completed"}\n', b'{"type":"turn.completed","usage":{"input_tokens":1000,"cached_input_tokens":1,"output_tokens":1}}\n'):
            with tempfile.TemporaryDirectory() as directory:
                root = pathlib.Path(directory); path = plan(root)
                with mock.patch.object(RUNNER.subprocess, "run", side_effect=[
                    subprocess.CompletedProcess(["codex", "--version"], 0, b"codex 1", b""),
                    subprocess.CompletedProcess(["codex", "exec"], 0, events, b""),
                ]):
                    record = RUNNER.run(path, "boolean:bend2:1", root / "evidence")
            self.assertEqual(record["status"], "ineligible")


if __name__ == "__main__":
    unittest.main()
