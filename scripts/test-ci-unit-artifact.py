#!/usr/bin/env python3
"""Pure fake-Cargo controls; never compile the compiler or execute an artifact."""
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import runpy
import subprocess
import tempfile
import unittest
from unittest.mock import patch

MODULE = runpy.run_path(str(Path(__file__).with_name("ci-unit-artifact.py")))
COMMIT = "a" * 40


class Artifact(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        (self.root / "src").mkdir()
        (self.root / "src/lib.rs").write_text("// actual fixture root\n")
        (self.root / "Cargo.lock").write_text("# fixture lock\n")
        self.binary = self.root / "target/debug/deps/semaprax-current"
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b"current unit executable" * 1024)
        self.binary.chmod(0o755)
        self.output = self.root / "capture"
        self.row = {"reason": "compiler-artifact", "target": {"name": "semaprax",
                    "kind": ["lib"], "src_path": str(self.root / "src/lib.rs")},
                    "profile": {"test": True}, "executable": str(self.binary)}

    def capture(self, rows, *, held=COMMIT, dirty=False):
        @contextlib.contextmanager
        def process(*args, **kwargs):
            self.assertEqual(args[0], MODULE["COMMAND"])
            fake = type("Process", (), {"stdout": io.StringIO("".join(json.dumps(r) + "\n" for r in rows)),
                                       "wait": lambda self: 0})()
            yield fake

        def output(command, **kwargs):
            if command[0] == "git":
                return held + "\n"
            return "rustc fixture\nhost: aarch64-apple-darwin\n"

        def run(command, **kwargs):
            if dirty:
                raise subprocess.CalledProcessError(1, command)
            return subprocess.CompletedProcess(command, 0)

        with patch.dict(os.environ, {"CARGO_TARGET_DIR": str(self.root / "target")}), \
                patch("subprocess.check_output", side_effect=output), \
                patch("subprocess.run", side_effect=run), \
                patch("subprocess.Popen", side_effect=process):
            return MODULE["capture"](self.root, self.output, COMMIT)

    def test_selects_exact_cargo_record_and_copies_full_bytes(self):
        unrelated = {**self.row, "target": {**self.row["target"], "kind": ["bin"]}}
        stale = self.binary.with_name("semaprax-stale")
        stale.write_bytes(b"unselected old bytes")
        stale.chmod(0o755)
        receipt = self.capture([{"reason": "build-script-executed"}, unrelated, self.row])
        self.assertEqual((self.output / "semaprax-unit").read_bytes(), self.binary.read_bytes())
        self.assertEqual(receipt["sha256"], hashlib.sha256(self.binary.read_bytes()).hexdigest())
        self.assertEqual(receipt["bytes"], self.binary.stat().st_size)
        self.assertEqual(receipt["commit"], COMMIT)
        self.assertEqual(json.loads((self.output / "provenance.json").read_text()), receipt)

    def test_missing_or_ambiguous_exact_target_refuses(self):
        for rows in [[], [self.row, self.row]]:
            with self.assertRaises(ValueError):
                self.capture(rows)
            self.assertFalse(self.output.exists())

    def test_wrong_commit_and_dirty_source_refuse_before_publishing(self):
        with self.assertRaises(ValueError):
            self.capture([self.row], held="b" * 40)
        self.assertFalse(self.output.exists())
        with self.assertRaises(subprocess.CalledProcessError):
            self.capture([self.row], dirty=True)
        self.assertFalse(self.output.exists())

    def test_outside_target_executable_refuses(self):
        external = self.root / "foreign-binary"
        external.write_bytes(b"foreign")
        external.chmod(0o755)
        with self.assertRaises(ValueError):
            self.capture([{**self.row, "executable": str(external)}])
        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
