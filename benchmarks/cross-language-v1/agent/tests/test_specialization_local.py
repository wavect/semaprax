"""No-model local-experiment tests. Fake reviews/scorers are test inputs only."""
from __future__ import annotations
import contextlib
import copy
import io
import json
import os
import pathlib
import re
import sys
import tempfile
import unittest
from unittest.mock import patch

SUITE = pathlib.Path(__file__).resolve().parents[2]
if str(SUITE) not in sys.path: sys.path.insert(0, str(SUITE))
TESTS = pathlib.Path(__file__).resolve().parent
if str(TESTS) not in sys.path: sys.path.insert(0, str(TESTS))
from test_local_ollama import FakeDaemon, NAME
from agent import specialization_local as local
from agent import specialization_inputs as inputs
from agent import specialization_native as native
from agent.local_ollama import Client, LocalTransportError, canonical, digest


class FakeScorer:
    """Never executes source. Used only to test orchestration and evidence I/O."""
    def __init__(self, *_):
        self.calls = 0
    def __enter__(self): return self
    def close(self): pass
    def score(self, task, files):
        self.calls += 1
        return {"result": {"status": "ok", "leak_check": "ok", "public": {"passed": True}, "hidden": {"passed": True}},
                "commands": [{"fixture": True, "task": task, "source_paths": sorted(files)}]}


def fixture_review(plan_hash, operator):
    review = local.review_template(plan_hash, operator)
    review.update(decision="approved", reviewer="fixture-independent-reviewer",
                  data_custodian="fixture-data-custodian", reviewed_at="2000-01-01T00:00:00Z",
                  independence_statement="Synthetic unit-test input; not an actual review.",
                  notes="Synthetic unit-test input; never real authorization.")
    for check in review["checks"].values():
        check.update(passed=True, evidence="Synthetic review fixture; no actual verification.")
    return review


