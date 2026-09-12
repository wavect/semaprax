#!/usr/bin/env python3
"""Offline regression tests for the agent-task-comparison external runner.

Run directly:

    python3 scripts/test-agent-task-comparison-runner.py

Exercises the deterministic fixture runner end to end and the negative cases
required by GitHub issue #104: a candidate agent's declared file edits cannot
escape its sandbox to reach protected trial/oracle state, a candidate cannot
claim its own acceptance outcome, drift is applied exactly once, the
unavailable Zero lane is refused, and the existing (unmodified) observation
validator rejects a missing metric or an altered evidence artifact produced
by this runner. All cases run offline with no model or network access.
"""

import copy
import importlib.util
import json
import os
import shutil
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def _load(name, relative_path):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative_path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner = _load("agent_task_comparison_runner", "scripts/agent-task-comparison-runner.py")
# Reuse the exact module instance the runner already imported (rather than a
# second independent import of the hyphenated sibling script), so classes
# such as agent_task_comparison.Failure compare equal by identity between
# the runner and this test file.
atc = runner.atc

MANIFEST = "benchmarks/agent-task-comparison-v1/manifest.json"
FIXTURE_DIR = ROOT / "benchmarks/agent-task-comparison-v1/fixture-runner"
EVIDENCE_ROOT = ROOT / "benchmarks/agent-task-comparison-v1/evidence"


def task_binding_for(task_id):
    _, _, loaded_tasks = atc.load_manifest(MANIFEST)
    return next(item for item in loaded_tasks if item["id"] == task_id)


class ScratchEvidenceMixin:
    def make_evidence_dir(self):
        # Must resolve inside the repository tree: agent-task-comparison.py's
        # observation/audit commands require a repository-relative ledger
        # path. This directory is gitignored (see its checked-in .gitignore)
        # so nothing generated here is ever committed.
        EVIDENCE_ROOT.mkdir(parents=True, exist_ok=True)
        directory = Path(tempfile.mkdtemp(prefix="atc-runner-test-", dir=str(EVIDENCE_ROOT)))
        self.addCleanup(shutil.rmtree, directory, True)
        return directory


class DeterminismTests(ScratchEvidenceMixin, unittest.TestCase):
    def test_two_runs_of_the_same_tuple_produce_byte_identical_ledger_and_observation(self):
        first = self.make_evidence_dir()
        second = self.make_evidence_dir()
        for evidence_dir in (first, second):
            runner.run(
                MANIFEST,
                "signature-migration-v1",
                "semaprax-graph-operational",
                1,
                "fixture",
                str(FIXTURE_DIR / "signature-migration-v1.semaprax-graph-operational.json"),
                evidence_dir,
            )
        self.assertEqual(
            (first / "ledger.json").read_bytes(), (second / "ledger.json").read_bytes()
        )
        self.assertEqual(
            (first / "observation.json").read_bytes(), (second / "observation.json").read_bytes()
        )

    def test_ledger_totals_equal_an_independent_recount_of_the_transcript(self):
        evidence_dir = self.make_evidence_dir()
        runner.run(
            MANIFEST,
            "signature-migration-v1",
            "semaprax-graph-operational",
            1,
            "fixture",
            str(FIXTURE_DIR / "signature-migration-v1.semaprax-graph-operational.json"),
            evidence_dir,
        )
        observation = json.loads((evidence_dir / "observation.json").read_bytes())
        fixture_script = json.loads(
            (FIXTURE_DIR / "signature-migration-v1.semaprax-graph-operational.json").read_bytes()
        )
        # Recount independently from the checked-in fixture script, without
        # calling any runner internals, and compare against the ledger-derived
        # observation metrics.
        expected = {
            "model_input_tokens": 0, "model_output_tokens": 0, "presented_context_bytes": 0,
            "tool_calls": 0, "tool_request_bytes": 0, "tool_response_bytes": 0,
        }
        for action in fixture_script["actions"]:
            kind = action["kind"]
            if kind == "model_usage":
                expected["model_input_tokens"] += action["input_tokens"]
                expected["model_output_tokens"] += action["output_tokens"]
            elif kind == "context_presentation":
                expected["presented_context_bytes"] += action["bytes"]
            elif kind == "tool_call":
                expected["tool_calls"] += 1
                expected["tool_request_bytes"] += action["request_bytes"]
                expected["tool_response_bytes"] += action["response_bytes"]
        for metric, value in expected.items():
            self.assertEqual(observation["metrics"][metric]["value"], value, metric)


