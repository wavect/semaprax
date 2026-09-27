#!/usr/bin/env python3
"""Focused, offline regressions for runnable-adapter v2.

Mirrors `test_runnable_adapter.py`'s conventions (a `setUpClass` that asserts
this specific host's local fixture tools are present, a `descriptor()`
builder, positive fixture tests, and hostile-input admission tests) rather
than reinventing them, because v2 reuses v1's audited primitives directly.
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
import pathlib
import os
import sys
import tempfile
import unittest
from unittest import mock


SUITE = pathlib.Path(__file__).resolve().parent
MODULE_SPEC = importlib.util.spec_from_file_location("runnable_adapter_v2", SUITE / "runnable_adapter_v2.py")
assert MODULE_SPEC and MODULE_SPEC.loader
V2 = importlib.util.module_from_spec(MODULE_SPEC)
MODULE_SPEC.loader.exec_module(V2)


def canonical(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=True) + "\n").encode("utf-8")


def digest(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


class RunnableAdapterV2Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.tools = {
            "clang": pathlib.Path("/usr/bin/clang"),
            "python3": pathlib.Path("/usr/bin/python3"),
            "swiftc": pathlib.Path("/usr/bin/swiftc"),
            "javac": pathlib.Path("/usr/bin/javac"),
            "java": pathlib.Path("/usr/bin/java"),
        }
        for placeholder, path in cls.tools.items():
            assert path.is_file(), f"local fixture tool {placeholder} is unavailable: {path}"
        cls.node = pathlib.Path("/Users/kevin/.nvm/versions/node/v22.12.0/bin/node")
        assert cls.node.is_file(), f"local fixture node is unavailable: {cls.node}"
        cls.typescript_lib = pathlib.Path(
            "/Users/kevin/Library/pnpm/global/5/.pnpm/typescript@5.8.3/node_modules/typescript"
        )
        assert cls.typescript_lib.is_dir(), f"local fixture typescript package is unavailable: {cls.typescript_lib}"
        cls.typescript_lib_sha256 = V2.v1._toolchain_digest(cls.typescript_lib)

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

    def _tool_entry(self, placeholder: str) -> dict:
        path = self.tools[placeholder]
        return {"placeholder": placeholder, "path": str(path), "sha256": digest(path.read_bytes())}

    def descriptor(self, adapter_id: str, task_id: str = "sequence-digest-v1", *, timeout_seconds: int = 30) -> bytes:
        if adapter_id == "typescript":
            tools = [{"placeholder": "node", "path": str(self.node), "sha256": digest(self.node.read_bytes())}]
            roots = [{"placeholder": "typescript_lib", "path": str(self.typescript_lib),
                      "sha256": self.typescript_lib_sha256, "entry_relative": "bin/tsc"}]
        else:
            tools = [self._tool_entry(placeholder) for placeholder in V2.V2_ADAPTERS[adapter_id]["tools"]]
            roots = []
        receipt = f"local {adapter_id} fixture; execution is not an external-language result"
        return canonical({
            "schema": V2.SCHEMA,
            "baseline": self.baseline(),
            "execution": {
                "classification": "local_fixture",
                "adapter_id": adapter_id,
                "task_id": task_id,
                "adapter_inventory_sha256": digest(self.adapters),
                "receipt": receipt,
                "receipt_sha256": digest(receipt.encode()),
                "timeout_seconds": timeout_seconds,
                "tools": tools,
                "copied_roots": roots,
            },
        })

    # ------------------------------------------------------------------
    # Positive fixture (scoring) tests: one per newly admitted lane, plus
    # TypeScript, which was already `implemented: true` at rest but had
    # never actually been executed by any runnable-adapter contract before.
    # ------------------------------------------------------------------

    def _assert_fixture_ok(self, adapter_id: str) -> None:
        result = V2.execute_local_fixture(self.descriptor(adapter_id), self.tasks, self.adapters, V2.v1.ROOT)
        self.assertEqual(result["status"], "fixture_ok", result)
        self.assertEqual(result["classification"], "local_fixture")
        self.assertEqual(result["result"]["id"], f"sequence-digest-v1::{adapter_id}")
        self.assertEqual(result["result"]["leak_check"], "ok")
        self.assertTrue(result["result"]["public"]["passed"])
        self.assertTrue(result["result"]["hidden"]["passed"])

    def test_local_c_fixture_replays_the_existing_public_hidden_scorer(self) -> None:
        self._assert_fixture_ok("c")

    def test_local_python_fixture_replays_the_existing_public_hidden_scorer(self) -> None:
        self._assert_fixture_ok("python")

    def test_local_swift_fixture_replays_the_existing_public_hidden_scorer(self) -> None:
        self._assert_fixture_ok("swift")

    def test_local_java_fixture_replays_the_existing_public_hidden_scorer(self) -> None:
        self._assert_fixture_ok("java")

    def test_local_typescript_fixture_replays_the_existing_public_hidden_scorer(self) -> None:
        self._assert_fixture_ok("typescript")

    # ------------------------------------------------------------------
    # Hostile-input admission tests.
    # ------------------------------------------------------------------

    def test_rust_and_reserved_lanes_are_not_admitted_under_v2(self) -> None:
        for adapter_id in ("rust", "semaprax", "semaprax-project", "zero", "ntnt", "moonbit"):
            descriptor = json.loads(self.descriptor("python"))
            descriptor["execution"]["adapter_id"] = adapter_id
            descriptor["execution"]["task_id"] = "sequence-digest-v1"
            self.assertEqual(
                V2.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
                "adapter_not_admitted_under_runnable_adapter_v2",
                adapter_id,
            )

    def test_mutation_and_external_claims_refuse_before_execution(self) -> None:
        descriptor = json.loads(self.descriptor("python"))
        descriptor["execution"]["adapter_inventory_sha256"] = digest(b"wrong")
        self.assertEqual(
            V2.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "adapter_inventory_drifted",
        )
        descriptor = json.loads(self.descriptor("python"))
        descriptor["execution"]["receipt"] = "tampered"
        self.assertEqual(
            V2.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "execution_receipt_digest_disagrees",
        )
        descriptor = json.loads(self.descriptor("python"))
        descriptor["execution"]["classification"] = "external"
        self.assertEqual(
            V2.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "external_execution_requires_provisioned_review",
        )
        self.assertEqual(
            V2.admit_runnable_descriptor(self.descriptor("python") + b" ", self.tasks, self.adapters)["reason"],
            "invalid_runnable_descriptor",
        )
        self.assertEqual(
            V2.execute_local_fixture(self.descriptor("python"), self.tasks + b" ", self.adapters, V2.v1.ROOT)["reason"],
            "invalid_required_task_inventory",
        )
        with tempfile.TemporaryDirectory(prefix="spx-runnable-v2-root-") as temporary:
            self.assertEqual(
                V2.execute_local_fixture(self.descriptor("python"), self.tasks, self.adapters,
                                          pathlib.Path(temporary))["reason"],
                "posix_snapshot_execution_unavailable",
            )

    def test_task_not_declaring_the_adapter_refuses(self) -> None:
        descriptor = json.loads(self.descriptor("python"))
        descriptor["execution"]["task_id"] = "bounded-counter-repair-v1"
        self.assertEqual(
            V2.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "task_does_not_declare_adapter",
        )

    def test_malformed_tool_bindings_refuse(self) -> None:
        # Extra tool beyond the adapter's declared requirement.
        descriptor = json.loads(self.descriptor("python"))
        descriptor["execution"]["tools"].append(self._tool_entry("clang"))
        self.assertEqual(
            V2.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "invalid_tool_binding",
        )
        # Missing the one required tool.
        descriptor = json.loads(self.descriptor("python"))
        descriptor["execution"]["tools"] = []
        self.assertEqual(
            V2.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "invalid_tool_binding",
        )
        # A relative (non-absolute) tool path.
        descriptor = json.loads(self.descriptor("python"))
        descriptor["execution"]["tools"][0]["path"] = "python3"
        self.assertEqual(
            V2.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "invalid_tool_binding",
        )
        # java requires two distinct placeholders; declaring the same one twice
        # must still be refused as missing the other required placeholder.
        descriptor = json.loads(self.descriptor("java"))
        descriptor["execution"]["tools"] = [self._tool_entry("javac"), self._tool_entry("javac")]
        self.assertEqual(
            V2.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "invalid_tool_binding",
        )

    def test_malformed_copied_root_bindings_refuse(self) -> None:
        # typescript requires exactly one copied root; c must declare none.
        descriptor = json.loads(self.descriptor("c"))
        descriptor["execution"]["copied_roots"] = [{
            "placeholder": "typescript_lib", "path": str(self.typescript_lib),
            "sha256": self.typescript_lib_sha256, "entry_relative": "bin/tsc",
        }]
        self.assertEqual(
            V2.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "invalid_copied_root_binding",
        )
        # An escaping entry_relative must be refused at admission, before any
        # copy is attempted.
        descriptor = json.loads(self.descriptor("typescript"))
        descriptor["execution"]["copied_roots"][0]["entry_relative"] = "../../../etc/passwd"
        self.assertEqual(
            V2.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "invalid_copied_root_binding",
        )
        descriptor = json.loads(self.descriptor("typescript"))
        descriptor["execution"]["copied_roots"][0]["entry_relative"] = "/etc/passwd"
        self.assertEqual(
            V2.admit_runnable_descriptor(canonical(descriptor), self.tasks, self.adapters)["reason"],
            "invalid_copied_root_binding",
        )

    def test_tampered_tool_digest_refuses_at_execution(self) -> None:
        descriptor = json.loads(self.descriptor("typescript"))
        descriptor["execution"]["tools"][0]["sha256"] = digest(b"not the real node binary")
        result = V2.execute_local_fixture(canonical(descriptor), self.tasks, self.adapters, V2.v1.ROOT)
        self.assertEqual(result["reason"], "node_identity_drifted")

    def test_tampered_root_digest_refuses_at_execution(self) -> None:
        descriptor = json.loads(self.descriptor("typescript"))
        descriptor["execution"]["copied_roots"][0]["sha256"] = digest(b"not the real typescript package")
        result = V2.execute_local_fixture(canonical(descriptor), self.tasks, self.adapters, V2.v1.ROOT)
        self.assertEqual(result["reason"], "typescript_lib_root_identity_drifted")

    def test_materialize_command_refuses_an_unbound_token(self) -> None:
        bound = {"clang": self.tools["clang"]}
        with self.assertRaisesRegex(V2.v1.SnapshotError, "selected_adapter_command_not_admitted"):
            V2._materialize_command("c", ["cargo", "build"], bound)
        # A locally-built binary is passed through untouched, matching v1.
        self.assertEqual(V2._materialize_command("c", ["./test_bin"], bound), ["./test_bin"])
        # TypeScript's declared `tsc` token expands to node + the bound script.
        node = pathlib.Path("/private/node")
        script = pathlib.Path("/private/tsc-root/bin/tsc")
        rewritten = V2._materialize_command(
            "typescript", ["tsc", "--strict", "index.ts"], {"node": node, "typescript_lib": script},
        )
        self.assertEqual(rewritten, [str(node), str(script), "--strict", "index.ts"])

    def test_closed_environment_excludes_path_sdk_and_startup_variables(self) -> None:
        observed: dict[str, str] = {}

        def capture(_command, _cwd, _deadline, environment):
            observed.update(environment)
            return None, b"", b"", "environment_captured"

        with mock.patch.dict(os.environ, {
            "PATH": "/attacker/bin", "SDKROOT": "/attacker/sdk", "DEVELOPER_DIR": "/attacker/dev",
            "PYTHONPATH": "/attacker/python", "DYLD_INSERT_LIBRARIES": "/attacker/inject.dylib",
        }, clear=False), mock.patch.object(V2.v1, "_run_bounded_group", side_effect=capture):
            result = V2.execute_local_fixture(self.descriptor("python"), self.tasks, self.adapters, V2.v1.ROOT)
        self.assertEqual(result["reason"], "environment_captured")
        self.assertEqual(set(observed), {"LANG", "LC_ALL", "TZ"})
        self.assertNotIn("/attacker", " ".join(observed.values()))

    def test_snapshot_remains_bound_when_original_is_replaced_before_launch(self) -> None:
        source = V2.v1.ROOT / "benchmarks/cross-language-v1/tasks/sequence-digest-v1/public/python/digest.py"
        original = source.read_bytes()

        def replace_after_snapshot(_snapshot_root: pathlib.Path) -> None:
            source.write_bytes(b"raise SystemExit('this replacement must never reach the scorer')\n")

        try:
            with mock.patch.object(V2.v1, "_after_snapshot_before_launch", side_effect=replace_after_snapshot):
                result = V2.execute_local_fixture(self.descriptor("python"), self.tasks, self.adapters, V2.v1.ROOT)
        finally:
            source.write_bytes(original)
        self.assertEqual(result["status"], "fixture_ok", result)
        self.assertTrue(result["result"]["public"]["passed"])

    def test_snapshot_adapter_refuses_a_broken_command_shape(self) -> None:
        # `execute_local_fixture` only ever accepts the exact bytes currently
        # on disk (drift fails closed before this point is reachable through
        # the public entry point), so this exercises `_snapshot_adapter`
        # directly: a committed `adapters.json` row whose command does not
        # start with a bound placeholder must never be silently accepted.
        tampered_document = json.loads(self.adapters)
        row = next(item for item in tampered_document["adapters"] if item["id"] == "c")
        row["build_command"] = ["cargo", "build"]
        tampered_bytes = canonical(tampered_document)
        bound = {"clang": self.tools["clang"]}
        with self.assertRaisesRegex(V2.v1.SnapshotError, "selected_adapter_command_not_admitted"):
            V2._snapshot_adapter(tampered_bytes, "c", bound)


if __name__ == "__main__":
    unittest.main()
