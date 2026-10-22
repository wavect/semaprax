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

    def _current_source(self, record, overrides=None):
        overrides = overrides or {}
        read = lambda path: overrides[path] if path in overrides else (self.root / path).read_text(encoding="utf-8")
        value = read(record["path"])
        revision = record.get("current_projection")
        if revision is None:
            return value
        digest = lambda text: hashlib.sha256(text.encode()).hexdigest()
        if digest(value) != revision["root_sha256"]:
            raise ValueError("physical current source drift")
        expected = (["src/test_allocation_observer/backing.rs"]
                    if record["path"] == "src/test_allocation_observer.rs" else [])
        if [item["path"] for item in revision["fragments"]] != expected:
            raise ValueError("foreign or missing current source fragment")
        for fragment in revision["fragments"]:
            body = read(fragment["path"])
            if digest(body) != fragment["sha256"] or len(body.encode()) != fragment["length"]:
                raise ValueError("physical current fragment drift")
            marker = 'include!("test_allocation_observer/backing.rs");'
            if fragment["marker"] != marker or value.count(marker) != 1:
                raise ValueError("nonunique current source fragment")
            value = value.replace(marker, body, 1)
        if digest(value) != revision["logical_sha256"]:
            raise ValueError("complete current logical source drift")
        return value

    def _lazy_stage(self, record, value):
        revision = record.get("current_projection")
        if revision is not None:
            if hashlib.sha256(value.encode()).hexdigest() != revision["logical_sha256"]:
                raise ValueError("current source revision drift")
            for change in revision["inverse"]:
                if value.count(change["after"]) != 1:
                    raise ValueError("nonunique current revision inverse")
                value = value.replace(change["after"], change["before"], 1)
        if hashlib.sha256(value.encode()).hexdigest() != record["after_sha256"]:
            raise ValueError("original lazy-stage source drift")
        return value

    def _stage_for(self, path):
        record = next(item for item in self.records if item["path"] == path)
        return self._lazy_stage(record, self._current_source(record))

    def test_whole_predecessors_and_tampered_inputs_remain_bound(self):
        digest = lambda value: hashlib.sha256(value.encode()).hexdigest()
        for record in self.records:
            with self.subTest(path=record["path"]):
                before = (self.evidence / record["archive"]).read_text()
                current = self._current_source(record)
                self.assertEqual(digest(before), record["before_sha256"])

                def restore(value):
                    value = self._lazy_stage(record, value)
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
        before, current = originals[path], self._stage_for(path)
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

        self.assertEqual(callbacks(originals[path]), callbacks(self._stage_for(path)))
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


    def test_actual_allocator_callbacks_and_windows_still_match_complete_originals(self):
        record = next(item for item in self.records if item["path"] == "src/test_allocation_observer.rs")
        before = (self.evidence / record["archive"]).read_text()
        self._current_source(record)  # Authenticate both actual current root and backing first.
        current = (self.root / "src/test_allocation_observer/backing.rs").read_text()
        for start, end in [
            ("    fn allocated(", "\nthread_local!"),
            ("unsafe impl GlobalAlloc for ObservedSystem", "\nstruct Guard;"),
            ("struct Guard;", "// Keep instrumentation"),
        ]:
            self.assertEqual(before.split(start, 1)[1].split(end, 1)[0],
                             current.split(start, 1)[1].split(end, 1)[0])
        start = "fn finish_observation("
        self.assertEqual(before.split(start, 1)[1].split("\n#[cfg(test)]\nmod tests", 1)[0],
                         current.split(start, 1)[1])

    def test_current_roots_and_relocated_backing_reject_independent_byte_drift(self):
        for record in self.records:
            if "current_projection" not in record:
                continue
            with self.subTest(path=record["path"]):
                raw = (self.root / record["path"]).read_text()
                with self.assertRaises(ValueError):
                    self._current_source(record, {record["path"]: raw + "\n"})
                for fragment in record["current_projection"]["fragments"]:
                    raw = (self.root / fragment["path"]).read_text()
                    with self.assertRaises(ValueError):
                        self._current_source(record, {fragment["path"]: "!" + raw[1:]})
                changed = dict(record, current_projection=dict(record["current_projection"], fragments=[dict(path="../foreign")]))
                with self.assertRaises(ValueError):
                    self._current_source(changed)

    def test_real_forecast_capture_evidence_preserves_original_fields_and_stricter_publication(self):
        path = self.root / "docs/evidence/lazy-test-instrumentation-current-20261022/callback-captures.json"
        with path.open("rb") as source:
            raw = source.read(123579 + 1)
        self.assertEqual(len(raw), 123579)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), "43a0f965be961ce44bd15febbcc6d167a02415014b6ab30334d8bced76d85458")
        data = json.loads(raw)
        record = next(item for item in self.records if item["path"] == "src/workspace_graph/forecast_observation.rs")
        self.assertEqual(data["old_complete_producer_sha256"], record["after_sha256"])
        self.assertEqual(data["current_complete_producer_sha256"], hashlib.sha256(self._current_source(record).encode()).hexdigest())
        self.assertEqual(data["pointer_bits"], 64)
        self.assertEqual([row["case"] for row in data["cases"]], list(range(5)))
        for row in data["cases"]:
            before, current = row["old"], row["current"]
            self.assertEqual({key: value for key, value in before.items() if key != "records"},
                             {key: value for key, value in current.items() if key != "records"})
            self.assertEqual(len(before["records"]), len(current["records"]))
            for old, new in zip(before["records"], current["records"]):
                self.assertEqual({key: value for key, value in old.items() if key != "values"},
                                 {key: value for key, value in new.items() if key != "values"})
                if old["global"]:
                    self.assertEqual(old["values"][:4], new["values"][:4])
                    self.assertEqual(old["values"][4:], [0] * 13)
                    self.assertEqual(new["values"][4:6], [(1 << 64) - 1, 1] if old["values"][0] else [0, 0])
                    self.assertEqual(new["values"][6:], [0] * 11)
                else:
                    self.assertEqual(old["values"], new["values"])
            self.assertTrue(row["current_published_passes"])
            for value in row["current_published_passes"]:
                self.assertIsNone(value["global_peak"])
                self.assertIsNone(value["retained_sum"])



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
