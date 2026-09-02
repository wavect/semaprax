#!/usr/bin/env python3
"""Run and record the exact local graph-operational workflow evidence."""

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
SCHEMA = "semaprax.graph-operational-execution-evidence.v1"
COMMAND = (
    "cargo",
    "test",
    "--locked",
    "--offline",
    "-p",
    "semaprax",
    "--test",
    "project_graph_operational_git_workflow_v1",
    "--",
    "--test-threads=1",
    "--nocapture",
)
TESTS = (
    "competing_real_git_ref_consumes_approval_without_overwriting_the_other_commit",
    "twelve_step_v5_review_to_real_sha1_git_commit",
    "twelve_step_v5_review_to_real_sha256_git_commit",
)
REPORTS = {
    "sha1": "agent-task-economics-sha1.json",
    "sha256": "agent-task-economics-sha256.json",
}
MANAGED_TEST = "signature_evolution_merge_reports_tests_and_separate_managed_publication"
MANAGED_REASON = "SPX-G150 wrong ACTIVE schema, needs workspace init fix"
MAX_LOG_BYTES = 16 * 1024 * 1024
MAX_REPORT_BYTES = 256 * 1024
MAX_COMMAND_SECONDS = 20 * 60
HEX_COMMIT = re.compile(r"[0-9a-f]{40}|[0-9a-f]{64}")
RESULT = re.compile(r"^test ([A-Za-z0-9_]+) \.\.\. (ok|FAILED|ignored)$", re.MULTILINE)
SUMMARY = re.compile(
    r"^test result: ok\. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [^\r\n]+$",
    re.MULTILINE,
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
        raise EvidenceError(f"command exceeded its {timeout}-second bound: {arguments[0]}") from error


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
    return require_command(("git",) + arguments, "Git state")


def tree_state():
    completed = run(("git", "status", "--porcelain=v1", "--untracked-files=all"))
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


def regular_bytes(path, maximum, label):
    try:
        status = path.lstat()
    except FileNotFoundError as error:
        raise EvidenceError(f"missing {label}: {path.name}") from error
    if path.is_symlink() or not path.is_file():
        raise EvidenceError(f"{label} is not a regular file: {path.name}")
    if status.st_nlink != 1:
        raise EvidenceError(f"{label} has more than one hard link: {path.name}")
    if status.st_size > maximum:
        raise EvidenceError(f"{label} exceeds {maximum} bytes: {path.name}")
    return path.read_bytes()


def report(path, object_format):
    body = regular_bytes(path, MAX_REPORT_BYTES, "task-economics report")
    if body.endswith(b"\n") or body.endswith(b"\r"):
        raise EvidenceError(f"task-economics report has a terminal newline: {path.name}")
    try:
        value = json.loads(body)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"task-economics report is not JSON: {path.name}") from error
    if canonical(value) != body:
        raise EvidenceError(f"task-economics report is not canonical JSON: {path.name}")
    if value.get("schema") != "semaprax.agent-task-economics.v1":
        raise EvidenceError(f"task-economics report has the wrong schema: {path.name}")
    if value.get("git_object_format") != object_format:
        raise EvidenceError(f"task-economics report has the wrong Git format: {path.name}")
    criteria = value.get("criteria")
    if (
        not isinstance(criteria, list)
        or len(criteria) != 12
        or any(row.get("passed") is not True for row in criteria if isinstance(row, dict))
        or any(not isinstance(row, dict) for row in criteria)
    ):
        raise EvidenceError(f"task-economics report lacks twelve passing criteria: {path.name}")
    return body, value


def parse_test_log(body):
    if len(body) > MAX_LOG_BYTES:
        raise EvidenceError(f"Cargo log exceeds {MAX_LOG_BYTES} bytes")
    try:
        text = body.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise EvidenceError("Cargo log is not UTF-8") from error
    rows = RESULT.findall(text)
    expected = [(name, "ok") for name in TESTS]
    if sorted(rows) != expected:
        raise EvidenceError(f"focused Cargo run selected unexpected tests: {sorted(rows)!r}")
    if len(SUMMARY.findall(text)) != 1:
        raise EvidenceError("focused Cargo run lacks its exact 3/0/0 test summary")
    return text


def tool(name, *arguments):
    executable = shutil.which(name)
    if executable is None:
        raise EvidenceError(f"required local tool is unavailable: {name}")
    version = require_command((executable,) + arguments, f"{name} version")
    return {"executable": os.path.realpath(executable), "version": version}


def inventory(directory):
    names = []
    for entry in directory.iterdir():
        if entry.is_symlink() or not entry.is_file():
            raise EvidenceError(f"unexpected exported evidence entry: {entry.name}")
        names.append(entry.name)
    if sorted(names) != sorted(REPORTS.values()):
        raise EvidenceError(f"unexpected exported evidence inventory: {sorted(names)!r}")


