"""No-provider #326 accounting controls; never model or independent-review evidence."""
from __future__ import annotations

import contextlib
import copy
import hashlib
import io
import json
import os
import pathlib
import socket
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SUITE = pathlib.Path(__file__).resolve().parents[2]
if str(SUITE) not in sys.path:
    sys.path.insert(0, str(SUITE))

from agent import specialization_accounting as accounting
from agent import specialization_protocol as protocol

EXPECTED_TASKS = [
    "module-import-refactor-v1", "booking-window-conflict-v1",
    "cold-chain-release-gate-v1", "stable-dispatch-order-v1",
    "owned-byte-sentinel-balance-v1", "stale-edit-preservation-v1",
    "telemetry-overflow-diagnosis-v1", "clean-install-calculator-v1",
    "concurrent-delta-merge-v1",
]
HAS_ACQUISITION = accounting.acquisition_available()


class AccountingCapabilityTests(unittest.TestCase):
    def test_missing_nofollow_primitives_refuse_before_reading(self):
        with patch.object(accounting, "acquisition_available", return_value=False), \
             patch.object(accounting.acquisition, "read_regular") as read:
            with self.assertRaisesRegex(accounting.acquisition.Error, "acquisition_unavailable"):
                accounting.report()
            read.assert_not_called()

    def test_capability_detection_requires_dir_fd(self):
        with patch.object(os, "supports_dir_fd", set()):
            self.assertFalse(accounting.acquisition_available())

    def test_capability_detection_requires_every_flag(self):
        for name in ("O_NOFOLLOW", "O_DIRECTORY", "O_CLOEXEC", "O_NONBLOCK"):
            with self.subTest(flag=name), patch.object(os, name, 0, create=True):
                self.assertFalse(accounting.acquisition_available())

    def test_cli_refusal_has_no_partial_stdout_or_ready_claim(self):
        out, err = io.StringIO(), io.StringIO()
        with patch.object(accounting, "acquisition_available", return_value=False), \
             contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            result = accounting.main([])
        self.assertEqual(result, 2)
        self.assertEqual(out.getvalue(), "")
        refusal = json.loads(err.getvalue())
        self.assertEqual(refusal["status"], "unavailable")
        self.assertEqual(refusal["execution"], "not_attempted")
        self.assertIs(refusal["issue_closable"], False)

    def test_cli_accepts_no_execution_authority_or_inventory_override(self):
        for flag in ("--execute", "--approved", "--api-key", "--budget", "--protocol", "--tasks",
                     "--output", "--source-root", "--model", "--result", "--review", "--task"):
            with self.subTest(flag=flag), patch.object(accounting, "report") as report, \
                 contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as error:
                    accounting.main([flag, "untrusted"])
                self.assertEqual(error.exception.code, 2)
                report.assert_not_called()


