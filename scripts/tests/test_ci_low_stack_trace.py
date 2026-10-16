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


if __name__ == "__main__":
    unittest.main()