def write_bundle(destination, evidence, log, reports):
    parent = destination.parent
    parent.mkdir(parents=True, exist_ok=True)
    if destination.exists() or destination.is_symlink():
        raise EvidenceError(f"evidence destination already exists: {destination}")
    stage = Path(tempfile.mkdtemp(prefix=".graph-operational-evidence-", dir=parent))
    try:
        (stage / "cargo.log").write_bytes(log)
        for name, body in reports.items():
            (stage / name).write_bytes(body)
        (stage / "evidence.json").write_bytes(canonical(evidence))
        stage.replace(destination)
    except BaseException:
        shutil.rmtree(stage, ignore_errors=True)
        raise


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        help="exact new evidence bundle directory (default: .semaprax/evidence/graph-operational/<commit>/<bundle-id>)",
    )
    args = parser.parse_args(argv)

    top = Path(git("rev-parse", "--show-toplevel")).resolve()
    if top != ROOT.resolve():
        raise EvidenceError(f"script repository root disagrees with Git: {top}")
    commit_before = git("rev-parse", "--verify", "HEAD^{commit}")
    if HEX_COMMIT.fullmatch(commit_before) is None:
        raise EvidenceError("Git HEAD is not one exact lowercase commit ID")
    if tree_state():
        raise EvidenceError("repository worktree is not clean before execution")

    tools = {
        "cargo": tool("cargo", "--version", "--verbose"),
        "rustc": tool("rustc", "--version", "--verbose"),
        "git": tool("git", "--version"),
        "python": {
            "executable": os.path.realpath(sys.executable),
            "version": platform.python_version(),
        },
    }
    with tempfile.TemporaryDirectory(prefix="semaprax-graph-workflow-") as exported:
        exported_path = Path(exported)
        environment = os.environ.copy()
        environment["CARGO_NET_OFFLINE"] = "true"
        environment["CARGO_TERM_COLOR"] = "never"
        environment["RUSTC"] = tools["rustc"]["executable"]
        environment["SEMAPRAX_TEST_GIT"] = tools["git"]["executable"]
        environment["SEMAPRAX_GRAPH_WORKFLOW_EVIDENCE_DIR"] = str(exported_path)
        completed = run(
            COMMAND, env=environment, combine=True, timeout=MAX_COMMAND_SECONDS
        )
        log = completed.stdout
        if completed.returncode != 0:
            raise EvidenceError(f"focused Cargo command failed with exit {completed.returncode}")
        parse_test_log(log)
        inventory(exported_path)
        report_bodies = {}
        for object_format, name in REPORTS.items():
            body, _value = report(exported_path / name, object_format)
            report_bodies[name] = body

    commit_after = git("rev-parse", "--verify", "HEAD^{commit}")
    if commit_after != commit_before:
        raise EvidenceError("Git HEAD changed during evidence execution")
    if tree_state():
        raise EvidenceError("repository worktree is not clean after execution")

    artifact_rows = [
        {"path": "cargo.log", "bytes": len(log), "sha256": sha256(log)},
    ]
    for object_format, name in REPORTS.items():
        body = report_bodies[name]
        artifact_rows.append(
            {
                "path": name,
                "kind": "agent_task_economics",
                "git_object_format": object_format,
                "bytes": len(body),
                "sha256": sha256(body),
            }
        )
    bundle_seed = b"semaprax.graph-operational-execution-evidence.bundle.v1\0" + b"\0".join(
        row["sha256"].encode("ascii") for row in artifact_rows
    )
    bundle_id = sha256(bundle_seed).split(":", 1)[1]
    evidence = {
        "schema": SCHEMA,
        "repository": {
            "commit": commit_before,
            "head_relation_at_capture": "HEAD",
            "clean_before": True,
            "clean_after": True,
            "head_unchanged": True,
        },
        "runner": {
            "scope": "local_exact_commit",
            "network": "not_requested_cargo_offline_local_git_fixture",
            "host": {
                "system": platform.system(),
                "release": platform.release(),
                "machine": platform.machine(),
            },
            "tools": tools,
        },
        "gates": [
            {
                "id": "graph_operational_git_workflow_v1",
                "selection": "default",
                "prerequisite": "local_unix_git",
                "provisioning": "not_required",
                "command": list(COMMAND),
                "outcome": "passed",
                "exit_code": completed.returncode,
                "counts": {
                    "selected": 3,
                    "passed": 3,
                    "failed": 0,
                    "ignored": 0,
                    "measured": 0,
                    "filtered_out": 0,
                },
                "tests": [{"name": name, "outcome": "passed"} for name in TESTS],
            },
            {
                "id": "graph_operational_managed_workflow_v1",
                "selection": "explicit_ignored_required",
                "prerequisite": "known_fixture_correction",
                "provisioning": "not_required",
                "outcome": "not_selected",
                "test": MANAGED_TEST,
                "reason": MANAGED_REASON,
            },
        ],
        "artifacts": artifact_rows,
        "claims": {
            "bounded_twelve_step_git_workflow": "executed",
            "managed_active_workflow": "not_executed",
            "native_target_execution": "not_claimed",
            "wasm_target_execution": "not_claimed",
            "hosted_or_cross_platform": "not_claimed",
            "full_quality_profile": "not_claimed",
            "programme_completion": "not_claimed",
        },
        "bundle_id": bundle_id,
    }

    if args.output is None:
        destination = (
            ROOT
            / ".semaprax"
            / "evidence"
            / "graph-operational"
            / commit_before
            / bundle_id
        )
    else:
        destination = args.output.expanduser()
        if not destination.is_absolute():
            destination = (Path.cwd() / destination).resolve()
    write_bundle(destination, evidence, log, report_bodies)
    if git("rev-parse", "--verify", "HEAD^{commit}") != commit_before or tree_state():
        shutil.rmtree(destination, ignore_errors=True)
        raise EvidenceError("writing the evidence bundle changed repository state")
    print(destination)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except EvidenceError as error:
        print(f"graph-operational evidence rejected: {error}", file=sys.stderr)
        sys.exit(1)
    except OSError as error:
        print(f"graph-operational evidence I/O failure: {error}", file=sys.stderr)
        sys.exit(1)
