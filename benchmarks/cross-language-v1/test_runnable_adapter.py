#!/usr/bin/env python3
"""Focused, offline regressions for runnable-adapter v1."""
from __future__ import annotations

import hashlib
import importlib.util
import json
import pathlib
import plistlib
import os
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock


SUITE = pathlib.Path(__file__).resolve().parent
MODULE_SPEC = importlib.util.spec_from_file_location("runnable_adapter", SUITE / "runnable_adapter.py")
assert MODULE_SPEC and MODULE_SPEC.loader
RUNNABLE = importlib.util.module_from_spec(MODULE_SPEC)
MODULE_SPEC.loader.exec_module(RUNNABLE)


def canonical(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=True) + "\n").encode("utf-8")


def digest(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


class RunnableAdapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.tool = pathlib.Path("/opt/homebrew/bin/rustc").resolve()
        assert cls.tool.is_file(), f"local Rust fixture compiler is unavailable: {cls.tool}"
        cls.tool_root = cls.tool.parent.parent
        cls.tool_root_sha256 = RUNNABLE._toolchain_digest(cls.tool_root)
        cls.linker = pathlib.Path("/usr/bin/cc").resolve()
        assert cls.linker.is_file(), f"local Rust fixture linker is unavailable: {cls.linker}"
        cls.link_editor = pathlib.Path("/Library/Developer/CommandLineTools/usr/bin/ld")
        assert cls.link_editor.is_file(), f"local Rust fixture link editor is unavailable: {cls.link_editor}"
        cls.sdk_root = pathlib.Path("/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk")
        assert cls.sdk_root.is_dir() and not cls.sdk_root.is_symlink(), "canonical macOS SDK root is unavailable"
        cls.sdk_settings = (cls.sdk_root / "SDKSettings.json").read_bytes()
        cls.sdk_system_version = (cls.sdk_root / "System/Library/CoreServices/SystemVersion.plist").read_bytes()
        cls.sdk_version = json.loads(cls.sdk_settings)["Version"]
        cls.sdk_build = plistlib.loads(cls.sdk_system_version)["ProductBuildVersion"]

    def setUp(self) -> None:
        self.tasks = (SUITE / "tasks.json").read_bytes()
        self.adapters = (SUITE / "adapters.json").read_bytes()

    def baseline(self) -> dict:
        task_ids = [row["id"] for row in json.loads(self.tasks)["tasks"]]
        return {
            "schema": "benchmark.cross_language.agent.baseline_admission.v1",
            "system": {"id": "zero", "display_name": "Zero"},
            "toolchain": {
                "official_source": "https://github.com/vercel-labs/zerolang",
                "revision": "eb2ed6c22fe3f6e3152efa0c0d05ffcf1ff4a2c7",
                "version": "fixture-only",
                "artifact_sha256": digest(b"fixture artifact"),
                "installation_receipt_sha256": digest(b"fixture install receipt"),
                "license": "fixture-only",
            },
            "agent_interface": {
                "version": "fixture-v1",
                "guidance_sha256": digest(b"fixture guidance"),
                "invocation_contract_sha256": digest(b"fixture contract"),
            },
            "model": {"provider": "fixture", "model": "fixture", "revision": "fixture-v1"},
            "ports": [{
                "task_id": task_id,
                "port_tree_sha256": digest(f"port:{task_id}".encode()),
                "oracle_sha256": digest(f"oracle:{task_id}".encode()),
                "equivalence_review_sha256": digest(f"review:{task_id}".encode()),
                "candidate_paths": ["src/candidate.zero"],
            } for task_id in task_ids],
            "execution": {"status": "not_executed"},
        }

    def descriptor(self) -> bytes:
        receipt = "local Rust fixture; execution is not an external-language result"
        return canonical({
            "schema": RUNNABLE.SCHEMA,
            "baseline": self.baseline(),
            "execution": {
                "classification": "local_fixture",
                "adapter_id": "rust",
                "task_id": "sequence-digest-v1",
                "adapter_inventory_sha256": digest(self.adapters),
                "receipt": receipt,
                "receipt_sha256": digest(receipt.encode()),
                "timeout_seconds": 120,
                "tool_path": str(self.tool),
                "tool_sha256": digest(self.tool.read_bytes()),
                "tool_root": str(self.tool_root),
                "tool_root_sha256": self.tool_root_sha256,
                "linker_path": str(self.linker),
                "linker_sha256": digest(self.linker.read_bytes()),
                "link_editor_path": str(self.link_editor),
                "link_editor_sha256": digest(self.link_editor.read_bytes()),
                "sdk_root": str(self.sdk_root),
                "sdk_version": self.sdk_version,
                "sdk_build": self.sdk_build,
                "sdk_settings_sha256": digest(self.sdk_settings),
                "sdk_system_version_sha256": digest(self.sdk_system_version),
            },
        })

    def test_local_rust_fixture_replays_the_existing_public_hidden_scorer(self) -> None:
        result = RUNNABLE.execute_local_fixture(self.descriptor(), self.tasks, self.adapters, RUNNABLE.ROOT)
        self.assertEqual(result["status"], "fixture_ok", result)
        self.assertEqual(result["classification"], "local_fixture")
        self.assertEqual(result["result"]["id"], "sequence-digest-v1::rust")
        self.assertEqual(result["result"]["leak_check"], "ok")
        self.assertTrue(result["result"]["public"]["passed"])
        self.assertTrue(result["result"]["hidden"]["passed"])

    def test_mutation_and_external_claims_refuse_before_execution(self) -> None:
        descriptor = json.loads(self.descriptor())
        descriptor["execution"]["adapter_inventory_sha256"] = digest(b"wrong")
        self.assertEqual(
            RUNNABLE.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "adapter_inventory_drifted",
        )
        descriptor = json.loads(self.descriptor())
        descriptor["execution"]["receipt"] = "tampered"
        self.assertEqual(
            RUNNABLE.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "execution_receipt_digest_disagrees",
        )
        descriptor = json.loads(self.descriptor())
        descriptor["execution"]["classification"] = "external"
        self.assertEqual(
            RUNNABLE.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "external_execution_requires_provisioned_review",
        )
        self.assertEqual(
            RUNNABLE.admit_runnable_descriptor(self.descriptor() + b" ", self.tasks, self.adapters)["reason"],
            "invalid_runnable_descriptor",
        )
        self.assertEqual(
            RUNNABLE.execute_local_fixture(self.descriptor(), self.tasks + b" ", self.adapters, RUNNABLE.ROOT)["reason"],
            "invalid_required_task_inventory",
        )
        with tempfile.TemporaryDirectory(prefix="spx-runnable-root-") as temporary:
            self.assertEqual(
                RUNNABLE.execute_local_fixture(self.descriptor(), self.tasks, self.adapters, pathlib.Path(temporary))["reason"],
                "posix_snapshot_execution_unavailable",
            )

    def test_malformed_bounds_and_containers_refuse_without_crashing(self) -> None:
        descriptor = json.loads(self.descriptor())
        descriptor["execution"]["timeout_seconds"] = True
        self.assertEqual(
            RUNNABLE.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "invalid_execution_bounds",
        )
        descriptor = json.loads(self.descriptor())
        descriptor["execution"] = []
        self.assertEqual(
            RUNNABLE.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "invalid_execution_provenance",
        )
        descriptor = json.loads(self.descriptor())
        descriptor["execution"]["adapter_id"] = []
        self.assertEqual(
            RUNNABLE.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "invalid_execution_provenance",
        )
        malformed_adapters = json.loads(self.adapters)
        malformed_adapters["adapters"][0]["id"] = []
        malformed_adapter_bytes = canonical(malformed_adapters)
        descriptor = json.loads(self.descriptor())
        descriptor["execution"]["adapter_inventory_sha256"] = digest(malformed_adapter_bytes)
        self.assertEqual(
            RUNNABLE.admit_runnable_descriptor(canonical(descriptor), self.tasks, malformed_adapter_bytes)["reason"],
            "invalid_adapter_inventory",
        )

    def test_internal_absolute_toolchain_link_is_private_after_original_replacement(self) -> None:
        with tempfile.TemporaryDirectory(prefix="spx-runnable-toolchain-") as temporary:
            root = pathlib.Path(temporary).resolve() / "toolchain"
            root.mkdir()
            target = root / "real-tool"
            target.write_bytes(b"original tool bytes")
            absolute_link = root / "tool-link"
            absolute_link.symlink_to(target)
            snapshot = pathlib.Path(temporary).resolve() / "snapshot"
            RUNNABLE._toolchain_digest(root, snapshot)
            self.assertTrue((snapshot / "tool-link").is_symlink())
            self.assertFalse(os.path.isabs(os.readlink(snapshot / "tool-link")))
            target.write_bytes(b"replacement tool bytes")
            self.assertEqual((snapshot / "tool-link").read_bytes(), b"original tool bytes")
        descriptor = json.loads(self.descriptor())
        descriptor["execution"] = None
        self.assertEqual(
            RUNNABLE.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "invalid_execution_provenance",
        )

    def test_closed_environment_excludes_path_and_startup_variables(self) -> None:
        observed: dict[str, str] = {}

        def capture(_command: list[str], _cwd: pathlib.Path, _deadline: float,
                    environment: dict[str, str]) -> tuple[int | None, bytes, bytes, str | None]:
            observed.update(environment)
            return None, b"", b"", "environment_captured"

        with mock.patch.dict(os.environ, {
            "PATH": "/attacker/bin", "RUSTUP_HOME": "/attacker/rustup",
            "CARGO_HOME": "/attacker/cargo", "DYLD_INSERT_LIBRARIES": "/attacker/inject.dylib",
        }, clear=False), mock.patch.object(RUNNABLE, "_run_bounded_group", side_effect=capture):
            result = RUNNABLE.execute_local_fixture(self.descriptor(), self.tasks, self.adapters, RUNNABLE.ROOT)
        self.assertEqual(result["reason"], "environment_captured")
        self.assertEqual(set(observed), {"LANG", "LC_ALL", "TZ", "SDKROOT", "DEVELOPER_DIR"})
        self.assertNotIn("/attacker", " ".join(observed.values()))

    def test_snapshot_remains_bound_when_original_is_replaced_before_launch(self) -> None:
        source = RUNNABLE.ROOT / "benchmarks/cross-language-v1/tasks/sequence-digest-v1/public/rust/main.rs"
        original = source.read_bytes()

        def replace_after_snapshot(_snapshot_root: pathlib.Path) -> None:
            source.write_bytes(b"this replacement must never reach the scorer\n")

        try:
            with mock.patch.object(RUNNABLE, "_after_snapshot_before_launch", side_effect=replace_after_snapshot):
                result = RUNNABLE.execute_local_fixture(self.descriptor(), self.tasks, self.adapters, RUNNABLE.ROOT)
        finally:
            source.write_bytes(original)
        self.assertEqual(result["status"], "fixture_ok", result)
        self.assertTrue(result["result"]["public"]["passed"])

    def test_timeout_kills_an_orphaned_process_group_child(self) -> None:
        with tempfile.TemporaryDirectory(prefix="spx-runnable-child-") as temporary:
            marker = pathlib.Path(temporary) / "child.pid"
            script = (
                "import pathlib, subprocess, sys, time; "
                "child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)']); "
                f"pathlib.Path({str(marker)!r}).write_text(str(child.pid)); time.sleep(30)"
            )
            _, _, _, reason = RUNNABLE._run_bounded_group(
                [sys.executable, "-c", script], pathlib.Path(temporary), time.monotonic() + 0.25,
                dict(RUNNABLE.CLOSED_ENVIRONMENT),
            )
            self.assertEqual(reason, "execution_timed_out")
            child_pid = int(marker.read_text())
            until = time.monotonic() + 3
            while True:
                try:
                    os.kill(child_pid, 0)
                except ProcessLookupError:
                    break
                if time.monotonic() >= until:
                    self.fail("process-group child survived timeout containment")
                time.sleep(0.05)

    def test_scorer_timeout_kills_adapter_descendants_in_the_outer_group(self) -> None:
        with tempfile.TemporaryDirectory(prefix="spx-runnable-nested-") as temporary:
            root = pathlib.Path(temporary)
            marker = root / "adapter-child.pid"
            adapter = (
                "import pathlib, subprocess, sys, time; "
                "child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)']); "
                f"pathlib.Path({str(marker)!r}).write_text(str(child.pid)); time.sleep(30)"
            )
            scorer = (
                "import importlib.util, os, pathlib, sys, time; "
                f"spec = importlib.util.spec_from_file_location('scorer', {str(RUNNABLE.RUNNER)!r}); "
                "scorer = importlib.util.module_from_spec(spec); spec.loader.exec_module(scorer); "
                "execution = scorer.HardenedExecution(environment={'LANG': 'C', 'LC_ALL': 'C', 'TZ': 'UTC'}, "
                "deadline=time.monotonic() + 0.3, output_limit=65536, group_id=os.getpgrp()); "
                f"scorer._run_hardened([sys.executable, '-c', {adapter!r}], pathlib.Path({str(root)!r}), execution)"
            )
            RUNNABLE._run_bounded_group(
                [sys.executable, "-c", scorer], root, time.monotonic() + 5,
                dict(RUNNABLE.CLOSED_ENVIRONMENT),
            )
            child_pid = int(marker.read_text())
            until = time.monotonic() + 3
            while True:
                try:
                    os.kill(child_pid, 0)
                except ProcessLookupError:
                    break
                if time.monotonic() >= until:
                    self.fail("adapter descendant survived scorer timeout containment")
                time.sleep(0.05)

    def test_oversized_and_fifo_inputs_and_outputs_refuse_without_blocking(self) -> None:
        with tempfile.TemporaryDirectory(prefix="spx-runnable-bounds-") as temporary:
            root = pathlib.Path(temporary)
            oversized = root / "oversized.json"
            oversized.write_bytes(b"x" * (RUNNABLE.MAX_DESCRIPTOR_BYTES + 1))
            with self.assertRaisesRegex(RUNNABLE.SnapshotError, "regular_file_type_or_size_refused"):
                RUNNABLE._read_regular(oversized.resolve(), RUNNABLE.MAX_DESCRIPTOR_BYTES)
            fifo = root / "input.fifo"
            os.mkfifo(fifo)
            with self.assertRaisesRegex(RUNNABLE.SnapshotError, "regular_file_type_or_size_refused"):
                RUNNABLE._read_regular(fifo.resolve(), RUNNABLE.MAX_DESCRIPTOR_BYTES)
            with self.assertRaisesRegex(RUNNABLE.SnapshotError, "result_exceeds_byte_bound"):
                RUNNABLE._write_new_regular(root / "oversized-result.json", b"x" * (RUNNABLE.MAX_RESULT_BYTES + 1))
            _, _, _, overflow = RUNNABLE._run_bounded_group(
                [sys.executable, "-c", "import sys; sys.stdout.write('x' * 70000)"], root,
                time.monotonic() + 5, dict(RUNNABLE.CLOSED_ENVIRONMENT),
            )
            self.assertEqual(overflow, "execution_output_exceeded")
            descriptor = root / "descriptor.json"
            descriptor.write_bytes(b"{}")
            output_fifo = root / "output.fifo"
            os.mkfifo(output_fifo)
            completed = subprocess.run([
                sys.executable, str(RUNNABLE.SUITE / "runnable_adapter.py"), "--descriptor", str(descriptor),
                "--output", str(output_fifo),
            ], capture_output=True, timeout=5, check=False)
            self.assertEqual(completed.returncode, 2)

    def test_scoring_inventory_retains_every_blocked_adapter(self) -> None:
        with tempfile.TemporaryDirectory(prefix="spx-runnable-plan-") as temporary:
            output = pathlib.Path(temporary) / "plan.json"
            completed = RUNNABLE.subprocess.run([
                RUNNABLE.sys.executable, str(RUNNABLE.RUNNER), "--dry-run",
                "--root", str(RUNNABLE.ROOT), "--tasks", str(RUNNABLE.TASKS),
                "--adapters", str(RUNNABLE.ADAPTERS), "--output", str(output),
            ], capture_output=True, text=True, timeout=120, check=False)
            self.assertEqual(completed.returncode, 0, completed.stderr)
            plan = json.loads(output.read_text())
        # 13 tasks x 14 adapters (10 pre-existing plus the c/python/swift/java
        # lanes runnable_adapter_v2 admits; see
        # docs/CROSS-LANGUAGE-RUNNABLE-ADAPTER-V2.md).
        self.assertEqual(len(plan["pairs"]), 182)
        by_language = {}
        for pair in plan["pairs"]:
            by_language[pair["language"]] = by_language.get(pair["language"], 0) + 1
        for language in ("zero", "ntnt", "aver", "vera", "hale", "moonbit"):
            self.assertEqual(by_language.get(language), 13)
            self.assertFalse(any(pair["language"] == language and pair["implemented"] for pair in plan["pairs"]))


if __name__ == "__main__":
    unittest.main()
