#!/usr/bin/env python3
"""Capture a bounded local project-incremental cache cell for LAW-16.

The cell executes two exact compiler unit tests from a caller-provided test
binary.  It binds the three-module calculator project, a body-only provider
edit that rechecks ``src/core.spx`` while reusing its two unchanged consumers,
and a signature-changing negative control that the warm and cold admission
routes both refuse.  The supplied binary's source association remains local:
this script does not attest how it was built.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import subprocess


ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
BENCHMARK = ROOT / "benchmarks/bend2-law-v1"
PROJECT = ROOT / "examples/calculator-project"
IMPLEMENTATION = ROOT / "src/project/incremental/clone_cost_report.rs"
SCHEMA = "semaprax.bend2-law-benchmark.project-incremental-cell.v1"
MAX_RAW_BYTES = 65_536
SUCCESS_TEST = "project::incremental::clone_cost_report::tests::provider_edit_clones_unaffected_consumers_and_reparses_only_the_provider"
ATTACK_TEST = "project::incremental::clone_cost_report::tests::provider_signature_change_still_invalidates_and_fails_the_same_way_cold_does"
PROJECT_FILES = (
    PROJECT / "semaprax.toml",
    PROJECT / "src/app.spx",
    PROJECT / "src/core.spx",
    PROJECT / "src/tests.spx",
)


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def reference(path: pathlib.Path, root: pathlib.Path = ROOT) -> dict:
    data = path.read_bytes()
    try:
        display = path.relative_to(root).as_posix()
    except ValueError:
        display = str(path.resolve())
    return {"path": display, "bytes": len(data), "sha256": digest(data)}


def command(test_binary: pathlib.Path, test: str) -> list[str]:
    return [str(test_binary), test, "--exact", "--nocapture"]


def accepted(returncode: int, output: bytes, test: str, required: tuple[str, ...] = ()) -> bool:
    text = output.decode("utf-8", "replace")
    return returncode == 0 and f"test {test} ... ok" in text and all(item in text for item in required)


def check_project_inputs() -> None:
    if not all(path.is_file() for path in (*PROJECT_FILES, IMPLEMENTATION)):
        raise ValueError("project incremental cell inputs are unavailable")
    core = (PROJECT / "src/core.spx").read_text()
    app = (PROJECT / "src/app.spx").read_text()
    tests = (PROJECT / "src/tests.spx").read_text()
    if "left + right" not in core or "from calculator.core as add" not in app or "from calculator.core as add" not in tests:
        raise ValueError("calculator provider-and-consumer fixture drifted")


def checked_head() -> str:
    completed = subprocess.run(
        ["git", "-C", str(ROOT), "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
        check=False,
        timeout=5,
    )
    head = completed.stdout.strip()
    if completed.returncode or len(head) != 40 or any(char not in "0123456789abcdef" for char in head):
        raise ValueError("repository HEAD is unavailable for project incremental source association")
    return head


def run_exact(test_binary: pathlib.Path, test: str, raw: pathlib.Path, label: str) -> dict:
    argv = command(test_binary, test)
    try:
        completed = subprocess.run(argv, cwd=ROOT, capture_output=True, check=False, timeout=30)
    except subprocess.TimeoutExpired as error:
        raise ValueError(f"{label} exact test timed out") from error
    if len(completed.stdout) > MAX_RAW_BYTES or len(completed.stderr) > MAX_RAW_BYTES:
        raise ValueError(f"{label} exact test output exceeds {MAX_RAW_BYTES} bytes")
    stdout, stderr = raw / f"{label}.stdout", raw / f"{label}.stderr"
    stdout.write_bytes(completed.stdout)
    stderr.write_bytes(completed.stderr)
    return {
        "test": test,
        "argv": argv,
        "command_sha256": digest(json.dumps(argv, separators=(",", ":")).encode()),
        "exit_code": completed.returncode,
        "stdout": reference(stdout, raw),
        "stderr": reference(stderr, raw),
    }


def capture(test_binary: pathlib.Path, output: pathlib.Path, source_commit: str) -> dict:
    if output.exists() or not output.parent.is_dir():
        raise ValueError("output must be new below an existing directory")
    if len(source_commit) != 40 or any(char not in "0123456789abcdef" for char in source_commit):
        raise ValueError("source commit must be a full lowercase hexadecimal Git revision")
    if source_commit != checked_head():
        raise ValueError("source commit does not match the checked-out repository HEAD")
    test_binary = test_binary.resolve(strict=True)
    if not test_binary.is_file():
        raise ValueError("test binary is unavailable")
    check_project_inputs()
    output.mkdir()
    raw = output / "raw"
    raw.mkdir()
    success = run_exact(test_binary, SUCCESS_TEST, raw, "provider-body-edit")
    attack = run_exact(test_binary, ATTACK_TEST, raw, "provider-signature-edit")
    success_output = (raw / "provider-body-edit.stdout").read_bytes() + (raw / "provider-body-edit.stderr").read_bytes()
    attack_output = (raw / "provider-signature-edit.stdout").read_bytes() + (raw / "provider-signature-edit.stderr").read_bytes()
    if not accepted(success["exit_code"], success_output, SUCCESS_TEST, ("provider edit:", "modules cloned", "module reparsed")):
        raise ValueError("provider body-edit test did not establish the expected reuse classification")
    if not accepted(attack["exit_code"], attack_output, ATTACK_TEST):
        raise ValueError("provider signature-edit negative control did not pass")
    result = {
        "schema": SCHEMA,
        "status": "completed_local_project_incremental_cell",
        "source_commit": source_commit,
        "test_binary": reference(test_binary),
        "implementation": reference(IMPLEMENTATION),
        "project": [reference(path) for path in PROJECT_FILES],
        "semantic_contract": {
            "project_modules": ["src/app.spx", "src/core.spx", "src/tests.spx"],
            "success": {
                "edit": "replace core body `left + right` with `right + left`",
                "reparsed": ["src/core.spx"],
                "reused": ["src/app.spx", "src/tests.spx"],
            },
            "negative_control": {
                "edit": "add a third parameter to calculator.add",
                "expected": "warm and cold admission both refuse the changed provider signature",
            },
        },
        "routes": {"provider_body_edit": success, "provider_signature_edit": attack},
        "raw_streams": 4,
        "nonclaims": [
            "test-binary source association is local rather than a reproducible-build attestation",
            "this is SEMAPRAX compiler-cache evidence only; no Bend incremental route was executed",
            "no checked-u32 source, matched cross-language execution, proof, timing, peak-memory, or winner claim",
            "the signature-edit control records a harness assertion of refusal and does not export a standalone diagnostic transcript",
            "this local cell does not close LAW-16 issue #392",
        ],
    }
    (output / "result.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    return result


def checked_reference(root: pathlib.Path, item: dict) -> pathlib.Path:
    if not isinstance(item, dict) or set(item) != {"path", "bytes", "sha256"}:
        raise ValueError("malformed retained file reference")
    relative = pathlib.PurePosixPath(item["path"])
    if relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts):
        raise ValueError("retained file reference escapes the capsule")
    path = (root / relative).resolve(strict=True)
    if root.resolve() not in path.parents or not path.is_file() or reference(path, root) != item:
        raise ValueError("retained file reference identity drifted")
    return path


def verify(capsule: pathlib.Path) -> dict:
    capsule = capsule.resolve(strict=True)
    result = json.loads((capsule / "result.json").read_text())
    if result.get("schema") != SCHEMA or result.get("status") != "completed_local_project_incremental_cell":
        raise ValueError("project incremental capsule identity or status drifted")
    if (not isinstance(result.get("source_commit"), str) or len(result["source_commit"]) != 40
            or any(char not in "0123456789abcdef" for char in result["source_commit"])):
        raise ValueError("project incremental capsule source association is absent")
    binary = result.get("test_binary")
    if (not isinstance(binary, dict) or set(binary) != {"path", "bytes", "sha256"}
            or not isinstance(binary["path"], str) or not isinstance(binary["bytes"], int)
            or binary["bytes"] <= 0 or not isinstance(binary["sha256"], str)
            or not binary["sha256"].startswith("sha256:") or len(binary["sha256"]) != 71):
        raise ValueError("project incremental test-binary provenance is malformed")
    if result.get("implementation") != reference(IMPLEMENTATION) or result.get("project") != [reference(path) for path in PROJECT_FILES]:
        raise ValueError("project incremental source inputs drifted")
    routes = result.get("routes", {})
    if set(routes) != {"provider_body_edit", "provider_signature_edit"} or result.get("raw_streams") != 4:
        raise ValueError("project incremental route inventory drifted")
    expected_tests = {"provider_body_edit": SUCCESS_TEST, "provider_signature_edit": ATTACK_TEST}
    for name, row in routes.items():
        if row.get("test") != expected_tests[name]:
            raise ValueError("project incremental exact test selection drifted")
        recorded_binary = pathlib.Path(binary["path"])
        if not recorded_binary.is_absolute():
            recorded_binary = ROOT / recorded_binary
        if row.get("argv") != command(recorded_binary, row["test"]):
            raise ValueError("project incremental executable or command selection drifted")
        stdout = checked_reference(capsule / "raw", row.get("stdout"))
        stderr = checked_reference(capsule / "raw", row.get("stderr"))
        output = stdout.read_bytes() + stderr.read_bytes()
        required = ("provider edit:", "modules cloned", "module reparsed") if row.get("test") == SUCCESS_TEST else ()
        if row.get("command_sha256") != digest(json.dumps(row.get("argv"), separators=(",", ":")).encode()) or not accepted(row.get("exit_code"), output, row.get("test"), required):
            raise ValueError("project incremental route transcript drifted")
    return {"schema": SCHEMA + ".review.v1", "status": result["status"], "project_modules": 3, "raw_streams": 4, "overall_law16": "incomplete"}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", required=True, type=pathlib.Path)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        capture(args.test_binary, args.output, args.source_commit)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
