#!/usr/bin/env python3
"""Bounded, offline runnable-adapter extension for cross-language v1 — v2.

`runnable_adapter.py` (v1) admits exactly one local fixture lane, `rust`, and
binds it through Rust-specific fields (a copied `rustc` toolchain root, an
external linker, a macOS SDK receipt). This module is a second, explicitly
versioned schema (`benchmark.cross_language.runnable_adapter.v2`) that
generalizes the same admission and containment guarantees to a data-driven
list of bound host tools, so a new language does not need its own bespoke
admission function. It does not replace, import as an override, or alter the
behavior of v1 in any way: `rust` stays exclusively v1's lane, admitted and
executed by the unmodified v1 module, and every hostile-input guarantee v1
already proves (closed environment, no shell, bounded reads, private
snapshot immutability, drift refusal) is reused here verbatim rather than
re-implemented — this module imports v1's primitives and calls them, it does
not fork them.

v2 currently admits five lanes: `c` (clang), `python` (CPython), `swift`
(swiftc), `java` (javac + java), and `typescript` (tsc + node — the adapter
was already declared `implemented: true` in `adapters.json` and already had
ported task fixtures; v1 only ever wired `rust`, so this is the first time
TypeScript is actually executed rather than merely admitted at rest). See
`docs/CROSS-LANGUAGE-RUNNABLE-ADAPTER-V2.md`.

Two trust shapes cover every lane's tools:

- A **SIP/root-owned executable** (macOS's `/usr/bin/clang`, `python3`,
  `swiftc`, `javac`, `java`): verified by absolute path, root ownership, a
  non-group/other-writable mode, and a SHA-256 digest, then referenced by its
  *original* path — never copied. This is exactly v1's existing treatment of
  the Rust fixture's external linker and link editor
  (`_admit_host_executable`), reused unchanged: these paths are already
  outside any single writer's ability to alter without administrator
  privileges, so a private copy adds no additional immutability guarantee.
- A **copied tool** (Node.js, and the TypeScript package tree it loads): not
  root-owned, so it is read, verified against its declared digest, and
  materialized into a private snapshot directory before launch, exactly as
  v1 already does for the entire Rust toolchain root.

None of these five lanes needs a linker or macOS SDK binding: unlike `rustc`,
`clang`/`swiftc`/`javac`+`java`/`python3`/`node` were confirmed (by direct,
closed-environment invocation on this host, in this repository's execution
evidence) to build and run a positive fixture with only `LANG`/`LC_ALL`/`TZ`
set — no `SDKROOT`, `DEVELOPER_DIR`, or external `-C linker=` is required, so
v2's closed environment is narrower than v1's, not merely different.

This is deliberately still a *local fixture*: `clang`/`swiftc`/`javac`/`java`/
`python3` are the host's locally installed Xcode Command Line Tools / system
Java, and Node.js/TypeScript are this host's locally installed npm packages —
none of these carries an offline-verifiable official upstream release digest
the way `runnable_adapter.py`'s Rust lane at least approximates via its
toolchain-root digest. Provenance here is recorded honestly as local-host
provenance (exact path, owner, mode, and byte digest actually observed on
this machine), never described as an authenticated official release.
"""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import sys
import tempfile
import time
from typing import Any

SUITE = pathlib.Path(__file__).resolve().parent

if str(SUITE) not in sys.path:
    sys.path.insert(0, str(SUITE))

import runnable_adapter as v1  # noqa: E402  (reuse every audited primitive)
from agent.baseline_admission import (  # noqa: E402
    ADMISSION_SCHEMA,
    _canonical_owner_task_inventory,
    admit_baseline_descriptor,
)

SCHEMA = "benchmark.cross_language.runnable_adapter.v2"