class OwnedOracleTests(unittest.TestCase):
    def _candidate(self):
        task_binding = task_binding_for("owned-signature-migration-v1")
        sandbox, candidate = runner.create_sandbox(task_binding)
        self.addCleanup(shutil.rmtree, sandbox, True)
        before = runner.snapshot_candidate(candidate)
        script = runner.load_fixture_script(
            str(FIXTURE_DIR / "owned-signature-migration-v1.semaprax-graph-operational.json")
        )
        runner.run_fixture_backend(script, candidate, task_binding["id"], None)
        return task_binding, candidate, before, runner.snapshot_candidate(candidate)

    def _compiler_evidence(self):
        commands = {name: {"returncode": 0, "stdout": "", "stderr": ""}
                    for name in ("check", "test", "run", "graph")}
        commands["graph"]["stdout"] = json.dumps({
            "schema": "semaprax.project-semantic-graph.v1",
            "project": "agent-owned-comparison",
            "declarations": [{"id": "benchmark.owned.select", "identity_origin": "explicit"}],
            "edges": [
                {"kind": "call", "caller": "benchmark.owned.main", "target": "benchmark.owned.evaluate"},
                {"kind": "call", "caller": "benchmark.owned.test", "target": "benchmark.owned.evaluate"},
            ],
        })
        return {"available": True, "binary_sha256_before": "sha256:fixture", "binary_sha256_after": "sha256:fixture", "candidate": commands, "baseline": commands}

    def test_owned_oracle_rejects_wrong_owner_even_with_admitted_compiler_evidence(self):
        binding, candidate, before, after = self._candidate()
        core = candidate / "src/core.spx"
        core.write_text(core.read_text().replace("right: own Bytes", "right: borrow Bytes", 1))
        after = runner.snapshot_candidate(candidate)
        with mock.patch.object(runner, "_owned_compiler_evidence", return_value=self._compiler_evidence()):
            rows, _ = runner.check_owned_signature_migration(candidate, binding, before, after, "unused")
        outcomes = {row["id"]: row["outcome"] for row in rows}
        self.assertEqual(outcomes["signature"], "failed")
        self.assertEqual(outcomes["ownership"], "failed")

    def test_owned_oracle_rejects_runtime_regression_against_baseline(self):
        binding, candidate, before, after = self._candidate()
        evidence = self._compiler_evidence()
        evidence["baseline"]["run"]["stdout"] = "41\n"
        with mock.patch.object(runner, "_owned_compiler_evidence", return_value=evidence):
            rows, _ = runner.check_owned_signature_migration(candidate, binding, before, after, "unused")
        self.assertEqual({row["id"]: row["outcome"] for row in rows}["meaning"], "failed")

    def test_owned_oracle_rejects_non_core_authority_mutation(self):
        binding, candidate, before, after = self._candidate()
        app = candidate / "src/app.spx"
        app.write_text(app.read_text() + "\n")
        after = runner.snapshot_candidate(candidate)
        with mock.patch.object(runner, "_owned_compiler_evidence", return_value=self._compiler_evidence()):
            rows, _ = runner.check_owned_signature_migration(candidate, binding, before, after, "unused")
        outcomes = {row["id"]: row["outcome"] for row in rows}
        self.assertEqual(outcomes["authority"], "failed")
        self.assertEqual(outcomes["review"], "failed")


class BoundedCompilerEvidenceTests(unittest.TestCase):
    def test_snapshot_rejects_symlink_directory(self):
        root = Path(tempfile.mkdtemp(prefix="atc-symlink-"))
        self.addCleanup(shutil.rmtree, root, True)
        os.symlink(root, root / "loop")
        with self.assertRaises(runner.RunnerFailure):
            runner.snapshot_candidate(root)

    def test_bounded_command_rejects_output_cap_and_timeout(self):
        loud = runner._bounded_command([sys.executable, "-c", "print('x' * 200000)"], ROOT, limit=1024)
        self.assertIn("output exceeds", loud["error"])
        slow = runner._bounded_command([sys.executable, "-c", "import time; time.sleep(1)"], ROOT, timeout=0.01)
        self.assertEqual(slow["error"], "timeout")


