"""Artifact provenance failures must stop before a debugger can execute anything."""

import contextlib
import ast
import hashlib
import io
import json
from pathlib import Path
import runpy
import struct
import sys
import types
import tempfile
import unittest
from unittest.mock import patch

TRACE = runpy.run_path(str(Path(__file__).resolve().parents[1] / "ci-low-stack-trace.py"))
SELECT = TRACE["select_executable"]


class InstrumentationSourceCustody(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[2]
        self.evidence = self.root / "docs/evidence/lazy-test-instrumentation-v1"
        self.records = json.loads((self.evidence / "sources.json").read_text())
        self.assertEqual({record["path"] for record in self.records}, {
            "src/test_allocation_observer.rs", "src/workspace_graph/forecast_observation.rs",
            "scripts/ci-low-stack-trace.py", "scripts/tests/test_ci_low_stack_trace.py",
        })

    def test_whole_predecessors_and_tampered_inputs_remain_bound(self):
        digest = lambda value: hashlib.sha256(value.encode()).hexdigest()
        for record in self.records:
            with self.subTest(path=record["path"]):
                before = (self.evidence / record["archive"]).read_text()
                current = (self.root / record["path"]).read_text()
                self.assertEqual(digest(before), record["before_sha256"])

                def restore(value):
                    if digest(value) != record["after_sha256"]:
                        raise ValueError("current source drift")
                    for change in record["inverse"]:
                        if value.count(change["after"]) != 1:
                            raise ValueError("nonunique inverse")
                        value = value.replace(change["after"], change["before"], 1)
                    return value

                self.assertEqual(restore(current), before)
                for changed in [current + "\n", "!" + current[1:]]:
                    with self.assertRaises(ValueError):
                        restore(changed)

    def test_complete_allocator_callbacks_windows_and_original_python_cases_are_retained(self):
        originals = {record["path"]: (self.evidence / record["archive"]).read_text()
                     for record in self.records}
        path = "src/test_allocation_observer.rs"
        before, current = originals[path], (self.root / path).read_text()
        for start, end in [
            ("    fn allocated(", "\nthread_local!"),
            ("unsafe impl GlobalAlloc for ObservedSystem", "\nstruct Guard;"),
            ("struct Guard;", "// Keep instrumentation"),
            ("fn finish_observation(", "\n#[cfg(test)]\nmod tests"),
        ]:
            self.assertEqual(before.split(start, 1)[1].split(end, 1)[0],
                             current.split(start, 1)[1].split(end, 1)[0])
        path = "src/workspace_graph/forecast_observation.rs"

        def callbacks(value):
            lines = value.split("pub(crate) fn record(", 1)[1].split(
                "pub(crate) fn observe<", 1)[0].rstrip().splitlines()
            while lines and lines[-1].startswith("///"):
                lines.pop()
            return "\n".join(lines)

        self.assertEqual(callbacks(originals[path]), callbacks((self.root / path).read_text()))
        path = "scripts/tests/test_ci_low_stack_trace.py"

        def cases(value):
            return {(item.name, method.name) for item in ast.parse(value).body
                    if isinstance(item, ast.ClassDef) for method in item.body
                    if isinstance(method, ast.FunctionDef) and method.name.startswith("test_")}

        self.assertTrue(cases(originals[path]).issubset(cases((self.root / path).read_text())))
        path = "scripts/ci-low-stack-trace.py"
        old_tree, new_tree = ast.parse(originals[path]), ast.parse((self.root / path).read_text())
        for name in ["CASE", "FRAME_LAYOUT_CODE"]:
            values = [next(ast.literal_eval(item.value) for item in tree.body
                           if isinstance(item, ast.Assign)
                           and any(isinstance(target, ast.Name) and target.id == name
                                   for target in item.targets)) for tree in [old_tree, new_tree]]
            self.assertEqual(values[0], values[1])
        functions = [next(item for item in tree.body if isinstance(item, ast.FunctionDef)
                          and item.name == "select_executable") for tree in [old_tree, new_tree]]
        self.assertEqual(ast.dump(functions[0]), ast.dump(functions[1]))


class ElfTlsLayouts(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "artifact"

    def artifact(self, entries):
        header = bytearray(64)
        header[:7] = b"\x7fELF\x02\x01\x01"
        struct.pack_into("<Q", header, 32, 64)
        struct.pack_into("<HH", header, 54, 56, len(entries))
        self.path.write_bytes(header + b"".join(entries) + b"\x00" * 8)

    def tls(self, *, initialized=8, total=16, alignment=8):
        entry = bytearray(56)
        struct.pack_into("<I", entry, 0, 7)
        struct.pack_into("<Q", entry, 8, 120)
        struct.pack_into("<QQQ", entry, 32, initialized, total, alignment)
        return entry

    def test_reads_tls_layout_without_executing_the_file(self):
        self.artifact([self.tls()])
        self.assertEqual(TRACE["elf_tls_layout"](self.path),
                         {"initialized_bytes": 8, "total_bytes": 16, "alignment": 8})
        self.artifact([bytearray(56)])
        self.assertIsNone(TRACE["elf_tls_layout"](self.path))

    def test_rejects_invalid_and_duplicate_tls_segments(self):
        for entries in [[self.tls(initialized=17)], [self.tls(total=7)],
                        [self.tls(alignment=3)], [self.tls(), self.tls()]]:
            with self.subTest(entries=len(entries)):
                self.artifact(entries)
                with self.assertRaises(ValueError):
                    TRACE["elf_tls_layout"](self.path)

    def test_rejects_truncation_and_excessive_header_counts(self):
        for data in [b"", b"\x7fELF\x02\x01\x01", b"x" * 64]:
            self.path.write_bytes(data)
            with self.assertRaises(ValueError):
                TRACE["elf_tls_layout"](self.path)
        self.artifact([self.tls()])
        data = bytearray(self.path.read_bytes())
        for count in [0, 4096, 4097]:
            struct.pack_into("<H", data, 56, count)
            self.path.write_bytes(data)
            with self.assertRaises(ValueError):
                TRACE["elf_tls_layout"](self.path)


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


class BoundedFrameLayouts(unittest.TestCase):
    def test_reads_only_six_frames_and_bounded_entry_instructions(self):
        class Frame:
            def __init__(self, index):
                self.index = index

            def select(self):
                pass

            def pc(self):
                return 0x1000 + self.index * 0x100

            def name(self):
                return "controlled frame"

            def read_register(self, name):
                registers.append(name)
                return 0x2000 + self.index * 0x100

            def older(self):
                return Frame(self.index + 1) if self.index < 8 else None

        commands = []
        registers = []
        debugger = types.ModuleType("gdb")
        debugger.newest_frame = lambda: Frame(0)

        def execute(command, to_string=False):
            commands.append(command)
            if command.startswith("info symbol "):
                return "controlled_symbol + 32 in section .text"
            return "controlled frame info" if to_string else None

        debugger.execute = execute
        output = io.StringIO()
        with patch.dict(sys.modules, {"gdb": debugger}), contextlib.redirect_stdout(output):
            exec(TRACE["FRAME_LAYOUT_CODE"], {})
        self.assertEqual(output.getvalue().count("SEMAPRAX_STACK_FRAME_LAYOUT_V1"), 6)
        self.assertEqual(commands.count("info frame"), 6)
        self.assertEqual(registers, ["sp"] * 6)
        disassembly = [command for command in commands if command.startswith("x/")]
        self.assertEqual(len(disassembly), 6)
        self.assertEqual(disassembly[0], "x/24i 0xfe0")
        for command in commands:
            self.assertRegex(command, r"^(info frame|info symbol 0x[0-9a-f]+|x/24i 0x[0-9a-f]+)$")

    def test_completed_inferior_has_no_frame_and_cannot_break_capture(self):
        debugger = types.ModuleType("gdb")

        def no_frame():
            raise RuntimeError("no stopped inferior")

        debugger.newest_frame = no_frame
        output = io.StringIO()
        with patch.dict(sys.modules, {"gdb": debugger}), contextlib.redirect_stdout(output):
            exec(TRACE["FRAME_LAYOUT_CODE"], {})
        self.assertIn("stopped frame layout unavailable: no stopped inferior", output.getvalue())



if __name__ == "__main__":
    unittest.main()