# Which tool/root placeholders each admitted lane must declare, exactly (no
# more, no fewer). This allowlist -- not caller-supplied metadata -- decides
# whether a placeholder is a verify-in-place SIP/root-owned executable or a
# copy-before-launch tool, so a caller cannot relabel an untrusted path as
# "trusted" through the descriptor.
V2_ADAPTERS: dict[str, dict[str, tuple[str, ...]]] = {
    "c": {"tools": ("clang",), "roots": ()},
    "python": {"tools": ("python3",), "roots": ()},
    "swift": {"tools": ("swiftc",), "roots": ()},
    "java": {"tools": ("javac", "java"), "roots": ()},
    "typescript": {"tools": ("node",), "roots": ("typescript_lib",)},
}
SIP_TOOLS = frozenset({"clang", "python3", "swiftc", "javac", "java"})
COPIED_TOOLS = frozenset({"node"})


def admit_runnable_descriptor(descriptor_bytes: bytes, owner_task_inventory_bytes: bytes,
                              adapter_inventory_bytes: bytes) -> dict[str, Any]:
    """Extend, rather than duplicate, unavailable baseline admission."""
    descriptor = v1._parse_canonical_descriptor(descriptor_bytes)
    if descriptor is None or set(descriptor) != {"schema", "baseline", "execution"} or descriptor.get("schema") != SCHEMA:
        return v1._unavailable("invalid_runnable_descriptor")
    baseline = descriptor["baseline"]
    if not isinstance(baseline, dict) or baseline.get("schema") != ADMISSION_SCHEMA:
        return v1._unavailable("invalid_baseline_descriptor")
    baseline_decision = admit_baseline_descriptor(baseline, owner_task_inventory_bytes)
    if baseline_decision.get("status") != "unavailable":
        return v1._unavailable("baseline_descriptor_not_admissible")
    if baseline_decision.get("reason") != "offline_admission_is_not_execution_evidence":
        return v1._unavailable(baseline_decision["reason"])
    if _canonical_owner_task_inventory(owner_task_inventory_bytes) is None:
        return v1._unavailable("invalid_required_task_inventory")
    by_id = v1._adapter_inventory(adapter_inventory_bytes)
    if by_id is None:
        return v1._unavailable("invalid_adapter_inventory")
    execution = descriptor["execution"]
    required = {"classification", "adapter_id", "task_id", "adapter_inventory_sha256", "receipt", "receipt_sha256",
                "timeout_seconds", "tools", "copied_roots"}
    if not isinstance(execution, dict) or set(execution) != required:
        return v1._unavailable("invalid_execution_provenance")
    if execution["classification"] != "local_fixture":
        return v1._unavailable("external_execution_requires_provisioned_review")
    if execution["adapter_inventory_sha256"] != v1._sha256(adapter_inventory_bytes):
        return v1._unavailable("adapter_inventory_drifted")
    if (not isinstance(execution["receipt"], str) or len(execution["receipt"].encode("utf-8")) > v1.MAX_RECEIPT_BYTES
            or not isinstance(execution["receipt_sha256"], str)):
        return v1._unavailable("invalid_execution_receipt")
    if execution["receipt_sha256"] != v1._sha256(execution["receipt"].encode("utf-8")):
        return v1._unavailable("execution_receipt_digest_disagrees")
    if type(execution["timeout_seconds"]) is not int or not 1 <= execution["timeout_seconds"] <= v1.MAX_TIMEOUT_SECONDS:
        return v1._unavailable("invalid_execution_bounds")
    if not isinstance(execution["adapter_id"], str) or not isinstance(execution["task_id"], str):
        return v1._unavailable("invalid_execution_provenance")
    adapter_id = execution["adapter_id"]
    if adapter_id not in V2_ADAPTERS:
        return v1._unavailable("adapter_not_admitted_under_runnable_adapter_v2")
    adapter = by_id.get(adapter_id)
    if not isinstance(adapter, dict):
        return v1._unavailable("undeclared_task_or_adapter")
    if adapter.get("implemented") is not True:
        return v1._unavailable("adapter_remains_unavailable")
    owner = json.loads(owner_task_inventory_bytes.decode("utf-8"))
    task = next((row for row in owner["tasks"] if row["id"] == execution["task_id"]), None)
    if not isinstance(task, dict) or adapter_id not in task.get("languages", {}):
        return v1._unavailable("task_does_not_declare_adapter")

    tools = execution["tools"]
    required_tools = set(V2_ADAPTERS[adapter_id]["tools"])
    if not isinstance(tools, list) or len(tools) != len(required_tools):
        return v1._unavailable("invalid_tool_binding")
    bound_tools: dict[str, tuple[pathlib.Path, str]] = {}
    for entry in tools:
        if not isinstance(entry, dict) or set(entry) != {"placeholder", "path", "sha256"}:
            return v1._unavailable("invalid_tool_binding")
        placeholder, path_value, sha256_value = entry["placeholder"], entry["path"], entry["sha256"]
        if (not isinstance(placeholder, str) or placeholder not in required_tools or placeholder in bound_tools
                or not isinstance(path_value, str) or not isinstance(sha256_value, str)):
            return v1._unavailable("invalid_tool_binding")
        path = pathlib.Path(path_value)
        if not path.is_absolute() or path != path.resolve():
            return v1._unavailable("invalid_tool_binding")
        bound_tools[placeholder] = (path, sha256_value)
    if set(bound_tools) != required_tools:
        return v1._unavailable("invalid_tool_binding")

    roots = execution["copied_roots"]
    required_roots = set(V2_ADAPTERS[adapter_id]["roots"])
    if not isinstance(roots, list) or len(roots) != len(required_roots):
        return v1._unavailable("invalid_copied_root_binding")
    bound_roots: dict[str, tuple[pathlib.Path, str, pathlib.PurePosixPath]] = {}
    for entry in roots:
        if not isinstance(entry, dict) or set(entry) != {"placeholder", "path", "sha256", "entry_relative"}:
            return v1._unavailable("invalid_copied_root_binding")
        placeholder, path_value, sha256_value = entry["placeholder"], entry["path"], entry["sha256"]
        if (not isinstance(placeholder, str) or placeholder not in required_roots or placeholder in bound_roots
                or not isinstance(path_value, str) or not isinstance(sha256_value, str)):
            return v1._unavailable("invalid_copied_root_binding")
        path = pathlib.Path(path_value)
        if not path.is_absolute() or path != path.resolve():
            return v1._unavailable("invalid_copied_root_binding")
        try:
            entry_relative = v1._safe_relative(entry["entry_relative"])
        except v1.SnapshotError:
            return v1._unavailable("invalid_copied_root_binding")
        bound_roots[placeholder] = (path, sha256_value, entry_relative)
    if set(bound_roots) != required_roots:
        return v1._unavailable("invalid_copied_root_binding")

    return {"schema": SCHEMA, "status": "fixture_admitted", "baseline": baseline_decision["provenance"],
            "task_id": execution["task_id"], "adapter_id": adapter_id, "timeout_seconds": execution["timeout_seconds"],
            "tools": bound_tools, "copied_roots": bound_roots}