class ProtectionTests(ScratchEvidenceMixin, unittest.TestCase):
    def test_candidate_declared_path_cannot_escape_the_sandbox_to_the_oracle(self):
        task_binding = task_binding_for("signature-migration-v1")
        sandbox, candidate = runner.create_sandbox(task_binding)
        self.addCleanup(shutil.rmtree, sandbox, True)
        oracle_path = ROOT / "benchmarks/agent-task-comparison-v1/tasks/signature-migration.json"
        before = oracle_path.read_bytes()
        traversal = "../../../../../../../../" + str(
            oracle_path.relative_to(ROOT)
        )
        with self.assertRaises(runner.RunnerFailure):
            runner.resolve_write_target(candidate, traversal)
        with self.assertRaises(runner.RunnerFailure):
            runner.resolve_write_target(candidate, str(oracle_path))  # absolute path
        after = oracle_path.read_bytes()
        self.assertEqual(before, after, "the task oracle file must be byte-unchanged")

    def test_candidate_declared_path_cannot_escape_through_a_symlink(self):
        task_binding = task_binding_for("signature-migration-v1")
        sandbox, candidate = runner.create_sandbox(task_binding)
        self.addCleanup(shutil.rmtree, sandbox, True)
        outside = Path(tempfile.mkdtemp(prefix="atc-runner-outside-"))
        self.addCleanup(shutil.rmtree, outside, True)
        escape_link = candidate / "escape"
        os.symlink(outside, escape_link)
        with self.assertRaises(runner.RunnerFailure):
            runner.resolve_write_target(candidate, "escape/oracle.json")

    def test_candidate_cannot_write_the_finalized_ledger_after_the_run_completes(self):
        evidence_dir = self.make_evidence_dir()
        runner.run(
            MANIFEST,
            "signature-migration-v1",
            "semaprax-graph-operational",
            1,
            "fixture",
            str(FIXTURE_DIR / "signature-migration-v1.semaprax-graph-operational.json"),
            evidence_dir,
        )
        ledger_path = evidence_dir / "ledger.json"
        before = ledger_path.read_bytes()
        with self.assertRaises(PermissionError):
            with open(ledger_path, "wb") as handle:
                handle.write(b"tampered")
        self.assertEqual(ledger_path.read_bytes(), before)

    def test_candidate_backend_cannot_claim_its_own_acceptance(self):
        """A "lying" scripted backend that leaves the signature unmigrated but
        declares success anywhere in its output must not influence the
        ledger: acceptance is computed only by the harness's independent
        checker over the resulting files, never read back from the backend.
        """
        task_binding = task_binding_for("signature-migration-v1")
        sandbox, candidate = runner.create_sandbox(task_binding)
        self.addCleanup(shutil.rmtree, sandbox, True)
        lying_script = {
            "schema": runner.FIXTURE_RUNNER_SCHEMA,
            "task": "signature-migration-v1",
            "lane": "semaprax-graph-operational",
            "model": "fixture-stub-v1",
            "tokenizer": "fixture-tokenizer-v1",
            "model_configuration": "deterministic-scripted-replay-v1",
            "harness": "agent-task-comparison-runner-v1-fixture",
            "host": "fixture-offline",
            "toolchain": "n/a-fixture",
            "claimed_outcome": "completed",
            "claimed_acceptance": [
                {"id": criterion, "outcome": "passed"}
                for criterion in ("signature", "identity", "callers", "meaning", "review", "authority")
            ],
            "actions": [
                {"kind": "model_usage", "input_tokens": 1, "output_tokens": 1},
            ],
        }
        events, replayed, drift_applications = runner.run_fixture_backend(
            lying_script, candidate, "signature-migration-v1", None
        )
        # The runner never reads "claimed_outcome" / "claimed_acceptance" at
        # all (build_ledger's signature does not accept them); prove the
        # independent checker alone still reports the true, unmigrated state.
        rows = runner.independent_acceptance("signature-migration-v1", candidate, drift_applications)
        outcomes = {row["id"]: row["outcome"] for row in rows}
        self.assertEqual(outcomes["signature"], "failed")
        self.assertNotEqual(outcomes, {row["id"]: "passed" for row in rows})
        self.assertNotIn("claimed_outcome", json.dumps(runner.build_ledger(
            atc.make_plan("benchmarks/agent-task-comparison-v1/manifest.json"),
            "signature-migration-v1", "semaprax-graph-operational", 1,
            atc.load_manifest(MANIFEST)[0], lying_script, events, replayed, rows,
            task_binding["prompt_sha256"],
        )[0]))


