#!/usr/bin/env python3
"""Run exact local generated-client and MCP graph-operational evidence."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parent.parent
GIT_EXECUTABLE = shutil.which("git")
SCHEMA = "semaprax.graph-operational-client-mcp-execution-evidence.v1"
MAX_LOG_BYTES = 16 * 1024 * 1024
MAX_INPUT_BYTES = 16 * 1024 * 1024
MAX_TOOL_BYTES = 256 * 1024 * 1024
MAX_COMMAND_SECONDS = 20 * 60
HEX_OBJECT = re.compile(r"[0-9a-f]{40}|[0-9a-f]{64}")
RESULT = re.compile(
    r"^test ([A-Za-z0-9_:]+) \.\.\. (ok|FAILED|ignored)(?:, [^\r\n]+)?$",
    re.MULTILINE,
)
SUMMARY = re.compile(
    r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; "
    r"(\d+) measured; (\d+) filtered out; finished in [^\r\n]+$",
    re.MULTILINE,
)

CLIENT_TESTS = (
    "selected_recursive_request_types_are_deterministic_and_preserve_legacy_helpers",
    "read_only_clients_cannot_acquire_constructor_or_publication_helpers",
    "generated_python_resolves_recursive_types_and_submits_exact_intent_for_compiler_admission",
    "all_languages_emit_deterministic_typed_responses_for_only_selected_methods",
    "concrete_chunk_schemas_keep_required_null_distinct_from_omission_and_opaque_contexts",
    "generated_python_typed_decoders_preserve_runtime_validation_and_opaque_report_boundaries",
    "full_repair_report_is_bundled_only_with_selected_diagnostics_and_all_clients_are_deterministic",
    "authored_python_harness_checks_actual_recursive_repair_payloads_and_hostile_nested_values",
    "authored_rust_harness_converts_recursive_typed_repairs_after_runtime_validation",
)
TYPESCRIPT_TEST = (
    "provisioned_typescript_harness_checks_actual_recursive_repair_payloads_and_"
    "hostile_nested_values"
)
MCP_ADAPTER_TESTS = (
    "pinned_lifecycle_negotiation_paging_and_self_contained_inputs_are_explicit",
    "tools_embed_exact_inner_v5_bytes_for_reads_mutations_holes_and_semantic_errors",
    "host_grants_control_discovery_and_notifications_never_execute_candidate_actions",
    "malformed_duplicate_fields_ids_and_arguments_fail_without_corrupting_lifecycle",
    "request_bounds_reject_before_forwarding_and_outer_overflow_is_terminal",
    "eof_and_live_source_drift_still_use_inner_workspace_authentication",
    "publication::writer_failure_after_mock_cas_preserves_known_or_uncertain_publication_classification",
    "publication::source_drift_after_delivered_mock_commit_keeps_g287_at_eof",
)
MCP_CLI_TESTS = (
    "help_pins_the_optional_mcp_command_without_replacing_v5",
    "real_stdio_catalogue_paging_and_notification_nonexecution_are_explicit",
    "all_six_fixed_host_policies_preserve_readonly_grants_and_exact_v5_read_bytes",
    "candidate_enabled_cli_replays_semantic_edits_without_source_or_execution_authority",
    "malformed_host_policy_fails_before_handshake_and_v5_frames_are_not_mcp_calls",
)


class EvidenceError(Exception):
    """A condition that prevents an honest evidence artifact."""


def run(arguments, *, env=None, combine=False, timeout=60):
    try:
        return subprocess.run(
            arguments,
            cwd=ROOT,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT if combine else subprocess.PIPE,
            check=False,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        raise EvidenceError(
            f"command exceeded its {timeout}-second bound: {arguments[0]}"
        ) from error


def require_command(arguments, label):
    completed = run(arguments)
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).decode("utf-8", "replace").strip()
        raise EvidenceError(f"cannot determine {label}: {detail}")
    try:
        value = completed.stdout.decode("utf-8", "strict").strip()
    except UnicodeDecodeError as error:
        raise EvidenceError(f"{label} is not UTF-8") from error
    if not value:
        raise EvidenceError(f"{label} is empty")
    return value


def git(*arguments):
    if GIT_EXECUTABLE is None:
        raise EvidenceError("required local tool is unavailable: git")
    return require_command((os.path.abspath(GIT_EXECUTABLE),) + arguments, "Git state")


def tree_state():
    if GIT_EXECUTABLE is None:
        raise EvidenceError("required local tool is unavailable: git")
    completed = run(
        (
            os.path.abspath(GIT_EXECUTABLE),
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
        )
    )
    if completed.returncode != 0:
        raise EvidenceError("cannot inspect the repository worktree")
    try:
        return completed.stdout.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise EvidenceError("Git worktree status is not UTF-8") from error


def sha256(body):
    return "sha256:" + hashlib.sha256(body).hexdigest()


def canonical(value):
    return json.dumps(
        value, ensure_ascii=False, allow_nan=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def regular_bytes(path, maximum, label, *, reject_links=True):
    try:
        status = path.lstat()
    except FileNotFoundError as error:
        raise EvidenceError(f"missing {label}: {path}") from error
    if path.is_symlink() or not path.is_file():
        raise EvidenceError(f"{label} is not a regular file: {path}")
    if reject_links and status.st_nlink != 1:
        raise EvidenceError(f"{label} has more than one hard link: {path}")
    if status.st_size > maximum:
        raise EvidenceError(f"{label} exceeds {maximum} bytes: {path}")
    return path.read_bytes()


def selected_tool(value, name):
    path = Path(value).expanduser()
    if not path.is_absolute():
        raise EvidenceError(f"{name} must be selected by an absolute path")
    if not os.access(path, os.X_OK):
        raise EvidenceError(f"{name} is not executable: {path}")
    resolved = path.resolve(strict=True)
    body = regular_bytes(resolved, MAX_TOOL_BYTES, f"{name} executable", reject_links=False)
    return path, resolved, body


def discovered_tool(name):
    selected = shutil.which(name)
    if selected is None:
        raise EvidenceError(f"required local tool is unavailable: {name}")
    return selected_tool(os.path.abspath(selected), name)


def tool_row(selected, resolved, body, version):
    return {
        "selected_path": str(selected),
        "resolved_path": str(resolved),
        "bytes": len(body),
        "sha256": sha256(body),
        "version": version,
    }


def require_tool_identity(bindings):
    for name, (selected, resolved, body) in bindings.items():
        _selected, current_resolved, current_body = selected_tool(selected, name)
        if current_resolved != resolved or current_body != body:
            raise EvidenceError(f"recorded {name} executable changed during execution")


def repository_inputs():
    bodies = {}
    rows = []
    for name in ("Cargo.toml", "Cargo.lock"):
        body = regular_bytes(ROOT / name, MAX_INPUT_BYTES, "repository input")
        bodies[name] = body
        rows.append({"path": name, "bytes": len(body), "sha256": sha256(body)})
    return bodies, rows


def require_repository_identity(commit, tree, inputs):
    if git("rev-parse", "--verify", "HEAD^{commit}") != commit:
        raise EvidenceError("Git HEAD changed during evidence execution")
    if git("rev-parse", "--verify", "HEAD^{tree}") != tree:
        raise EvidenceError("Git HEAD tree changed during evidence execution")
    current, _rows = repository_inputs()
    for path, original in inputs.items():
        if current.get(path) != original:
            raise EvidenceError(f"repository input changed during evidence execution: {path}")
    if tree_state():
        raise EvidenceError("repository worktree is not clean after execution")


def parse_test_log(
    body, expected_tests, expected_summaries, label, expected_ignored=()
):
    if len(body) > MAX_LOG_BYTES:
        raise EvidenceError(f"{label} Cargo log exceeds {MAX_LOG_BYTES} bytes")
    try:
        text = body.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise EvidenceError(f"{label} Cargo log is not UTF-8") from error
    rows = RESULT.findall(text)
    expected_rows = sorted(
        [(name, "ok") for name in expected_tests]
        + [(name, "ignored") for name in expected_ignored]
    )
    if sorted(rows) != expected_rows:
        raise EvidenceError(f"{label} selected unexpected test rows: {sorted(rows)!r}")
    summaries = sorted(tuple(int(value) for value in row) for row in SUMMARY.findall(text))
    if summaries != sorted(expected_summaries):
        raise EvidenceError(f"{label} has unexpected test summaries: {summaries!r}")


def execute(
    command,
    environment,
    expected_tests,
    expected_summaries,
    label,
    expected_ignored=(),
):
    completed = run(
        command, env=environment, combine=True, timeout=MAX_COMMAND_SECONDS
    )
    if completed.returncode != 0:
        raise EvidenceError(f"{label} failed with exit {completed.returncode}")
    parse_test_log(
        completed.stdout,
        expected_tests,
        expected_summaries,
        label,
        expected_ignored,
    )
    return completed.stdout


def write_bundle(destination, evidence, logs):
    parent = destination.parent
    parent.mkdir(parents=True, exist_ok=True)
    if destination.exists() or destination.is_symlink():
        raise EvidenceError(f"evidence destination already exists: {destination}")
    stage = Path(tempfile.mkdtemp(prefix=".graph-client-mcp-evidence-", dir=parent))
    try:
        for name, body in logs.items():
            (stage / name).write_bytes(body)
        (stage / "evidence.json").write_bytes(canonical(evidence))
        stage.replace(destination)
    except BaseException:
        shutil.rmtree(stage, ignore_errors=True)
        raise


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tsc", required=True, type=Path, help="absolute TypeScript 5.8.3 compiler path")
    parser.add_argument("--node", required=True, type=Path, help="absolute Node.js >=22 executable path")
    parser.add_argument(
        "--output",
        type=Path,
        help="exact new bundle directory (default: .semaprax/evidence/graph-operational-client-mcp/<commit>/<bundle-id>)",
    )
    args = parser.parse_args(argv)

    top = Path(git("rev-parse", "--show-toplevel")).resolve()
    if top != ROOT.resolve():
        raise EvidenceError(f"script repository root disagrees with Git: {top}")
    commit = git("rev-parse", "--verify", "HEAD^{commit}")
    tree = git("rev-parse", "--verify", "HEAD^{tree}")
    if HEX_OBJECT.fullmatch(commit) is None or HEX_OBJECT.fullmatch(tree) is None:
        raise EvidenceError("Git subject is not an exact lowercase commit and tree")
    if tree_state():
        raise EvidenceError("repository worktree is not clean before execution")
    input_bodies, input_rows = repository_inputs()

    cargo_selected, cargo_resolved, cargo_body = discovered_tool("cargo")
    rustc_selected, rustc_resolved, rustc_body = discovered_tool("rustc")
    git_selected, git_resolved, git_body = discovered_tool("git")
    python_selected, python_resolved, python_body = selected_tool(sys.executable, "python")
    node_selected, node_resolved, node_body = selected_tool(args.node, "node")
    tsc_selected, tsc_resolved, tsc_body = selected_tool(args.tsc, "tsc")

    cargo_version = require_command((str(cargo_selected), "--version", "--verbose"), "Cargo version")
    rustc_version = require_command((str(rustc_selected), "--version", "--verbose"), "Rust version")
    git_version = require_command((str(git_selected), "--version"), "Git version")
    python_version = require_command((str(python_selected), "--version"), "Python version")
    node_version = require_command((str(node_selected), "--version"), "Node version")
    tsc_version = require_command((str(tsc_selected), "--version"), "TypeScript version")
    if tsc_version != "Version 5.8.3":
        raise EvidenceError(f"TypeScript compiler is not exactly 5.8.3: {tsc_version!r}")
    match = re.fullmatch(r"v(\d+)\.\d+\.\d+(?:[-+].*)?", node_version)
    if match is None or int(match.group(1)) < 22:
        raise EvidenceError(f"Node.js must report a semantic version with major >=22: {node_version!r}")

    tools = {
        "cargo": tool_row(cargo_selected, cargo_resolved, cargo_body, cargo_version),
        "rustc": tool_row(rustc_selected, rustc_resolved, rustc_body, rustc_version),
        "git": tool_row(git_selected, git_resolved, git_body, git_version),
        "python": tool_row(python_selected, python_resolved, python_body, python_version),
        "node": tool_row(node_selected, node_resolved, node_body, node_version),
        "tsc": tool_row(tsc_selected, tsc_resolved, tsc_body, tsc_version),
    }
    tool_bindings = {
        "cargo": (cargo_selected, cargo_resolved, cargo_body),
        "rustc": (rustc_selected, rustc_resolved, rustc_body),
        "git": (git_selected, git_resolved, git_body),
        "python": (python_selected, python_resolved, python_body),
        "node": (node_selected, node_resolved, node_body),
        "tsc": (tsc_selected, tsc_resolved, tsc_body),
    }
    require_tool_identity(tool_bindings)
    require_repository_identity(commit, tree, input_bodies)

    cargo = str(cargo_selected)
    client_command = (
        cargo, "test", "--locked", "--offline", "-p", "semaprax",
        "--test", "image_typed_request_clients_v5",
        "--test", "image_typed_response_clients_v5",
        "--test", "image_recursive_repair_response_clients_v5",
        "--", "--test-threads=1", "--nocapture",
    )
    typescript_command = (
        cargo, "test", "--locked", "--offline", "-p", "semaprax",
        "--test", "image_recursive_repair_response_clients_v5", TYPESCRIPT_TEST,
        "--", "--exact", "--ignored", "--nocapture",
    )
    adapter_command = (
        cargo, "test", "--locked", "--offline", "-p", "semaprax",
        "--test", "image_mcp_transport_v1", "--", "--test-threads=1", "--nocapture",
    )
    cli_command = (
        cargo, "test", "--locked", "--offline", "-p", "semaprax",
        "--test", "workspace_mcp_cli_v1", "--", "--test-threads=1", "--nocapture",
    )
    environment = os.environ.copy()
    environment.update(
        {
            "CARGO_INCREMENTAL": "0",
            "CARGO_NET_OFFLINE": "true",
            "CARGO_TERM_COLOR": "never",
            "RUSTC": str(rustc_selected),
            "SEMAPRAX_TEST_CARGO": str(cargo_selected),
            "SEMAPRAX_TEST_PYTHON": str(python_selected),
            "SEMAPRAX_TEST_NODE": str(node_selected),
            "SEMAPRAX_TEST_TSC": str(tsc_selected),
        }
    )

    logs = {}
    logs["clients-cargo.log"] = execute(
        client_command, environment, CLIENT_TESTS,
        ((3, 0, 0, 0, 0), (3, 0, 0, 0, 0), (3, 0, 1, 0, 0)),
        "ordinary generated-client suites",
        (TYPESCRIPT_TEST,),
    )
    require_tool_identity(tool_bindings)
    require_repository_identity(commit, tree, input_bodies)
    logs["typescript-cargo.log"] = execute(
        typescript_command, environment, (TYPESCRIPT_TEST,), ((1, 0, 0, 0, 3),),
        "provisioned TypeScript suite",
    )
    require_tool_identity(tool_bindings)
    require_repository_identity(commit, tree, input_bodies)
    logs["mcp-adapter-cargo.log"] = execute(
        adapter_command, environment, MCP_ADAPTER_TESTS, ((8, 0, 0, 0, 0),),
        "MCP adapter suite",
    )
    require_tool_identity(tool_bindings)
    require_repository_identity(commit, tree, input_bodies)
    logs["mcp-cli-cargo.log"] = execute(
        cli_command, environment, MCP_CLI_TESTS, ((5, 0, 0, 0, 0),),
        "MCP CLI stdio suite",
    )
    require_tool_identity(tool_bindings)
    require_repository_identity(commit, tree, input_bodies)

    artifact_rows = [
        {"path": name, "bytes": len(body), "sha256": sha256(body)}
        for name, body in logs.items()
    ]
    bundle_seed = (
        b"semaprax.graph-operational-client-mcp-execution-evidence.bundle.v1\0"
        + b"\0".join(row["sha256"].encode("ascii") for row in artifact_rows)
    )
    bundle_id = sha256(bundle_seed).split(":", 1)[1]

    def gate(gate_id, selection, command, tests, counts, log, ignored_tests=()):
        return {
            "id": gate_id,
            "selection": selection,
            "prerequisite": "clean_exact_local_subject",
            "provisioning": "explicit_local_tools" if selection == "explicit_ignored" else "recorded_local_tools",
            "command": list(command),
            "outcome": "passed",
            "exit_code": 0,
            "counts": counts,
            "tests": (
                [{"name": name, "outcome": "passed"} for name in tests]
                + [{"name": name, "outcome": "ignored"} for name in ignored_tests]
            ),
            "log": log,
        }

    evidence = {
        "schema": SCHEMA,
        "bundle_id": bundle_id,
        "repository": {
            "commit": commit,
            "tree": tree,
            "inputs": input_rows,
            "head_relation_at_capture": "HEAD",
            "clean_before": True,
            "clean_after": True,
            "head_unchanged": True,
        },
        "runner": {
            "scope": "local_exact_commit",
            "network": "not_requested_cargo_offline_not_os_sandboxed",
            "cargo_incremental": False,
            "host": {
                "system": platform.system(),
                "release": platform.release(),
                "machine": platform.machine(),
            },
            "tools": tools,
        },
        "executions": [
            gate("generated_clients_ordinary_v1", "default", client_command, CLIENT_TESTS,
                 {"selected": 10, "passed": 9, "failed": 0, "ignored": 1, "measured": 0, "filtered_out": 0}, "clients-cargo.log",
                 (TYPESCRIPT_TEST,)),
            gate("generated_client_typescript_provisioned_v1", "explicit_ignored", typescript_command, (TYPESCRIPT_TEST,),
                 {"selected": 1, "passed": 1, "failed": 0, "ignored": 0, "measured": 0, "filtered_out": 3}, "typescript-cargo.log"),
            gate("workspace_mcp_adapter_v1", "default", adapter_command, MCP_ADAPTER_TESTS,
                 {"selected": 8, "passed": 8, "failed": 0, "ignored": 0, "measured": 0, "filtered_out": 0}, "mcp-adapter-cargo.log"),
            gate("workspace_mcp_cli_stdio_v1", "default", cli_command, MCP_CLI_TESTS,
                 {"selected": 5, "passed": 5, "failed": 0, "ignored": 0, "measured": 0, "filtered_out": 0}, "mcp-cli-cargo.log"),
        ],
        "observations": {
            "generated_client_sources_typescript_python_rust": "passed",
            "generated_client_python_runtime": "passed",
            "generated_client_rust_compile_runtime": "passed",
            "generated_client_typescript_compile_runtime": "passed_provisioned_local",
            "mcp_adapter_in_process": "passed",
            "mcp_cli_stdio_local_subprocess": "passed",
            "independent_mcp_client_conformance": "not_selected",
            "mcp_http_transport": "not_selected",
            "mcp_editor_host": "not_selected",
            "native_target_runtime": "not_selected",
            "wasm_target_runtime": "not_selected",
            "hosted_cross_platform": "not_observed",
            "full_quality_profile": "not_selected",
            "programme_completion": "not_selected",
        },
        "artifacts": artifact_rows,
        "claims": {
            "selected_generated_client_sources": "executed",
            "python_generated_client_runtime": "executed",
            "rust_generated_client_runtime": "executed",
            "typescript_generated_client_runtime": "executed_provisioned_local",
            "mcp_adapter": "executed_in_process",
            "mcp_cli_stdio": "executed_local_subprocess",
            "independent_mcp_client_conformance": "not_claimed",
            "real_git_or_durability": "not_claimed",
            "network_isolation": "not_claimed",
            "native_target_execution": "not_claimed",
            "wasm_target_execution": "not_claimed",
            "hosted_or_cross_platform": "not_claimed",
            "full_quality_profile": "not_claimed",
            "programme_completion": "not_claimed",
        },
    }

    if args.output is None:
        destination = ROOT / ".semaprax" / "evidence" / "graph-operational-client-mcp" / commit / bundle_id
    else:
        destination = args.output.expanduser()
        if not destination.is_absolute():
            destination = (Path.cwd() / destination).resolve()
    write_bundle(destination, evidence, logs)
    try:
        require_tool_identity(tool_bindings)
        require_repository_identity(commit, tree, input_bodies)
    except EvidenceError:
        shutil.rmtree(destination, ignore_errors=True)
        raise
    print(destination)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except EvidenceError as error:
        print(f"graph-operational client/MCP evidence rejected: {error}", file=sys.stderr)
        sys.exit(1)
    except OSError as error:
        print(f"graph-operational client/MCP evidence I/O failure: {error}", file=sys.stderr)
        sys.exit(1)
