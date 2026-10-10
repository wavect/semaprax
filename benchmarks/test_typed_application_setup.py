"""Owning source-installation guards; no models, compiler builds or acceptance runs."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import typed_application_setup as setup


class TypedApplicationSetupTests(unittest.TestCase):
    def setUp(self):
        self.temporary = self.enterContext(tempfile.TemporaryDirectory())
        self.root = Path(self.temporary).resolve()

    def pair(self, application="catalog", fault=None):
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

        result = setup.derive_pair(application, self.root / "compiler", bootstrap, generated, run, unchanged)
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


if __name__ == "__main__":
    unittest.main()