class ZeroLaneAndDriftTests(ScratchEvidenceMixin, unittest.TestCase):
    def test_zero_lane_is_refused(self):
        with self.assertRaises(atc.Failure):
            runner.run(
                MANIFEST,
                "signature-migration-v1",
                "zero-graph-native",
                1,
                "fixture",
                str(FIXTURE_DIR / "signature-migration-v1.semaprax-graph-operational.json"),
                self.make_evidence_dir(),
            )

    def test_drift_is_applied_exactly_once_and_both_lanes_start_from_identical_fixture_bytes(self):
        graph_evidence = self.make_evidence_dir()
        source_evidence = self.make_evidence_dir()
        graph_result = runner.run(
            MANIFEST, "stale-signature-recovery-v1", "semaprax-graph-operational", 1, "fixture",
            str(FIXTURE_DIR / "stale-signature-recovery-v1.semaprax-graph-operational.json"),
            graph_evidence,
        )
        source_result = runner.run(
            MANIFEST, "stale-signature-recovery-v1", "semaprax-source-first", 1, "fixture",
            str(FIXTURE_DIR / "stale-signature-recovery-v1.semaprax-source-first.json"),
            source_evidence,
        )
        self.assertEqual(graph_result["drift_applications"], 1)
        self.assertEqual(source_result["drift_applications"], 1)
        graph_trial = graph_result["trial"]
        source_trial = source_result["trial"]
        self.assertEqual(
            graph_trial["task"]["fixture"], source_trial["task"]["fixture"],
            "both lanes must be bound to identical starting fixture bytes",
        )

    def test_a_fixture_script_that_triggers_drift_twice_is_rejected(self):
        task_binding = task_binding_for("stale-signature-recovery-v1")
        sandbox, candidate = runner.create_sandbox(task_binding)
        self.addCleanup(shutil.rmtree, sandbox, True)
        script = json.loads(
            (FIXTURE_DIR / "stale-signature-recovery-v1.semaprax-graph-operational.json").read_bytes()
        )
        script = copy.deepcopy(script)
        script["actions"].insert(0, {"kind": "identifying_inspection"})
        patch_bytes = (ROOT / task_binding["drift_patch"]["path"]).read_bytes()
        with self.assertRaises(runner.RunnerFailure):
            runner.run_fixture_backend(script, candidate, "stale-signature-recovery-v1", patch_bytes)

    def test_drift_signaled_for_a_task_with_no_drift_patch_is_rejected(self):
        task_binding = task_binding_for("signature-migration-v1")
        sandbox, candidate = runner.create_sandbox(task_binding)
        self.addCleanup(shutil.rmtree, sandbox, True)
        script = {
            "schema": runner.FIXTURE_RUNNER_SCHEMA,
            "task": "signature-migration-v1",
            "lane": "semaprax-graph-operational",
            "model": "x", "tokenizer": "x", "model_configuration": "x",
            "harness": "x", "host": "x", "toolchain": "x",
            "actions": [{"kind": "identifying_inspection"}],
        }
        with self.assertRaises(runner.RunnerFailure):
            runner.run_fixture_backend(script, candidate, "signature-migration-v1", None)


class ExistingValidatorRejectionTests(ScratchEvidenceMixin, unittest.TestCase):
    """These reuse the existing, unmodified agent-task-comparison.py
    validator to confirm it rejects evidence this runner could otherwise
    produce -- the runner adds no leniency of its own.
    """

    def test_missing_metric_status_is_rejected_by_the_existing_observation_validator(self):
        evidence_dir = self.make_evidence_dir()
        runner.run(
            MANIFEST, "signature-migration-v1", "semaprax-graph-operational", 1, "fixture",
            str(FIXTURE_DIR / "signature-migration-v1.semaprax-graph-operational.json"),
            evidence_dir,
        )
        observation_path = evidence_dir / "observation.json"
        os.chmod(observation_path, 0o644)
        value = json.loads(observation_path.read_bytes())
        value["metrics"]["human_interventions"]["status"] = "unavailable"
        observation_path.write_bytes(atc.canonical(value) + b"\n")
        plan = atc.make_plan(MANIFEST)
        manifest, _, _ = atc.load_manifest(MANIFEST)
        with self.assertRaises(atc.Failure):
            atc.load_observation(str(observation_path.relative_to(ROOT)), plan, manifest)

    def test_an_altered_evidence_artifact_is_rejected_by_the_existing_observation_validator(self):
        evidence_dir = self.make_evidence_dir()
        runner.run(
            MANIFEST, "signature-migration-v1", "semaprax-graph-operational", 1, "fixture",
            str(FIXTURE_DIR / "signature-migration-v1.semaprax-graph-operational.json"),
            evidence_dir,
        )
        transcript_path = evidence_dir / "transcript.json"
        os.chmod(transcript_path, 0o644)
        body = bytearray(transcript_path.read_bytes())
        body[-2] ^= 0xFF  # flip one byte inside the recorded transcript
        transcript_path.write_bytes(bytes(body))
        with self.assertRaises(atc.Failure):
            atc.make_observation(MANIFEST, str((evidence_dir / "ledger.json").relative_to(ROOT)), str(evidence_dir / "observation-reject.json"))

    def test_audit_accepts_the_runner_produced_observation_for_its_exact_tuple(self):
        evidence_dir = self.make_evidence_dir()
        runner.run(
            MANIFEST, "signature-migration-v1", "semaprax-graph-operational", 1, "fixture",
            str(FIXTURE_DIR / "signature-migration-v1.semaprax-graph-operational.json"),
            evidence_dir,
        )
        audit = json.loads((evidence_dir / "audit.json").read_bytes())
        self.assertEqual(audit["schema"], "semaprax.agent-task-comparison-audit.v1")
        self.assertEqual(audit["outcome"], "completed")
        self.assertTrue(all(row["outcome"] == "passed" for row in audit["acceptance"]))


if __name__ == "__main__":
    unittest.main()
