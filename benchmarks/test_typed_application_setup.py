"""Owning source-installation guards; no models, compiler builds or acceptance runs."""
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import typed_application_setup as setup
from typed_application_setup_support import installation


class TypedApplicationSetupTests(unittest.TestCase):
    def setUp(self):
        self.temporary = self.enterContext(tempfile.TemporaryDirectory())
        self.root = Path(self.temporary).resolve()

    def pair(self, application="catalog", fault=None, on_replayed=None):
        bootstrap, generated = self.root / "bootstrap", self.root / "generated"
        bootstrap.mkdir()
        generated.mkdir()
        inventory = self.root / "bootstrap-state"
        inventory.write_bytes(b"original authored bootstrap")
        commands = []

        def unchanged():
            self.assertEqual(inventory.read_bytes(), b"original authored bootstrap")

        def run(argv, label):
            commands.append((argv, label))
            self.assertEqual(inventory.read_bytes(), b"original authored bootstrap")
            if fault == "second" and label == "derive-1":
                raise ValueError("second generation refused")
            if fault == "drift" and label == "derive-0":
                inventory.write_bytes(b"drift")
            index = argv[argv.index("--type") + 1]
            payload = (index + " ordinary mocked canonical replacement\n").encode()
            if fault == "replay" and label == "replay-1":
                payload += b"changed"
            if fault == "oversize" and label == "derive-0":
                payload = b"x" * (setup.MAX_GENERATED_BYTES + 1)
            Path(argv[-1]).write_bytes(payload)

        result = setup.derive_pair(application, self.root / "compiler", bootstrap, generated, run, unchanged, on_replayed)
        return result, commands

    def test_both_outputs_replay_against_same_unchanged_bootstrap(self):
        result, commands = self.pair()
        self.assertEqual(list(result), ["src/request.spx", "src/response.spx"])
        self.assertEqual([label for _, label in commands], ["derive-0", "replay-0", "derive-1", "replay-1"])
        self.assertTrue(all("--output" in argv for argv, _ in commands))
        self.assertFalse((self.root / "candidate").exists())
        self.assertFalse((self.root / "bootstrap/src/request.spx").exists())

    def test_failed_second_derivation_never_publishes_or_installs_first(self):
        with self.assertRaisesRegex(ValueError, "second generation refused"):
            self.pair(fault="second")
        self.assertTrue((self.root / "generated/0.spx").is_file())
        self.assertFalse((self.root / "candidate").exists())
        self.assertFalse((self.root / "bootstrap/src/request.spx").exists())

    def test_failed_second_derivation_keeps_only_first_confirmed_replay_binding(self):
        retained = []
        with self.assertRaisesRegex(ValueError, "second generation refused"):
            self.pair(fault="second", on_replayed=lambda path, data: retained.append(
                {"path": path, "sha256": setup.sha(data), "bytes": len(data)}))
        self.assertEqual(len(retained), 1)
        self.assertEqual(retained[0]["path"], "src/request.spx")
        data = (self.root / "generated/0.spx").read_bytes()
        self.assertEqual(retained[0]["sha256"], setup.sha(data))
        self.assertEqual(retained[0]["bytes"], len(data))
        self.assertFalse((self.root / "candidate").exists())

    def test_replay_drift_is_refused_before_installation(self):
        with self.assertRaisesRegex(ValueError, "same-bootstrap replay"):
            self.pair(fault="replay")
        self.assertFalse((self.root / "candidate").exists())

    def test_bootstrap_drift_is_refused_before_second_generator(self):
        with self.assertRaises(AssertionError):
            self.pair(fault="drift")
        self.assertFalse((self.root / "generated/1.spx").exists())

    def test_generated_output_byte_bound_is_enforced(self):
        with self.assertRaisesRegex(ValueError, "bounded regular input"):
            self.pair(fault="oversize")
        self.assertFalse((self.root / "candidate").exists())

    def test_original_ascii_shiftsim_selector_is_preserved(self):
        _, commands = self.pair("shiftsim")
        self.assertEqual(commands[0][0][commands[0][0].index("--profile") + 1], "stream-owned-request.v1")
        self.assertNotIn("--max-string-bytes", commands[0][0])
        self.assertEqual(commands[2][0][commands[2][0].index("--type") + 1], "shiftsim.report")

    def test_explicit_scaffolding_uses_real_native_and_source_test_routes(self):
        for application in setup.EXAMPLES:
            scripts = setup.qualification_scaffolding(application)
            self.assertEqual(set(scripts), {"build.sh", "run.sh", "test.sh"})
            self.assertIn(b"${SEMAPRAX_BIN:?", scripts["build.sh"])
            self.assertIn(b"build --manifest-path semaprax.toml --target native --output dist/", scripts["build.sh"])
            self.assertIn(b'"$SEMAPRAX_BIN" test .', scripts["test.sh"])
            self.assertIn(f"exec dist/{application}\n".encode(), scripts["run.sh"])
            self.assertNotIn(b"json-codec", b"".join(scripts.values()))
            self.assertNotIn(b"codex", b"".join(scripts.values()))

    def test_completion_is_exclusive_and_keeps_regular_complete_tree(self):
        project, entry = self.root / "installed-project", self.root / "completion.json"
        project.mkdir()
        (project / "request.spx").write_bytes(b"complete request")
        (project / "response.spx").write_bytes(b"complete response")
        setup.record_completion(project, entry, [])
        self.assertFalse(project.is_symlink())
        self.assertEqual((project / "request.spx").read_bytes(), b"complete request")
        self.assertEqual((project / "response.spx").read_bytes(), b"complete response")
        self.assertIsNone(json.loads(entry.read_bytes())["runtime_qualification"])
        with self.assertRaises(FileExistsError):
            setup.record_completion(project, entry, [])

    def test_existing_destination_and_symlink_ancestors_are_never_overwritten(self):
        project = self.root / "installed-project"
        project.mkdir()
        entry = self.root / "completion.json"
        entry.write_bytes(b"sentinel")
        with self.assertRaises(FileExistsError):
            setup.record_completion(project, entry, [])
        self.assertEqual(entry.read_bytes(), b"sentinel")
        link = self.root / "redirect"
        link.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(OSError):
            setup.create_new_directory(link / "outside-write")
        self.assertFalse((self.root / "outside-write").exists())
        with self.assertRaises(OSError):
            setup.record_completion(link / "installed-project", link / "new-completion", [])
        self.assertFalse((self.root / "new-completion").exists())

    def test_failed_completion_persistence_keeps_no_usable_receipt(self):
        project = self.root / "installed-project"
        project.mkdir()
        receipt = self.root / "completion.json"
        with patch.object(setup.os, "fsync", side_effect=OSError("disk refusal")):
            with self.assertRaisesRegex(OSError, "disk refusal"):
                setup.record_completion(project, receipt, [])
        self.assertFalse(receipt.exists())
        self.assertTrue(project.is_dir())

    def test_regular_source_rejects_symlink_and_capacity_excess(self):
        source = self.root / "source"
        source.write_bytes(b"bytes")
        alias = self.root / "alias"
        alias.symlink_to(source)
        with self.assertRaises(OSError):
            setup.read_regular(alias)
        with self.assertRaisesRegex(ValueError, "bounded regular input"):
            setup.read_regular(source, 4)

    def test_build_receipt_binds_source_binary_and_retained_log(self):
        source, binary = "1" * 40, "2" * 64
        log = self.root / "build.log"
        log.write_bytes(b"actual retained build log fixture")
        path = self.root / "build.json"
        receipt = {"schema": "semaprax.loglens.compiler-build.v1", "compiler_source_commit": source,
                   "compiler_binary_sha256": binary, "build_command": ["cargo", "build", "--locked"],
                   "build_log": {"path": "build.log", "sha256": setup.sha(log.read_bytes())}}
        path.write_text(json.dumps(receipt))
        bound = setup.verify_build_receipt(path, source, binary)
        self.assertIn("caller-supplied", bound["provenance"])
        with self.assertRaisesRegex(ValueError, "exact source/binary"):
            setup.verify_build_receipt(path, "3" * 40, binary)
        log.write_bytes(b"stale log")
        with self.assertRaisesRegex(ValueError, "build log changed"):
            setup.verify_build_receipt(path, source, binary)

    def test_actual_public_manifests_match_closed_v31_inventory(self):
        repo = Path(__file__).resolve().parents[1]
        for application, example in setup.EXAMPLES.items():
            data = (repo / "examples" / example["directory"] / "semaprax.toml").read_bytes()
            setup.validate_manifest(application, data)
            with self.assertRaisesRegex(ValueError, "closed v31 route"):
                setup.validate_manifest(application, data.replace(setup.PROFILE.encode(), b"language-command-io.owned-data.v1"))

    def test_bundled_closure_uses_real_registry_and_rejects_unclosed_dependencies(self):
        repo = Path(__file__).resolve().parents[1]
        registry = setup.dependencies.parse_registry((repo / "src/project/standard_dependencies.rs").read_text())
        for application, example in setup.EXAMPLES.items():
            manifest = setup.validate_manifest(application, (repo / "examples" / example["directory"] / "semaprax.toml").read_bytes())
            selected = setup.dependencies.closure(manifest, registry)
            self.assertIn("std.data.json.scan", selected)
            self.assertIn("std.data.json.query", selected)
            self.assertGreaterEqual(len(selected), 5)
        with self.assertRaisesRegex(ValueError, "unclosed or nonbundled"):
            setup.dependencies.closure({"dependencies": {"unavailable": "=0.1.0"}}, registry)
        with self.assertRaisesRegex(ValueError, "exact declared bundled version"):
            setup.dependencies.closure({"dependencies": {"std.text": "^0.1.0"}}, registry)

    def completed_fixture(self):
        """Mock installation evidence, explicitly no compiler/provider execution."""
        source_repo = Path(__file__).resolve().parents[1]
        repo, output = self.root / "repo", self.root / "completed"
        repo.mkdir()
        output.mkdir()
        prefix = "examples/catalog-scoped-record-successor/"
        authored_paths = setup.selected_paths("catalog")
        registry = setup.dependencies.parse_registry((source_repo / "src/project/standard_dependencies.rs").read_text())
        manifest = setup.validate_manifest("catalog", (source_repo / prefix / "semaprax.toml").read_bytes())
        dependencies = setup.dependencies.closure(manifest, registry)
        packages = json.loads((source_repo / "std/packages.json").read_bytes())["packages"]
        directories = {row["module"]: row["directory"] for row in packages}
        selected = {prefix + path for path in authored_paths} | {
            "benchmarks/typed_application_setup.py", "benchmarks/compiler_output_provenance.py",
            "benchmarks/typed_application_setup_support/dependencies.py",
            "benchmarks/typed_application_setup_support/installation.py",
            "src/project/standard_dependencies.rs", "std/packages.json"}
        for name in dependencies:
            selected.update(("std/" + directories[name] + "/semaprax.toml", registry[name]["source"]))
        committed = {path: (source_repo / path).read_bytes() for path in selected}
        for path, data in committed.items():
            target = repo / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        bootstrap, project = output / "bootstrap", output / "installed-project"
        bootstrap.mkdir()
        project.mkdir()
        for path in authored_paths:
            target = bootstrap / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(committed[prefix + path])
        scaffold = setup.qualification_scaffolding("catalog")
        generated = {path: committed[prefix + path] + b"\n// mocked generated bytes\n"
                     for path, *_ in setup.generator_jobs("catalog")}
        (output / "generated").mkdir()
        for index, data in enumerate(generated.values()):
            (output / "generated" / f"{index}.spx").write_bytes(data)
            (output / "generated" / f"{index}.replay.spx").write_bytes(data)
        paths = sorted((set(authored_paths) - {"src/app.command.spx"}) | set(scaffold))
        for path in paths:
            data = (scaffold[path] if path in scaffold else generated[path] if path in generated
                    else committed[prefix + ("src/app.command.spx" if path == "src/app.spx" else path)])
            target = project / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        compiler, build = self.root / "compiler", self.root / "build.json"
        compiler.write_bytes(b"mock compiler bytes, not executable evidence")
        log = self.root / "build.log"
        log.write_bytes(b"mock build log, not hosted proof")
        source, binary = "a" * 40, setup.sha(compiler.read_bytes())
        build.write_text(json.dumps({"schema": "semaprax.loglens.compiler-build.v1",
            "compiler_source_commit": source, "compiler_binary_sha256": binary,
            "build_command": ["mock", "not-executed"],
            "build_log": {"path": "build.log", "sha256": setup.sha(log.read_bytes())}}))
        result = {"schema": "semaprax.typed-application-setup.v1", "application": "catalog",
            "profile": setup.PROFILE, "status": "checked_source_runtime_qualification_pending",
            "compiler_source_commit": source, "compiler_binary_sha256": binary,
            "compiler_build_receipt": setup.verify_build_receipt(build, source, binary),
            "bundled_dependencies": dependencies, "runtime_qualification": None,
            "generated_outputs": [{"path": path, "sha256": setup.sha(data), "bytes": len(data)}
                                  for path, data in sorted(generated.items())]}
        for label, candidate, selection in (("selected", repo, sorted(selected)),
                ("authored", bootstrap, authored_paths), ("installed", project, paths)):
            receipt, digest = setup.provenance.capture_inputs(candidate, output / (label + "-inputs"), selection)
            result[label + "_input_snapshot"] = {"path": str(receipt), "sha256": digest}
        result["installed_source_inventory"] = [{"path": path, "sha256": setup.sha((project / path).read_bytes())}
                                                for path in paths]
        setup.write_json(output / "setup-result.json", result)
        setup.record_completion(project, output / "completion-receipt.json", result["installed_source_inventory"])
        args = SimpleNamespace(output=output, repo=repo, application="catalog", compiler_source=source,
                               compiler_sha256=binary, compiler=compiler, compiler_build_receipt=build)

        def git(argv):
            operation = argv[3]
            if operation == "rev-parse":
                return (source + "\n").encode()
            if operation == "ls-tree":
                return ("100644 blob " + "b" * 40 + "\t" + argv[-1] + "\n").encode()
            if operation == "show":
                return committed[argv[-1].split(":", 1)[1]]
            self.fail(f"unexpected Git operation: {argv}")

        return args, git, result

    def test_completed_installation_recheck_is_read_only_and_not_qualification(self):
        args, git, _ = self.completed_fixture()
        before = (args.output / "setup-result.json").read_bytes()
        with patch.object(installation.subprocess, "check_output", side_effect=git), \
                patch.object(setup.subprocess, "Popen", side_effect=AssertionError("must not execute compiler")):
            self.assertEqual(installation.verify(args, api=setup), args.output / "installed-project")
        self.assertEqual((args.output / "setup-result.json").read_bytes(), before)
        self.assertIsNone(json.loads(before)["runtime_qualification"])

    def test_completed_installation_recheck_rejects_live_source_drift(self):
        args, git, _ = self.completed_fixture()
        (args.output / "installed-project/src/order.spx").write_bytes(b"changed reviewed algorithm")
        with patch.object(installation.subprocess, "check_output", side_effect=git):
            with self.assertRaisesRegex(ValueError, "installed reviewed source drift: src/order.spx"):
                installation.verify(args, api=setup)

    def test_completed_installation_recheck_rejects_schema_and_retained_snapshot_drift(self):
        args, git, _ = self.completed_fixture()
        (args.repo / "examples/catalog-scoped-record-successor/src/request.spx").write_bytes(b"changed schema")
        with patch.object(installation.subprocess, "check_output", side_effect=git):
            with self.assertRaisesRegex(ValueError, "selected committed setup source changed"):
                installation.verify(args, api=setup)

    def test_completed_installation_recheck_rejects_compiler_drift(self):
        args, git, _ = self.completed_fixture()
        args.compiler.write_bytes(b"different compiler")
        with patch.object(installation.subprocess, "check_output", side_effect=git):
            with self.assertRaisesRegex(ValueError, "compiler source/binary changed"):
                installation.verify(args, api=setup)

    def test_completed_installation_recheck_rejects_incomplete_or_relabelled_receipt(self):
        args, git, result = self.completed_fixture()
        result["application"] = "shiftsim"
        (args.output / "setup-result.json").write_text(json.dumps(result))
        with patch.object(installation.subprocess, "check_output", side_effect=git):
            with self.assertRaisesRegex(ValueError, "setup source/completion receipt differs"):
                installation.verify(args, api=setup)
        result["application"] = "catalog"
        original = dict(result["selected_input_snapshot"])
        outside = self.root / "outside-sentinel"
        outside.write_bytes(b"outside is not setup evidence")
        for path in (str(outside), "../outside-sentinel", str(args.output / "selected-alias/snapshot.json")):
            result["selected_input_snapshot"]["path"] = path
            (args.output / "setup-result.json").write_text(json.dumps(result))
            with self.subTest(path=path), patch.object(installation.subprocess, "check_output", side_effect=git):
                with self.assertRaisesRegex(ValueError, "selected-inputs snapshot selection differs"):
                    installation.verify(args, api=setup)
            self.assertEqual(outside.read_bytes(), b"outside is not setup evidence")
        result["selected_input_snapshot"] = original
        # Even a recomputed receipt digest cannot authorize a traversal row.
        receipt = Path(original["path"])
        value = json.loads(receipt.read_bytes())
        value["input_files"][0]["path"] = "../../outside-sentinel"
        encoded = (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()
        receipt.write_bytes(encoded)
        result["selected_input_snapshot"]["sha256"] = setup.sha(encoded)
        (args.output / "setup-result.json").write_text(json.dumps(result))
        with patch.object(installation.subprocess, "check_output", side_effect=git):
            with self.assertRaises(ValueError):
                installation.verify(args, api=setup)
        self.assertEqual(outside.read_bytes(), b"outside is not setup evidence")

    def test_completed_installation_recheck_rejects_symlink_ancestor(self):
        args, git, _ = self.completed_fixture()
        project = args.output / "installed-project"
        (project / "src").rename(args.output / "outside-source")
        (project / "src").symlink_to(args.output / "outside-source", target_is_directory=True)
        with patch.object(installation.subprocess, "check_output", side_effect=git):
            with self.assertRaisesRegex(ValueError, "installed path must be regular: src"):
                installation.verify(args, api=setup)

    def test_completed_installation_recheck_requires_original_generated_bytes(self):
        args, git, result = self.completed_fixture()
        result["generated_outputs"][0]["sha256"] = "0" * 64
        (args.output / "setup-result.json").write_text(json.dumps(result))
        with patch.object(installation.subprocess, "check_output", side_effect=git):
            with self.assertRaisesRegex(ValueError, "retained generator output differs: src/request.spx"):
                installation.verify(args, api=setup)

    def test_completed_installation_recheck_requires_exact_inventory(self):
        args, git, result = self.completed_fixture()
        result["installed_source_inventory"] = result["installed_source_inventory"][:-1]
        (args.output / "setup-result.json").write_text(json.dumps(result))
        with patch.object(installation.subprocess, "check_output", side_effect=git):
            with self.assertRaisesRegex(ValueError, "installed source inventory differs"):
                installation.verify(args, api=setup)
        result["installed_source_inventory"] = json.loads((args.output / "completion-receipt.json").read_bytes())["source_inventory"]
        (args.output / "setup-result.json").write_text(json.dumps(result))
        project = args.output / "installed-project"
        (project / "src/unlisted.spx").write_bytes(b"unlisted authored source")
        with patch.object(installation.subprocess, "check_output", side_effect=git):
            with self.assertRaisesRegex(ValueError, "unlisted installed file: src/unlisted.spx"):
                installation.verify(args, api=setup)
        (project / "src/unlisted.spx").unlink()
        (project / "dist").mkdir()
        (project / "dist/catalog").write_bytes(b"existing operator build output")
        (project / "dist/catalog.c").write_bytes(b"existing emitted C")
        with patch.object(installation.subprocess, "check_output", side_effect=git):
            self.assertEqual(installation.verify(args, api=setup), project)


if __name__ == "__main__":
    unittest.main()
