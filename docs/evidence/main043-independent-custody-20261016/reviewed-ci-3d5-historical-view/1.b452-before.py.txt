"""Artifact provenance failures must stop before a debugger can execute anything."""

import json
from pathlib import Path
import runpy
import tempfile
import unittest

SELECT = runpy.run_path(str(Path(__file__).resolve().parents[1] / "ci-low-stack-trace.py"))["select_executable"]


class ArtifactProvenance(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.target = self.root / "target"
        self.target.mkdir()
        self.executable = self.target / "semaprax-test"
        self.executable.write_text("fixture, never executed")
        self.executable.chmod(0o700)
        self.artifact = {
            "reason": "compiler-artifact", "manifest_path": str(self.root / "Cargo.toml"),
            "target": {"name": "semaprax", "kind": ["lib"], "src_path": str(self.root / "src/lib.rs")},
            "profile": {"test": True}, "executable": str(self.executable),
        }

    def select(self, *artifacts):
        return SELECT("\n".join(json.dumps(item) for item in artifacts), self.root, self.target)

    def test_requires_exactly_one_current_root_library_artifact(self):
        dependency = self.artifact | {"manifest_path": str(self.root / "dependency/Cargo.toml")}
        self.assertEqual(self.select(dependency, self.artifact), self.executable)
        for artifacts in [(), (dependency,), (self.artifact, self.artifact)]:
            with self.subTest(artifacts=artifacts), self.assertRaises(ValueError):
                self.select(*artifacts)

    def test_refuses_non_test_foreign_source_missing_and_nonexecutable_artifacts(self):
        cases = [
            self.artifact | {"profile": {"test": False}},
            self.artifact | {"executable": None},
            self.artifact | {"executable": str(self.target / "missing")},
            self.artifact | {"target": self.artifact["target"] | {"src_path": str(self.root / "foreign.rs")}},
        ]
        for artifact in cases:
            with self.subTest(artifact=artifact), self.assertRaises(ValueError):
                self.select(artifact)
        self.executable.chmod(0o600)
        with self.assertRaises(ValueError):
            self.select(self.artifact)

    def test_refuses_path_escape_and_symlink_escape(self):
        foreign = self.root / "foreign"
        foreign.write_text("fixture, never executed")
        foreign.chmod(0o700)
        with self.assertRaises(ValueError):
            self.select(self.artifact | {"executable": str(foreign)})
        self.executable.unlink()
        self.executable.symlink_to(foreign)
        with self.assertRaises(ValueError):
            self.select(self.artifact)


class ManualDiagnosticWorkflow(unittest.TestCase):
    def test_manual_trace_has_no_automatic_trigger_or_release_artifact(self):
        root = Path(__file__).resolve().parents[2]
        workflow = (root / ".github/workflows/stack-diagnostic.yml").read_text()
        triggers = workflow.split("\non:\n", 1)[1].split("\npermissions:", 1)[0]
        self.assertEqual(triggers.strip(), "workflow_dispatch:")
        self.assertIn("contents: read", workflow)
        self.assertIn("cancel-in-progress: false", workflow)
        self.assertIn("group: stack-diagnostic-${{ github.ref }}", workflow)
        self.assertNotIn("continue-on-error", workflow)
        self.assertNotIn("actions/upload-artifact", workflow)
        self.assertEqual(workflow.count("python3 scripts/ci-low-stack-trace.py"), 1)
        self.assertIn("run: cargo fetch --locked", workflow)
        install = "sudo apt-get install --yes --no-install-recommends gdb"
        self.assertIn(install, workflow)
        unit = (root / ".github/workflows/ci.yml").read_text()
        trace = unit.split("      - name: Trace the unchanged low-stack case", 1)[1].split("\n      - name:", 1)[0]
        self.assertIn(install, trace)
        self.assertLess(trace.index(install), trace.index("python3 scripts/ci-low-stack-trace.py"))


    def test_manual_trace_keeps_the_unit_shard_profile_and_compiler(self):
        root = Path(__file__).resolve().parents[2]
        workflow = (root / ".github/workflows/stack-diagnostic.yml").read_text()
        unit = (root / ".github/workflows/ci.yml").read_text().split("  verify-tests:\n", 1)[1]
        profile = unit.split("    strategy:", 1)[0]
        for name in ("CARGO_PROFILE_DEV_DEBUG", "CARGO_PROFILE_TEST_DEBUG",
                     "CARGO_PROFILE_TEST_OPT_LEVEL", "CARGO_INCREMENTAL", "CARGO_BUILD_JOBS"):
            value = profile.split(name + ": ", 1)[1].splitlines()[0]
            self.assertIn(name + ": " + value, workflow)
        compiler = unit.split("toolchain: ", 1)[1].splitlines()[0]
        self.assertIn("toolchain: " + compiler, workflow)
        self.assertIn("RUSTC=$(rustup which rustc)", workflow)


if __name__ == "__main__":
    unittest.main()
