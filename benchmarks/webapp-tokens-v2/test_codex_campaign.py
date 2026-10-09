import os
import json
import stat
import sys
import contextlib
import io
import hashlib
import importlib.util
import shutil
import subprocess
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import patch

import codex_campaign as campaign
import codex_rescore as rescore
import typescript_bootstrap as bootstrap
import dependency_bundle as dependencies


ROOT = Path(__file__).resolve().parents[2]


class WebappCampaignTests(unittest.TestCase):
    def setUp(self):
        # These fixtures fake model/acceptance execution; resource admission is
        # independently exercised with low-space and ENOSPC controls.
        self.enterContext(patch("campaign_resources.snapshot", return_value=[
            {"device": 1, "path": "/fixture", "free_bytes": 10 * 1024**3}]))

    def bootstrap_fixture(self, root):
        self.enterContext(patch.object(bootstrap, "runtime_identity", return_value={
            "node_sha256": "node", "node": {"versions": {"node": "24.3.0"},
                                            "platform": "fixture", "arch": "fixture"},
            "host_platform": "fixture", "host_machine": "fixture"}))
        reference = root / "reference"; reference.mkdir()
        packages = {}
        for name in bootstrap.CORE_PACKAGES:
            path = reference / "node_modules" / name / "package.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps({"name": name, "version": "1.2.3"}))
            packages[f"node_modules/{name}"] = {"version": "1.2.3"}
        executable = reference / "node_modules/typescript/bin/tsc"
        executable.parent.mkdir(); executable.write_text("#!/bin/sh\nexit 0\n"); executable.chmod(0o555)
        link = reference / "node_modules/.bin/tsc"; link.parent.mkdir(); link.symlink_to("../typescript/bin/tsc")
        (reference / "package.json").write_text(json.dumps({"scripts": {"secret": "reference app"}}))
        (reference / "package-lock.json").write_text(json.dumps({"packages": packages}))
        (reference / "app.ts").write_text("qualified app must never be supplied")
        qualification = {"passed": True, "cases": 912, "failures": [], "missingCases": [], "missingGroups": []}
        report = root / "report.json"
        report.write_text(json.dumps({"schema": "semaprax.teamdesk.acceptance.v1", "arm": "typescript",
            "node": "v24.3.0", "spec_sha256": "spec", "qualification": qualification,
            "candidate_before": [{"name": name, "sha256": dependencies.digest(reference / name)}
                                 for name in ("package.json", "package-lock.json")]}))
        summary = root / "summary.json"
        summary.write_text(json.dumps({"schema": "semaprax.teamdesk.reference.qualification.v1",
            "qualified": True, "spec_sha256": "spec", "gate_source": "gate", "arms": {
                "typescript": {"qualification": qualification, "report_file": str(report),
                               "report_sha256": dependencies.digest(report)}}}))
        bootstrap.prepare(reference, summary, root / "tooling", "fixture-node")
        return root / "tooling/receipt.json", reference, summary

    def test_bootstrap_private_writable_copy_has_tools_and_no_app_or_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); receipt, reference, summary = self.bootstrap_fixture(root)
            candidate = root / "candidate"
            setup = bootstrap.provision(receipt, candidate, "fixture-node", spec_sha256="spec",
                                        summary_sha256=dependencies.digest(summary))
            self.assertEqual({p.name for p in candidate.iterdir()}, {"node_modules"})
            private = candidate / "node_modules/typescript/bin/tsc"
            source = root / "tooling/bundle/node_modules/typescript/bin/tsc"
            self.assertEqual(private.stat().st_mode & 0o777, 0o755)
            self.assertNotEqual(private.stat().st_ino, source.stat().st_ino)
            self.assertEqual(os.readlink(candidate / "node_modules/.bin/tsc"), "../typescript/bin/tsc")
            private.write_text("candidate can modify its tools")
            self.assertIn("exit 0", source.read_text())
            self.assertIn("exit 0", (reference / "node_modules/typescript/bin/tsc").read_text())
            self.assertFalse(setup["application_source_supplied"])
            self.assertFalse(setup["package_manifest_supplied"])
            self.assertEqual(setup["initial_inventory_sha256"], bootstrap.load(receipt)["inventory_sha256"])

    def test_bootstrap_refuses_helper_runtime_platform_and_qualification_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); receipt, _, summary = self.bootstrap_fixture(root)
            for attribute, replacement in (("module_hashes", {"dependency_bundle.py": "drift"}),
                                           ("runtime_identity", {"host_platform": "different"})):
                with patch.object(bootstrap, attribute, return_value=replacement):
                    with self.assertRaisesRegex(ValueError, "helper, runtime or platform"):
                        bootstrap.validate(receipt, "fixture-node")
            with self.assertRaisesRegex(ValueError, "differs from campaign"):
                bootstrap.validate(receipt, "fixture-node", spec_sha256="different")
            summary.write_text(summary.read_text() + "\n")
            with self.assertRaisesRegex(ValueError, "provenance drift"):
                bootstrap.validate(receipt, "fixture-node")

    def test_bootstrap_refuses_bundle_file_and_reference_dependency_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); receipt, reference, _ = self.bootstrap_fixture(root)
            executable = root / "tooling/bundle/node_modules/typescript/bin/tsc"
            original = executable.read_bytes(); executable.write_text("drift")
            with self.assertRaisesRegex(ValueError, "inventory drift"):
                bootstrap.validate(receipt, "fixture-node")
            executable.write_bytes(original)
            (reference / "node_modules/react/package.json").write_text("{}")
            with self.assertRaisesRegex(ValueError, "reference dependency drift"):
                bootstrap.validate(receipt, "fixture-node")

    def test_bootstrap_refuses_external_links_and_candidate_dependency_collision(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); receipt, _, summary = self.bootstrap_fixture(root)
            candidate = root / "candidate"; (candidate / "node_modules").mkdir(parents=True)
            with self.assertRaisesRegex(ValueError, "already contains"):
                bootstrap.provision(receipt, candidate, "fixture-node", spec_sha256="spec",
                                    summary_sha256=dependencies.digest(summary))
            escape = root / "tooling/bundle/node_modules/.bin/escape"
            escape.symlink_to("/private/outside")
            with self.assertRaisesRegex(ValueError, "external symlink"):
                bootstrap.validate(receipt, "fixture-node")

    def test_bootstrap_prompt_and_frozen_closure_keep_legacy_rescore_contract(self):
        self.assertIs(rescore.dependency_inventory, rescore.dependencies.dependency_inventory)
        self.assertIs(rescore.copy_dependency_bundle, rescore.dependencies.copy_dependency_bundle)
        self.assertIn("benchmarks/webapp-tokens-v2/dependency_bundle.py", campaign.HARNESS_SOURCE_FILES)
        self.assertIn("benchmarks/webapp-tokens-v2/typescript_bootstrap.py", campaign.HARNESS_SOURCE_FILES)
        candidate, compiler = Path("/candidate"), Path("/compiler")
        legacy = campaign.prompt_for("typescript", candidate, compiler)
        self.assertEqual(legacy, campaign.prompt_for("typescript", candidate, compiler, None))
        tooling = {"packages": {"react": "18.3.1", "typescript": "5.9.3"}}
        supplied = campaign.prompt_for("typescript", candidate, compiler, tooling)
        self.assertIn("react 18.3.1", supplied)
        self.assertIn("Author your own package.json", supplied)
        self.assertIn("already exists with private dependency tools", supplied)
        self.assertNotIn("Create the new implementation root", supplied)
        self.assertNotIn("node_modules", legacy)
        self.assertEqual(campaign.prompt_for("semaprax", candidate, compiler),
                         campaign.prompt_for("semaprax", candidate, compiler, tooling))

    def test_bootstrap_sibling_loading_ignores_cwd_and_ambient_helper_modules(self):
        benchmark = Path(__file__).resolve().parent
        shadow = types.ModuleType("untrusted_shadow")
        original_cwd = Path.cwd()
        original_path = list(sys.path)
        with tempfile.TemporaryDirectory() as directory, \
                patch.dict(sys.modules, {"typescript_bootstrap": shadow, "dependency_bundle": shadow}):
            try:
                os.chdir(directory)
                sys.path[:] = [p for p in original_path if p and Path(p).resolve() != benchmark]
                loaded = {}
                for name in ("codex_campaign", "typescript_bootstrap", "codex_rescore"):
                    spec = importlib.util.spec_from_file_location(f"external_{name}", benchmark / f"{name}.py")
                    module = importlib.util.module_from_spec(spec)
                    spec.loader.exec_module(module)
                    loaded[name] = module
                self.assertEqual(Path(loaded["codex_campaign"].bootstrap.__file__).resolve(),
                                 benchmark / "typescript_bootstrap.py")
                for module in (loaded["codex_campaign"].bootstrap, loaded["typescript_bootstrap"], loaded["codex_rescore"]):
                    self.assertEqual(Path(module.dependencies.__file__).resolve(), benchmark / "dependency_bundle.py")
                    self.assertTrue(callable(module.dependencies.copy_dependency_bundle))
                self.assertIs(sys.modules["typescript_bootstrap"], shadow)
                self.assertIs(sys.modules["dependency_bundle"], shadow)
            finally:
                os.chdir(original_cwd)
                sys.path[:] = original_path

    def test_bootstrap_drift_refuses_a_paid_request_before_invocation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); receipt, _, summary = self.bootstrap_fixture(root)
            compiler = root / "compiler"
            compiler.write_text("#!/bin/sh\nexit 0\n"); compiler.chmod(0o755)
            settings = {"typescript_bootstrap": {"receipt_path": str(receipt),
                        "receipt_sha256": dependencies.digest(receipt)},
                        "qualification": {"spec_sha256": "spec", "receipt_sha256": dependencies.digest(summary)},
                        "acceptance": {"capabilities": {"node_binary": "fixture-node"}},
                        "compiler_source_commit": "a" * 40,
                        "source_binary_sha256": campaign.common.digest(compiler)}
            receipt.write_text(receipt.read_text() + "\n")
            with patch.object(campaign, "add_seed_worktree", return_value=None), \
                    patch.object(campaign, "cleanup_trial"), patch.object(campaign, "run_codex") as paid:
                row = campaign.launch_trial(root, root / "artifacts", "commit",
                                            {"arm": "typescript", "number": 1}, settings, compiler)
            paid.assert_not_called()
            self.assertFalse(row["paid_request_launched"])
            self.assertIn("receipt changed after campaign plan", row["failure"])

    def test_new_rescore_retains_hash_of_relocated_dependency_module(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); module = root / "tooling-source/dependency_bundle.py"
            module.parent.mkdir(); module.write_text("copy implementation")
            sidecar = root / "rescore.json"
            sidecar.write_text(json.dumps({"schema": rescore.SCHEMA, "dependency_copy_module": {
                "snapshot_path": "tooling-source/dependency_bundle.py", "sha256": dependencies.digest(module)}}))
            module.write_text("drift")
            with self.assertRaisesRegex(ValueError, "copy module hash drifted"):
                rescore.validate_sidecar(sidecar, ROOT)

    def test_rescore_requires_a_terminal_complete_ten_trial_receipt(self):
        rows = [{"arm": arm, "number": number}
                for arm in campaign.ARMS for number in range(1, 6)]
        receipt = {"schema": rescore.TERMINAL_SCHEMA, "status": "complete",
                   "campaign_sha256": "campaign", "results_sha256": "results",
                   "process_exit_code": 0, "trials": rows}
        self.assertEqual(rescore.terminal_trials({"campaign_status": "complete", "trials": rows},
                                                 receipt, "results", "campaign"), rows)
        with self.assertRaisesRegex(ValueError, "ten-trial"):
            rescore.terminal_trials({"campaign_status": "running", "trials": rows[:4]},
                                    receipt, "results", "campaign")

    def test_rescore_admits_receipt_finalized_full_interruption_but_not_running(self):
        rows = [{"arm": arm, "number": number, "resource_assessment": {"contaminated": arm == "typescript" and number == 5}}
                for arm in campaign.ARMS for number in range(1, 6)]
        receipt = {"schema": rescore.TERMINAL_SCHEMA, "status": "finalized", "campaign_status": "interrupted",
                   "campaign_sha256": "campaign", "results_sha256": "results", "process_exit_code": 0,
                   "actual_process_exit_code": 0, "unlaunched_trial_order": [], "trial_ids": [
                       {"arm": row["arm"], "number": row["number"]} for row in rows]}
        self.assertEqual(rescore.terminal_trials({"campaign_status": "interrupted", "unlaunched_trial_order": [], "trials": rows}, receipt, "results", "campaign"), rows)
        with self.assertRaisesRegex(ValueError, "finalized"):
            rescore.terminal_trials({"campaign_status": "running", "unlaunched_trial_order": [], "trials": rows}, receipt, "results", "campaign")

    def test_rescore_rejects_duplicate_or_missing_trial_identity(self):
        rows = [{"arm": arm, "number": number}
                for arm in campaign.ARMS for number in range(1, 6)]
        duplicate = [*rows[:-1], {"arm": "typescript", "number": 4}]
        receipt = {"schema": rescore.TERMINAL_SCHEMA, "status": "complete",
                   "campaign_sha256": "campaign", "results_sha256": "results",
                   "process_exit_code": 0, "trials": duplicate}
        with self.assertRaisesRegex(ValueError, "duplicate"):
            rescore.terminal_trials({"campaign_status": "complete", "trials": duplicate},
                                    receipt, "results", "campaign")

    def test_rescore_archive_copy_rejects_symlinks_and_preserves_file_mode(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); archive = root / "archive"; archive.mkdir()
            source = archive / "run.sh"; source.write_text("#!/bin/sh\n")
            source.chmod(0o755)
            expected = rescore.rel_file_inventory(archive)
            copied = rescore.copy_closed_archive(archive, root / "copy", expected)
            self.assertEqual(copied, expected)
            self.assertEqual(stat.S_IMODE((root / "copy/run.sh").stat().st_mode), 0o755)
            (archive / "escape").symlink_to(source)
            with self.assertRaisesRegex(ValueError, "symlink"):
                rescore.rel_file_inventory(archive)

    def test_rescore_reference_report_hash_drift_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); report = root / "reference.json"; report.write_text("{}")
            receipt = {"arms": {"typescript": {"report_file": str(report), "report_sha256": "0" * 64}}}
            with self.assertRaisesRegex(ValueError, "hash binding"):
                rescore.reference_case_groups(receipt, "typescript", {"qualification": {"spec_sha256": "x"},
                    "harness_source_snapshot": {"files_sha256": {}}, "compiler_source_commit": "y"}, "z")

    def test_rescore_reference_summary_ids_cannot_replace_full_report(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); report = root / "reference.json"; report.write_text(json.dumps({"checks": []}))
            receipt = {"arms": {"typescript": {"report_file": str(report),
                       "report_sha256": rescore.digest(report), "spotlight": [{"id": f"forged-{i}"} for i in range(912)]}}}
            with self.assertRaisesRegex(ValueError, "full accepted"):
                rescore.reference_case_groups(receipt, "typescript", {"qualification": {"spec_sha256": "x"},
                    "harness_source_snapshot": {"files_sha256": {}}, "compiler_source_commit": "y"}, "z")

    def test_rescore_accepted_report_requires_exact_reference_case_groups(self):
        runner = "benchmarks/webapp-tokens-v2/acceptance/run.mjs"
        settings = {"qualification": {"spec_sha256": "x"},
                    "harness_source_snapshot": {"files_sha256": {runner: "a" * 64}},
                    "compiler_source_commit": "y"}
        groups = {"one", "two"}; required = {f"case-{index}": "one" if index % 2 else "two" for index in range(912)}
        checks = [{"id": case, "group": group, "status": "passed"} for case, group in required.items()]
        report = {"schema": "semaprax.teamdesk.acceptance.v1", "arm": "typescript", "spec_sha256": "x",
                  "gate": [{"name": "run.mjs", "sha256": "a" * 64}], "checks": checks,
                  "qualification": {"passed": True, "cases": 912, "missingCases": [], "missingGroups": [], "failures": []}}
        self.assertTrue(rescore.report_is_accepted(report, "typescript", settings, "unused", required, groups))
        report["checks"][0]["group"] = "one" if report["checks"][0]["group"] == "two" else "two"
        self.assertFalse(rescore.report_is_accepted(report, "typescript", settings, "unused", required, groups))


    def test_rescore_hostile_settings_candidate_and_wall_bindings(self):
        campaign_record = {"artifacts": "original", "harness_source_snapshot": {"path": "old", "files_sha256": {}},
                           "qualification": {"spec_sha256": "old", "required_cases": 1}}
        gate = {"path": "gate-source", "runner_files_sha256": {"runner": "a" * 64},
                "original_frozen_inputs": {"spec_sha256": "b" * 64}}
        settings = rescore.rescore_settings(campaign_record, Path("/fresh-output"), gate)
        self.assertEqual(settings["artifacts"], "/fresh-output")
        self.assertNotEqual({**settings, "artifacts": "tampered"}, settings)
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "archive"; archive.mkdir(); (archive / "app").write_text("v1")
            inventory = rescore.rel_file_inventory(archive)
            source, row = {"candidate_archive": str(archive), "candidate_files_sha256": inventory}, {
                "candidate_files_sha256": inventory, "new_acceptance": {"seconds": 1.25},
                "new_acceptance_wall_seconds": 1.25}
            self.assertTrue(rescore.retained_candidate_matches(source, row))
            self.assertTrue(rescore.scored_wall_matches(row))
            row["new_acceptance_wall_seconds"] = 1.5
            self.assertFalse(rescore.scored_wall_matches(row))
            (archive / "app").write_text("tampered")
            self.assertFalse(rescore.retained_candidate_matches(source, row))

    def test_rescore_dependency_bundle_copy_preserves_modes_and_internal_bin_link(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); bundle = root / "bundle"; candidate = root / "candidate"
            executable = bundle / "node_modules/pkg/bin/tool"
            executable.parent.mkdir(parents=True); executable.write_text("#!/bin/sh\n"); executable.chmod(0o755)
            link = bundle / "node_modules/.bin/tool"; link.parent.mkdir(parents=True)
            link.symlink_to("../pkg/bin/tool")
            candidate.mkdir(); (candidate / "app.ts").write_text("source")
            inventory = rescore.dependency_inventory(bundle)
            copied = rescore.copy_dependency_bundle(bundle, candidate, inventory)
            self.assertEqual(copied, rescore.dependency_fingerprint(inventory))
            self.assertEqual((candidate / "node_modules/pkg/bin/tool").stat().st_mode & 0o777, 0o755)
            self.assertEqual(os.readlink(candidate / "node_modules/.bin/tool"), "../pkg/bin/tool")
            escape = candidate / "node_modules/.bin/escape"
            escape.symlink_to("../../app.ts")
            with self.assertRaisesRegex(ValueError, "root symlink"):
                rescore.dependency_inventory(candidate, allow_other=True)
            escape.unlink()
            executable.write_text("drift")
            (root / "other").mkdir()
            with self.assertRaisesRegex(ValueError, "inventory or hashes"):
                rescore.copy_dependency_bundle(bundle, root / "other", inventory)

    def test_rescore_dependency_bundle_refuses_external_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            bundle = Path(directory) / "bundle"
            link = bundle / "node_modules/.bin/tool"; link.parent.mkdir(parents=True)
            link.symlink_to("/private/outside")
            with self.assertRaisesRegex(ValueError, "external symlink"):
                rescore.dependency_inventory(bundle)

    def test_rescore_malformed_candidate_rows_are_not_accepted(self):
        settings = {"qualification": {"spec_sha256": "x"},
                    "harness_source_snapshot": {"files_sha256": {}}, "compiler_source_commit": "y"}
        malformed = {"schema": "semaprax.teamdesk.acceptance.v1", "arm": "typescript", "spec_sha256": "x",
                     "gate": [{"name": [], "sha256": "a"}], "checks": [{"id": [], "group": "g", "status": "passed"}],
                     "qualification": {"passed": True, "cases": 912, "missingCases": [], "missingGroups": [], "failures": []}}
        self.assertFalse(rescore.report_is_accepted(malformed, "typescript", settings, "unused", {}, {"g"}))

    def test_rescore_snapshot_keeps_contract_outside_runtime_gate_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); output = root / "output"; contract = "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md"
            for relative in campaign.ACCEPTANCE_SOURCE_FILES:
                path = root / relative; path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("export const COVERAGE = ['g'];\n" if path.name == "contract.mjs" else f"fixture {relative}\n")
            contract_path = root / contract; contract_path.write_text("contract fixture\n")
            receipt, clarification = root / "receipt.json", root / "clarification.md"
            receipt.write_text("{}"); clarification.write_text("clarification\n")
            original = {"seed_files_sha256": {campaign.FROZEN_SPEC: rescore.digest(root / campaign.FROZEN_SPEC),
                        contract: rescore.digest(contract_path)}}
            commit = "a" * 40
            with patch.object(campaign, "validate_qualification_receipt", return_value=commit), \
                    patch.object(campaign, "resolve_commit", return_value=commit), \
                    patch.object(campaign, "_git_file", side_effect=lambda _repo, _commit, rel: (root / rel).read_bytes()):
                gate = rescore.snapshot_gate(root, output, original, clarification, receipt, commit)
            runtime = output / "gate-source/benchmarks/webapp-tokens-v2/acceptance"
            self.assertFalse((runtime / "CONTRACT.md").exists())
            self.assertEqual(rescore.contract_groups(runtime / "contract.mjs"), {"g"})
            self.assertEqual({path.relative_to(runtime).as_posix() for path in runtime.rglob("*") if path.is_file()},
                             {Path(name).name for name in campaign.ACCEPTANCE_SOURCE_FILES if "/acceptance/" in name})
            self.assertEqual(rescore.digest(output / gate["contract_document"]["snapshot_path"]), gate["contract_document"]["sha256"])
            self.assertTrue(rescore.gate_inventory_shapes(gate))

    def test_rescore_refuses_invalid_jobs_and_nonfinite_wall_before_reading_originals(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "rescore.json"
            for jobs, wall in [(0, 1), (3, 1), (True, 1), ([], 1), (1, float("nan")),
                               (2, float("inf")), (1, -1), (1, True), (1, "slow")]:
                with self.subTest(jobs=jobs, wall=wall):
                    path.write_text(json.dumps({"schema": rescore.SCHEMA,
                                               "rescore_jobs": jobs, "rescore_wall_seconds": wall}))
                    with self.assertRaisesRegex(ValueError, "invalid jobs or wall-time"):
                        rescore.validate_sidecar(path, Path(directory))

    def test_rescore_hostile_gate_inventory_shape_is_refused(self):
        runner = set(campaign.ACCEPTANCE_SOURCE_FILES)
        hashes = {name: "a" * 64 for name in runner}
        gate = {"files_sha256": hashes, "runner_files_sha256": {name: hashes[name] for name in runner},
                "contract_document": {"repo_path": "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md",
                                      "sha256": "b" * 64, "snapshot_path": "gate-metadata/CONTRACT.md"}}
        self.assertTrue(rescore.gate_inventory_shapes(gate))
        gate["runner_files_sha256"]["forged"] = "b" * 64
        self.assertFalse(rescore.gate_inventory_shapes(gate))
        gate["runner_files_sha256"].pop("forged")
        for key, value in (("repo_path", "wrong"), ("snapshot_path", "gate-source/CONTRACT.md"), ("sha256", 7)):
            hostile = {**gate, "contract_document": {**gate["contract_document"], key: value}}
            self.assertFalse(rescore.gate_inventory_shapes(hostile))

    def test_plan_pins_public_seed_receipt_and_matched_order(self):
        directory = Path(self.enterContext(tempfile.TemporaryDirectory()))
        tokenizer_dir = directory / "tokenizer"
        tokenizer_dir.mkdir()
        tokenizer_fixture = tokenizer_dir / "fixture.json"
        tokenizer_fixture.write_bytes(b'{"fixture":"offline tokenizer metadata"}\n')
        tokenizer = {"package": campaign.common.TOKENIZER_PACKAGE,
                     "version": campaign.common.TOKENIZER_VERSION,
                     "tokenizer_dir": str(tokenizer_dir),
                     "fingerprint_sha256": campaign.common.digest(tokenizer_fixture)}
        metadata = self.enterContext(patch.object(campaign.common, "tokenizer_metadata", return_value=tokenizer))
        args = type("Args", (), {
            "repo": str(ROOT), "base_ref": "HEAD", "compiler_source_ref": "HEAD",
            "artifacts": str(directory / "artifacts"), "semaprax_bin": "/bin/sh",
            "tokenizer_dir": str(tokenizer_dir), "codex_binary": "/usr/bin/true",
            "node_binary": "node", "playwright_root": str(ROOT / "benchmarks/webapp-tokens-v2/acceptance"),
            "model": campaign.MODEL, "effort": campaign.EFFORT, "trials_per_arm": 5,
            "timeout_seconds": campaign.TIMEOUT_SECONDS,
        })
        receipt_path = ROOT / campaign.QUALIFICATION_RECEIPT
        receipt = json.loads(receipt_path.read_text())
        receipt["compiler_source_commit"] = campaign.resolve_commit(ROOT, "HEAD")
        receipt["compiler_binary_sha256"] = campaign.common.digest(Path(args.semaprax_bin).resolve())
        original_read_text = Path.read_text

        def read_current_receipt(path, *read_args, **read_kwargs):
            if path == receipt_path:
                return json.dumps(receipt)
            return original_read_text(path, *read_args, **read_kwargs)

        self.enterContext(patch.object(Path, "read_text", read_current_receipt))
        settings = campaign.plan(args)
        metadata.assert_called_once_with(str(tokenizer_dir))
        self.assertEqual(settings["authored_source_tokenizer"], tokenizer)
        self.assertEqual(settings["attempt_denominator"], 10)
        self.assertEqual(settings["trial_order"], ["semaprax", "typescript", "typescript", "semaprax",
                                                      "semaprax", "typescript", "typescript", "semaprax",
                                                      "semaprax", "typescript"])
        self.assertEqual(settings["qualification"]["required_cases"], 912)
        self.assertEqual(settings["round"], 1)
        self.assertEqual(set(settings["seed_files_sha256"]), {
            "benchmarks/webapp-tokens-v2/SPEC.md",
            "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md",
        })
        args.round = 2
        self.assertEqual(campaign.plan(args)["round"], 2)
        args.round = 0
        with self.assertRaisesRegex(ValueError, "positive integer"):
            campaign.plan(args)

    def test_qualification_rejects_stale_compiler_with_the_same_gate_and_spec(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative in campaign.ACCEPTANCE_SOURCE_FILES:
                source = root / relative
                source.parent.mkdir(parents=True, exist_ok=True)
                source.write_text(f"fixture {relative}\n")
            spec_sha = campaign.common.digest(root / campaign.FROZEN_SPEC)
            compiler_source = "a" * 40
            compiler_binary = "b" * 64
            qualification = {"passed": True, "cases": 912, "missingCases": [],
                             "missingGroups": [], "failures": []}
            receipt = {"schema": "semaprax.teamdesk.reference.qualification.v1",
                       "qualified": True, "spec_sha256": spec_sha, "gate_source": "gate",
                       "compiler_source_commit": compiler_source,
                       "compiler_binary_sha256": compiler_binary,
                       "arms": {arm: {"qualification": qualification} for arm in campaign.ARMS}}
            with patch.object(campaign, "resolve_commit", return_value="c" * 40), \
                    patch.object(campaign, "_git_file",
                                 side_effect=lambda repo, commit, relative: (repo / relative).read_bytes()):
                self.assertEqual(campaign.validate_qualification_receipt(
                    root, receipt, spec_sha, compiler_source_commit=compiler_source,
                    compiler_binary_sha256=compiler_binary), "c" * 40)
                for field, stale in (("compiler_source_commit", "d" * 40),
                                     ("compiler_binary_sha256", "e" * 64),
                                     ("compiler_source_commit", None),
                                     ("compiler_binary_sha256", None)):
                    with self.subTest(field=field, stale=stale):
                        changed = {**receipt, field: stale}
                        with self.assertRaisesRegex(ValueError, "fresh reference qualification"):
                            campaign.validate_qualification_receipt(
                                root, changed, spec_sha, compiler_source_commit=compiler_source,
                                compiler_binary_sha256=compiler_binary)
                        # Historical rescore still authenticates its own gate/SPEC
                        # without relabelling its compiler as the current one.
                        self.assertEqual(campaign.validate_qualification_receipt(
                            root, changed, spec_sha), "c" * 40)

    def test_cli_rejects_non_positive_round_before_planning(self):
        with patch.object(sys, "argv", ["campaign", "plan", "--round", "0"]), \
                contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as error:
                campaign.main()
        self.assertEqual(error.exception.code, 2)

    def test_seed_hashes_and_repository_are_exported_from_base_ref(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, seed = root / "repo", root / "seed"
            (repo / "benchmarks/webapp-tokens-v2/acceptance").mkdir(parents=True)
            subprocess.run(["git", "init", "--quiet", "--template="], cwd=repo, check=True)
            spec = repo / campaign.FROZEN_SPEC
            contract = repo / "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md"
            spec.write_text("spec-v1\n"); contract.write_text("contract-v1\n")
            subprocess.run(["git", "add", "."], cwd=repo, check=True)
            subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                            "commit", "--quiet", "-m", "v1"], cwd=repo, check=True)
            first = subprocess.run(["git", "rev-parse", "HEAD"], cwd=repo, text=True,
                                   capture_output=True, check=True).stdout.strip()
            spec.write_text("spec-v2\n"); contract.write_text("contract-v2\n")
            subprocess.run(["git", "add", "."], cwd=repo, check=True)
            subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                            "commit", "--quiet", "-m", "v2"], cwd=repo, check=True)
            hashes = campaign.pinned_seed_hashes(repo, first)
            self.assertEqual(hashes[campaign.FROZEN_SPEC], hashlib.sha256(b"spec-v1\n").hexdigest())
            self.assertNotEqual(hashes[campaign.FROZEN_SPEC], campaign.common.digest(spec))
            created = campaign.create_seed_repository(repo, first, seed)
            self.assertEqual(created["source_files_sha256"], hashes)
            self.assertEqual((seed / campaign.FROZEN_SPEC).read_bytes(), b"spec-v1\n")

    def test_harness_snapshot_contains_complete_acceptance_runtime_closure(self):
        commit = campaign.resolve_commit(ROOT, "HEAD")
        seed_hashes = campaign.pinned_seed_hashes(ROOT, commit)
        inventory = campaign.harness_source_inventory(ROOT, seed_hashes[campaign.FROZEN_SPEC])
        imports = set()
        for relative in campaign.ACCEPTANCE_SOURCE_FILES:
            if not relative.endswith(".mjs"):
                continue
            for line in (ROOT / relative).read_text().splitlines():
                if " from './" in line:
                    imported = line.split(" from './", 1)[1].split("'", 1)[0]
                    imports.add(f"benchmarks/webapp-tokens-v2/acceptance/{imported}")
        self.assertTrue(imports.issubset(inventory))
        self.assertIn("benchmarks/webapp-tokens-v2/acceptance/kdf-observer.mjs", inventory)
        self.assertIn(campaign.QUALIFICATION_RECEIPT, inventory)
        with tempfile.TemporaryDirectory() as directory:
            artifacts = Path(directory) / "artifacts"; artifacts.mkdir()
            snapshot = campaign.snapshot_harness_sources(
                ROOT, artifacts, inventory, commit, seed_hashes[campaign.FROZEN_SPEC])
            copied = {path.relative_to(artifacts / "harness-source").as_posix()
                      for path in (artifacts / "harness-source").rglob("*") if path.is_file()}
            self.assertEqual(copied, {*campaign.HARNESS_SOURCE_FILES, "manifest.json"})
            self.assertEqual(snapshot["files_sha256"], inventory)

    def test_seed_contains_only_spec_and_launch_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            seed = Path(directory) / "seed"
            source_commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT,
                                           text=True, capture_output=True, check=True).stdout.strip()
            info = campaign.create_seed_repository(ROOT, source_commit, seed)
            self.assertEqual(info["seed_files_sha256"], info["source_files_sha256"])
            listed = subprocess.run(["git", "ls-tree", "-r", "--name-only", "HEAD"], cwd=seed,
                                    text=True, capture_output=True, check=True).stdout.splitlines()
            self.assertEqual(listed, [path.lstrip("/") for path in campaign.SEED_FILES])
            self.assertNotIn("benchmarks/webapp-tokens-v2/acceptance/run.mjs", listed)

    def test_rollout_usage_reconciles_only_matching_final_total(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            stream = root / "exec.jsonl"
            rollout = root / "rollout.jsonl"
            usage = {"input_tokens": 20, "cached_input_tokens": 4,
                     "cache_write_input_tokens": 8, "output_tokens": 3,
                     "reasoning_output_tokens": 1}
            stream.write_text(json.dumps({"type": "turn.completed", "usage": usage}) + "\n")
            rollout.write_text("\n".join([
                json.dumps({"type": "turn_context", "payload": {"model": campaign.MODEL, "effort": campaign.EFFORT}}),
                json.dumps({"type": "token_usage_record", "payload": {"response_id": "r1", "usage": usage}}),
            ]) + "\n")
            observed = campaign.trace_usage(campaign.parse_exec_jsonl(stream), rollout)
            self.assertTrue(observed["reconciled"])
            self.assertEqual(observed["model_request_count"], 1)
            self.assertEqual(campaign.list_price_estimate(observed["model_requests"])["actual_billed_usd"], None)

    def test_rollout_usage_rejects_malformed_line_when_totals_match(self):
        usage = {"input_tokens": 20, "cached_input_tokens": 4,
                 "cache_write_input_tokens": 8, "output_tokens": 3,
                 "reasoning_output_tokens": 1}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rollout = root / "rollout.jsonl"
            rollout.write_text("\n".join([
                json.dumps({"type": "turn_context", "payload": {"model": campaign.MODEL, "effort": campaign.EFFORT}}),
                json.dumps({"type": "token_usage_record", "payload": {"response_id": "r1", "usage": usage}}),
                "{malformed",
            ]))
            observed = campaign.trace_usage({"final_turn_usage": usage}, rollout)
        self.assertFalse(observed["reconciled"])
        self.assertEqual(observed["rollout_malformed_lines"], 1)
        self.assertEqual(observed["rollout_orphan_usage_records"], 0)
        self.assertEqual(observed["model_requests"], [])

    def test_rollout_usage_rejects_orphan_record_when_totals_match(self):
        usage = {"input_tokens": 20, "cached_input_tokens": 4,
                 "cache_write_input_tokens": 8, "output_tokens": 3,
                 "reasoning_output_tokens": 1}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rollout = root / "rollout.jsonl"
            rollout.write_text("\n".join([
                json.dumps({"type": "turn_context", "payload": {"model": campaign.MODEL, "effort": campaign.EFFORT}}),
                json.dumps({"type": "token_usage_record", "payload": {"response_id": "r1", "usage": usage}}),
                json.dumps({"type": "token_usage_record", "payload": {"usage": {key: 0 for key in usage}}}),
            ]) + "\n")
            observed = campaign.trace_usage({"final_turn_usage": usage}, rollout)
        self.assertFalse(observed["reconciled"])
        self.assertEqual(observed["rollout_malformed_lines"], 0)
        self.assertEqual(observed["rollout_orphan_usage_records"], 1)
        self.assertEqual(observed["model_requests"], [])

    def test_model_task_timeout_keeps_all_ten_matched_attempts(self):
        with tempfile.TemporaryDirectory() as directory:
            artifacts = Path(directory) / "campaign"
            order = ["semaprax", "typescript", "typescript", "semaprax", "semaprax",
                     "typescript", "typescript", "semaprax", "semaprax", "typescript"]
            snapshot = {"files_sha256": {}}
            settings = {"round": 1, "model_requested": campaign.MODEL, "effort_requested": campaign.EFFORT,
                "timeout_seconds": campaign.TIMEOUT_SECONDS, "attempt_denominator": 10,
                "capabilities": {"status": "ready"},
                "acceptance": {"capabilities": {"status": "ready"}},
                "artifacts": str(artifacts), "harness_source_snapshot": snapshot,
                "compiler_source_commit": "a" * 40,
                "source_binary_sha256": campaign.common.digest(Path("/bin/sh").resolve()),
                "repository_commit": "seed", "seed_files_sha256": {campaign.FROZEN_SPEC: "hash"},
                "trial_order": order}
            attempts = []
            def trial(_repo, _artifacts, _commit, requested, _settings, _compiler):
                attempts.append(requested)
                timeout = len(attempts) == 1
                return {**requested, "status": "failed" if timeout else "accepted",
                    "timed_out": timeout, "process_exit_code": -15 if timeout else 0,
                    "telemetry_valid": not timeout}
            argv = ["campaign", "run", "--repo", str(ROOT), "--base-ref", "HEAD",
                "--compiler-source-ref", "HEAD", "--artifacts", str(artifacts),
                "--semaprax-bin", "/bin/sh", "--tokenizer-dir", "/tmp/tokenizer",
                "--acknowledge-paid-attempts"]
            with patch.object(sys, "argv", argv), patch.object(campaign, "plan", return_value=settings), \
                    patch.object(campaign, "snapshot_harness_sources", return_value=snapshot), \
                    patch.object(campaign, "create_seed_repository", return_value={"seed_repository_commit": "seed"}), \
                    patch.object(campaign, "launch_calibration", return_value={"status": "ready"}), \
                    patch.object(campaign, "launch_trial", side_effect=trial), \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(campaign.main(), 0)
            result = json.loads((artifacts / "results.json").read_text())
            self.assertEqual(result["campaign"]["round"], 1)
            self.assertEqual(result["campaign"]["model_requested"], campaign.MODEL)
            self.assertEqual(result["campaign"]["effort_requested"], campaign.EFFORT)
            self.assertEqual([row["arm"] for row in attempts], order)
            self.assertEqual(result["campaign_status"], "complete")
            self.assertEqual(result["summary"]["failed_or_rejected_attempts"], 1)
            self.assertIsNone(result["summary"]["list_price_estimate_per_accepted_task_usd"])

    def test_changed_compiler_refuses_dispatch_before_artifacts_worktrees_or_model(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            compiler = root / "compiler"
            compiler.write_text("#!/bin/sh\nexit 0\n"); compiler.chmod(0o755)
            wanted = campaign.common.digest(compiler)
            settings = {"compiler_source_commit": "a" * 40, "source_binary_sha256": wanted,
                        "qualification": {"compiler_source_commit": "a" * 40,
                                          "compiler_binary_sha256": wanted},
                        "artifacts": str(root / "artifacts"),
                        "capabilities": {"status": "ready"},
                        "acceptance": {"capabilities": {"status": "ready"}}}
            compiler.write_text("#!/bin/sh\nexit 1\n")
            argv = ["campaign", "run", "--repo", str(ROOT), "--base-ref", "HEAD",
                    "--compiler-source-ref", "HEAD", "--artifacts", settings["artifacts"],
                    "--semaprax-bin", str(compiler), "--tokenizer-dir", "/tmp/tokenizer",
                    "--acknowledge-paid-attempts"]
            with patch.object(campaign, "add_seed_worktree") as worktree, \
                    patch.object(campaign, "create_seed_repository") as seed, \
                    patch.object(campaign, "run_codex") as paid:
                # The main guard runs before the resource-monitor wrapper and
                # therefore refuses before creating any campaign artifacts.
                with patch.object(sys, "argv", argv), patch.object(campaign, "plan", return_value=settings), \
                        contextlib.redirect_stderr(io.StringIO()) as error:
                    self.assertEqual(campaign.main(), 2)
                self.assertIn("compiler", error.getvalue().lower())
                self.assertFalse((root / "artifacts").exists())
                # Individual attempt wrappers retain their failure audit
                # receipts while preventing worktrees and paid dispatch.
                rows = [campaign.launch_trial(root, root / "artifacts", "seed",
                                              {"arm": "semaprax", "number": 1}, settings, compiler),
                        campaign.launch_calibration(root, root / "artifacts", "seed", settings, compiler)]
                for row in rows:
                    self.assertEqual(row["status"], "failed")
                    self.assertTrue(row["runner_error"])
                    self.assertIn("compiler binary differs from the immutable campaign plan", row["failure"])
                    receipt = Path(row["resource_assessment"]["receipt"])
                    document = json.loads(receipt.read_text())
                    self.assertEqual(document["state"], "finished")
                    self.assertEqual(document["row"]["failure"], row["failure"])
                worktree.assert_not_called(); seed.assert_not_called(); paid.assert_not_called()
                self.assertFalse((root / "artifacts" / "worktrees").exists())

    def test_compiler_changed_during_preparation_refuses_model_dispatch(self):
        for kind in ("calibration", "trial"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                compiler = root / "compiler"
                compiler.write_text("#!/bin/sh\nexit 0\n"); compiler.chmod(0o755)
                settings = {"compiler_source_commit": "a" * 40,
                            "source_binary_sha256": campaign.common.digest(compiler)}

                def prepare(_repo, _workspace, _commit):
                    compiler.write_text("#!/bin/sh\nexit 1\n")
                    return None

                with patch.object(campaign, "add_seed_worktree", side_effect=prepare), \
                        patch.object(campaign, "run_codex") as paid, \
                        patch.object(campaign, "prompt_for", return_value="task"), \
                        patch.object(campaign, "cleanup_trial"), \
                        patch.object(campaign, "workspace_guard", return_value={"status": "failed"}), \
                        patch.object(campaign.common, "authored_source_metrics", return_value={}):
                    if kind == "calibration":
                        row = campaign.launch_calibration(root, root / "artifacts", "seed", settings, compiler)
                    else:
                        row = campaign.launch_trial(root, root / "artifacts", "seed",
                                                    {"arm": "semaprax", "number": 1}, settings, compiler)
                    self.assertEqual(row["status"], "failed" if kind == "calibration" else "not_accepted")
                    self.assertTrue(row["runner_error"])
                    paid.assert_not_called()

    def test_paid_acceptance_timeout_returns_a_persistable_failed_row(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifacts = root / "artifacts"; artifacts.mkdir()
            seed = root / "seed"; seed.mkdir()
            spec_bytes, contract_bytes = b"spec\n", b"contract\n"
            compiler = root / "semaprax"
            compiler.write_text("#!/bin/sh\nexit 0\n"); compiler.chmod(0o755)
            settings = {
                "codex_binary": "codex", "timeout_seconds": 1800,
                "acceptance_timeout_seconds": 2700, "authored_source_tokenizer": None,
                "compiler_source_commit": "a" * 40,
                "source_binary_sha256": campaign.common.digest(compiler),
                "seed_files_sha256": {
                    campaign.FROZEN_SPEC: hashlib.sha256(spec_bytes).hexdigest(),
                    "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md": hashlib.sha256(contract_bytes).hexdigest(),
                },
            }

            def add_worktree(_repo, workspace, _commit):
                (workspace / "benchmarks/webapp-tokens-v2/acceptance").mkdir(parents=True)
                (workspace / campaign.FROZEN_SPEC).write_bytes(spec_bytes)
                (workspace / "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md").write_bytes(contract_bytes)
                return None

            def paid_run(_command, workspace, _env, stream, stderr, _timeout):
                self.assertFalse((workspace / "candidate").exists())
                (workspace / "candidate").mkdir()
                (workspace / "candidate/app.ts").write_text("export {};\n")
                stream.write_text("\n"); stderr.write_text("")
                return {"process_exit_code": 0, "timed_out": False, "elapsed_seconds": 1.25}

            trace = {"reconciled": True, "model_observed": campaign.MODEL,
                     "effort_observed": campaign.EFFORT, "model_requests": [],
                     "request_usage_sum": {}, "legacy_net_input_tokens": 0}
            (root / "trace.jsonl").write_text("{}\n")
            parsed = {"thread_ids": ["00000000-0000-0000-0000-000000000000"],
                      "invalid_stream_lines": 0, "tool_item_counts": {}}
            with patch.object(campaign, "add_seed_worktree", side_effect=add_worktree), \
                    patch.object(campaign, "run_codex", side_effect=paid_run), \
                    patch.object(campaign, "parse_exec_jsonl", return_value=parsed), \
                    patch.object(campaign, "copy_task_rollout", return_value=root / "trace.jsonl"), \
                    patch.object(campaign, "trace_usage", return_value=trace), \
                    patch.object(campaign, "check_candidate",
                                 side_effect=subprocess.TimeoutExpired(["node", "run.mjs"], 2700)), \
                    patch.object(campaign, "cleanup_trial"):
                row = campaign.launch_trial(seed, artifacts, "seed-commit",
                                            {"arm": "typescript", "number": 1}, settings, compiler)
            self.assertEqual(row["status"], "failed")
            self.assertTrue(row["runner_error"])
            self.assertGreaterEqual(row["acceptance_elapsed_seconds"], 0)
            self.assertIn("timed out", row["failure"])
            self.assertEqual(row["elapsed_seconds"], 1.25)
            self.assertEqual(row["candidate_files_sha256"], {"app.ts": campaign.common.digest(
                Path(row["candidate_archive"]) / "app.ts")})

    def test_workspace_guard_refuses_seed_and_outside_candidate_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            spec = workspace / campaign.FROZEN_SPEC
            contract = workspace / "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md"
            contract.parent.mkdir(parents=True); spec.write_text("spec\n"); contract.write_text("contract\n")
            (workspace / "candidate").mkdir(); (workspace / "candidate/app.ts").write_text("ok\n")
            settings = {"seed_files_sha256": {
                campaign.FROZEN_SPEC: campaign.common.digest(spec),
                "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md": campaign.common.digest(contract),
            }}
            self.assertEqual(campaign.workspace_guard(workspace, settings)["status"], "passed")
            self.assertEqual(campaign.workspace_guard(workspace, settings, allow_candidate=False)["status"], "failed")
            (workspace / "outside.txt").write_text("escape\n")
            self.assertEqual(campaign.workspace_guard(workspace, settings)["status"], "failed")
            (workspace / "outside.txt").unlink(); spec.write_text("changed\n")
            self.assertEqual(campaign.workspace_guard(workspace, settings)["status"], "failed")

    def test_missing_failed_candidate_archives_as_an_empty_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            hashes, omitted = campaign.archive_candidate(root / "missing-candidate", root / "archive")
            self.assertEqual((hashes, omitted), ({}, []))
            self.assertEqual(list((root / "archive").iterdir()), [])

    def test_candidate_check_runs_one_snapshotted_gate_and_requires_exact_report_schema(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); candidate = root / "candidate"; candidate.mkdir()
            gate_relative = "benchmarks/webapp-tokens-v2/acceptance/run.mjs"
            gate = root / "artifacts/harness-source" / gate_relative
            gate.parent.mkdir(parents=True); gate.write_text("// frozen gate\n")
            gate_hash = campaign.common.digest(gate)
            settings = {
                "artifacts": str(root / "artifacts"),
                "harness_source_snapshot": {"path": "harness-source", "files_sha256": {gate_relative: gate_hash}},
                "acceptance": {"runner": gate_relative, "capabilities": {
                    "node_binary": "/pinned/node", "playwright_root": "/pinned/playwright",
                }},
                "qualification": {"spec_sha256": "a" * 64},
                "compiler_source_commit": "b" * 40, "source_binary_sha256": "c" * 64,
                "acceptance_timeout_seconds": 2700,
            }

            def run_gate(command, **kwargs):
                self.assertEqual(command[0], "/pinned/node")
                self.assertNotIn("/bin/sh", command)
                output = Path(command[command.index("--output") + 1]); output.mkdir(parents=True)
                report = {
                    "schema": "semaprax.teamdesk.acceptance.v1", "arm": "typescript",
                    "spec_sha256": "a" * 64,
                    "gate": [{"name": "run.mjs", "sha256": gate_hash}],
                    "checks": [{"id": f"case-{index}", "status": "passed"} for index in range(912)],
                    "qualification": {"passed": True, "cases": 912, "missingCases": [],
                                      "missingGroups": [], "failures": []},
                    "candidate_after": [],
                }
                (output / "report.json").write_text(json.dumps(report))
                return subprocess.CompletedProcess(command, 0, "ok", "")

            with patch.object(campaign.subprocess, "run", side_effect=run_gate) as invoked:
                accepted = campaign.check_candidate(candidate, root / "evidence-good", "typescript",
                                                    settings, root / "semaprax")
            self.assertTrue(accepted["accepted"])
            self.assertEqual(invoked.call_count, 1)

            def run_bad_gate(command, **kwargs):
                result = run_gate(command, **kwargs)
                output = Path(command[command.index("--output") + 1])
                report = json.loads((output / "report.json").read_text())
                report["schema"] = "wrong"
                (output / "report.json").write_text(json.dumps(report))
                return result

            with patch.object(campaign.subprocess, "run", side_effect=run_bad_gate):
                rejected = campaign.check_candidate(candidate, root / "evidence-bad", "typescript",
                                                    settings, root / "semaprax")
            self.assertFalse(rejected["accepted"])
            self.assertIn("report schema", rejected["failure"])

    def test_snapshotted_node_gate_receives_launch_paths_and_writes_real_report(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); artifacts = root / "artifacts"; artifacts.mkdir()
            commit = campaign.resolve_commit(ROOT, "HEAD")
            seeds = campaign.pinned_seed_hashes(ROOT, commit)
            inventory = campaign.harness_source_inventory(ROOT, seeds[campaign.FROZEN_SPEC])
            snapshot = campaign.snapshot_harness_sources(
                ROOT, artifacts, inventory, commit, seeds[campaign.FROZEN_SPEC])
            candidate = root / "candidate"; candidate.mkdir()
            (candidate / "build.sh").write_text("printf 'build\\n' >> \"$TEAMDESK_DATA_DIR/scripts.log\"\n")
            (candidate / "test.sh").write_text("printf 'test\\n' >> \"$TEAMDESK_DATA_DIR/scripts.log\"\n")
            (candidate / "run.sh").write_text(
                "printf '%s\\n' \"$TEAMDESK_ARM|$TEAMDESK_HOST|$TEAMDESK_PORT|$TEAMDESK_UI_PORT|$TEAMDESK_DATA_DIR\" "
                "> \"$TEAMDESK_DATA_DIR/launch-env.txt\"\nexit 1\n")
            node = shutil.which("node")
            self.assertIsNotNone(node)
            settings = {
                "artifacts": str(artifacts), "harness_source_snapshot": snapshot,
                "acceptance": {"runner": "benchmarks/webapp-tokens-v2/acceptance/run.mjs",
                               "capabilities": {"node_binary": node,
                                                "playwright_root": str(ROOT / "benchmarks/webapp-tokens-v2/acceptance")}},
                "qualification": {"spec_sha256": seeds[campaign.FROZEN_SPEC]},
                "compiler_source_commit": commit, "source_binary_sha256": "unused",
                "acceptance_timeout_seconds": 30,
            }
            output = root / "evidence"
            result = campaign.check_candidate(candidate, output, "typescript", settings, root / "semaprax")
            self.assertFalse(result["accepted"])
            report = result["report"]
            self.assertEqual(report["schema"], "semaprax.teamdesk.acceptance.v1")
            self.assertEqual(report["arm"], "typescript")
            self.assertEqual(report["spec_sha256"], seeds[campaign.FROZEN_SPEC])
            expected_gate = {name.removeprefix("benchmarks/webapp-tokens-v2/acceptance/"): digest
                             for name, digest in inventory.items()
                             if name.startswith("benchmarks/webapp-tokens-v2/acceptance/")}
            self.assertEqual({row["name"]: row["sha256"] for row in report["gate"]}, expected_gate)
            self.assertEqual((output / "data/scripts.log").read_text().splitlines(), ["build", "test"])
            arm, host, api_port, ui_port, data = (output / "data/launch-env.txt").read_text().strip().split("|")
            self.assertEqual((arm, host), ("typescript", "127.0.0.1"))
            self.assertTrue(api_port.isdecimal() and ui_port.isdecimal())
            self.assertEqual(Path(data), output / "data")

    def test_runner_capture_reproduces_complete_raw_tree_and_build_failure_still_rejects(self):
        # Collector-only fixture evidence: this executable is deliberately not
        # a real compiler or an application, and cannot satisfy acceptance.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); artifacts = root / "artifacts"; artifacts.mkdir()
            commit = campaign.resolve_commit(ROOT, "HEAD")
            seeds = campaign.pinned_seed_hashes(ROOT, commit)
            inventory = campaign.harness_source_inventory(ROOT, seeds[campaign.FROZEN_SPEC])
            snapshot = campaign.snapshot_harness_sources(
                ROOT, artifacts, inventory, commit, seeds[campaign.FROZEN_SPEC])
            candidate = root / "candidate"; (candidate / "src").mkdir(parents=True)
            (candidate / "src/app.spx").write_text("// collector fixture source; no application\n")
            (candidate / "build.sh").write_text(
                "printf 'build\\n' >> \"$TEAMDESK_DATA_DIR/scripts.log\"\nexit 23\n")
            for script, marker in (("test.sh", "test"), ("run.sh", "run")):
                (candidate / script).write_text(
                    f"printf '{marker}\\n' >> \"$TEAMDESK_DATA_DIR/scripts.log\"\nexit 99\n")
            # The declaration closes only retained input source. A preexisting
            # excluded candidate directory must not enter that source closure.
            (candidate / "out").mkdir()
            (candidate / "out/preexisting.txt").write_text("excluded candidate cache\n")
            retained = [{"path": name, "sha256": campaign.common.digest(candidate / name)}
                        for name in ("build.sh", "run.sh", "src/app.spx", "test.sh")]
            declaration = {"schema": "semaprax.compiler-output-capture.v1",
                           "argv": ["webapp", "src/app.spx", "-o", "{output}"],
                           "input_files": retained, "output_directory": "generated"}
            (candidate / "compiler-output-capture.json").write_text(json.dumps(declaration))

            payloads = {"index.html": b"collector-only deterministic HTML\n",
                        "nested/node_modules/raw.bin": b"\x00\xffcomplete raw evidence\n",
                        "server.mjs": b"// collector-only fixture output\n"}
            calls = root / "fixture-compiler-invocations.jsonl"
            compiler = root / "fixture-compiler"
            compiler.write_text(
                f"#!{sys.executable}\n"
                "import json, os, sys\nfrom pathlib import Path\n"
                "assert sys.argv[1:4] == ['webapp', 'src/app.spx', '-o']\n"
                "destination = Path(sys.argv[4])\n"
                "before = sorted(path.name for path in destination.iterdir())\n"
                "assert before == [], 'each direct capture starts in a fresh empty root'\n"
                f"with Path({str(calls)!r}).open('a') as log:\n"
                "    log.write(json.dumps({'argv': sys.argv[1:], 'cwd': os.getcwd(), "
                "'output_entries_before': before}) + '\\n')\n"
                f"payloads = json.loads({json.dumps({name: value.hex() for name, value in payloads.items()})!r})\n"
                "for name, value in payloads.items():\n"
                "    target = destination / name\n"
                "    target.parent.mkdir(parents=True, exist_ok=True)\n"
                "    target.write_bytes(bytes.fromhex(value))\n")
            compiler.chmod(0o755)
            node = shutil.which("node")
            self.assertIsNotNone(node, "Node24 is required for the runner integration gate")
            settings = {"artifacts": str(artifacts), "harness_source_snapshot": snapshot,
                        "acceptance": {"runner": "benchmarks/webapp-tokens-v2/acceptance/run.mjs",
                                       "capabilities": {"node_binary": node}},
                        "qualification": {"spec_sha256": seeds[campaign.FROZEN_SPEC]},
                        "compiler_source_commit": commit,
                        "source_binary_sha256": campaign.common.digest(compiler),
                        "acceptance_timeout_seconds": 30}
            settings["acceptance"]["capabilities"]["playwright_root"] = str(
                ROOT / "benchmarks/webapp-tokens-v2/acceptance")
            output = root / "evidence"
            result = campaign.check_candidate(candidate, output, "semaprax", settings, compiler)
            self.assertFalse(result["accepted"])
            self.assertEqual(result["exit_code"], 1)
            report = result["report"]
            self.assertGreaterEqual(int(report["node"].split(".")[0].removeprefix("v")), 24)
            self.assertTrue(calls.is_file(), "compiler capture never invoked fixture: " + json.dumps({
                "report": {"checks": report["checks"], "compiler": report.get("compiler"),
                           "qualification": {key: value for key, value in report["qualification"].items()
                                             if key not in ("missingCases", "missingGroups")}},
                "stdout": result["stdout"], "stderr": result["stderr"],
                "process_log": (output / "process.log").read_text()}, indent=2))
            invocations = [json.loads(line) for line in calls.read_text().splitlines()]
            self.assertEqual(len(invocations), 2)
            capture_roots = [output / "compiler-output-raw", output / "compiler-output-repeat"]
            self.assertNotEqual(*capture_roots)
            for invocation, capture_root in zip(invocations, capture_roots):
                self.assertEqual(invocation, {"argv": ["webapp", "src/app.spx", "-o", str(capture_root)],
                                              "cwd": str(candidate.resolve()), "output_entries_before": []})
                actual = {path.relative_to(capture_root).as_posix(): path.read_bytes()
                          for path in capture_root.rglob("*") if path.is_file()}
                self.assertEqual(actual, payloads)
            receipt_path = output / "compiler-output-receipt.json"
            receipt = json.loads(receipt_path.read_text())
            self.assertEqual(receipt["schema"], "semaprax.compiler-output-provenance.v1")
            self.assertEqual(receipt["cwd"], ".")
            expected_raw = [{"raw_path": name, "sha256": hashlib.sha256(payload).hexdigest()}
                            for name, payload in sorted(payloads.items())]
            expected_final = [{**row, "final_path": "generated/" + row["raw_path"]}
                              for row in expected_raw]
            self.assertEqual(receipt["raw_outputs"], expected_final)
            self.assertEqual(receipt["repeat_outputs"], expected_raw)
            self.assertEqual(receipt["input_files"], retained)
            self.assertEqual(receipt["argv"], declaration["argv"])
            self.assertEqual(receipt["raw_root"], "compiler-output-raw")
            self.assertEqual(receipt["compiler"], {"source_sha": commit,
                                                   "binary_sha256": campaign.common.digest(compiler)})
            proof = report["compiler_output_receipt"]
            self.assertEqual(Path(proof["path"]), receipt_path)
            self.assertEqual(proof["sha256"], hashlib.sha256(receipt_path.read_bytes()).hexdigest())
            self.assertEqual(proof["compiler"], receipt["compiler"])
            self.assertEqual(proof["raw_outputs"], expected_final)
            self.assertEqual((output / "data/scripts.log").read_text().splitlines(), ["build"])
            self.assertEqual(report["candidate_after"], report["candidate_before"])
            self.assertFalse(report["qualification"]["passed"])
            self.assertEqual(report["qualification"]["cases"], 1)
            self.assertEqual(len(report["qualification"]["missingCases"]), 912)
            self.assertEqual(report["qualification"]["failures"], ["runner-fatal"])
            self.assertEqual([row["id"] for row in report["checks"]], ["runner-fatal"])
            self.assertIn("build.sh failed", report["checks"][0]["error"])
            self.assertIn("912-case inventory", result["failure"])

    def test_summary_keeps_usage_cost_and_wall_time_separate_from_calibration(self):
        def row(status, raw, net, authored, price, agent, acceptance):
            return {"status": status, "observed": {"request_usage_sum": {
                        "input_tokens": raw, "cached_input_tokens": 2,
                        "cache_write_input_tokens": 3, "output_tokens": 4,
                    }, "legacy_net_input_tokens": net},
                    "final_candidate_source_metrics": {"total_tokens": authored},
                    "list_price": {"standard_short_context_api_equivalent_usd": price},
                    "elapsed_seconds": agent, "acceptance": {"seconds": acceptance}}
        summary = campaign.summarize([
            row("accepted", 100, 40, 800, 1.25, 10, 20),
            row("not_accepted", 200, 60, 900, 2.75, 30, 40),
        ])
        self.assertEqual(summary["usage_all_attempts"]["raw_input_tokens"], 300)
        self.assertEqual(summary["usage_all_attempts"]["legacy_net_input_tokens"], 100)
        self.assertEqual(summary["authored_source_tokens_per_accepted_task"], 800)
        self.assertEqual(summary["list_price_estimate_per_accepted_task_usd"], 4.0)
        self.assertEqual(summary["agent_wall_seconds_per_accepted_task"], 40)
        self.assertEqual(summary["acceptance_wall_seconds_per_accepted_task"], 60)
        self.assertTrue(summary["calibration_separate"])


if __name__ == "__main__":
    unittest.main()