@unittest.skipUnless(local.accounting.acquisition_available(), "POSIX no-follow acquisition required")
class LocalSpecializationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        _, cls.sources = local.provenance.source_snapshot()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="spx-local-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name).resolve()
        self.compiler = self.root / "semaprax-fixture"
        self.compiler.write_text("fixture bytes; never executed\n")
        self.compiler.chmod(0o700)

    def prepare(self, daemon):
        return local.prepare(self.compiler, daemon.endpoint, NAME, "fixture-operator")

    def files(self, plan):
        plan_path, review_path = self.root / "plan.json", self.root / "review.json"
        plan_hash = local.write_json(plan_path, plan)
        review = fixture_review(plan_hash, plan["operator"])
        review_hash = local.write_json(review_path, review)
        return plan_path, plan_hash, review_path, review_hash, self.root / "run"

    def model_files_from_prompt(self, daemon):
        def update(handler):
            if handler.path != "/api/generate": return False
            body = daemon.generations[-1]
            paths = json.loads(re.search(r"exactly these relative files as JSON strings: (\[[^\n]*?\])\.", body["prompt"]).group(1))
            daemon.reply["response"] = json.dumps({"files": {path: "synthetic candidate" for path in paths}})
            return False
        daemon.override = update

    def test_source_inventory_is_still_298_pinned_files_and_all_182_rows(self):
        manifest, source = local.provenance.source_snapshot()
        self.assertEqual(len(source), 298)
        self.assertEqual(len(manifest["comparison_inventory"]), 182)
        self.assertEqual(source, self.sources)

    def test_model_projection_excludes_hidden_equivalence_and_candidate_bodies(self):
        source = dict(self.sources)
        task = "cold-chain-release-gate-v1"
        prefix = inputs.PREFIX + "tasks/" + task + "/"
        source[prefix + "EQUIVALENCE.md"] = b"DO_NOT_SEND_EQUIVALENCE_SENTINEL"
        source[prefix + "hidden/semaprax/secret"] = b"DO_NOT_SEND_HIDDEN_SENTINEL"
        source[prefix + "public/semaprax/README.md"] = b"DO_NOT_SEND_README_SENTINEL"
        path = prefix + "public/semaprax/src/candidate.spx"
        source[path] = source[path].replace(b"{", b"{\n// DO_NOT_SEND_REFERENCE_BODY_SENTINEL", 1)
        prompt, projection = inputs.model_prompt(task, "guided", source)
        self.assertNotIn("DO_NOT_SEND", prompt)
        self.assertTrue(all("/public/semaprax/" in r["path"] for r in projection))
        self.assertIn("[2, 8]", prompt)
        self.assertIn("[95, 105]", prompt)
        self.assertNotIn("EQUIVALENCE", prompt)
        self.assertIn("fn ", prompt)
        self.assertEqual(next(r["projection"] for r in projection if r["path"] == path), "interface_only")

    def test_all_admitted_task_prompts_keep_guidance_and_constraint_controls_separate(self):
        for task in local.accounting.FROZEN_TASK_IDS:
            if task in inputs.UNAVAILABLE: continue
            with self.subTest(task=task):
                base, _ = inputs.model_prompt(task, "base", self.sources)
                guided, _ = inputs.model_prompt(task, "guided", self.sources)
                constrained, _ = inputs.model_prompt(task, "constrained", self.sources)
                self.assertEqual(guided, constrained)
                self.assertNotIn(inputs.GUIDANCE, base)
                self.assertIn(inputs.GUIDANCE, guided)
                self.assertNotIn("/hidden/", base)
                self.assertLess(len(guided.encode()), 30 * 1024)

    def test_unknown_task_control_and_unreviewed_discount_refuse(self):
        for task, control in (("iterative-repair-workflow-v1", "base"),
                              ("stale-edit-preservation-v1", "base"),
                              ("cold-chain-release-gate-v1", "adapted")):
            with self.subTest(task=task), self.assertRaises(LocalTransportError):
                inputs.model_prompt(task, control, self.sources)

    def test_unsupported_interface_shape_is_not_loosely_parsed(self):
        with self.assertRaises(LocalTransportError):
            inputs.public_surface(b'module x;\nfn unbound() { return 0; }')

    def test_prepare_is_metadata_only_and_preserves_81_slots_and_old_proposal(self):
        with FakeDaemon() as daemon:
            plan = self.prepare(daemon)
            self.assertEqual(len(plan["plan"]["rows"]), 81)
            self.assertEqual(len(plan["prompts"]), 24)
            self.assertEqual(len(plan["excluded_tasks"]), 1)
            self.assertEqual(plan["proposal"]["authorization"]["status"], "not_authorized")
            self.assertEqual(plan["status"], "pending_independent_preflight_review")
            self.assertFalse(plan["issue_closable"])
            self.assertFalse(daemon.generations)
            self.assertLess(len(canonical(plan)), local.bounds.MAX_RESULT_BYTES)
            self.assertNotEqual(plan["oracle_change"]["original_proposal"]["sha256"],
                                plan["oracle_change"]["local_candidate"]["sha256"])
            frozen = local.snapshot()[-1]
            self.assertEqual([local.accounting._cell_key(r) for r in plan["plan"]["rows"]],
                             [local.accounting._cell_key(r) for r in frozen["rows"]])
            self.assertEqual(plan["plan"]["resource_policy"], frozen["resource_policy"])
            self.assertEqual({r["variant"] for r in plan["plan"]["rows"]}, {"base", "guided", "constrained"})

    def test_prepare_roundtrip_verifies_and_tampered_budget_is_rejected(self):
        with FakeDaemon() as daemon:
            plan = json.loads(canonical(self.prepare(daemon)))
            source, built = local.verify_plan(plan)
            self.assertEqual(source, self.sources)
            self.assertEqual(len(built["rows"]), 81)
            plan["sampling"]["max_output_tokens"] += 1
            with self.assertRaisesRegex(LocalTransportError, "prepared_plan_or_runtime_drift"):
                local.verify_plan(plan)
            self.assertFalse(daemon.generations)

    def test_prepared_input_and_model_drift_refuse(self):
        with FakeDaemon() as daemon:
            plan = self.prepare(daemon)
            changed = copy.deepcopy(plan)
            changed["runtime_files"][0]["sha256"] = "sha256:" + "0" * 64
            with self.assertRaisesRegex(LocalTransportError, "prepared_input_identity_drift"):
                local.verify_plan(changed)
            daemon.model["digest"] = "b" * 64
            with self.assertRaisesRegex(LocalTransportError, "prepared_plan_or_runtime_drift"):
                local.verify_plan(plan)
            self.assertFalse(daemon.generations)

    def test_model_prompt_and_source_pins_are_not_caller_overrides(self):
        with FakeDaemon() as daemon:
            plan = self.prepare(daemon)
            for key in ("prompts", "source_manifest_sha256", "excluded_tasks", "original_plan_sha256"):
                changed = copy.deepcopy(plan)
                changed[key] = [] if key == "prompts" else "changed"
                with self.subTest(key=key), self.assertRaises(LocalTransportError):
                    local.verify_plan(changed)
            self.assertFalse(daemon.generations)

    def test_review_template_cannot_be_executed_or_self_approved(self):
        phash = "sha256:" + "a" * 64
        pending = local.review_template(phash, "fixture-operator")
        with self.assertRaises(LocalTransportError): local.validate_review(pending, phash, "fixture-operator")
        approved = fixture_review(phash, "fixture-operator")
        local.validate_review(approved, phash, "fixture-operator")
        for name in ("fixture-operator", "FIXTURE-OPERATOR", "implementation-assistant"):
            approved["reviewer"] = name
            with self.subTest(name=name), self.assertRaisesRegex(LocalTransportError, "not_independent"):
                local.validate_review(approved, phash, "fixture-operator")

    def test_each_preflight_check_and_review_subject_is_required(self):
        phash = "sha256:" + "a" * 64
        for name in local.PRECHECKS:
            record = fixture_review(phash, "fixture-operator")
            record["checks"][name]["passed"] = False
            with self.subTest(name=name), self.assertRaises(LocalTransportError):
                local.validate_review(record, phash, "fixture-operator")
        record = fixture_review(phash, "fixture-operator")
        record["subject_sha256"] = "sha256:" + "b" * 64
        with self.assertRaises(LocalTransportError): local.validate_review(record, phash, "fixture-operator")

    def test_expected_review_digest_is_checked_before_any_runtime_probe(self):
        with FakeDaemon() as daemon:
            args = list(self.files(self.prepare(daemon)))
            args[3] = "sha256:" + "0" * 64
            before = len(daemon.calls)
            with patch.object(local, "NativeScorer") as score, self.assertRaises(LocalTransportError):
                local.execute(*args)
            score.assert_not_called()
            self.assertEqual(len(daemon.calls), before)
            self.assertFalse(args[-1].exists())

    def test_complete_fixture_orchestration_retains_81_rows_72_responses_9_exclusions(self):
        with FakeDaemon() as daemon:
            args = self.files(self.prepare(daemon))
            self.model_files_from_prompt(daemon)
            with patch.object(local, "NativeScorer", FakeScorer):
                report = local.execute(*args)
            self.assertEqual(report["counts"], {"planned_cells": 81, "generation_dispatched": 72,
                "actual_model_responses": 72, "unexecuted_cells": 9})
            self.assertEqual(len(daemon.generations), 72)
            self.assertEqual(len(report["row_files"]), 81)
            self.assertFalse(report["issue_closable"])
            self.assertEqual(report["post_run_independent_review"], "not_recorded")
            for ref in report["row_files"]:
                row, raw = local.read_json(args[-1] / ref["path"])
                self.assertEqual(digest(raw), ref["sha256"])
                self.assertNotIn("execution", row["cell"])
                if row["cell"]["task"] in inputs.UNAVAILABLE:
                    self.assertTrue(all(x is None for x in row["metrics"].values()))
                else:
                    self.assertIs(row["metrics"]["accepted_outcome"], True)
                    self.assertEqual(row["metrics"]["model_input_tokens"], 10)
                    self.assertIsNone(row["metrics"]["review_wall_ms"])
                    self.assertEqual(len(row["evidence"]), 2)
            for control in report["summary"]["controls"]:
                self.assertEqual((control["planned_cells"], control["known_correctness"]), (27, 24))
                self.assertEqual(control["planned_denominator_bounds"], [24/27, 1.0])

    def test_reference_failure_prevents_every_generation_and_retains_all_cells(self):
        class BadReference(FakeScorer):
            def score(self, *_): return {"result": {"status": "failed"}, "commands": []}
        with FakeDaemon() as daemon:
            args = self.files(self.prepare(daemon))
            with patch.object(local, "NativeScorer", BadReference): report = local.execute(*args)
            self.assertIn("reference_scorer_preflight_failed", report["halted"])
            self.assertEqual(report["counts"]["unexecuted_cells"], 81)
            self.assertFalse(daemon.generations)
            self.assertEqual(len(list(args[-1].glob("reference-*.json"))), 1)

    def test_scorer_failure_after_reply_preserves_actual_usage_and_stops_remaining_cells(self):
        class ScoringFailure(FakeScorer):
            def score(self, task, files):
                if self.calls >= 8: raise LocalTransportError("fixture_scorer_identity_drift")
                return super().score(task, files)
        with FakeDaemon() as daemon:
            args = self.files(self.prepare(daemon))
            self.model_files_from_prompt(daemon)
            with patch.object(local, "NativeScorer", ScoringFailure): report = local.execute(*args)
            self.assertEqual(len(daemon.generations), 1)
            row, _ = local.read_json(args[-1] / "cell-000.json")
            self.assertIsNone(row["metrics"]["accepted_outcome"])
            self.assertEqual(row["metrics"]["model_input_tokens"], 10)
            self.assertTrue(row["actual_model_response"])
            self.assertEqual(len(row["evidence"]), 2)
            self.assertEqual(len(report["row_files"]), 81)

    def test_daemon_integrity_failure_retains_unverified_output_without_fabricated_usage(self):
        with FakeDaemon() as daemon:
            args = self.files(self.prepare(daemon))
            self.model_files_from_prompt(daemon)
            daemon.after_generate = lambda: daemon.model.update(digest="b" * 64)
            with patch.object(local, "NativeScorer", FakeScorer): report = local.execute(*args)
            row, _ = local.read_json(args[-1] / "cell-000.json")
            self.assertEqual(row["status"], "attempt_outcome_unavailable")
            self.assertTrue(all(x is None for x in row["metrics"].values()))
            self.assertEqual(len(daemon.generations), 1)
            self.assertEqual(len(row["evidence"]), 2)
            self.assertEqual(report["counts"]["unexecuted_cells"], 80)

    def test_interrupt_during_inference_is_ambiguous_not_silently_retried(self):
        with FakeDaemon() as daemon:
            args = self.files(self.prepare(daemon))
            original = Client.request
            def interrupt(client, path, body=None):
                if path == "/api/generate":
                    client.last_generation_started = True
                    raise KeyboardInterrupt()
                return original(client, path, body)
            with patch.object(local, "NativeScorer", FakeScorer), patch.object(Client, "request", interrupt):
                report = local.execute(*args)
            self.assertEqual(report["counts"]["generation_dispatched"], 1)
            self.assertEqual(report["counts"]["actual_model_responses"], 0)
            self.assertEqual(report["counts"]["unexecuted_cells"], 80)
            self.assertIn("KeyboardInterrupt", report["halted"])

    def test_comparison_refuses_missing_and_duplicate_rows(self):
        frozen = local.snapshot()[-1]
        rows = [local.not_run(row, "fixture") for row in frozen["rows"]]
        report = local.compare(rows)
        self.assertIsNone(report["controls"][0]["observed_acceptance_rate"])
        self.assertEqual(report["controls"][0]["planned_denominator_bounds"], [0, 1])
        self.assertIsNone(report["comparisons"][0]["mean_paired_acceptance_difference"])
        for bad in (rows[:-1], rows[:-1] + [rows[0]]):
            with self.assertRaises(LocalTransportError): local.compare(bad)

    def test_exclusive_evidence_writes_refuse_links_ancestors_overwrites_and_bounds(self):
        path = self.root / "evidence.json"
        local.write_json(path, {"evidence": "fixture"})
        with self.assertRaises(FileExistsError): local.write_json(path, {"replace": True})
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        link = self.root / "linked"
        link.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(OSError): local.write_json(link / "new.json", {})
        target = self.root / "leaf.json"
        target.symlink_to(path)
        with self.assertRaises(OSError): local.write_json(target, {})
        with self.assertRaises(LocalTransportError):
            local.write_json(self.root / "big.json", {"x": "x" * local.bounds.MAX_RESULT_BYTES})
        self.assertFalse((self.root / "big.json").exists())

    def test_native_policy_does_not_authorize_parent_or_other_phase_or_network(self):
        tool, public, hidden = self.root / "tool", self.root / "public", self.root / "hidden"
        policy = native.sandbox_policy(tool, public)
        self.assertIn("(deny default)", policy)
        self.assertNotIn("allow network", policy)
        self.assertNotIn(str(hidden), policy)
        self.assertNotIn('(subpath "' + str(self.root) + '")', policy)
        self.assertIn('(literal "' + str(tool) + '")', policy)
        self.assertNotIn("allow process-fork", policy)

    def test_unprovisioned_native_host_refuses_before_any_process(self):
        with patch.object(native.provenance, "host_identity", side_effect=LocalTransportError("fixture_wrong_host")), \
             patch.object(native.bounds, "_run_bounded_group") as run:
            with self.assertRaises(LocalTransportError):
                with native.NativeScorer(self.sources, self.compiler, digest(self.compiler.read_bytes())): pass
            run.assert_not_called()

    @contextlib.contextmanager
    def fixture_native(self, source=None):
        sandbox = self.root / "sandbox-fixture"
        sandbox.write_text("fixture sandbox bytes; never executed")
        observed = []
        def process(command, cwd, deadline, environment):
            # No subprocess here: exercise real frozen scorer staging while
            # making the absence of genuine compiler execution explicit.
            if command[-1] == "version":
                return 0, b"fixture-semaprax-version\n", b"", None
            observed.append({"cwd": cwd, "files": {
                str(p.relative_to(cwd)): p.read_bytes() for p in cwd.rglob("*") if p.is_file()},
                "argv": command, "environment": environment})
            return 0, b"0\n", b"", None
        with patch.object(native, "SANDBOX", sandbox), \
             patch.object(native.provenance, "host_identity", return_value={"fixture": True}), \
             patch.object(native.bounds, "_run_bounded_group", side_effect=process):
            with native.NativeScorer(source or self.sources, self.compiler, digest(self.compiler.read_bytes())) as scorer:
                yield scorer, observed

    def test_frozen_scorer_staging_is_public_hidden_disjoint_without_candidate_fallback(self):
        task = "module-import-refactor-v1"
        candidates = {name: "model fixture contents " + name for name in inputs.CANDIDATES[task]}
        with self.fixture_native() as (scorer, observed):
            report = scorer.score(task, candidates)
            self.assertEqual(report["result"]["status"], "ok")
            self.assertEqual(report["result"]["leak_check"], "ok")
            self.assertEqual(len(observed), 2)
            self.assertNotEqual(observed[0]["cwd"], observed[1]["cwd"])
            hidden = {name.removeprefix(inputs.PREFIX + "tasks/" + task + "/hidden/semaprax/"): data
                      for name, data in self.sources.items()
                      if name.startswith(inputs.PREFIX + "tasks/" + task + "/hidden/semaprax/")}
            public = inputs.public_tree(self.sources, task)
            for path in set(hidden) - set(public):
                self.assertNotIn(path, observed[0]["files"])
                self.assertIn(path, observed[1]["files"])
            for phase in observed:
                for path, value in candidates.items(): self.assertEqual(phase["files"][path], value.encode())
                self.assertEqual(phase["environment"], dict(native.bounds.CLOSED_ENVIRONMENT))
            self.assertEqual(len(report["commands"]), 3)
            self.assertIn("stdout_base64", report["commands"][0])

    def test_hidden_overlay_cannot_overwrite_model_candidate(self):
        task = "module-import-refactor-v1"
        source = dict(self.sources)
        source[inputs.PREFIX + "tasks/" + task + "/hidden/semaprax/src/candidate.spx"] = b"replacement"
        with self.fixture_native(source) as (scorer, observed):
            with self.assertRaisesRegex(LocalTransportError, "hidden_overlay_replaces_candidate"):
                scorer.score(task, {name: "fixture" for name in inputs.CANDIDATES[task]})
            self.assertFalse(observed)

    def test_native_compiler_substitution_and_extra_command_authority_are_refused(self):
        with self.fixture_native() as (scorer, observed):
            with self.assertRaisesRegex(LocalTransportError, "unbound_compiler_command"):
                scorer.command(["/bin/sh", "-c", "true"], scorer.version_phase)
            with self.assertRaisesRegex(LocalTransportError, "unbound_compiler_command"):
                scorer.command([str(scorer.tool), "run", "."], self.root)
            scorer.tool.chmod(0o700)
            scorer.tool.write_bytes(b"different compiler")
            with self.assertRaisesRegex(LocalTransportError, "compiler_snapshot_drift"):
                scorer.command([str(scorer.tool), "version"], scorer.version_phase)
            self.assertFalse(observed)

    def test_cli_prepare_has_no_generation_and_emits_pending_template(self):
        with FakeDaemon() as daemon, contextlib.redirect_stdout(io.StringIO()) as out:
            code = local.main(["prepare", "--compiler", str(self.compiler), "--endpoint", daemon.endpoint,
                               "--operator", "fixture-operator", "--output", str(self.root / "prepared.json")])
            self.assertEqual(code, 0)
            result = json.loads(out.getvalue())
            self.assertEqual(result["execution"], "not_attempted")
            record, _ = local.read_json(result["review_template"])
            self.assertEqual(record["decision"], "pending")
            self.assertIsNone(record["reviewer"])
            self.assertFalse(daemon.generations)

    def fixture_run(self):
        with FakeDaemon() as daemon:
            args = self.files(self.prepare(daemon))
            self.model_files_from_prompt(daemon)
            with patch.object(local, "NativeScorer", FakeScorer):
                report = local.execute(*args)
        return args[-1], report

    def test_receipt_audit_replays_all_public_requests_and_denominators_without_network(self):
        directory, report = self.fixture_run()
        expected = digest((directory / "summary.json").read_bytes())
        with patch.object(Client, "request", side_effect=AssertionError("audit must not contact daemon")):
            result = local.audit(directory, expected)
        self.assertEqual(result["public_projection_requests_checked"], 72)
        self.assertEqual(result["counts"], report["counts"])
        self.assertFalse(result["actual_execution_independently_attested"])
        self.assertFalse(result["issue_closable"])
        self.assertEqual(result["model_calls_made_by_audit"], 0)

    def test_receipt_audit_detects_missing_or_modified_evidence(self):
        directory, _ = self.fixture_run()
        expected = digest((directory / "summary.json").read_bytes())
        (directory / "cell-000.response.json").write_bytes(b"{}\n")
        with self.assertRaisesRegex(LocalTransportError, "receipt_digest_mismatch"):
            local.audit(directory, expected)

    def mutate_fixture_row(self, directory, report, mutation):
        # Attacker can recompute all receipt hashes, but cannot turn an altered
        # prompt/control/cell inventory into the approved source projection.
        path = directory / "cell-000.json"
        row = json.loads(path.read_bytes())
        mutation(row)
        path.write_bytes(canonical(row))
        report["row_files"][0]["sha256"] = digest(path.read_bytes())
        summary = directory / "summary.json"
        summary.write_bytes(canonical(report))
        return digest(summary.read_bytes())

    def test_rehashed_prompt_leak_is_rejected_by_source_projection_not_just_hashes(self):
        directory, report = self.fixture_run()
        path = directory / "cell-000.request.json"
        value = json.loads(path.read_bytes())
        value["prompt"] += " hidden-only injected material"
        path.write_bytes(canonical(value))
        def mutation(row): row["evidence"][0]["sha256"] = digest(path.read_bytes())
        expected = self.mutate_fixture_row(directory, report, mutation)
        with self.assertRaisesRegex(LocalTransportError, "public_projection_drift"):
            local.audit(directory, expected)

    def test_duplicate_request_cannot_replace_response_evidence(self):
        directory, report = self.fixture_run()
        def mutation(row): row["evidence"][1] = dict(row["evidence"][0])
        expected = self.mutate_fixture_row(directory, report, mutation)
        with self.assertRaisesRegex(LocalTransportError, "matching_evidence"):
            local.audit(directory, expected)

    def test_audit_refuses_receipt_path_escape_even_with_rehashed_summary(self):
        directory, report = self.fixture_run()
        report["row_files"][0]["path"] = "../plan.json"
        summary = directory / "summary.json"
        summary.write_bytes(canonical(report))
        with self.assertRaisesRegex(LocalTransportError, "unbound_receipt_path"):
            local.audit(directory, digest(summary.read_bytes()))

    def test_run_refuses_existing_output_instead_of_replaying(self):
        with FakeDaemon() as daemon:
            args = self.files(self.prepare(daemon))
            args[-1].mkdir()
            with self.assertRaises(FileExistsError): local.execute(*args)
            self.assertFalse(daemon.generations)


if __name__ == "__main__": unittest.main()
