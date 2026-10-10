"""Owning mocked catalog protocol regressions; no paid dispatch in tests."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from argparse import Namespace
from unittest.mock import patch

import codex_campaign as adapter
catalog = adapter.catalog


class CatalogCampaignTests(unittest.TestCase):
    def setUp(self):
        self.enterContext(patch("campaign_resources.snapshot", return_value=[
            {"device": 1, "path": "/fixture", "free_bytes": 10 * 1024**3}]))

    def report(self, command):
        corpus = json.loads((catalog.BENCHMARK / "acceptance/corpus.json").read_bytes())
        return {"schema": catalog.REPORT_SCHEMA, "accepted": True, "required_cases": 23,
            "corpus_sha256": catalog.common.digest(catalog.BENCHMARK / "acceptance/corpus.json"),
            "command": command, "cases": [{"name": row["name"], "accepted": True,
                "status": row["status"], "stdout_hex": row["stdout_hex"], "stderr_hex": row["stderr_hex"],
                "input_bytes": len(bytes.fromhex(row["input_hex"])),
                "input_sha256": catalog.sha_bytes(bytes.fromhex(row["input_hex"]))} for row in corpus["cases"]]}

    def fixture(self, root, authoring_profile=catalog.AUTHORING_PROFILE_V30):
        route = catalog.ROUTE_BY_PROFILE[authoring_profile]
        repo = root / "pinned"
        repo.mkdir()
        for relative in catalog.FROZEN_INPUTS:
            destination = repo / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((catalog.REPO / relative).read_bytes())
        for command in (["git", "init", "-q", str(repo)], ["git", "add", "."],
            ["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
             "-c", "commit.gpgsign=false", "commit", "-qm", "Pinned original catalog23"]):
            subprocess.run(command, cwd=repo, capture_output=True, check=True)
        commit = catalog.resolve_commit(repo, "HEAD")
        candidate = root / "candidate"
        candidate.mkdir()
        (candidate / "semaprax.toml").write_text(f'''schema = "semaprax.manifest.v1"
[package]
profile = "{route["project_profile"]}"
[command]
function = "run"
input = "argv-utf8+stdin-stream.v1"
[exports]
web = ["run"]
[capabilities]
required = ["process.args.read", "process.stderr.write", "process.stdin.read", "process.stdout.write"]
''')
        (candidate / "app.spx").write_text("module fixture;\n")
        for name in ("build.sh", "test.sh", "run.sh"):
            (candidate / name).write_text("exit 0\n")
        compiler = root / "compiler"
        compiler.write_bytes(b"compiler fixture")
        native = root / "native"
        native.write_bytes(b"native fixture")
        inventory = root / "inventory.json"
        closed = catalog.closed_authored_inventory(candidate)
        inventory.write_text(json.dumps(closed))
        manifest = candidate / "semaprax.toml"
        report = root / "report.json"
        report.write_text(json.dumps(self.report([str(native)])))
        subject = {"compiler_source_commit": commit, "compiler_binary_sha256": catalog.common.digest(compiler),
            "closed_authored_inventory_sha256": closed["sha256"],
            "candidate_manifest_sha256": catalog.common.digest(manifest), "native_binary_sha256": catalog.common.digest(native)}
        receipt = root / "receipt.json"
        receipt.write_text(json.dumps({"schema": catalog.BUILD_RECEIPT_SCHEMA, "qualification_subject": subject,
            "acceptance_report_sha256": catalog.common.digest(report)}))
        def reference(path): return {"path": str(path), "sha256": catalog.common.digest(path)}
        evidence = root / "evidence.json"
        evidence_document = {"schema": catalog.QUALIFICATION_SCHEMA,
            "native_project_route": route,
            "benchmark_inputs_sha256": catalog.FROZEN_INPUTS, "compiler_source_commit": commit,
            "compiler_binary_sha256": catalog.common.digest(compiler), "qualification_subject": subject,
            "acceptance_report": reference(report), "candidate_source": {"inventory": reference(inventory), "manifest": reference(manifest)},
            "qualified_native_binary": reference(native), "qualification_build_receipt": reference(receipt)}
        if authoring_profile != catalog.AUTHORING_PROFILE_V30:
            evidence_document["authoring_profile"] = authoring_profile
        evidence.write_text(json.dumps(evidence_document))
        qualification = catalog.validate_qualification_evidence(
            evidence, repo, commit, catalog.common.digest(compiler), authoring_profile)
        profile = catalog.AUTHORING_PROFILES[authoring_profile]
        settings = {"cohort": profile["cohort"], "authoring_profile": authoring_profile,
            "native_project_route": route, "qualification_repository": str(repo), "repository_commit": commit,
            "compiler_source_commit": commit, "source_binary_sha256": catalog.common.digest(compiler), "qualification": qualification,
            "typescript_bootstrap": {"receipt_path": str(root / "tooling.json")}, "timeout_seconds": 1800,
            "model": adapter.MODEL, "effort": adapter.EFFORT, "codex_binary": "/fixture/codex", "authored_source_tokenizer": None}
        return settings, repo, compiler, candidate, evidence, native, inventory, report

    def test_original23_report_requires_exact_complete_ordered_inputs_and_outputs(self):
        report = self.report(["/native"])
        self.assertEqual(len(catalog.verify_report(json.dumps(report).encode(), ["/native"])["cases"]), 23)
        for mutation in ("missing", "duplicate", "stdout", "input", "invalid-status", "command"):
            altered = json.loads(json.dumps(report))
            if mutation == "missing": altered["cases"].pop()
            elif mutation == "duplicate": altered["cases"][-1] = altered["cases"][0]
            elif mutation == "stdout": altered["cases"][0]["stdout_hex"] += "00"
            elif mutation == "input": altered["cases"][0]["input_sha256"] = "a" * 64
            elif mutation == "invalid-status":
                next(row for row in altered["cases"] if row["status"] == 2)["status"] = 0
            else: altered["command"] = ["/different-native"]
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                catalog.verify_report(json.dumps(altered).encode(), ["/native"])

    def test_fresh_qualification_and_retained_copies_replay_exact_source_native_bindings(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            settings, _, compiler, candidate, evidence, native, inventory, report = self.fixture(root)
            with patch.object(catalog.ts_bootstrap, "verify_plan", return_value={}):
                catalog.require_authoring_eligibility(settings, compiler)
                retained = root / "retained"
                retained.mkdir()
                catalog.copy_qualification_artifacts(settings["qualification"], retained)
                catalog.require_authoring_eligibility(settings, compiler)
                for target in (compiler, native, inventory, report, evidence, candidate / "semaprax.toml",
                               Path(settings["qualification"]["qualified_native_binary_artifact"])):
                    original = target.read_bytes()
                    target.write_bytes(original + b"changed")
                    with self.subTest(target=target.name), self.assertRaises(ValueError):
                        catalog.require_authoring_eligibility(settings, compiler)
                    target.write_bytes(original)
                settings["native_project_route"] = {**catalog.ROUTE, "project_schema": "semaprax.project.v27"}
                with self.assertRaisesRegex(ValueError, "cohort/profile"):
                    catalog.require_authoring_eligibility(settings, compiler)

    def test_stale_or_unqualified_all_paid_endpoints_refuse_before_worktree_and_model(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            settings, _, compiler, _, evidence, _, _, _ = self.fixture(root)
            evidence.write_text("{}")
            with patch.object(catalog.common, "add_seed_worktree") as worktree, \
                 patch.object(adapter.codex, "run_codex") as paid:
                calls = [lambda: adapter.launch_calibration(root, root / "artifacts", "seed", settings, compiler)]
                calls.extend(lambda arm=arm: adapter.launch_trial(root, root / "artifacts", "seed",
                    {"arm": arm, "number": 1}, settings, compiler) for arm in adapter.ARMS)
                for launch in calls:
                    row = launch()
                    self.assertEqual(row["status"], "failed")
                    self.assertTrue(row["runner_error"])
                    self.assertIn("qualification", row["failure"])
                worktree.assert_not_called()
                paid.assert_not_called()
            self.assertEqual({path.name for path in (root / "artifacts").iterdir()}, {"resource-receipts"})

    def test_unqualified_metadata_never_substitutes_for_the_fresh_report(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            settings, _, compiler, _, _, _, _, _ = self.fixture(root)
            for qualification in ({}, {"scored_trials_allowed": True}):
                settings["qualification"] = qualification
                with self.subTest(qualification=qualification), self.assertRaisesRegex(ValueError, "all23 qualification"):
                    catalog.require_authoring_eligibility(settings, compiler)

    def test_plan_preserves_matched_model_five_attempts_and_strong_ts_tooling(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            settings, repo, compiler, _, evidence, _, _, _ = self.fixture(root)
            tooling = root / "tooling.json"
            tooling.write_text("{}")
            receipt = {key: "a" * 64 for key in ("inventory_sha256", "package_json_sha256", "package_lock_sha256", "helper_sha256", "dependency_helper_sha256")}
            receipt.update({"runtime": {"node_binary_sha256": "b" * 64}, "packages": {"typescript": "5.9.3"}})
            args = Namespace(repo=str(repo), base_ref="HEAD", compiler_source_ref="HEAD", semaprax_bin=str(compiler),
                qualification_evidence=str(evidence), artifacts=str(root / "artifacts"),
                authoring_profile=catalog.AUTHORING_PROFILE_V30, trials_per_arm=5, model=adapter.MODEL,
                effort=adapter.EFFORT, timeout_seconds=1800, max_budget_usd=None, tokenizer_dir=None,
                codex_binary="/fixture/codex", typescript_bootstrap_receipt=str(tooling), node_binary="node", npm_binary="npm")
            original_run = subprocess.run
            def execute(command, **kwargs):
                if command == ["/fixture/codex", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "fixture-version\n", "")
                return original_run(command, **kwargs)
            with patch.object(adapter.codex, "capabilities", return_value={"status": "ready"}), \
                 patch.object(catalog.ts_bootstrap, "validate", return_value=receipt), \
                 patch.object(catalog.common, "tokenizer_metadata", return_value=None), \
                 patch.object(adapter.subprocess, "run", side_effect=execute):
                planned = adapter.plan(args)
            self.assertEqual(planned["schema"], "semaprax.catalog-codex-campaign.v1")
            self.assertEqual(planned["cohort"], "catalog-owned-data-v1")
            self.assertEqual(planned["authoring_profile"], catalog.AUTHORING_PROFILE_V30)
            self.assertEqual((planned["model"], planned["effort"]), ("gpt-6.1-sol", "medium"))
            self.assertEqual(planned["attempt_denominator"], 10)
            self.assertEqual(planned["trial_order"].count("semaprax"), 5)
            self.assertEqual(planned["trial_order"].count("typescript"), 5)
            self.assertEqual(planned["qualification"]["acceptance_cases_passed"], 23)
            self.assertEqual(planned["typescript_bootstrap"]["runtime"], receipt["runtime"])
            self.assertIsNone(planned["language_setup"]["fixed_harness_context_tokens"])
            self.assertIsNone(planned["codex_execution"]["actual_billed_usd"])
            self.assertEqual(planned["seed_checkout"]["included_files"], list(catalog.SEED_FILES))
            self.assertFalse(planned["language_setup"]["reference_solution_supplied"])
            self.assertFalse((root / "artifacts").exists())

    def test_v31_plan_requires_profile_bound_all23_qualification(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            profile_name = catalog.AUTHORING_PROFILE_V31
            self.assertIn(profile_name, catalog.shared.PINNED_AUTHORING_PROFILES)
            settings, repo, compiler, candidate, evidence, _, _, _ = self.fixture(root, profile_name)
            admission = catalog.candidate_authoring_admission(candidate, "semaprax", profile_name)
            self.assertEqual(admission["status"], "passed")
            self.assertEqual(admission["route"]["schema"], "semaprax.manifest.v1")
            self.assertEqual(admission["route"]["profile"], catalog.ROUTE_BY_PROFILE[profile_name]["project_profile"])
            self.assertEqual(admission["route"]["input"], catalog.ROUTE_BY_PROFILE[profile_name]["input_route"])
            self.assertEqual(admission["route"]["function"], "run")
            self.assertEqual(admission["route"]["exports_web"], ["run"])
            self.assertEqual(admission["route"]["capabilities"], [
                "process.args.read", "process.stderr.write", "process.stdin.read", "process.stdout.write"])
            self.assertEqual(settings["qualification"]["acceptance_cases_passed"], 23)
            with patch.object(catalog.ts_bootstrap, "verify_plan", return_value={}):
                catalog.require_authoring_eligibility(settings, compiler)

            tooling = root / "tooling.json"
            tooling.write_text("{}")
            receipt = {key: "a" * 64 for key in ("inventory_sha256", "package_json_sha256", "package_lock_sha256", "helper_sha256", "dependency_helper_sha256")}
            receipt.update({"runtime": {"node_binary_sha256": "b" * 64}, "packages": {"typescript": "5.9.3"}})
            args = Namespace(repo=str(repo), base_ref="HEAD", compiler_source_ref="HEAD", semaprax_bin=str(compiler),
                qualification_evidence=str(evidence), artifacts=str(root / "artifacts"), authoring_profile=profile_name,
                trials_per_arm=5, model=adapter.MODEL, effort=adapter.EFFORT, timeout_seconds=1800,
                max_budget_usd=None, tokenizer_dir=None, codex_binary="/fixture/codex",
                typescript_bootstrap_receipt=str(tooling), node_binary="node", npm_binary="npm")
            original_run = subprocess.run
            def execute(command, **kwargs):
                if command == ["/fixture/codex", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "fixture-version\n", "")
                return original_run(command, **kwargs)
            with patch.object(adapter.codex, "capabilities", return_value={"status": "ready"}), \
                 patch.object(catalog.ts_bootstrap, "validate", return_value=receipt), \
                 patch.object(catalog.common, "tokenizer_metadata", return_value=None), \
                 patch.object(adapter.subprocess, "run", side_effect=execute):
                planned = adapter.plan(args)
            self.assertEqual(planned["schema"], "semaprax.catalog-codex-campaign.v2")
            self.assertEqual(planned["cohort"], "catalog-collection-record-v1")
            self.assertEqual(planned["authoring_profile"], profile_name)
            self.assertEqual(planned["native_project_route"]["project_schema"], "semaprax.project.v31")
            self.assertEqual(planned["qualification"]["acceptance_cases_passed"], 23)
            self.assertEqual((planned["model"], planned["effort"], planned["timeout_seconds"],
                              planned["trials_per_arm"]), ("gpt-6.1-sol", "medium", 1800, 5))
            self.assertEqual(planned["arms"], list(adapter.ARMS))
            self.assertEqual(planned["typescript_bootstrap"]["runtime"], receipt["runtime"])
            self.assertEqual(planned["typescript_bootstrap"]["packages"], receipt["packages"])
            altered = json.loads(evidence.read_text())
            altered["authoring_profile"] = catalog.AUTHORING_PROFILE_V30
            evidence.write_text(json.dumps(altered))
            with self.assertRaisesRegex(ValueError, "compiler/source/profile"):
                catalog.validate_qualification_evidence(
                    evidence, repo, settings["compiler_source_commit"], catalog.common.digest(compiler), profile_name)

    def test_qualification_cli_requires_explicit_authoring_profile(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            missing_profile = ["codex_campaign", "qualify", "--repo", str(root),
                "--compiler-source-ref", "HEAD", "--semaprax-bin", str(root / "compiler"),
                "--candidate", str(root / "candidate"), "--output", str(root / "evidence")]
            with patch.object(adapter.sys, "argv", missing_profile), self.assertRaises(SystemExit):
                adapter.main()
            selected_profile = missing_profile + ["--authoring-profile", catalog.AUTHORING_PROFILE_V31]
            with patch.object(adapter.sys, "argv", selected_profile), \
                 patch.object(catalog, "resolve_commit", return_value="source-commit"), \
                 patch.object(catalog, "qualify", return_value={"status": "qualified"}) as qualify, \
                 patch("builtins.print"):
                self.assertEqual(adapter.main(), 0)
            self.assertEqual(qualify.call_args.args[-1], catalog.AUTHORING_PROFILE_V31)

    def test_type_script_runtime_closure_is_pinned_and_deep_module_drift_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate = root / "candidate"
            candidate.mkdir()
            for name in ("run.sh", "build.sh", "test.sh"):
                (candidate / name).write_text("exit 0\n")
            (candidate / "catalog.mts").write_text("export {}\n")
            (candidate / "helper.cts").write_text("export {}\n")
            (candidate / "tsconfig.json").write_text("{}")
            dist = candidate / "dist"
            dist.mkdir()
            (dist / "catalog.mjs").write_text("import './nested/helper.mjs';\n")
            (dist / "nested").mkdir()
            (dist / "nested/helper.mjs").write_text("export {}\n")
            node = root / "node"
            node.write_bytes(b"node fixture")
            tsc = root / "tsc"
            tsc.write_bytes(b"tsc fixture")
            env = {"CATALOG_NODE_BINARY": str(node), "CATALOG_NODE_SHA256": catalog.common.digest(node),
                   "CATALOG_TSC_JS": str(tsc)}
            commands = []
            def execute(command, **kwargs):
                commands.append(command)
                if "--outDir" in command:
                    target = Path(command[command.index("--outDir") + 1])
                    import shutil
                    shutil.copytree(dist, target)
                if "--report-json" in command:
                    app = json.loads(command[command.index("--command-json") + 1])
                    Path(command[command.index("--report-json") + 1]).write_text(json.dumps(self.report(app)))
                return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")
            with patch.object(catalog.subprocess, "run", side_effect=execute):
                result = catalog.check_program(candidate, 10, env, harness_output=root / "harness/catalog",
                                               arm="typescript", exclude_verified_node_modules=True)
            self.assertTrue(result["accepted"])
            retained_sources = {row["path"] for row in result["closed_authored_inventory"]["files"]}
            self.assertTrue({"catalog.mts", "helper.cts"} <= retained_sources)
            # Historical cohorts retain their frozen extension policy.
            historical_sources = {row["path"] for row in catalog.shared.closed_authored_inventory(candidate)["files"]}
            self.assertTrue({"catalog.mts", "helper.cts"}.isdisjoint(historical_sources))
            with patch.object(catalog.common, "tokenize_texts", side_effect=lambda texts, metadata: [1] * len(texts)):
                measured = catalog.common.authored_source_metrics(candidate, {"fixture": True},
                    additional_suffixes=catalog.ADDITIONAL_AUTHORED_SUFFIXES)
            self.assertEqual({row["path"] for row in measured["files"]}, retained_sources)
            archive = root / "candidate-archive"
            catalog.common.archive_candidate(candidate, archive, exclude_verified_node_modules=True)
            archived_inventory = catalog.closed_authored_inventory(archive)
            self.assertEqual(archived_inventory, result["closed_authored_inventory"])
            with patch.object(catalog.common, "tokenize_texts", side_effect=lambda texts, metadata: [1] * len(texts)):
                archived_metrics = catalog.common.authored_source_metrics(
                    archive, {"fixture": True}, additional_suffixes=catalog.ADDITIONAL_AUTHORED_SUFFIXES)
            self.assertEqual({row["path"] for row in archived_metrics["files"]}, retained_sources)

            def fail_tsc(command, **kwargs):
                if "--outDir" in command:
                    return subprocess.CompletedProcess(command, 1, stdout=b"", stderr=b"pinned tsc failed")
                return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")

            with patch.object(catalog.subprocess, "run", side_effect=fail_tsc):
                failed = catalog.check_program(candidate, 10, env,
                    harness_output=root / "typescript-failed-parent" / "catalog", arm="typescript",
                    exclude_verified_node_modules=True)
            self.assertFalse(failed["accepted"])
            self.assertEqual(failed["pinned_typescript_build"]["status"], "failed")
            self.assertNotIn("independent_acceptance", failed)

            def forge_report(command, **kwargs):
                if "--outDir" in command:
                    target = Path(command[command.index("--outDir") + 1])
                    import shutil
                    shutil.copytree(dist, target)
                if "--report-json" in command:
                    actual_command = json.loads(command[command.index("--command-json") + 1])
                    forged = self.report(actual_command)
                    forged["cases"][0]["stdout_hex"] += "00"
                    Path(command[command.index("--report-json") + 1]).write_text(json.dumps(forged))
                return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")

            with patch.object(catalog.subprocess, "run", side_effect=forge_report), \
                    self.assertRaisesRegex(ValueError, "differs from frozen case"):
                catalog.check_program(candidate, 10, env,
                    harness_output=root / "typescript-forged-report" / "catalog", arm="typescript",
                    exclude_verified_node_modules=True)

            (candidate / "helper.cts").write_text("export const changed = true;\n")
            source_intact, _ = catalog._phase_source_and_binary_guard(candidate,
                result["closed_authored_inventory"], Path(result["native_binary"]["path"]), result["native_binary"]["sha256"],
                None, None, expected_runtime=result["runtime_artifacts"])
            self.assertFalse(source_intact)
            (candidate / "helper.cts").write_text("export {}\n")
            self.assertEqual(result["pinned_typescript_build"]["command"][:2], [str(node), str(tsc)])
            actual = json.loads(next(command for command in commands if "--command-json" in command)[
                next(command for command in commands if "--command-json" in command).index("--command-json") + 1])
            self.assertEqual(actual[0], str(node))
            self.assertEqual(Path(actual[1]).parent, root / "harness/typescript-runtime")
            retained = Path(result["runtime_artifacts"]["root"])
            (retained / "nested/helper.mjs").write_text("changed\n")
            passed, guard = catalog._phase_source_and_binary_guard(candidate,
                result["closed_authored_inventory"], Path(result["native_binary"]["path"]), result["native_binary"]["sha256"],
                None, None, exclude_verified_node_modules=True, expected_runtime=result["runtime_artifacts"])
            self.assertFalse(passed)
            self.assertEqual(guard["typescript_runtime"]["status"], "failed")
            (dist / "link.mjs").symlink_to(node)
            with self.assertRaisesRegex(ValueError, "symlinks"):
                catalog.runtime_inventory(dist)

    def test_native_qualification_producer_binds_actual_all23_command_and_retains_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            settings, repo, compiler, candidate, _, _, _, _ = self.fixture(root)
            output = root / "new-qualification"
            original_run = subprocess.run
            commands = []
            def execute(command, **kwargs):
                if command[0] == "git":
                    return original_run(command, **kwargs)
                commands.append(command)
                if len(command) > 1 and command[1] == "build":
                    Path(command[command.index("--output") + 1]).write_bytes(b"actual harness native fixture")
                if "--report-json" in command:
                    app = json.loads(command[command.index("--command-json") + 1])
                    Path(command[command.index("--report-json") + 1]).write_text(json.dumps(self.report(app)))
                return subprocess.CompletedProcess(command, 0, stdout=b"fixture stdout", stderr=b"")
            with patch.object(catalog.subprocess, "run", side_effect=execute):
                result = catalog.qualify(candidate, compiler, repo, settings["compiler_source_commit"], output, 10)
            self.assertEqual(result["status"], "qualified")
            qualified = catalog.validate_qualification_evidence(Path(result["evidence"]), repo,
                settings["compiler_source_commit"], catalog.common.digest(compiler))
            self.assertEqual(qualified["acceptance_cases_passed"], 23)
            native = output / "runtime/catalog"
            accepted = next(command for command in commands if "--command-json" in command)
            self.assertEqual(json.loads(accepted[accepted.index("--command-json") + 1]), [str(native)])
            v31_root = root / "v31"
            v31_root.mkdir()
            v31_settings, v31_repo, v31_compiler, v31_candidate, _, _, _, _ = self.fixture(
                v31_root, catalog.AUTHORING_PROFILE_V31)
            v31_output = v31_root / "new-qualification"
            with patch.object(catalog.subprocess, "run", side_effect=execute):
                v31_result = catalog.qualify(v31_candidate, v31_compiler, v31_repo,
                    v31_settings["compiler_source_commit"], v31_output, 10, catalog.AUTHORING_PROFILE_V31)
            v31_qualification = catalog.validate_qualification_evidence(Path(v31_result["evidence"]),
                v31_repo, v31_settings["compiler_source_commit"], catalog.common.digest(v31_compiler),
                catalog.AUTHORING_PROFILE_V31)
            self.assertEqual(v31_qualification["acceptance_cases_passed"], 23)
            self.assertEqual(v31_qualification["native_project_route"], catalog.ROUTE_BY_PROFILE[catalog.AUTHORING_PROFILE_V31])
            failed_output = root / "failed-qualification"
            def fail(command, **kwargs):
                if command[0] == "git":
                    return original_run(command, **kwargs)
                return subprocess.CompletedProcess(command, 1, stdout=b"", stderr=b"controlled compiler refusal")
            with patch.object(catalog.subprocess, "run", side_effect=fail), self.assertRaisesRegex(ValueError, "qualification failed"):
                catalog.qualify(candidate, compiler, repo, settings["compiler_source_commit"], failed_output, 10)
            failed = json.loads((failed_output / "qualification-result.json").read_text())
            self.assertEqual(failed["status"], "failed")
            self.assertIn("controlled compiler refusal", failed["checks"]["pinned_compiler_check"]["stderr"])
            self.assertFalse((failed_output / "qualification-evidence.json").exists())

    def test_nested_outcome_cohort_requires_its_own_profile_and_fresh_all23_binding(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            settings, repo, compiler, candidate, evidence, _, _, _ = self.fixture(
                root, catalog.AUTHORING_PROFILE_V32)
            self.assertEqual(settings["cohort"], "catalog-nested-outcome-v1")
            self.assertEqual(settings["native_project_route"]["project_schema"], "semaprax.project.v32")
            self.assertEqual(settings["native_project_route"]["project_profile"],
                "language-command-io.nested-outcome.v1")
            admitted = catalog.candidate_authoring_admission(candidate, "semaprax", catalog.AUTHORING_PROFILE_V32)
            self.assertEqual(admitted["status"], "passed")
            for old_profile in (catalog.AUTHORING_PROFILE_V30, catalog.AUTHORING_PROFILE_V31):
                self.assertEqual(catalog.candidate_authoring_admission(candidate, "semaprax", old_profile)["status"], "failed")
                with self.assertRaises(ValueError):
                    catalog.validate_qualification_evidence(evidence, repo,
                        settings["compiler_source_commit"], catalog.common.digest(compiler), old_profile)
            self.assertEqual(catalog.shared.AUTHORING_PROFILES[catalog.AUTHORING_PROFILE_V32]["rounds"], (5,))
            # Only the Project setup differs; the strong TS prompt and all23 contract remain intact.
            self.assertEqual(catalog.prompt_for("typescript", candidate, compiler, catalog.AUTHORING_PROFILE_V32),
                catalog.prompt_for("typescript", candidate, compiler, catalog.AUTHORING_PROFILE_V30))
            prompt = catalog.prompt_for("semaprax", candidate, compiler, catalog.AUTHORING_PROFILE_V32)
            self.assertIn("all 23 original functional and output requirements stay binding", prompt)
            self.assertIn("supersedes only the SPEC's historical v30 profile/route clause", prompt)
            self.assertIn("Project v32 profile", prompt)
            self.assertIn("language-command-io.nested-outcome.v1", prompt)
            for requirement in ("unlimited raw whitespace", "owned String label and Bytes payload",
                "Allocate, clone and replace Bytes-bearing owners outside loops"):
                self.assertIn(requirement, prompt)

    def test_prompts_and_fixed_context_supply_no_solution_or_codegen_guess(self):
        for arm in adapter.ARMS:
            prompt = catalog.prompt_for(arm, Path("/candidate"), Path("/compiler"), catalog.AUTHORING_PROFILE_V30)
            self.assertIn(catalog.SPEC_RELATIVE, prompt)
            self.assertIn("No application source", prompt)
            self.assertNotIn("json_Request", prompt)
            self.assertNotIn("vec_sort_owned", prompt)
            row = {}
            catalog.retain_fixed_harness_context(row, {}, prompt)
            self.assertEqual(row["fixed_harness_context"]["prompt_utf8_bytes"], len(prompt.encode()))
            self.assertIsNone(row["fixed_harness_context"]["tokens"])
            self.assertIsNone(row["fixed_harness_context"]["actual_billed_usd"])
        self.assertIn("idiomatic native", catalog.prompt_for("typescript", Path("/candidate"), Path("/compiler"), catalog.AUTHORING_PROFILE_V30))

        v31 = catalog.prompt_for("semaprax", Path("/candidate"), Path("/compiler"), catalog.AUTHORING_PROFILE_V31)
        self.assertIn("all 23 original functional and output requirements stay binding", v31)
        self.assertIn("Preserve every requirement, including repeated\nkeys, decoded identifiers, maximum cardinalities and unlimited raw whitespace.", v31)
        self.assertIn("owned String id and scalar mark", v31)
        self.assertIn("owned String label and Bytes payload", v31)
        self.assertIn("Allocate, clone and replace Bytes-bearing owners outside loops", v31)
        self.assertIn("supersedes only the SPEC's historical v30 profile/route clause", v31)
        self.assertIn("Project v31 profile", v31)
        self.assertIn("language-command-io.collection-record.v1", v31)
        self.assertEqual(
            catalog.prompt_for("typescript", Path("/candidate"), Path("/compiler"), catalog.AUTHORING_PROFILE_V31),
            catalog.prompt_for("typescript", Path("/candidate"), Path("/compiler"), catalog.AUTHORING_PROFILE_V30),
        )


if __name__ == "__main__":
    unittest.main()
