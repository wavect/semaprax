#!/usr/bin/env python3
"""Run the narrow future admission gate for a matched checked-u32 route.

This is not a LAW-16 task result. It records whether one pinned SEMAPRAX
image accepts the fixed u32 success source and rejects the fixed overflow
source. A successful receipt only establishes the required numeric capability
precondition; each task still needs its own equal-spec, proof, and runtime
controls.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
import time

SCHEMA = "semaprax.bend2-law-benchmark.checked-u32-admission.v1"
ROOT = pathlib.Path(__file__).parent
SUCCESS = ROOT / "fixtures" / "semaprax-checked-u32-success-v1.spx"
OVERFLOW = ROOT / "fixtures" / "semaprax-checked-u32-overflow-v1.spx"


def sha256(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def file_reference(path, root=None):
    data = path.read_bytes()
    return {
        "path": str(path.relative_to(root)) if root else str(path.resolve()),
        "bytes": len(data),
        "sha256": sha256(data),
    }


def command(value, label):
    try:
        row = json.loads(value)
    except json.JSONDecodeError as error:
        raise ValueError(f"{label} must be a JSON command array: {error}") from error
    if not isinstance(row, list) or not row or any(not isinstance(part, str) or not part for part in row):
        raise ValueError(f"{label} must be a nonempty JSON array of nonempty strings")
    if "{source}" not in row:
        raise ValueError(f"{label} must include the exact {{source}} placeholder")
    return row


def git_head(root):
    return subprocess.check_output(
        ["git", "-C", str(root), "rev-parse", "HEAD"], text=True, stderr=subprocess.DEVNULL
    ).strip()


def invoke(template, source, raw_root, label, timeout):
    argv = [part.replace("{source}", str(source.resolve())) for part in template]
    started = time.monotonic_ns()
    try:
        process = subprocess.run(argv, capture_output=True, timeout=timeout)
        exit_code, timed_out = process.returncode, False
        stdout, stderr = process.stdout, process.stderr
    except FileNotFoundError:
        exit_code, timed_out, stdout, stderr = None, False, b"", b""
    except subprocess.TimeoutExpired as error:
        exit_code, timed_out = None, True
        stdout, stderr = error.stdout or b"", error.stderr or b""
    stdout_path, stderr_path = raw_root / f"{label}.stdout", raw_root / f"{label}.stderr"
    stdout_path.write_bytes(stdout)
    stderr_path.write_bytes(stderr)
    return {
        "argv": argv,
        "command_sha256": sha256(json.dumps(argv, separators=(",", ":")).encode()),
        "elapsed_ns": time.monotonic_ns() - started,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "stdout": file_reference(stdout_path, raw_root),
        "stderr": file_reference(stderr_path, raw_root),
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--semaprax-root", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax-commit", required=True)
    parser.add_argument("--semaprax", required=True, type=pathlib.Path)
    parser.add_argument("--success-command", required=True)
    parser.add_argument("--overflow-command", required=True)
    parser.add_argument("--raw-artifact-dir", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--timeout-seconds", type=float, default=30)
    args = parser.parse_args(argv)
    if args.output.exists() or args.raw_artifact_dir.exists():
        parser.error("--output and --raw-artifact-dir must be new")
    if not args.semaprax.is_file() or not args.semaprax_root.is_dir():
        parser.error("--semaprax must be a file and --semaprax-root must be a directory")
    try:
        success_command = command(args.success_command, "--success-command")
        overflow_command = command(args.overflow_command, "--overflow-command")
        observed_commit = git_head(args.semaprax_root)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.error(str(error))
    if observed_commit != args.semaprax_commit:
        parser.error("--semaprax-root does not have --semaprax-commit checked out")
    args.raw_artifact_dir.mkdir(parents=True)
    success = invoke(success_command, SUCCESS, args.raw_artifact_dir, "success", args.timeout_seconds)
    overflow = invoke(overflow_command, OVERFLOW, args.raw_artifact_dir, "overflow", args.timeout_seconds)
    admitted = success["exit_code"] == 0 and not success["timed_out"] and overflow["exit_code"] not in (0, None) and not overflow["timed_out"]
    document = {
        "schema": SCHEMA,
        "status": "admitted" if admitted else "not_admitted",
        "numeric_domain": "u32 checked",
        "semaprax": {
            "commit": observed_commit,
            "executable": file_reference(args.semaprax),
        },
        "sources": {
            "success": file_reference(SUCCESS),
            "overflow": file_reference(OVERFLOW),
        },
        "checks": {"success": success, "overflow": overflow},
        "raw_artifact_dir": str(args.raw_artifact_dir.resolve()),
        "nonclaims": [
            "not a task proof or runtime benchmark",
            "not an equal-spec control result",
            "not evidence of SMT or Lean checked-u32 support",
            "not evidence of all-u32 overflow behavior beyond the fixed probe",
        ],
    }
    args.output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    return 0 if admitted else 1


if __name__ == "__main__":
    raise SystemExit(main())
