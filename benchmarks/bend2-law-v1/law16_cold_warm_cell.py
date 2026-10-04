#!/usr/bin/env python3
"""Capture bounded fresh-path and repeat-path LAW-16 process evidence.

The two command arrays are deliberately operator supplied: this tool does not
provision a compiler, solver, or cache. A fresh process and a fresh artifact
path cannot clear macOS page, executable, solver, or tool caches, so this is
not an OS-cache-cold benchmark.
"""
import argparse
import hashlib
import json
import os
import pathlib
import statistics
import subprocess
import time

SCHEMA = "semaprax.bend2-law-benchmark.cold-warm-process-cell.v2"
MIN_SAMPLES = 30


def sha256(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def file_reference(path, root=None):
    data = path.read_bytes()
    reference = {"bytes": len(data), "sha256": sha256(data)}
    reference["path"] = str(path.relative_to(root)) if root else str(path.resolve())
    return reference


def parse_command(value, label):
    try:
        command = json.loads(value)
    except json.JSONDecodeError as error:
        raise ValueError(f"{label} must be a JSON command array: {error}") from error
    if not isinstance(command, list) or not command or any(not isinstance(part, str) or not part for part in command):
        raise ValueError(f"{label} must be a nonempty JSON array of nonempty strings")
    return command


def invoke(argv, environment, raw_root, cell, ordinal, timeout_seconds):
    started = time.monotonic_ns()
    try:
        process = subprocess.run(argv, capture_output=True, env=environment, timeout=timeout_seconds)
        outcome, exit_code = "completed", process.returncode
        stdout, stderr = process.stdout, process.stderr
    except subprocess.TimeoutExpired as expired:
        outcome, exit_code = "nonresult_timeout", None
        stdout, stderr = expired.stdout or b"", expired.stderr or b""
    prefix = f"{cell}-{ordinal:03d}"
    stdout_path = raw_root / f"{prefix}.stdout"
    stderr_path = raw_root / f"{prefix}.stderr"
    stdout_path.write_bytes(stdout)
    stderr_path.write_bytes(stderr)
    return {
        "argv": argv,
        "command_sha256": sha256(json.dumps(argv, separators=(",", ":")).encode()),
        "elapsed_ns": time.monotonic_ns() - started,
        "outcome": outcome,
        "timeout_seconds": timeout_seconds,
        "exit_code": exit_code,
        "stdout": file_reference(stdout_path, raw_root),
        "stderr": file_reference(stderr_path, raw_root),
    }


def summarize(samples):
    elapsed = [sample["elapsed_ns"] for sample in samples]
    if any(sample["outcome"] != "completed" or sample["exit_code"] != 0 for sample in samples):
        return {"status": "unavailable", "samples": samples}
    ordered = sorted(elapsed)
    return {
        "status": "completed",
        "samples": samples,
        "summary": {
            "count": len(samples),
            "p50_ns": statistics.median(elapsed),
            "p95_ns": ordered[(95 * len(ordered) + 99) // 100 - 1],
        },
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fresh-command", "--cold-command", dest="fresh_command", required=True)
    parser.add_argument("--repeat-command", "--warm-command", dest="repeat_command", required=True)
    parser.add_argument("--fresh-input", required=True, type=pathlib.Path)
    parser.add_argument("--repeat-input", required=True, type=pathlib.Path)
    parser.add_argument("--artifact", action="append", type=pathlib.Path, default=[])
    parser.add_argument("--samples", type=int, default=MIN_SAMPLES)
    parser.add_argument("--timeout-seconds", required=True, type=int)
    parser.add_argument("--raw-artifact-dir", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.samples < MIN_SAMPLES or args.output.exists():
        parser.error("--samples must be at least 30 and --output must be new")
    if args.timeout_seconds < 1:
        parser.error("--timeout-seconds must be positive")
    if args.raw_artifact_dir.exists():
        parser.error("--raw-artifact-dir must be new")
    if not args.fresh_input.is_file() or not args.repeat_input.is_file():
        parser.error("--fresh-input and --repeat-input must be regular files")
    if args.fresh_input.read_bytes() != args.repeat_input.read_bytes():
        parser.error("fresh and repeat inputs must have identical bytes")
    if any(not artifact.is_file() for artifact in args.artifact):
        parser.error("every --artifact must be a regular file")
    try:
        fresh_command = parse_command(args.fresh_command, "--fresh-command")
        repeat_command = parse_command(args.repeat_command, "--repeat-command")
    except ValueError as error:
        parser.error(str(error))
    args.raw_artifact_dir.mkdir(parents=True)
    environment = dict(os.environ, BEND_NO_TELEMETRY="1")
    document = {
        "schema": SCHEMA,
        "configuration": {
            "samples": args.samples,
            "timeout_seconds": args.timeout_seconds,
            "BEND_NO_TELEMETRY": "1",
            "fresh_process_meaning": "new child process using the supplied fresh artifact-path command",
            "repeat_process_meaning": "new child process using the supplied repeat artifact-path command",
        },
        "artifacts": [
            file_reference(args.fresh_input) | {"role": "fresh_input"},
            file_reference(args.repeat_input) | {"role": "repeat_input"},
            *[file_reference(path) | {"role": "operator_input"} for path in args.artifact],
        ],
        "raw_artifact_dir": str(args.raw_artifact_dir.resolve()),
        "cells": {
            "fresh_process": summarize([
                invoke(fresh_command, environment, args.raw_artifact_dir, "fresh-process", ordinal, args.timeout_seconds)
                for ordinal in range(1, args.samples + 1)
            ]),
            "repeat_process": summarize([
                invoke(repeat_command, environment, args.raw_artifact_dir, "repeat-process", ordinal, args.timeout_seconds)
                for ordinal in range(1, args.samples + 1)
            ]),
        },
        "cold_state": {
            "status": "unavailable",
            "reason": "process separation and fresh paths cannot isolate macOS page, executable, solver, or tool caches",
        },
        "nonclaims": [
            "fresh_process is not an OS-cache-cold observation",
            "no cold-versus-warm ratio, winner, or cache-isolation claim",
        ],
    }
    args.output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    return 0 if all(cell["status"] == "completed" for cell in document["cells"].values()) else 1


if __name__ == "__main__":
    raise SystemExit(main())