def _materialize_command(adapter_id: str, command: list[str], bound: dict[str, pathlib.Path]) -> list[str]:
    """Rewrite one declared adapters.json command to invoke only bound paths.

    A locally-built binary from this run's own build step (`./test_bin`) is
    never a placeholder and is passed through unchanged, matching v1's
    identical treatment of Rust's `./test_bin` run step.
    """
    line = list(command)
    if not line:
        return line
    if line[0].startswith("./"):
        return line
    if adapter_id == "typescript" and line[0] == "tsc":
        line[0:1] = [str(bound["node"]), str(bound["typescript_lib"])]
        return line
    if line[0] not in bound:
        raise v1.SnapshotError("selected_adapter_command_not_admitted")
    line[0] = str(bound[line[0]])
    return line


def _snapshot_adapter(adapters_bytes: bytes, adapter_id: str, bound: dict[str, pathlib.Path]) -> bytes:
    document = json.loads(adapters_bytes.decode("utf-8"))
    adapter = next(row for row in document["adapters"] if row["id"] == adapter_id)
    for key in ("version_command", "build_command", "run_command"):
        command = adapter.get(key)
        if command is None:
            continue
        if not isinstance(command, list) or not command or any(not isinstance(arg, str) for arg in command):
            raise v1.SnapshotError("selected_adapter_command_not_admitted")
        adapter[key] = _materialize_command(adapter_id, command, bound)
    return v1._canonical_bytes(document)


