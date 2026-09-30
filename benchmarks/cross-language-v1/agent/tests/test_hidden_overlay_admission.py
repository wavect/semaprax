"""Regression contract for hidden overlay admission (issue #211).

These tests deliberately use tiny Python adapters and an in-process transport;
they need no compiler, credentials, network, or model service.
"""
from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

_SUITE = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_SUITE))

from agent.contracts import (
    Budget, ModelIdentity, PricingRates, SamplingParams, SolverRequest,
    SolverResponse, Usage,
)
from agent.orchestrator import evaluate_agent_pair
from agent._harness import run_module


class CountingTransport:
    def __init__(self, candidate=None):
        self.calls = 0
        self.candidate = candidate or {"candidate.py": "print('candidate')\n"}

    def complete(self, request):
        self.calls += 1
        return SolverResponse(self.candidate, Usage(), [])


class HiddenOverlayAdmissionTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="hidden-overlay-")
        self.root = pathlib.Path(self.tmp.name)
        self.public = self.root / "public" / "mock"
        self.hidden = self.root / "hidden" / "mock"
        self.public.mkdir(parents=True)
        self.hidden.mkdir(parents=True)
        self.public.joinpath("prog.py").write_text("import sys\nsys.exit(0)\n")
        self.task = {
            "id": "overlay-task",
            "category": "greenfield",
            "languages": {"mock": {"public": "public/mock", "hidden": "hidden/mock"}},
        }
        self.adapter = {
            "implemented": True,
            "version_command": [sys.executable, "--version"],
            "run_command": [sys.executable, "prog.py"],
            "success": {"kind": "exit_code_zero"},
        }

    def tearDown(self):
        self.tmp.cleanup()

    def fixed(self):
        return run_module().evaluate_pair(self.root, self.task, "mock", self.adapter, sys.executable)

    def request(self):
        return SolverRequest(
            "overlay-task", "mock", "prompt",
            ModelIdentity("fixture", "model", "rev-1"),
            SamplingParams(0, 1, 1, 100),
            Budget(1000, 1000, 1000, 1, 1), PricingRates(),
        )

    def agent(self, transport=None):
        return evaluate_agent_pair(
            self.root, self.task, "mock", self.adapter,
            transport or CountingTransport(), self.request(),
            sys.executable, ["candidate.py"],
        )

    def assert_admission_failure(self, setup, reason):
        setup()
        for name, method in (("fixed", self.fixed), ("agent", self.agent)):
            with self.subTest(method=name):
                transport = CountingTransport()
                run = run_module()
                with mock.patch.object(run, "tool_version", side_effect=AssertionError("version called")), \
                     mock.patch.object(run, "stage", side_effect=AssertionError("stage called")), \
                     mock.patch.object(run, "scratch_dir", side_effect=AssertionError("scratch called")):
                    result = method() if name == "fixed" else self.agent(transport)
                self.assertEqual(result["status"], "failed")
                self.assertTrue(result["reason"].startswith(reason), result)
                self.assertNotIn("public", result)
                self.assertNotIn("hidden", result)
                self.assertNotIn("leak_check", result)
                self.assertNotIn("provenance", result)
                for field in ("usage", "evidence", "transcript", "candidate"):
                    self.assertNotIn(field, result)
                if name == "agent":
                    self.assertEqual(transport.calls, 0)

    def test_T01_missing_hidden_fails_before_execution(self):
        self.assert_admission_failure(lambda: self.hidden.rmdir(), "missing hidden directory:")

    def test_T02_file_instead_of_hidden_directory(self):
        def setup():
            self.hidden.rmdir()
            self.hidden.write_text("file")
        self.assert_admission_failure(setup, "missing hidden directory:")

    def test_T03_empty_hidden_fails(self):
        self.assert_admission_failure(lambda: None, "empty hidden overlay:")

    def test_T04_only_empty_nested_directories_fails(self):
        self.assert_admission_failure(lambda: (self.hidden / "nested").mkdir(), "empty hidden overlay:")

    def test_T05_identical_bytes_with_different_mtime_fails(self):
        def setup():
            target = self.hidden / "prog.py"
            target.write_bytes((self.public / "prog.py").read_bytes())
            os.utime(target, (1, 2))
        self.assert_admission_failure(setup, "hidden overlay adds no changes:")

    def test_T06_changed_same_path_is_admitted(self):
        (self.hidden / "prog.py").write_text("# harmless overlay\nimport sys\nsys.exit(0)\n")
        self.assertEqual(self.fixed()["status"], "ok")
        transport = CountingTransport()
        self.assertEqual(self.agent(transport)["status"], "ok")
        self.assertEqual(transport.calls, 1)

    def test_T07_new_nested_path_is_admitted_to_overlay_check(self):
        (self.hidden / "nested").mkdir()
        (self.hidden / "nested/extra.txt").write_text("hidden replacement")
        result = self.fixed()
        self.assertEqual(result["status"], "ok")
        self.assertTrue(result["public"]["passed"])
        self.assertTrue(result["hidden"]["passed"])
        transport = CountingTransport()
        result = self.agent(transport)
        self.assertEqual(result["status"], "ok")
        self.assertTrue(result["public"]["passed"])
        self.assertTrue(result["hidden"]["passed"])
        self.assertEqual(transport.calls, 1)

    def test_T08_mixed_identical_and_changed_files_is_admitted(self):
        (self.public / "same.txt").write_text("same")
        (self.hidden / "same.txt").write_text("same")
        (self.hidden / "prog.py").write_text("# changed\nimport sys\nsys.exit(0)\n")
        result = self.fixed()
        self.assertEqual(result["status"], "ok")
        self.assertTrue(result["public"]["passed"])
        self.assertTrue(result["hidden"]["passed"])
        transport = CountingTransport()
        result = self.agent(transport)
        self.assertEqual(result["status"], "ok")
        self.assertTrue(result["public"]["passed"])
        self.assertTrue(result["hidden"]["passed"])
        self.assertEqual(transport.calls, 1)

    def test_T09_public_pass_hidden_fail_is_preserved(self):
        (self.hidden / "prog.py").write_text("import sys\nsys.exit(1)\n")
        result = self.fixed()
        self.assertEqual(result["status"], "failed")
        self.assertTrue(result["public"]["passed"])
        self.assertFalse(result["hidden"]["passed"])
        self.public.joinpath("prog.py").write_text("import candidate\nassert candidate.VALUE == 7\n")
        self.hidden.joinpath("prog.py").write_text("import candidate\nassert candidate.VALUE == 99\n")
        transport = CountingTransport({"candidate.py": "VALUE = 7\n"})
        result = self.agent(transport)
        self.assertEqual(result["status"], "failed")
        self.assertTrue(result["public"]["passed"])
        self.assertFalse(result["hidden"]["passed"])

    def test_T10_both_phases_success_have_provenance(self):
        (self.hidden / "prog.py").write_text("# changed\nimport sys\nsys.exit(0)\n")
        result = self.fixed()
        self.assertEqual(result["status"], "ok")
        self.assertIn("provenance", result)

    def test_T11_agent_invalid_overlay_does_not_call_transport(self):
        transport = CountingTransport()
        result = self.agent(transport)
        self.assertEqual(result["status"], "failed")
        self.assertEqual(transport.calls, 0)
        self.assertNotIn("public", result)

    def test_T12_agent_invalid_overlay_precedes_version_and_stage(self):
        transport = CountingTransport()
        with mock.patch.object(run_module(), "tool_version", side_effect=AssertionError("version called")), mock.patch.object(run_module(), "stage", side_effect=AssertionError("stage called")):
            result = self.agent(transport)
        self.assertEqual(result["status"], "failed")
        self.assertEqual(transport.calls, 0)

    def test_T13_dry_run_rejects_empty_overlay_but_keeps_exists(self):
        tasks = self.root / "tasks.json"
        adapters = self.root / "adapters.json"
        tasks.write_text(json.dumps({"schema": "benchmark.cross_language.tasks.v1", "tasks": [self.task]}))
        adapters.write_text(json.dumps({"schema": "benchmark.cross_language.adapters.v1", "adapters": [{"id": "mock", **self.adapter}]}))
        for label, setup, reason in (
            ("missing", lambda: self.hidden.rmdir(), "missing hidden directory:"),
            ("empty", lambda: self.hidden.mkdir(parents=True, exist_ok=True), "empty hidden overlay:"),
            ("noop", lambda: (self.hidden / "prog.py").write_bytes((self.public / "prog.py").read_bytes()), "hidden overlay adds no changes:"),
        ):
            with self.subTest(label=label):
                setup()
                out = self.root / ("plan-" + label + ".json")
                run = subprocess.run([sys.executable, str(_SUITE / "run.py"), "--root", str(self.root), "--tasks", str(tasks), "--adapters", str(adapters), "--output", str(out), "--dry-run"], capture_output=True, text=True)
                self.assertEqual(run.returncode, 1)
                plan = json.loads(out.read_text())
                self.assertEqual(plan["pairs"][0]["exists"], label != "missing")
                self.assertIn(reason, run.stderr)
                if label == "noop":
                    (self.hidden / "prog.py").unlink()
                if label == "missing":
                    self.hidden.mkdir(parents=True)

    def test_T14_cli_invalid_overlay_reports_failed_json(self):
        tasks = self.root / "tasks.json"
        adapters = self.root / "adapters.json"
        tasks.write_text(json.dumps({"schema": "benchmark.cross_language.tasks.v1", "tasks": [self.task]}))
        adapters.write_text(json.dumps({"schema": "benchmark.cross_language.adapters.v1", "adapters": [{"id": "mock", **self.adapter}]}))
        for label, setup, reason in (
            ("missing", lambda: self.hidden.rmdir(), "missing hidden directory:"),
            ("empty", lambda: self.hidden.mkdir(parents=True, exist_ok=True), "empty hidden overlay:"),
            ("noop", lambda: (self.hidden / "prog.py").write_bytes((self.public / "prog.py").read_bytes()), "hidden overlay adds no changes:"),
        ):
            with self.subTest(label=label):
                setup()
                out = self.root / ("result-" + label + ".json")
                run = subprocess.run([sys.executable, str(_SUITE / "run.py"), "--root", str(self.root), "--tasks", str(tasks), "--adapters", str(adapters), "--output", str(out)], capture_output=True, text=True)
                self.assertEqual(run.returncode, 1)
                self.assertNotIn("Traceback", run.stderr)
                document = json.loads(out.read_text())
                self.assertEqual(document["summary"]["failed"], 1)
                self.assertEqual(document["summary"]["ok"], 0)
                self.assertTrue(document["results"][0]["reason"].startswith(reason))
                if label == "noop":
                    (self.hidden / "prog.py").unlink()
                if label == "missing":
                    self.hidden.mkdir(parents=True)

    def test_T15_agent_missing_hidden_has_no_evidence(self):
        self.hidden.rmdir()
        tasks = self.root / "tasks.json"
        adapters = self.root / "adapters.json"
        fixture = self.root / "replay.json"
        out = self.root / "agent-result.json"
        tasks.write_text(json.dumps({"schema": "benchmark.cross_language.tasks.v1", "tasks": [self.task]}))
        adapters.write_text(json.dumps({"schema": "benchmark.cross_language.adapters.v1", "adapters": [{"id": "mock", **self.adapter}]}))
        fixture.write_text(json.dumps({
            "schema": "benchmark.cross_language.agent.replay_fixture.v1",
            "attempts": [{"attempt": 1, "kind": "final", "prompt_tokens": 1,
                          "completion_tokens": 1, "cost_usd": 0,
                          "response_text": "candidate", "candidate_files": {"candidate.py": "VALUE = 7\n"}}],
        }))
        command = [sys.executable, str(_SUITE / "agent/run_agent.py"),
                   "--task", "overlay-task", "--language", "mock",
                   "--candidate-path", "candidate.py", "--output", str(out),
                   "--root", str(self.root), "--tasks", str(tasks),
                   "--adapters", str(adapters), "--transport", "replay",
                   "--fixture", str(fixture), "--model-provider", "fixture",
                   "--model-name", "mock", "--model-revision", "rev-1",
                   "--temperature", "0", "--top-p", "1", "--seed", "1",
                   "--max-output-tokens", "100", "--max-prompt-tokens", "1000",
                   "--max-completion-tokens", "1000", "--max-total-tokens", "1000",
                   "--max-retries", "1", "--max-cost-usd", "1"]
        run = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(run.returncode, 1)
        self.assertNotIn("Traceback", run.stderr)
        result = json.loads(out.read_text())["result"]
        self.assertEqual(result["status"], "failed")
        self.assertTrue(result["reason"].startswith("missing hidden directory:"))
        self.assertNotIn("public", result)
        self.assertNotIn("evidence", result)

    def test_T16_wrong_expected_digest_remains_drifted_for_valid_overlay(self):
        (self.hidden / "prog.py").write_text("# changed\nimport sys\nsys.exit(0)\n")
        self.task["languages"]["mock"]["expected_digest"] = "sha256:" + "0" * 64
        self.assertEqual(self.fixed()["status"], "drifted")

    def test_T17_unimplemented_adapter_remains_blocked(self):
        self.adapter["implemented"] = False
        self.adapter["blocked_reason"] = "reserved"
        self.assertEqual(self.fixed()["status"], "blocked")

    def test_T18_observed_oserror_fails_closed(self):
        (self.hidden / "prog.py").write_text("changed")
        real_read = pathlib.Path.read_bytes
        def broken(path):
            if path == self.hidden / "prog.py":
                raise OSError("denied")
            return real_read(path)
        with mock.patch.object(pathlib.Path, "read_bytes", broken):
            result = self.fixed()
        self.assertEqual(result["status"], "failed")
        self.assertTrue(result["reason"].startswith("cannot inspect hidden overlay:"), result)
        transport = CountingTransport()
        with mock.patch.object(pathlib.Path, "read_bytes", broken):
            result = self.agent(transport)
        self.assertEqual(result["status"], "failed")
        self.assertTrue(result["reason"].startswith("cannot inspect hidden overlay:"), result)
        self.assertEqual(transport.calls, 0)

    def test_T19_budget_terminal_status_is_preserved_for_valid_overlay(self):
        from agent.budget import BudgetExceededError
        class Bomb(CountingTransport):
            def complete(self, request):
                self.calls += 1
                raise BudgetExceededError("budget", Usage(), request.budget)
        (self.hidden / "prog.py").write_text("# changed\nimport sys\nsys.exit(0)\n")
        result = self.agent(Bomb())
        self.assertEqual(result["status"], "budget_exceeded")


if __name__ == "__main__":
    unittest.main()
