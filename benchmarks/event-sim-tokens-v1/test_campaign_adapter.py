import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from argparse import Namespace
from contextlib import ExitStack
from pathlib import Path
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import live_campaign_common as common
import campaign as live_campaign


class ShiftSimCampaignTests(unittest.TestCase):
    def _qualification_evidence(self, root: Path):
        repo = live_campaign.REPO
        commit = live_campaign.resolve_commit(repo, "HEAD")
        spec = live_campaign.blob_at_commit(repo, commit, live_campaign.SPEC_RELATIVE)
        corpus_bytes = live_campaign.blob_at_commit(repo, commit, live_campaign.CORPUS_RELATIVE)
        corpus = json.loads(corpus_bytes.decode("utf-8"))
        rows = []
        for kind, cases in (("valid", corpus["valid"]), ("invalid", corpus["invalid"])):
            for case in cases:
                request = live_campaign.acceptance_case_request(case, kind)
                expected = (
                    json.dumps(case["expected"], ensure_ascii=False, separators=(",", ":")).encode("utf-8") + b"\n"
                    if kind == "valid" else b""
                )
                rows.append({
                    "name": case["name"], "kind": kind, "status": "passed",
                    "input_bytes": len(request), "input_sha256": live_campaign.sha_bytes(request),
                    "leading_whitespace_bytes": case.get("leading_whitespace_bytes", 0),
                    "expected_exit_code": 0 if kind == "valid" else 2,
                    "exit_code": 0 if kind == "valid" else 2,
                    "stdout_sha256": live_campaign.sha_bytes(expected),
                    "expected_stdout_sha256": live_campaign.sha_bytes(expected),
                    "stderr_nonempty": kind == "invalid",
                })
        report = {
            "schema": live_campaign.ACCEPTANCE_REPORT_SCHEMA,
            "corpus_sha256": live_campaign.sha_bytes(corpus_bytes),
            "status": "passed", "valid_cases": len(corpus["valid"]),
            "invalid_cases": len(corpus["invalid"]), "cases": rows,
        }
        report_path = root / "acceptance-report.json"
        report_bytes = (json.dumps(report, indent=2) + "\n").encode("utf-8")
        report_path.write_bytes(report_bytes)
        evidence = {
            "schema": live_campaign.QUALIFICATION_EVIDENCE_SCHEMA,
            "spec_sha256": live_campaign.sha_bytes(spec),
            "acceptance_corpus_sha256": live_campaign.sha_bytes(corpus_bytes),
            "compiler_source_commit": commit,
            "compiler_binary_sha256": "a" * 64,
            "native_project_route": {
                "project_profile": live_campaign.NATIVE_PROJECT_PROFILE,
                "input_route": live_campaign.NATIVE_INPUT_ROUTE,
            },
            "acceptance_report": {
                "path": str(report_path), "sha256": live_campaign.sha_bytes(report_bytes),
            },
        }
        evidence_path = root / "qualification-evidence.json"
        evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
        return evidence_path, evidence, report_path

    def _run_trial_with_spec_edit(self, root: Path, edit_stage: str):
        spec_text = "# Frozen public spec\n"
        spec_hash = hashlib.sha256(spec_text.encode()).hexdigest()
        workspace = root / "artifacts" / "worktrees" / "semaprax-01"
        settings = {
            "timeout_seconds": 10,
            "observed_model_id": live_campaign.MODEL,
            "model": live_campaign.MODEL,
            "seed_files_sha256": {"benchmarks/event-sim-tokens-v1/SPEC.md": spec_hash},
        }

        def add_worktree(_seed, path, _commit, _files):
            (path / "benchmarks/event-sim-tokens-v1").mkdir(parents=True)
            (path / "benchmarks/event-sim-tokens-v1/SPEC.md").write_text(spec_text, encoding="utf-8")
            return None

        def run_process(_command, _cwd, _env, stream, _stderr, _timeout):
            stream.write_text("{}\n", encoding="utf-8")
            if edit_stage == "before_acceptance":
                (workspace / "benchmarks/event-sim-tokens-v1/SPEC.md").write_text("# Changed spec\n")
            return {"timed_out": False, "process_exit_code": 0, "elapsed_seconds": 0.1}

        def check_program(_candidate, _timeout, _env, qualification_mode):
            self.assertEqual(qualification_mode, "preflight_only")
            if edit_stage == "during_acceptance":
                (workspace / "benchmarks/event-sim-tokens-v1/SPEC.md").write_text("# Changed spec\n")
            return {"accepted": True}

        with ExitStack() as stack:
            stack.enter_context(patch.object(common, "add_seed_worktree", side_effect=add_worktree))
            stack.enter_context(patch.object(live_campaign, "prompt_for", return_value="test prompt"))
            stack.enter_context(patch.object(live_campaign, "run_process", side_effect=run_process))
            stack.enter_context(patch.object(common, "claude_command", return_value=[]))
            stack.enter_context(patch.object(live_campaign, "trial_environment", return_value={}))
            stack.enter_context(patch.object(common, "stream_usage", return_value={
                "models_observed": [live_campaign.MODEL], "usage": {}, "turns_with_usage": 1,
                "legacy_net_input": {"net_input_tokens": 0},
            }))
            stack.enter_context(patch.object(common, "observed_model_matches", return_value=True))
            stack.enter_context(patch.object(common, "rate_card_estimate_details", return_value={"usd": 0}))
            stack.enter_context(patch.object(common, "authored_source_metrics", return_value={"total_tokens": 0}))
            acceptance = stack.enter_context(patch.object(live_campaign, "check_program", side_effect=check_program))
            archive = stack.enter_context(patch.object(common, "archive_candidate", return_value=({}, [])))
            row = live_campaign.launch_trial(
                root / "seed", root / "artifacts", "commit", {"arm": "semaprax", "number": 1},
                settings, root / "semaprax",
            )
        return row, acceptance, archive, workspace

    def test_plan_marks_campaign_unqualified_for_open_stdin_boundary(self):
        with tempfile.TemporaryDirectory() as directory:
            settings = live_campaign.plan(Namespace(
                repo=str(live_campaign.REPO), base_ref="HEAD", artifacts=str(Path(directory) / "campaign"),
                trials_per_arm=5, model=live_campaign.MODEL, effort=live_campaign.EFFORT,
                timeout_seconds=1800, max_budget_usd=None,
            ))
        self.assertEqual(settings["qualification"]["status"], "preflight_not_qualified")
        self.assertEqual(settings["qualification"]["blocking_issue"], 611)
        self.assertFalse(settings["qualification"]["scored_trials_allowed"])
        self.assertEqual(settings["trial_order"], [
            "semaprax", "typescript", "typescript", "semaprax", "semaprax",
            "typescript", "typescript", "semaprax", "semaprax", "typescript",
        ])

    def test_single_arm_preflight_is_one_unscored_trial_and_keeps_scored_minimum(self):
        with tempfile.TemporaryDirectory() as directory:
            args = Namespace(
                repo=str(live_campaign.REPO), base_ref="HEAD", artifacts=str(Path(directory) / "campaign"),
                model=live_campaign.MODEL, effort=live_campaign.EFFORT, timeout_seconds=1800,
                max_budget_usd=None,
            )
            settings = live_campaign.single_arm_preflight_plan(args, "semaprax")
            self.assertEqual(settings["campaign_kind"], "single_arm_preflight")
            self.assertEqual(settings["trial_order"], ["semaprax"])
            self.assertEqual(settings["trials_per_arm"], 1)
            self.assertEqual(settings["attempt_denominator"], 1)
            self.assertFalse(settings["qualification"]["scored_trials_allowed"])
            self.assertEqual(settings["qualification"]["issue_611_status"], "open")

            args.artifacts = str(Path(directory) / "short-scored-plan")
            args.trials_per_arm = live_campaign.MIN_TRIALS_PER_ARM - 1
            with self.assertRaisesRegex(ValueError, "at least 5 trials per arm"):
                live_campaign.plan(args)

            with self.assertRaisesRegex(ValueError, "preflight arm"):
                live_campaign.single_arm_preflight_plan(args, "unknown")

    def test_pinned_native_evidence_gates_scored_trials_and_keeps_issue_open(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence_path, evidence, report_path = self._qualification_evidence(Path(directory))
            result = live_campaign.validate_qualification_evidence(
                evidence_path, live_campaign.REPO, evidence["compiler_source_commit"], "a" * 64,
            )
            self.assertEqual(result["status"], "evidence_gate_passed")
            self.assertTrue(result["scored_trials_allowed"])
            self.assertIn("open", result["issue_611_status"])
            self.assertEqual(result["acceptance_cases_passed"], 11)
            self.assertGreater(
                json.loads(report_path.read_text(encoding="utf-8"))["cases"][1]["input_bytes"], 65_536
            )

            with self.assertRaisesRegex(ValueError, "binary hash"):
                live_campaign.validate_qualification_evidence(
                    evidence_path, live_campaign.REPO, evidence["compiler_source_commit"], "b" * 64,
                )

            for key, value in (
                ("spec_sha256", "0" * 64),
                ("compiler_source_commit", "0" * 40),
                ("native_project_route", {"project_profile": "wrong", "input_route": "wrong"}),
            ):
                changed = dict(evidence)
                changed[key] = value
                evidence_path.write_text(json.dumps(changed), encoding="utf-8")
                with self.assertRaises(ValueError, msg=f"{key} must be bound"):
                    live_campaign.validate_qualification_evidence(
                        evidence_path, live_campaign.REPO, evidence["compiler_source_commit"], "a" * 64,
                    )

            evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["cases"][1]["status"] = "failed"
            report_bytes = (json.dumps(report, indent=2) + "\n").encode("utf-8")
            report_path.write_bytes(report_bytes)
            evidence["acceptance_report"]["sha256"] = live_campaign.sha_bytes(report_bytes)
            evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "large-leading-whitespace"):
                live_campaign.validate_qualification_evidence(
                    evidence_path, live_campaign.REPO, evidence["compiler_source_commit"], "a" * 64,
                )

    def test_acceptance_report_records_each_case_and_oversized_request(self):
        with tempfile.TemporaryDirectory() as directory:
            report_path = Path(directory) / "acceptance.json"
            result = subprocess.run(
                [
                    sys.executable, str(HERE / "acceptance" / "run.py"), "--report-json", str(report_path),
                    "--command-json", json.dumps([sys.executable, str(HERE / "oracle.py")]),
                ],
                text=True, capture_output=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            report = json.loads(report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["status"], "passed")
            self.assertEqual(len(report["cases"]), 11)
            oversized = next(row for row in report["cases"] if row["name"] == "large-leading-whitespace")
            self.assertEqual(oversized["status"], "passed")
            self.assertEqual(oversized["leading_whitespace_bytes"], 65_537)
            self.assertGreater(oversized["input_bytes"], 65_536)

    def test_seed_history_exposes_only_public_spec_not_acceptance_oracle(self):
        source = live_campaign.REPO
        commit = live_campaign.resolve_commit(source, "HEAD")
        with tempfile.TemporaryDirectory() as directory:
            artifacts = Path(directory)
            seed = artifacts / "seed"
            metadata = common.create_seed_repository(source, commit, seed, live_campaign.SEED_FILES)
            workspace = artifacts / "worktree"
            error = common.add_seed_worktree(seed, workspace, metadata["seed_repository_commit"],
                                             live_campaign.SEED_FILES)
            self.assertIsNone(error, error)
            files = subprocess.run(["git", "ls-tree", "-r", "--name-only", "HEAD"], cwd=workspace,
                                   text=True, capture_output=True, check=True).stdout.splitlines()
            self.assertEqual(files, ["benchmarks/event-sim-tokens-v1/SPEC.md"])
            self.assertNotEqual(subprocess.run(
                ["git", "show", "HEAD:benchmarks/event-sim-tokens-v1/acceptance/corpus.json"],
                cwd=workspace, capture_output=True, check=False,
            ).returncode, 0)
            self.assertEqual(metadata["source_files_sha256"], metadata["seed_files_sha256"])
            subprocess.run(["git", "worktree", "remove", "--force", str(workspace)], cwd=seed,
                           check=True, capture_output=True)

    def test_common_accounting_keeps_missing_usage_unknown_and_reports_proxy_separately(self):
        with tempfile.TemporaryDirectory() as directory:
            stream = Path(directory) / "stream.jsonl"
            stream.write_text(json.dumps({"type": "assistant", "message": {
                "id": "turn", "model": live_campaign.MODEL, "usage": {"input_tokens": 12},
                "content": [{"type": "text", "text": "done"}],
            }}) + "\n", encoding="utf-8")
            usage = common.stream_usage(stream, live_campaign.MODEL)
        self.assertEqual(usage["usage"]["input_tokens"], 12)
        self.assertIsNone(usage["usage"]["cache_creation_input_tokens"])
        self.assertIsNone(usage["usage"]["cache_read_input_tokens"])
        self.assertIsNone(usage["legacy_net_input"]["net_input_tokens"])
        self.assertEqual(usage["provider_output_tokens"], None)

    def test_summary_preserves_turn_counts_per_attempt_and_total(self):
        summary = live_campaign.summarize([
            {"arm": "semaprax", "number": 1, "status": "accepted",
             "observed": {"turns_with_usage": 2}},
            {"arm": "semaprax", "number": 2, "status": "failed",
             "observed": {"turns_with_usage": 1}},
        ])
        self.assertEqual(summary["arms"]["semaprax"]["turns"], 3)
        self.assertEqual(summary["arms"]["semaprax"]["turns_with_usage_per_attempt"], [2, 1])

    def test_spec_drift_before_acceptance_is_rejected_and_workspace_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            row, acceptance, archive, workspace = self._run_trial_with_spec_edit(
                Path(directory), "before_acceptance"
            )
            acceptance.assert_not_called()
            archive.assert_not_called()
            self.assertEqual(row["seeded_spec_integrity"]["status"], "failed")
            self.assertEqual(row["status"], "failed")
            self.assertTrue(row["workspace_retained_for_review"])
            self.assertTrue((workspace / "benchmarks/event-sim-tokens-v1/SPEC.md").is_file())

    def test_spec_drift_during_acceptance_invalidates_and_preserves_workspace(self):
        with tempfile.TemporaryDirectory() as directory:
            row, acceptance, archive, workspace = self._run_trial_with_spec_edit(
                Path(directory), "during_acceptance"
            )
            acceptance.assert_called_once()
            archive.assert_not_called()
            self.assertEqual(row["status"], "not_accepted")
            self.assertTrue(row["acceptance"]["accepted"])
            self.assertEqual(row["seeded_spec_integrity_before_archive"]["status"], "failed")
            self.assertTrue(row["workspace_retained_for_review"])
            self.assertTrue((workspace / "benchmarks/event-sim-tokens-v1/SPEC.md").is_file())

    def test_candidate_archive_retains_sources_and_records_only_cache_omissions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate, archive = root / "candidate", root / "archive"
            candidate.mkdir()
            (candidate / "main.ts").write_text("export {}\n", encoding="utf-8")
            (candidate / "node_modules").mkdir()
            (candidate / "node_modules" / "hidden.js").write_text("cache", encoding="utf-8")
            hashes, excluded = common.archive_candidate(candidate, archive)
        self.assertEqual(set(hashes), {"main.ts"})
        self.assertEqual(excluded, ["node_modules"])


if __name__ == "__main__":
    unittest.main()