def execute_local_fixture(descriptor_bytes: bytes, owner_task_inventory_bytes: bytes,
                          adapter_inventory_bytes: bytes, root: pathlib.Path) -> dict[str, Any]:
    """Execute one immutable snapshot; never fall back to ambient tooling."""
    admitted = admit_runnable_descriptor(descriptor_bytes, owner_task_inventory_bytes, adapter_inventory_bytes)
    if admitted.get("status") != "fixture_admitted":
        return admitted
    if os.name != "posix" or root.resolve() != v1.ROOT.resolve():
        return v1._unavailable("posix_snapshot_execution_unavailable")
    try:
        tasks_now, _ = v1._read_regular(v1.TASKS, v1.MAX_DESCRIPTOR_BYTES)
        adapters_now, _ = v1._read_regular(v1.ADAPTERS, v1.MAX_DESCRIPTOR_BYTES)
        if tasks_now != owner_task_inventory_bytes or adapters_now != adapter_inventory_bytes:
            return v1._unavailable("execution_inputs_drifted")
        deadline = time.monotonic() + admitted["timeout_seconds"]
        with tempfile.TemporaryDirectory(prefix="spx-runnable-adapter-v2-") as temporary:
            snapshot_root = pathlib.Path(temporary) / "repository"
            snapshot_suite = snapshot_root / "benchmarks" / "cross-language-v1"
            runner_bytes, runner_stat = v1._read_regular(v1.RUNNER, v1.MAX_SOURCE_FILE_BYTES)
            v1._write_snapshot_file(snapshot_suite / "run.py", runner_bytes, runner_stat.st_mode)
            v1._write_snapshot_file(snapshot_suite / "runnable-descriptor.json", descriptor_bytes)
            v1._write_snapshot_file(snapshot_suite / "tasks.json", owner_task_inventory_bytes)
            inventory = json.loads(owner_task_inventory_bytes.decode("utf-8"))
            task = next(row for row in inventory["tasks"] if row["id"] == admitted["task_id"])
            paths = task["languages"][admitted["adapter_id"]]
            tree_digest = hashlib.sha256()
            budget = [0]
            for kind in ("public", "hidden"):
                relative = v1._safe_relative(paths[kind])
                v1._copy_tree(root / relative, snapshot_root / relative, tree_digest, budget, relative)
            equivalence = v1._safe_relative(task["equivalence"])
            equivalence_source = root / "benchmarks" / "cross-language-v1" / equivalence
            equivalence_bytes, equivalence_stat = v1._read_regular(equivalence_source, v1.MAX_SOURCE_FILE_BYTES)
            budget[0] += len(equivalence_bytes)
            if budget[0] > v1.MAX_SOURCE_TOTAL_BYTES:
                raise v1.SnapshotError("selected_source_exceeds_byte_bound")
            equivalence_identity = pathlib.PurePosixPath("benchmarks/cross-language-v1") / equivalence
            tree_digest.update(str(equivalence_identity).encode("utf-8"))
            tree_digest.update(b"\0\0")
            tree_digest.update(hashlib.sha256(equivalence_bytes).digest())
            v1._write_snapshot_file(snapshot_root / equivalence_identity, equivalence_bytes, equivalence_stat.st_mode)

            bound: dict[str, pathlib.Path] = {}
            tools_dir = snapshot_root / "tools"
            for placeholder, (path, sha256_value) in admitted["tools"].items():
                if placeholder in SIP_TOOLS:
                    bound[placeholder] = v1._admit_host_executable(path, sha256_value, placeholder)
                elif placeholder in COPIED_TOOLS:
                    data, source_stat = v1._read_regular(path, v1.MAX_TOOL_BYTES)
                    if v1._sha256(data) != sha256_value:
                        raise v1.SnapshotError(f"{placeholder}_identity_drifted")
                    destination = tools_dir / placeholder
                    v1._write_snapshot_file(destination, data, source_stat.st_mode)
                    bound[placeholder] = destination
                else:
                    raise v1.SnapshotError("invalid_tool_binding")
            for placeholder, (path, sha256_value, entry_relative) in admitted["copied_roots"].items():
                destination_root = tools_dir / placeholder
                if v1._toolchain_digest(path, destination_root) != sha256_value:
                    raise v1.SnapshotError(f"{placeholder}_root_identity_drifted")
                bound[placeholder] = destination_root / entry_relative

            snapshotted_adapters = _snapshot_adapter(adapter_inventory_bytes, admitted["adapter_id"], bound)
            v1._write_snapshot_file(snapshot_suite / "adapters.json", snapshotted_adapters)

            output = snapshot_root / "result.json"
            command = [sys.executable, str(snapshot_suite / "run.py"), "--hardened-posix",
                       "--execution-deadline-monotonic", repr(deadline), "--execution-output-bytes",
                       str(v1.MAX_PROCESS_OUTPUT_BYTES), "--root", str(snapshot_root), "--tasks",
                       str(snapshot_suite / "tasks.json"), "--adapters", str(snapshot_suite / "adapters.json"),
                       "--only", admitted["task_id"], "--language", admitted["adapter_id"], "--output", str(output)]
            v1._after_snapshot_before_launch(snapshot_root)
            code, _, _, reason = v1._run_bounded_group(command, snapshot_root, deadline, dict(v1.CLOSED_ENVIRONMENT))
            if reason is not None:
                return v1._unavailable(reason)
            if code != 0:
                return v1._unavailable("fixture_execution_failed")
            result_bytes, _ = v1._read_regular(output, v1.MAX_RESULT_BYTES)
            result = json.loads(result_bytes.decode("utf-8"))
    except v1.SnapshotError as error:
        return v1._unavailable(str(error))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, KeyError, TypeError, ValueError):
        return v1._unavailable("snapshot_or_result_refused")
    rows = result.get("results") if isinstance(result, dict) else None
    if not isinstance(rows, list) or len(rows) != 1 or not isinstance(rows[0], dict) or rows[0].get("status") != "ok":
        return v1._unavailable("fixture_scoring_failed")
    return {"schema": SCHEMA, "status": "fixture_ok", "classification": "local_fixture",
            "snapshot_sha256": v1._sha256(tree_digest.digest()), "result": rows[0]}


def main() -> int:
    import argparse
    parser = argparse.ArgumentParser(description="bounded runnable-adapter v2 fixture executor")
    parser.add_argument("--descriptor", required=True)
    parser.add_argument("--root", default=str(v1.ROOT))
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    try:
        descriptor_bytes, _ = v1._read_regular(pathlib.Path(args.descriptor).absolute(), v1.MAX_DESCRIPTOR_BYTES)
        tasks_bytes, _ = v1._read_regular(v1.TASKS, v1.MAX_DESCRIPTOR_BYTES)
        adapters_bytes, _ = v1._read_regular(v1.ADAPTERS, v1.MAX_DESCRIPTOR_BYTES)
        result = execute_local_fixture(descriptor_bytes, tasks_bytes, adapters_bytes, pathlib.Path(args.root))
    except v1.SnapshotError as error:
        result = v1._unavailable(str(error))
    try:
        v1._write_new_regular(pathlib.Path(args.output).absolute(), v1._canonical_bytes(result))
    except (v1.SnapshotError, OSError):
        return 2
    return 0 if result.get("status") == "fixture_ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
