import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from argparse import Namespace
from contextlib import ExitStack
from pathlib import Path
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
CORPUS = json.loads((HERE / "acceptance" / "corpus.json").read_text(encoding="utf-8"))
CORPUS_CASES = CORPUS["valid"] + CORPUS["invalid"]
sys.path.insert(0, str(HERE.parent))
import live_campaign_common as common
import campaign as live_campaign


class ShiftSimCampaignTests(unittest.TestCase):
    def _qualification_evidence(self, root: Path):
        repo = root / "pinned-source"
        repo.mkdir()
        pinned_paths = [live_campaign.SPEC_RELATIVE, live_campaign.CORPUS_RELATIVE,
                        live_campaign.ORACLE_RELATIVE]
        for relative in pinned_paths:
            destination = repo / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((live_campaign.REPO / relative).read_bytes())
        subprocess.run(["git", "init", "--quiet", str(repo)], check=True, capture_output=True)
        subprocess.run(["git", "add", *pinned_paths], cwd=repo, check=True, capture_output=True)
        subprocess.run(
            ["git", "-c", "user.name=Benchmark Test", "-c", "user.email=test@example.invalid",
             "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "Pinned acceptance inputs"],
            cwd=repo, check=True, capture_output=True,
        )
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
                    "input_encoding": case.get("request_encoding", "compact"),
                    "input_bytes": len(request), "input_sha256": live_campaign.sha_bytes(request),
                    "leading_whitespace_bytes": case.get("leading_whitespace_bytes", 0),
                    "expected_exit_code": 0 if kind == "valid" else 2,
                    "exit_code": 0 if kind == "valid" else 2,
                    "stdout_sha256": live_campaign.sha_bytes(expected),
                    "expected_stdout_sha256": live_campaign.sha_bytes(expected),
                    "stderr_nonempty": kind == "invalid",
                    "stderr_one_diagnostic_line": kind == "invalid",
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
            "native_project_route": live_campaign.NATIVE_PROJECT_ROUTE,
            "acceptance_report": {
                "path": str(report_path), "sha256": live_campaign.sha_bytes(report_bytes),
            },
        }
        evidence_path = root / "qualification-evidence.json"
        evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
        return evidence_path, evidence, report_path, repo

    def _v3_qualification_evidence(self, root: Path, *,
                                   profile=live_campaign.AUTHORING_PROFILE_V27,
                                   compiler_hash="a" * 64):
        evidence_path, evidence, report_path, repo = self._qualification_evidence(root)
        evidence["compiler_binary_sha256"] = compiler_hash
        candidate = root / "qualified-candidate"
        candidate.mkdir()
        manifest = candidate / "semaprax.toml"
        manifest.write_text('''schema = "semaprax.manifest.v1"
[package]
profile = "language-command-io.stream-data.v1"
[command]
function = "run"
input = "argv-utf8+stdin-stream.v1"
[exports]
web = ["run"]
[capabilities]
required = ["process.args.read", "process.stderr.write", "process.stdin.read", "process.stdout.write"]
''', encoding="utf-8")
        if profile == live_campaign.AUTHORING_PROFILE_V30:
            manifest.write_text(manifest.read_text().replace(
                "language-command-io.stream-data.v1", "language-command-io.owned-data.v1"), encoding="utf-8")
        manifest_hash = live_campaign.sha_bytes(manifest.read_bytes())
        inventory = root / "candidate-source-inventory.json"
        files = [{"path": "semaprax.toml", "bytes": manifest.stat().st_size,
                  "sha256": manifest_hash}]
        closed_hash = live_campaign.sha_bytes(json.dumps(
            files, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode())
        inventory.write_text(json.dumps({
            "schema": "semaprax.closed-authored-inventory.v1",
            "files": files, "sha256": closed_hash,
        }), encoding="utf-8")
        inventory_hash = live_campaign.sha_bytes(inventory.read_bytes())
        native = root / "qualified-native"
        native.write_bytes(b"qualified native binary")
        native_hash = live_campaign.sha_bytes(native.read_bytes())
        subject = {
            "compiler_source_commit": evidence["compiler_source_commit"],
            "compiler_binary_sha256": evidence["compiler_binary_sha256"],
            "closed_authored_inventory_sha256": closed_hash,
            "candidate_manifest_sha256": manifest_hash,
            "native_binary_sha256": native_hash,
        }
        receipt = root / "qualification-build-receipt.json"
        receipt.write_text(json.dumps({
            "schema": live_campaign.QUALIFICATION_BUILD_RECEIPT_SCHEMA,
            "qualification_subject": subject,
            "acceptance_report_sha256": evidence["acceptance_report"]["sha256"],
        }), encoding="utf-8")
        evidence.update({
            "schema": live_campaign.AUTHORING_PROFILES[profile]["qualification_schema"],
            "native_project_route": live_campaign.AUTHORING_PROFILES[profile]["route"],
            "candidate_source": {
                "inventory": {"path": str(inventory),
                              "sha256": inventory_hash},
                "manifest": {"path": str(manifest), "sha256": manifest_hash},
            },
            "qualification_subject": subject,
            "qualification_build_receipt": {
                "path": str(receipt), "sha256": live_campaign.sha_bytes(receipt.read_bytes()),
            },
            "qualified_native_binary": {"path": str(native), "sha256": native_hash},
        })
        evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
        return evidence_path, evidence, report_path, repo, inventory, manifest

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

    def test_round_two_freezes_all_inputs_and_records_current_prices_without_repricing_history(self):
        with tempfile.TemporaryDirectory() as directory:
            args = Namespace(
                repo=str(live_campaign.REPO), base_ref="HEAD", artifacts=str(Path(directory) / "round"),
                trials_per_arm=5, model=live_campaign.MODEL, effort=live_campaign.EFFORT,
                timeout_seconds=1800, max_budget_usd=None,
            )
            historical = live_campaign.plan(args)
            args.round = 2
            current = live_campaign.plan(args)
            self.assertEqual(historical["round"], 1)
            self.assertEqual(current["round"], 2)
            self.assertEqual(current["benchmark_inputs_sha256"], live_campaign.FROZEN_BENCHMARK_SHA256)
            self.assertEqual(current["seed_files_sha256"], historical["seed_files_sha256"])
            self.assertEqual(current["trial_order"], historical["trial_order"])
            self.assertEqual(current["price_book"]["per_million_tokens"]["cache_read"], 0.1)
            self.assertEqual(historical["price_book"]["per_million_tokens"]["cache_read"], 0.2)
            usage = {field: 0 for field in common.ALL_USAGE_FIELDS}
            usage["cache_read_input_tokens"] = 1_000_000
            self.assertEqual(common.rate_card_estimate_details(usage)["usd"], 0.2)
            self.assertEqual(common.rate_card_estimate_details(
                usage, current["price_book"]["per_million_tokens"],
            )["usd"], 0.1)
            for invalid_round in (0, 5, True):
                args.round = invalid_round
                with self.assertRaisesRegex(ValueError, "unsupported ShiftSim round"):
                    live_campaign.plan(args)
            args.round = 2
            changed = {**live_campaign.FROZEN_BENCHMARK_SHA256,
                       live_campaign.ORACLE_RELATIVE: "0" * 64}
            with patch.object(live_campaign, "FROZEN_BENCHMARK_SHA256", changed):
                with self.assertRaisesRegex(ValueError, "unchanged frozen"):
                    live_campaign.plan(args)

    def test_round_three_requires_explicit_profile_and_v3_source_manifest_binding(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence_path, evidence, _, repo, inventory, manifest = self._v3_qualification_evidence(root)
            args = Namespace(repo=str(repo), base_ref="HEAD", artifacts=str(root / "artifacts"),
                round=3, trials_per_arm=5, model=live_campaign.MODEL, effort=live_campaign.EFFORT,
                timeout_seconds=1800, max_budget_usd=None, semaprax_bin=str(root / "compiler"),
                qualification_evidence=str(evidence_path), tokenizer_dir=None)
            with self.assertRaisesRegex(ValueError, "requires --authoring-profile"):
                live_campaign.plan(args, "a" * 64)
            args.authoring_profile = live_campaign.AUTHORING_PROFILE_V24
            with self.assertRaisesRegex(ValueError, "requires authoring profile"):
                live_campaign.plan(args, "a" * 64)
            args.authoring_profile = live_campaign.AUTHORING_PROFILE_V27
            settings = live_campaign.plan(args, "a" * 64)
            self.assertEqual(settings["schema"], "semaprax.event-sim-campaign.v2")
            self.assertEqual(settings["prompt_schema"], "semaprax.event-sim-prompt.v2")
            self.assertEqual(settings["native_project_route"], live_campaign.NATIVE_PROJECT_ROUTE_V27)
            self.assertEqual(settings["qualification"]["candidate_manifest_sha256"],
                             live_campaign.sha_bytes(manifest.read_bytes()))
            self.assertEqual(settings["qualification"]["candidate_source_inventory_path"], str(inventory.resolve()))
            receipt = Path(evidence["qualification_build_receipt"]["path"])
            receipt_original = receipt.read_bytes()
            wrong_receipt = json.loads(receipt_original)
            wrong_receipt["acceptance_report_sha256"] = "c" * 64
            receipt.write_text(json.dumps(wrong_receipt), encoding="utf-8")
            changed = json.loads(evidence_path.read_text())
            changed["qualification_build_receipt"]["sha256"] = live_campaign.sha_bytes(receipt.read_bytes())
            evidence_path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "accepted report"):
                live_campaign.validate_qualification_evidence(
                    evidence_path, repo, evidence["compiler_source_commit"], "a" * 64,
                    live_campaign.AUTHORING_PROFILE_V27)
            receipt.write_bytes(receipt_original)
            evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
            wrong_binary = json.loads(evidence_path.read_text())
            wrong_binary["qualification_subject"]["native_binary_sha256"] = "c" * 64
            evidence_path.write_text(json.dumps(wrong_binary), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "qualification subject"):
                live_campaign.validate_qualification_evidence(
                    evidence_path, repo, evidence["compiler_source_commit"], "a" * 64,
                    live_campaign.AUTHORING_PROFILE_V27)
            evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
            native = Path(evidence["qualified_native_binary"]["path"])
            native_bytes = native.read_bytes()
            native.write_bytes(b"different binary")
            with self.assertRaisesRegex(ValueError, "native binary hash"):
                live_campaign.validate_qualification_evidence(
                    evidence_path, repo, evidence["compiler_source_commit"], "a" * 64,
                    live_campaign.AUTHORING_PROFILE_V27)
            native.write_bytes(native_bytes)
            with self.assertRaisesRegex(ValueError, "schema must be"):
                live_campaign.validate_qualification_evidence(
                    evidence_path, repo, evidence["compiler_source_commit"], "a" * 64,
                    live_campaign.AUTHORING_PROFILE_V24)
            manifest_bytes = manifest.read_bytes()
            manifest.write_text(manifest.read_text().replace("stream-data.v1", "stream.v2"))
            with self.assertRaisesRegex(ValueError, "manifest hash"):
                live_campaign.validate_qualification_evidence(
                    evidence_path, repo, evidence["compiler_source_commit"], "a" * 64,
                    live_campaign.AUTHORING_PROFILE_V27)
            manifest.write_bytes(manifest_bytes)
            inventory.write_text(json.dumps({"files": []}), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "inventory hash"):
                live_campaign.validate_qualification_evidence(
                    evidence_path, repo, evidence["compiler_source_commit"], "a" * 64,
                    live_campaign.AUTHORING_PROFILE_V27)

    def test_v27_prompt_and_candidate_manifest_admission_are_exact(self):
        historical = live_campaign.prompt_for("typescript", Path("/candidate"), Path("/semaprax"))
        prompt = live_campaign.prompt_for("typescript", Path("/candidate"), Path("/semaprax"),
                                          live_campaign.AUTHORING_PROFILE_V27)
        self.assertIn("native Project v27", prompt)
        self.assertIn("language-command-io.stream-data.v1", prompt)
        marker = "The TypeScript arm must use Node from `PATH` and provide the same stdin and process status behavior."
        self.assertIn(marker, " ".join(historical.split()))
        self.assertIn(marker, " ".join(prompt.split()))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, _, _, _, _, manifest = self._v3_qualification_evidence(root)
            candidate = manifest.parent
            passed = live_campaign.candidate_authoring_admission(
                candidate, "semaprax", live_campaign.AUTHORING_PROFILE_V27)
            self.assertEqual(passed["status"], "passed")
            manifest.write_text(manifest.read_text().replace("process.stdin.read", "network.http"))
            self.assertEqual(live_campaign.candidate_authoring_admission(
                candidate, "semaprax", live_campaign.AUTHORING_PROFILE_V27)["status"], "failed")
            self.assertEqual(live_campaign.candidate_authoring_admission(
                candidate, "typescript", live_campaign.AUTHORING_PROFILE_V27)["status"], "not_applicable")

    def test_qualification_artifact_copy_rechecks_source_and_destination_hashes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifacts = root / "artifacts"
            artifacts.mkdir()
            evidence = root / "evidence.json"
            evidence.write_bytes(b"reviewed")
            qualification = {"evidence_path": str(evidence),
                             "evidence_sha256": live_campaign.sha_bytes(b"reviewed")}
            original = live_campaign.shutil.copyfile
            def drift(source, destination):
                result = original(source, destination)
                Path(source).write_bytes(b"changed")
                return result
            with patch.object(live_campaign.shutil, "copyfile", side_effect=drift):
                with self.assertRaisesRegex(ValueError, "during copy"):
                    live_campaign.copy_qualification_artifacts(qualification, artifacts)
    def test_closed_inventory_sorts_full_relative_paths_before_binding(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ("z.sh", "src/z.spx", "tests.py", "tests/unit.spx"):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(name, encoding="utf-8")
            inventory = live_campaign.closed_authored_inventory(root)
            self.assertEqual([row["path"] for row in inventory["files"]],
                             ["src/z.spx", "tests.py", "tests/unit.spx", "z.sh"])
            self.assertEqual(live_campaign._validate_closed_inventory_document(inventory),
                             inventory["files"])

    def test_provider_quota_requires_a_structured_failed_result(self):
        for result in (
            {"is_error": True, "api_error_status": 429, "api_error": "usage_limit_reached"},
            {"is_error": True, "api_error": "usage_limit_reached"},
        ):
            self.assertEqual(live_campaign.provider_quota_failure({"result_event": result})["source"],
                             "provider_result_event")
        for result in (None, {"is_error": False, "api_error_status": 429},
                       {"is_error": True, "api_error_status": 500},
                       {"is_error": True, "result": "You've hit your weekly limit"}):
            self.assertIsNone(live_campaign.provider_quota_failure({"result_event": result}))

    def test_round_two_quota_stop_preserves_the_attempt_and_does_not_launch_later_sessions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "semaprax"
            binary.write_bytes(b"offline mock")
            for round_number, expected_launches in ((1, 10), (2, 1)):
                artifacts = root / f"round-{round_number}"
                settings = {
                    "repository_commit": "commit", "artifacts": str(artifacts), "round": round_number,
                    "model": live_campaign.MODEL, "trials_per_arm": 5,
                    "qualification": {"scored_trials_allowed": False},
                    "trial_order": list(live_campaign.ARMS) * 5,
                }
                def trial(_seed, _artifacts, _commit, identity, _settings, _binary):
                    return {**identity, "status": "failed", "provider_quota": {"api_error_status": 429}}
                with ExitStack() as stack:
                    stack.enter_context(patch.object(sys, "argv", ["campaign.py", "run", "--base-ref", "HEAD",
                        "--artifacts", str(artifacts), "--semaprax-bin", str(binary), "--round", str(round_number)]))
                    stack.enter_context(patch.object(live_campaign, "plan", return_value=settings))
                    stack.enter_context(patch.object(common, "create_seed_repository",
                        return_value={"seed_repository_commit": "seed"}))
                    stack.enter_context(patch.object(live_campaign, "launch_calibration",
                        return_value={"status": "ready", "observed_model_id": live_campaign.MODEL}))
                    stack.enter_context(patch.object(subprocess, "run",
                        return_value=subprocess.CompletedProcess([], 0, stdout="offline", stderr="")))
                    launched = stack.enter_context(patch.object(live_campaign, "launch_trial", side_effect=trial))
                    result = live_campaign.main()
                self.assertEqual(result, 2 if round_number == 2 else 0)
                self.assertEqual(launched.call_count, expected_launches)
                report = json.loads((artifacts / "results.json").read_text())
                self.assertEqual(len(report["trials"]), expected_launches)
                self.assertEqual(len(report["unlaunched_trial_order"]), 10 - expected_launches)
                self.assertEqual(report["campaign_status"],
                    "interrupted_provider_quota" if round_number == 2 else "complete")

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
            self.assertEqual(settings["native_project_route"], live_campaign.NATIVE_PROJECT_ROUTE)

            args.artifacts = str(Path(directory) / "short-scored-plan")
            args.trials_per_arm = live_campaign.MIN_TRIALS_PER_ARM - 1
            with self.assertRaisesRegex(ValueError, "at least 5 trials per arm"):
                live_campaign.plan(args)

            with self.assertRaisesRegex(ValueError, "preflight arm"):
                live_campaign.single_arm_preflight_plan(args, "unknown")

    def test_both_arm_prompts_pin_the_v2_application_status_contract(self):
        native = live_campaign.prompt_for("semaprax", Path("/candidate"), Path("/semaprax"))
        typescript = live_campaign.prompt_for("typescript", Path("/candidate"), Path("/semaprax"))
        self.assertEqual(native.replace("SEMAPRAX native Project", "TypeScript on Node.js"), typescript)
        for arm in live_campaign.ARMS:
            with self.subTest(arm=arm):
                prompt = live_campaign.prompt_for(arm, Path("/candidate"), Path("/semaprax"))
                self.assertIn("Project v24", prompt)
                self.assertIn("language-command-io.stream.v2", prompt)
                self.assertIn("argv-utf8+stdin-stream.v1", prompt)
                self.assertIn("returning `i64` process status in the range 0..255", prompt)
                self.assertIn("Return 0 for valid requests and 2 for invalid requests", prompt)
                self.assertIn("same stdin and process", prompt)

    def test_candidate_requires_successful_build_authored_tests_and_independent_acceptance(self):
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory)
            for script in ("build.sh", "test.sh", "run.sh"):
                (candidate / script).write_text("exit 0\n")
            for exits, expected_calls, accepted in (
                ([1], 1, False), ([0, 1], 2, False), ([0, 0, 1], 3, False), ([0, 0, 0], 3, True),
            ):
                with patch.object(subprocess, "run", side_effect=[
                    subprocess.CompletedProcess([], code, stdout=b"", stderr=b"") for code in exits
                ]) as calls:
                    result = live_campaign.check_program(candidate, 10, {})
                self.assertEqual(result["accepted"], accepted)
                self.assertEqual(calls.call_count, expected_calls)
                if expected_calls == 3:
                    command = calls.call_args.args[0]
                    self.assertIn(str(HERE / "acceptance" / "run.py"), command)
                    self.assertIn(json.dumps(["/bin/sh", str(candidate / "run.sh")]), command)

    def test_v27_check_builds_and_accepts_only_the_harness_native_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate = root / "candidate"
            candidate.mkdir()
            for name, text in (("semaprax.toml", "manifest\n"), ("app.spx", "module app;\n"),
                               ("build.sh", "exit 0\n"), ("test.sh", "exit 0\n"),
                               ("run.sh", "exit 99\n")):
                (candidate / name).write_text(text)
            compiler = root / "semaprax"
            compiler.write_bytes(b"pinned compiler")
            output = root / "harness" / "shiftsim"
            commands = []
            def execute(command, **_kwargs):
                commands.append(command)
                if command[:2] == [str(compiler), "build"]:
                    output.write_bytes(b"fresh native")
                return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")
            with patch.object(subprocess, "run", side_effect=execute):
                result = live_campaign.check_program(candidate, 10,
                    {"SEMAPRAX_BIN": str(compiler)}, "evidence_gated_scored",
                    live_campaign.AUTHORING_PROFILE_V27, output,
                    live_campaign.sha_bytes(compiler.read_bytes()), None, "semaprax")
            self.assertTrue(result["accepted"])
            self.assertEqual(commands[0][1], "check")
            self.assertEqual(commands[1][1], "build")
            accepted = json.loads(commands[-1][commands[-1].index("--command-json") + 1])
            self.assertEqual(accepted, [str(output)])
            self.assertNotIn(str(candidate / "run.sh"), accepted)
            self.assertEqual(result["native_binary"]["sha256"],
                             live_campaign.sha_bytes(b"fresh native"))

            (candidate / "semaprax.toml").unlink()
            (candidate / "main.mjs").write_text("process.exit(0);\n")
            (candidate / "run.sh").write_text('''#!/bin/sh
set -eu
cd "$(dirname "$0")"
if [ ! -f dist/cli.js ]; then ./build.sh; fi
exec node dist/cli.js
''')
            typescript_commands = []
            def execute_typescript(command, **_kwargs):
                typescript_commands.append(command)
                return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")
            with patch.object(subprocess, "run", side_effect=execute_typescript):
                typescript = live_campaign.check_program(candidate, 10,
                    {"PATH": os.environ.get("PATH", "")}, "evidence_gated_scored",
                    live_campaign.AUTHORING_PROFILE_V27, root / "unused" / "shiftsim",
                    None, None, "typescript")
            self.assertTrue(typescript["accepted"])
            self.assertEqual(typescript["typescript_route"]["command"],
                             ["/bin/sh", str(candidate / "run.sh")])
            self.assertFalse(any(command[0] == str(compiler) for command in typescript_commands))

    def test_v3_qualification_builder_binds_the_binary_actually_passed_to_acceptance(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, _, accepted_report, repo, _, manifest = self._v3_qualification_evidence(root)
            candidate = manifest.parent
            (candidate / "app.spx").write_text("module app;\n")
            compiler = root / "semaprax"
            compiler.write_bytes(b"compiler")
            output = root / "generated-qualification"
            commands = []
            original_run = subprocess.run
            def execute(command, **_kwargs):
                if command[0] == "git":
                    return original_run(command, **_kwargs)
                commands.append(command)
                if command[:2] == [str(compiler.resolve()), "build"]:
                    Path(command[command.index("--output") + 1]).write_bytes(b"accepted native")
                if "--report-json" in command:
                    Path(command[command.index("--report-json") + 1]).write_bytes(
                        accepted_report.read_bytes())
                return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")
            commit = live_campaign.resolve_commit(repo, "HEAD")
            with patch.object(subprocess, "run", side_effect=execute):
                result = live_campaign.generate_v3_qualification(
                    candidate, compiler, repo, commit, output, 10)
            self.assertEqual(result["status"], "qualified")
            acceptance = next(command for command in commands if "--report-json" in command)
            accepted_command = json.loads(acceptance[acceptance.index("--command-json") + 1])
            self.assertEqual(accepted_command, [str(output.resolve() / "qualified-native-binary")])
            evidence = json.loads((output / "qualification-evidence.json").read_text())
            self.assertEqual(evidence["qualification_subject"]["native_binary_sha256"],
                             live_campaign.sha_bytes(b"accepted native"))
            receipt = json.loads((output / "qualification-build-receipt.json").read_text())
            self.assertEqual(receipt["acceptance_report_sha256"],
                             live_campaign.sha_bytes(accepted_report.read_bytes()))

            drift_output = root / "compiler-drift-qualification"
            def drift_compiler(command, **_kwargs):
                if command[:2] == [str(compiler.resolve()), "check"]:
                    compiler.write_bytes(b"changed compiler")
                return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")
            with patch.object(subprocess, "run", side_effect=drift_compiler):
                with self.assertRaisesRegex(ValueError, "pinned compiler changed"):
                    live_campaign.generate_v3_qualification(
                        candidate, compiler, repo, commit, drift_output, 10)
            compiler.write_bytes(b"compiler")

    def test_v27_source_mutation_and_retained_symlink_fail_closed_but_node_modules_links_are_ignored(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate = root / "candidate"
            candidate.mkdir()
            for name in ("semaprax.toml", "app.spx", "build.sh", "test.sh", "run.sh"):
                (candidate / name).write_text("source\n")
            compiler = root / "semaprax"
            compiler.write_bytes(b"compiler")
            output = root / "harness" / "shiftsim"
            def mutate(command, **_kwargs):
                if command[:2] == [str(compiler), "build"]:
                    output.write_bytes(b"native")
                if command[:2] == ["/bin/sh", str(candidate / "build.sh")]:
                    (candidate / "app.spx").write_text("changed\n")
                return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")
            with patch.object(subprocess, "run", side_effect=mutate):
                result = live_campaign.check_program(candidate, 10,
                    {"SEMAPRAX_BIN": str(compiler)}, "evidence_gated_scored",
                    live_campaign.AUTHORING_PROFILE_V27, output,
                    live_campaign.sha_bytes(compiler.read_bytes()), None, "semaprax")
            self.assertFalse(result["accepted"])
            self.assertEqual(result["build_source_consistency"]["status"], "failed")

            (candidate / "app.spx").write_text("source\n")
            output2 = root / "harness-2" / "shiftsim"
            def overwrite_binary(command, **_kwargs):
                if command[:2] == [str(compiler), "build"]:
                    output2.write_bytes(b"native")
                if command[:2] == ["/bin/sh", str(candidate / "test.sh")]:
                    output2.write_bytes(b"candidate overwrite")
                return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")
            with patch.object(subprocess, "run", side_effect=overwrite_binary):
                changed_binary = live_campaign.check_program(candidate, 10,
                    {"SEMAPRAX_BIN": str(compiler)}, "evidence_gated_scored",
                    live_campaign.AUTHORING_PROFILE_V27, output2,
                    live_campaign.sha_bytes(compiler.read_bytes()), None, "semaprax")
            self.assertFalse(changed_binary["accepted"])
            self.assertEqual(changed_binary["candidate_tests_source_consistency"]["status"], "failed")

            output3 = root / "harness-3" / "shiftsim"
            compiler_hash = live_campaign.sha_bytes(compiler.read_bytes())
            def overwrite_compiler(command, **_kwargs):
                if command[:2] == [str(compiler), "build"]:
                    output3.write_bytes(b"native")
                if command[:2] == ["/bin/sh", str(candidate / "build.sh")]:
                    compiler.write_bytes(b"compiler changed by candidate")
                return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")
            with patch.object(subprocess, "run", side_effect=overwrite_compiler):
                changed_compiler = live_campaign.check_program(candidate, 10,
                    {"SEMAPRAX_BIN": str(compiler)}, "evidence_gated_scored",
                    live_campaign.AUTHORING_PROFILE_V27, output3, compiler_hash, None, "semaprax")
            self.assertFalse(changed_compiler["accepted"])
            self.assertEqual(changed_compiler["build_source_consistency"]["status"], "failed")
            compiler.write_bytes(b"compiler")

            with patch.object(subprocess, "run") as calls:
                wrong_arm = live_campaign.check_program(candidate, 10, {"PATH": os.environ.get("PATH", "")},
                    "evidence_gated_scored", live_campaign.AUTHORING_PROFILE_V27,
                    root / "typescript-harness" / "shiftsim", None, None, "typescript")
            self.assertFalse(wrong_arm["accepted"])
            self.assertIn("must not contain", wrong_arm["typescript_route"]["error"])
            calls.assert_not_called()

            (candidate / "app.spx").unlink()
            (candidate / "app.spx").symlink_to(root / "outside.spx")
            with self.assertRaisesRegex(ValueError, "regular"):
                live_campaign.closed_authored_inventory(candidate)
            (candidate / "app.spx").unlink()
            (candidate / "app.spx").write_text("source\n")
            linked = candidate / "node_modules" / ".bin"
            linked.mkdir(parents=True)
            (root / "outside-tool").write_text("tool\n")
            (linked / "tool").symlink_to(root / "outside-tool")
            inventory = live_campaign.closed_authored_inventory(candidate)
            self.assertNotIn("node_modules/.bin/tool", [row["path"] for row in inventory["files"]])

    def test_pinned_native_evidence_gates_scored_trials_and_keeps_issue_open(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence_path, evidence, report_path, repo = self._qualification_evidence(Path(directory))
            result = live_campaign.validate_qualification_evidence(
                evidence_path, repo, evidence["compiler_source_commit"], "a" * 64,
            )
            self.assertEqual(result["status"], "evidence_gate_passed")
            self.assertTrue(result["scored_trials_allowed"])
            self.assertIn("open", result["issue_611_status"])
            self.assertEqual(result["native_project_route"], live_campaign.NATIVE_PROJECT_ROUTE)
            self.assertEqual(evidence["schema"], "semaprax.event-sim-qualification-evidence.v2")
            corpus = json.loads((HERE / "acceptance" / "corpus.json").read_text(encoding="utf-8"))
            expected_case_count = sum(len(corpus[kind]) for kind in ("valid", "invalid"))
            self.assertEqual(result["acceptance_cases_passed"], expected_case_count)
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report_rows = {row["name"]: row for row in report["cases"]}
            for name in ("large-leading-whitespace", "max-cardinality-escaped-keys-and-ids"):
                self.assertGreater(report_rows[name]["input_bytes"], 65_536)
            self.assertLessEqual(report_rows["max-cardinality-compact"]["input_bytes"], 65_536)

            with self.assertRaisesRegex(ValueError, "binary hash"):
                live_campaign.validate_qualification_evidence(
                    evidence_path, repo, evidence["compiler_source_commit"], "b" * 64,
                )

            wrong_result_route = dict(live_campaign.NATIVE_PROJECT_ROUTE)
            wrong_result_route["command_result_type"] = "bool"
            wrong_range_route = dict(live_campaign.NATIVE_PROJECT_ROUTE)
            wrong_range_route["process_status_range"] = [0, 1]
            for key, value in (
                ("spec_sha256", "0" * 64),
                ("compiler_source_commit", "0" * 40),
                ("native_project_route", {"project_profile": "wrong", "input_route": "wrong"}),
                ("native_project_route", wrong_result_route),
                ("native_project_route", wrong_range_route),
                ("schema", "semaprax.event-sim-qualification-evidence.v1"),
            ):
                changed = dict(evidence)
                changed[key] = value
                evidence_path.write_text(json.dumps(changed), encoding="utf-8")
                with self.assertRaises(ValueError, msg=f"{key} must be bound"):
                    live_campaign.validate_qualification_evidence(
                        evidence_path, repo, evidence["compiler_source_commit"], "a" * 64,
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
                    evidence_path, repo, evidence["compiler_source_commit"], "a" * 64,
                )

            report["cases"][1]["status"] = "passed"
            invalid_row = next(row for row in report["cases"] if row["name"] == "nine-servers-exceeds-capacity")
            invalid_row["stderr_one_diagnostic_line"] = False
            report_bytes = (json.dumps(report, indent=2) + "\n").encode("utf-8")
            report_path.write_bytes(report_bytes)
            evidence["acceptance_report"]["sha256"] = live_campaign.sha_bytes(report_bytes)
            evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "exactly one diagnostic line"):
                live_campaign.validate_qualification_evidence(
                    evidence_path, repo, evidence["compiler_source_commit"], "a" * 64,
                )

            invalid_row["stderr_one_diagnostic_line"] = True
            invalid_row["stderr_ends_with_newline"] = False
            report_bytes = (json.dumps(report, indent=2) + "\n").encode("utf-8")
            report_path.write_bytes(report_bytes)
            evidence["acceptance_report"]["sha256"] = live_campaign.sha_bytes(report_bytes)
            evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
            result = live_campaign.validate_qualification_evidence(
                evidence_path, repo, evidence["compiler_source_commit"], "a" * 64,
            )
            self.assertEqual(result["status"], "evidence_gate_passed")

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
            self.assertEqual(len(report["cases"]), len(CORPUS_CASES))
            oversized = next(row for row in report["cases"] if row["name"] == "large-leading-whitespace")
            self.assertEqual(oversized["status"], "passed")
            self.assertEqual(oversized["leading_whitespace_bytes"], 65_537)
            self.assertGreater(oversized["input_bytes"], 65_536)
            escaped = next(row for row in report["cases"] if row["name"] == "max-cardinality-escaped-keys-and-ids")
            compact = next(row for row in report["cases"] if row["name"] == "max-cardinality-compact")
            self.assertEqual(escaped["status"], "passed")
            self.assertEqual(escaped["input_encoding"], live_campaign.ESCAPED_KEYS_AND_IDENTIFIERS)
            self.assertGreater(escaped["input_bytes"], 65_536)
            self.assertLessEqual(compact["input_bytes"], 65_536)
            self.assertEqual(escaped["expected_stdout_sha256"], compact["expected_stdout_sha256"])
            for name in ("nine-servers-exceeds-capacity", "257-patients-exceeds-capacity"):
                invalid = next(row for row in report["cases"] if row["name"] == name)
                self.assertEqual(invalid["expected_exit_code"], 2)
                self.assertEqual(invalid["exit_code"], 2)
                self.assertEqual(invalid["stdout_sha256"], live_campaign.sha_bytes(b""))
                self.assertEqual(invalid["stderr_one_diagnostic_line"], True)

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

    def test_summary_keeps_unlaunched_attempt_totals_unknown(self):
        summary = live_campaign.summarize([])
        arm = summary["arms"]["semaprax"]
        self.assertEqual(arm["attempts"], 0)
        self.assertIsNone(arm["list_price_estimate_total_usd_including_failures"])
        self.assertIsNone(arm["list_price_estimate_usd_per_accepted_task_including_failures"])
        self.assertTrue(all(value is None for value in arm["raw_usage_known_subtotal_by_bucket"].values()))
        self.assertIsNone(arm["aggregate_attempt_wall_seconds"])
        self.assertIsNone(arm["turns"])

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


    def test_round_four_requires_explicit_fresh_v30_all_fifteen_qualification(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            compiler = root / "compiler"
            compiler.write_bytes(b"v30 compiler fixture")
            digest = common.digest(compiler)
            path, evidence, report, repo, inventory, manifest = self._v3_qualification_evidence(
                root, profile=live_campaign.AUTHORING_PROFILE_V30, compiler_hash=digest)
            args = Namespace(repo=str(repo), base_ref="HEAD", artifacts=str(root / "artifacts"),
                round=4, trials_per_arm=5, model=live_campaign.MODEL, effort=live_campaign.EFFORT,
                timeout_seconds=1800, max_budget_usd=None, semaprax_bin=str(compiler),
                qualification_evidence=str(path), tokenizer_dir=None)
            with self.assertRaisesRegex(ValueError, "requires --authoring-profile"):
                live_campaign.plan(args, digest)
            args.authoring_profile = live_campaign.AUTHORING_PROFILE_V27
            with self.assertRaisesRegex(ValueError, "requires authoring profile"):
                live_campaign.plan(args, digest)
            args.authoring_profile = live_campaign.AUTHORING_PROFILE_V30
            with patch.object(common, "tokenizer_metadata", return_value=None):
                settings = live_campaign.plan(args, digest)
            self.assertEqual(settings["schema"], "semaprax.event-sim-campaign.v3")
            self.assertEqual(settings["qualification"]["acceptance_cases_passed"], 15)
            self.assertEqual(settings["benchmark_inputs_sha256"], live_campaign.FROZEN_BENCHMARK_SHA256)
            self.assertEqual(settings["attempt_denominator"], 10)
            self.assertIsNone(settings["language_setup"]["semaprax"]["fixed_harness_context_tokens"])
            live_campaign.require_authoring_eligibility(settings, compiler)
            copies = root / "retained-qualification"
            copies.mkdir()
            live_campaign.copy_qualification_artifacts(settings["qualification"], copies)
            live_campaign.require_authoring_eligibility(settings, compiler)
            copied_native = Path(settings["qualification"]["qualified_native_binary_artifact"])
            original = copied_native.read_bytes()
            copied_native.write_bytes(b"changed retained binary")
            with self.assertRaisesRegex(ValueError, "retained qualification copy changed"):
                live_campaign.require_authoring_eligibility(settings, compiler)
            copied_native.write_bytes(original)
            for target in (path, inventory, manifest,
                           Path(evidence["qualified_native_binary"]["path"]), compiler, report):
                with self.subTest(target=target.name):
                    original = target.read_bytes()
                    target.write_bytes(original + b"changed")
                    with self.assertRaises((ValueError, json.JSONDecodeError)):
                        live_campaign.require_authoring_eligibility(settings, compiler)
                    target.write_bytes(original)
            historical = dict(evidence, schema=live_campaign.QUALIFICATION_EVIDENCE_SCHEMA_V3)
            path.write_text(json.dumps(historical), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "schema must be"):
                live_campaign.require_authoring_eligibility(settings, compiler)
            path.write_text(json.dumps(evidence), encoding="utf-8")
            # A self-consistent envelope with only fourteen rows still cannot qualify.
            report_original = report.read_bytes()
            receipt = Path(evidence["qualification_build_receipt"]["path"])
            receipt_original = receipt.read_bytes()
            partial_report = json.loads(report_original)
            partial_report["cases"].pop()
            report.write_text(json.dumps(partial_report), encoding="utf-8")
            partial_evidence = json.loads(json.dumps(evidence))
            partial_evidence["acceptance_report"]["sha256"] = common.digest(report)
            partial_receipt = json.loads(receipt_original)
            partial_receipt["acceptance_report_sha256"] = common.digest(report)
            receipt.write_text(json.dumps(partial_receipt), encoding="utf-8")
            partial_evidence["qualification_build_receipt"]["sha256"] = common.digest(receipt)
            path.write_text(json.dumps(partial_evidence), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "every pinned acceptance case"):
                live_campaign.require_authoring_eligibility(settings, compiler)
            report.write_bytes(report_original)
            receipt.write_bytes(receipt_original)
            path.write_text(json.dumps(evidence), encoding="utf-8")
            args.qualification_evidence = None
            with self.assertRaisesRegex(ValueError, "all-15 qualification"):
                live_campaign.plan(args, digest)

    def test_v30_context_changes_only_sem_setup_and_preserves_ts_prompt(self):
        profile = live_campaign.AUTHORING_PROFILE_V30
        historical = live_campaign.prompt_for("typescript", Path("/candidate"), Path("/compiler"),
                                              live_campaign.AUTHORING_PROFILE_V27)
        self.assertEqual(live_campaign.prompt_for("typescript", Path("/candidate"), Path("/compiler"),
                                                profile), historical)
        prompt = live_campaign.prompt_for("semaprax", Path("/candidate"), Path("/compiler"), profile)
        self.assertIn("native Project v30", prompt)
        self.assertIn("language-command-io.owned-data.v1", prompt)
        self.assertIn("author:owned-data", prompt)
        self.assertIn("Bytes allocation/clone loop restrictions", prompt)
        self.assertNotIn("only when T is a Copy scalar", prompt)
        self.assertNotIn("vec_sort_owned", prompt)  # Setup is not an authored scheduling solution.
        row = {}
        live_campaign.retain_fixed_harness_context(row, {"authoring_profile": profile}, prompt)
        self.assertEqual(row["fixed_harness_context"]["prompt_utf8_bytes"], len(prompt.encode()))
        self.assertEqual(row["fixed_harness_context"]["prompt_sha256"], live_campaign.sha_text(prompt))
        self.assertIsNone(row["fixed_harness_context"]["tokens"])
        self.assertIsNone(row["fixed_harness_context"]["actual_billed_usd"])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, _, _, _, _, manifest = self._v3_qualification_evidence(root, profile=profile)
            self.assertEqual(live_campaign.candidate_authoring_admission(
                manifest.parent, "semaprax", profile)["status"], "passed")
            self.assertEqual(live_campaign.candidate_authoring_admission(
                manifest.parent, "semaprax", live_campaign.AUTHORING_PROFILE_V27)["status"], "failed")
            manifest.write_text(manifest.read_text().replace("process.stdin.read", "network.read"))
            self.assertEqual(live_campaign.candidate_authoring_admission(
                manifest.parent, "semaprax", profile)["status"], "failed")

    def test_v30_qualification_builder_accepts_only_its_harness_native_subject(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, _, report, repo, _, manifest = self._v3_qualification_evidence(
                root, profile=live_campaign.AUTHORING_PROFILE_V30)
            compiler = root / "compiler"
            compiler.write_bytes(b"compiler fixture")
            output = root / "fresh-qualification"
            commands = []
            original_run = subprocess.run
            def execute(command, **kwargs):
                if command[0] == "git":
                    return original_run(command, **kwargs)
                commands.append(command)
                if command[:2] == [str(compiler.resolve()), "build"]:
                    Path(command[command.index("--output") + 1]).write_bytes(b"new native fixture")
                if "--report-json" in command:
                    Path(command[command.index("--report-json") + 1]).write_bytes(report.read_bytes())
                return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")
            commit = live_campaign.resolve_commit(repo, "HEAD")
            with patch.object(subprocess, "run", side_effect=execute):
                result = live_campaign.generate_v3_qualification(manifest.parent, compiler, repo,
                    commit, output, 10, authoring_profile=live_campaign.AUTHORING_PROFILE_V30)
            self.assertEqual(result["status"], "qualified")
            acceptance = next(command for command in commands if "--command-json" in command)
            self.assertEqual(json.loads(acceptance[acceptance.index("--command-json") + 1]),
                             [str(output / "qualified-native-binary")])
            evidence = json.loads((output / "qualification-evidence.json").read_text())
            self.assertEqual(evidence["schema"], live_campaign.QUALIFICATION_EVIDENCE_SCHEMA_V4)
            self.assertEqual(evidence["native_project_route"]["project_schema"], "semaprax.project.v30")
            admitted = live_campaign.validate_qualification_evidence(output / "qualification-evidence.json",
                repo, commit, common.digest(compiler), live_campaign.AUTHORING_PROFILE_V30)
            self.assertEqual(admitted["acceptance_cases_passed"], 15)

    def test_v30_claude_dispatch_refuses_unqualified_setup_before_worktree_or_paid_call(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            settings = {"round": 4, "authoring_profile": live_campaign.AUTHORING_PROFILE_V30,
                        "qualification": {"scored_trials_allowed": False}}
            with patch.object(common, "add_seed_worktree") as worktree, \
                 patch.object(live_campaign, "run_process") as paid:
                for launch in (lambda: live_campaign.launch_calibration(root, root / "artifacts", "seed", settings, root / "compiler"),
                               lambda: live_campaign.launch_trial(root, root / "artifacts", "seed",
                                   {"arm": "semaprax", "number": 1}, settings, root / "compiler")):
                    with self.assertRaisesRegex(ValueError, "all-15 qualification"):
                        launch()
                worktree.assert_not_called()
                paid.assert_not_called()


if __name__ == "__main__":
    unittest.main()