@unittest.skipUnless(HAS_ACQUISITION, "requires existing POSIX no-follow input acquisition")
class SpecializationAccountingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.document = accounting.report()

    def test_original_denominator_and_order_are_exactly_81(self):
        expected = [(variant, task, repeat) for variant in ("base", "guided", "constrained")
                    for task in EXPECTED_TASKS for repeat in range(1, 4)]
        rows = self.document["rows"]
        self.assertEqual([(row["variant"], row["task"], row["trial"]) for row in rows], expected)
        self.assertEqual(len(rows), 81)
        self.assertEqual(self.document["counts"], {
            "controls": 3, "held_out_tasks": 9, "repeats": 3, "planned_cells": 81,
            "unexecuted_cells": 81, "actual_model_trials": 0, "observed_outcomes": 0})

    def test_all_cells_have_explicit_reasons_and_no_observations(self):
        mandatory = {
            "base_model_not_approved", "controls_not_approved", "adaptation_and_split_not_approved",
            "oracle_and_source_not_approved", "leakage_review_missing",
            "credential_spend_egress_authority_missing", "independent_reviewer_custody_missing",
            "live_transport_unimplemented",
        }
        self.assertEqual({item["id"] for item in self.document["blockers"]}, mandatory)
        for item in self.document["blockers"]:
            self.assertTrue(item["reason"])
            self.assertIsNone(item["resolution_record_sha256"])
        for row in self.document["rows"]:
            self.assertEqual(row["execution"], "not_attempted")
            self.assertEqual(row["classification"], "unexecuted_authorization_and_review_missing")
            self.assertEqual(set(row["reason_ids"]), mandatory)
            self.assertEqual(row["language"], "semaprax-project")
            self.assertEqual(row["split"], "held_out")
            self.assertEqual(row["metrics"], dict.fromkeys(protocol.REQUIRED_METRICS))
            self.assertIsNone(row["trial_wall_ms"])
            self.assertIsNone(row["outcome_artifact_sha256"])

    def test_extra_nine_cells_remain_separate_and_explicit(self):
        extra = self.document["current_example_expansion"]
        self.assertEqual(extra["planned_cells"], 90)
        self.assertEqual(extra["in_original_matrix"], 81)
        self.assertEqual(extra["additional_cells"], 9)
        self.assertIs(extra["authorized_expansion"], False)
        self.assertEqual([(row["variant"], row["task"], row["trial"]) for row in extra["rows"]],
                         [(variant, "iterative-repair-workflow-v1", repeat)
                          for variant in ("base", "guided", "constrained") for repeat in range(1, 4)])
        for row in extra["rows"]:
            self.assertEqual(row["classification"], "outside_original_frozen_matrix")
            self.assertIn("outside_original_frozen_matrix", row["reason_ids"])
            self.assertEqual(row["execution"], "not_attempted")
            self.assertTrue(all(value is None for value in row["metrics"].values()))

    def test_archived_inputs_match_independent_historical_git_blob_ids(self):
        for role, blob in (
            ("frozen_protocol", "457e56e77d68155c562068ede4e1f9c5fecd6a6f"),
            ("frozen_tasks", "ae30aba4943a2e52bf056edcd7bb5f7d680114a8"),
        ):
            data = (SUITE / accounting.INPUTS[role][0]).read_bytes()
            self.assertEqual(hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest(), blob)
        self.assertEqual(self.document["frozen_plan_sha256"],
                         "sha256:cfed39a0f84f0c44dce9d27b978878798fc6ec4733f035bb756a3f3ea65a6ef4")

    def test_source_records_distinguish_retained_path_and_origin(self):
        sources = {row["role"]: row for row in self.document["source_records"]}
        self.assertEqual(sources["frozen_tasks"]["origin_path"], "benchmarks/cross-language-v1/tasks.json")
        self.assertIn("/provenance/", sources["frozen_tasks"]["path"])
        self.assertEqual(sources["frozen_protocol"]["origin_path"],
                         "benchmarks/cross-language-v1/agent/specialization-protocol.example.json")
        self.assertEqual(sources["frozen_tasks"]["source_commit"],
                         "ebe4235ebc2b9621f7fd174f8bf7647744dc88c5")
        self.assertEqual(sources["current_tasks"]["source_commit"],
                         "6a7a341273c30b81cf53f3fa9ef94dc7930b4c72")
        self.assertNotEqual(sources["frozen_tasks"]["sha256"], sources["current_tasks"]["sha256"])

    def test_original_and_current_metadata_denominators_preserve_exclusions(self):
        original = {row["task"]: row for row in self.document["original_task_inventory"]}
        current = {row["task"]: row for row in self.document["current_task_inventory"]}
        self.assertEqual(len(original), 12)
        self.assertEqual(len(current), 13)
        self.assertEqual(original["sequence-digest-v1"]["selection"], "not_held_out")
        self.assertEqual(original["structured-input-error-handling-v1"]["selection"], "not_held_out")
        self.assertEqual(original["bounded-counter-repair-v1"]["selection"], "no_declared_evaluation_language")
        self.assertEqual(current["iterative-repair-workflow-v1"]["selection"], "outside_original_matrix")
        self.assertEqual([row["task"] for row in original.values() if row["selection"] == "in_original_matrix"],
                         EXPECTED_TASKS)

    def test_oracle_drift_is_reported_not_repaired_or_approved(self):
        oracle = self.document["oracle"]
        self.assertEqual(oracle["frozen_proposal"]["sha256"],
                         "sha256:4fda29764163f6c4d1e8e16a8b9afaa3d9f7af10aa16fd2b32fed03f12fa02f2")
        self.assertEqual(oracle["current_runner_sha256"],
                         "sha256:29b9955133f70efabf9f240f5254a5654e0aec21ef7acf4e12b975b45322d5aa")
        self.assertIs(oracle["current_matches_frozen"], False)
        self.assertEqual(oracle["status"], "not_approved_requires_source_and_oracle_review")

    def test_authorization_remains_missing_and_fixture_model_is_only_a_proposal(self):
        self.assertEqual(self.document["proposed_base_model"],
                         {"provider": "offline-fixture", "model": "fixture-model", "revision": "fixture-2026-09-21"})
        self.assertEqual(self.document["authorization"]["status"], "not_authorized")
        self.assertIsNone(self.document["authorization"]["record_sha256"])
        self.assertEqual(self.document["proposed_resource_policy"]["max_cost_usd"], 0)
        self.assertEqual([row["id"] for row in self.document["proposed_variants"]],
                         ["base", "guided", "constrained"])

    def test_adaptation_is_not_fabricated_or_extended_to_held_out(self):
        adaptation = self.document["adaptation"]
        self.assertEqual(adaptation["status"], "not_approved")
        self.assertIs(adaptation["arm_in_original_matrix"], False)
        self.assertIsNone(adaptation["method"])
        self.assertIsNone(adaptation["dataset_manifest_sha256"])
        self.assertEqual(adaptation["owner_declared_development_tasks"], ["sequence-digest-v1"])

    def test_unobserved_controls_have_no_numeric_effect_or_uncertainty(self):
        for row in self.document["control_summary"]:
            self.assertEqual(row["planned_cells"], 27)
            self.assertEqual(row["unexecuted_cells"], 27)
            self.assertEqual(row["actual_model_trials"], 0)
            self.assertEqual(row["status"], "not_estimable")
            for field in ("accepted_outcome_rate", "mean_cost_usd", "mean_trial_wall_ms"):
                self.assertIsNone(row[field])
        comparisons = self.document["control_comparisons"]
        self.assertEqual(len(comparisons), 3)
        for row in comparisons:
            self.assertEqual(row["paired_model_trials"], 0)
            self.assertEqual(row["status"], "not_estimable")
            self.assertIsNone(row["effect"])
            self.assertIsNone(row["uncertainty"])

    def test_missing_reviews_and_custody_are_not_self_attested(self):
        for row in self.document["independent_reviews"].values():
            self.assertEqual(row, {"status": "not_recorded", "reviewer": None,
                                   "independence_verified": False, "record_sha256": None})
        self.assertEqual(self.document["data_custody"],
                         {"status": "unassigned", "custodian": None, "record_sha256": None})
        self.assertIs(self.document["acceptance"]["independent_control_comparison_recorded"], False)
        self.assertIs(self.document["acceptance"]["independent_leakage_review_recorded"], False)

    def test_accounting_does_not_claim_experiment_completion(self):
        self.assertEqual(self.document["record_kind"], "unexecuted_cell_accounting")
        self.assertEqual(self.document["status"], "blocked_not_authorized")
        self.assertEqual(self.document["execution"], "not_attempted")
        self.assertIs(self.document["is_model_result"], False)
        self.assertIs(self.document["issue_closable"], False)
        self.assertIs(self.document["acceptance"]["every_original_cell_classified"], True)
        self.assertIs(self.document["acceptance"]["approved_matrix_executed"], False)

    def test_report_is_deterministic_and_within_unchanged_result_bound(self):
        data = protocol.canonical_bytes(self.document)
        self.assertEqual(data, protocol.canonical_bytes(accounting.report()))
        self.assertLessEqual(len(data), accounting.v1.MAX_RESULT_BYTES)

    def test_oversized_report_is_refused(self):
        with patch.object(accounting.v1, "MAX_RESULT_BYTES", 1):
            with self.assertRaisesRegex(accounting.acquisition.Error, "result_exceeds_bound"):
                accounting.report()

    def test_no_hidden_task_bytes_provider_or_subprocess_are_accessed(self):
        reader = accounting.acquisition.read_regular
        seen = []
        expected = {SUITE / relative for relative, _, _ in accounting.INPUTS.values()}

        def only_allowed(path, bound):
            self.assertIn(path, expected)
            self.assertNotIn("hidden", path.parts)
            seen.append(path)
            return reader(path, bound)

        with patch.object(accounting.acquisition, "read_regular", side_effect=only_allowed), \
             patch.object(pathlib.Path, "read_bytes", side_effect=AssertionError("no reader bypass")), \
             patch.object(pathlib.Path, "read_text", side_effect=AssertionError("no reader bypass")), \
             patch.object(subprocess, "Popen", side_effect=AssertionError("no process authority")), \
             patch.object(socket, "socket", side_effect=AssertionError("no network authority")), \
             patch.object(os, "getenv", side_effect=AssertionError("no environment credential lookup")):
            result = accounting.report()
        self.assertEqual(seen, list(SUITE / relative for relative, _, _ in accounting.INPUTS.values()))
        self.assertEqual(result, self.document)

    def test_every_input_digest_is_checked_before_plan_construction(self):
        reader = accounting.acquisition.read_regular
        for role, (relative, _, _) in accounting.INPUTS.items():
            def tampered(path, limit):
                data = reader(path, limit)
                return (b"x" + data[1:]) if path == SUITE / relative else data
            with self.subTest(role=role), \
                 patch.object(accounting.acquisition, "read_regular", side_effect=tampered), \
                 patch.object(protocol, "build_plan") as builder:
                with self.assertRaisesRegex(accounting.acquisition.Error, "input_drift:" + role):
                    accounting.report()
                builder.assert_not_called()

    def test_input_length_is_not_trusted_even_from_reader(self):
        reader = accounting.acquisition.read_regular
        with patch.object(accounting.acquisition, "read_regular",
                          side_effect=lambda path, limit: reader(path, limit) + b" "):
            with self.assertRaisesRegex(accounting.acquisition.Error, "input_drift"):
                accounting.report()

    def test_both_original_and_current_plan_bytes_are_authenticated(self):
        builder = protocol.build_plan
        for target in (81, 90):
            def changed(*args):
                plan = builder(*args)
                if len(plan["rows"]) == target:
                    plan["rows"][0]["execution"] = "executed"
                return plan
            with self.subTest(rows=target), patch.object(protocol, "build_plan", side_effect=changed):
                with self.assertRaisesRegex(accounting.acquisition.Error, "plan_drift"):
                    accounting.report()

    def test_actual_cli_is_nonzero_blocked_not_a_successful_model_gate(self):
        result = subprocess.run([sys.executable, str(SUITE / "agent/specialization_accounting.py")],
                                capture_output=True, timeout=15,
                                env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1"))
        self.assertEqual(result.returncode, 3, result.stderr.decode())
        self.assertEqual(result.stderr, b"")
        self.assertEqual(json.loads(result.stdout), self.document)

    def test_leaf_and_ancestor_symlinks_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory).resolve()
            real = root / "real"
            real.mkdir()
            file = real / "input.json"
            file.write_bytes(b"{}")
            (root / "leaf").symlink_to(file)
            (root / "parent").symlink_to(real, target_is_directory=True)
            for path in (root / "leaf", root / "parent/input.json"):
                with self.subTest(path=path), self.assertRaises(accounting.acquisition.Error):
                    accounting.acquisition.read_regular(path, 2)

    def test_nonregular_and_oversized_inputs_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory).resolve()
            file = root / "large"
            file.write_bytes(b"abc")
            for path in (root, file):
                with self.subTest(path=path), self.assertRaises(accounting.acquisition.Error):
                    accounting.acquisition.read_regular(path, 2)

    def test_cli_has_empty_stdout_on_input_drift(self):
        out, err = io.StringIO(), io.StringIO()
        with patch.object(accounting.acquisition, "read_regular", return_value=b"{}"), \
             contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            result = accounting.main([])
        self.assertEqual(result, 2)
        self.assertEqual(out.getvalue(), "")
        self.assertEqual(json.loads(err.getvalue())["status"], "unavailable")

    def test_current_input_changes_do_not_silently_repin_the_original(self):
        reader = accounting.acquisition.read_regular
        frozen_bytes = (SUITE / accounting.INPUTS["frozen_tasks"][0]).read_bytes()
        def replace_current(path, bound):
            return frozen_bytes if path == SUITE / "tasks.json" else reader(path, bound)
        with patch.object(accounting.acquisition, "read_regular", side_effect=replace_current):
            with self.assertRaisesRegex(accounting.acquisition.Error, "input_drift:current_tasks"):
                accounting.report()

    def test_environment_values_cannot_authorize_or_supply_model_results(self):
        with patch.dict(os.environ, {
            "SEMAPRAX_SPECIALIZATION_AUTHORIZED": "true",
            "MODEL_API_KEY": "synthetic-noncredential-control",
            "SEMAPRAX_SPECIALIZATION_BUDGET": "10000",
        }):
            self.assertEqual(accounting.report(), self.document)

    def test_fifo_input_is_refused_without_blocking(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory).resolve() / "fifo"
            os.mkfifo(path)
            with self.assertRaises(accounting.acquisition.Error):
                accounting.acquisition.read_regular(path, 2)

    def test_pending_review_objects_do_not_alias(self):
        doc = copy.deepcopy(self.document)
        doc["independent_reviews"]["leakage"]["status"] = "synthetic-test-value"
        self.assertEqual(doc["independent_reviews"]["control_comparison"]["status"], "not_recorded")


if __name__ == "__main__":
    unittest.main()
