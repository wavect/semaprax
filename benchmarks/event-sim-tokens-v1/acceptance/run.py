#!/usr/bin/env python3
"""Run an arm against ShiftSim's frozen shared acceptance corpus."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "acceptance" / "corpus.json"


def request_text(case: dict[str, object]) -> str:
    """Render a request, including compactly declared leading whitespace."""
    prefix_bytes = case.get("leading_whitespace_bytes", 0)
    if type(prefix_bytes) is not int or prefix_bytes < 0:
        raise ValueError("leading_whitespace_bytes must be a nonnegative integer")
    request = json.dumps(case["input"], ensure_ascii=False, separators=(",", ":"))
    return " " * prefix_bytes + request + "\n"


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run(command: list[str], report_json: Path | None = None) -> int:
    corpus_bytes = CORPUS.read_bytes()
    corpus = json.loads(corpus_bytes.decode("utf-8"))
    failures = []
    results = []
    for case in corpus["valid"]:
        request = request_text(case)
        request_bytes = request.encode("utf-8")
        expected = json.dumps(case["expected"], ensure_ascii=False, separators=(",", ":")) + "\n"
        expected_bytes = expected.encode("utf-8")
        completed = subprocess.run(
            command,
            input=request,
            text=True,
            capture_output=True,
            check=False,
            timeout=10,
        )
        passed = completed.returncode == 0 and completed.stdout == expected
        results.append({
            "name": case["name"], "kind": "valid", "status": "passed" if passed else "failed",
            "input_bytes": len(request_bytes), "input_sha256": _sha256(request_bytes),
            "leading_whitespace_bytes": case.get("leading_whitespace_bytes", 0),
            "expected_exit_code": 0, "exit_code": completed.returncode,
            "stdout_sha256": _sha256(completed.stdout.encode("utf-8")),
            "expected_stdout_sha256": _sha256(expected_bytes),
            "stderr_nonempty": bool(completed.stderr.strip()),
        })
        if not passed:
            failures.append(
                f"{case['name']}: exit={completed.returncode}, "
                f"stdout={completed.stdout[:300]!r}, stderr={completed.stderr[:300]!r}"
            )
    for case in corpus["invalid"]:
        request = json.dumps(case["input"], separators=(",", ":")) + "\n"
        request_bytes = request.encode("utf-8")
        completed = subprocess.run(
            command,
            input=request,
            text=True,
            capture_output=True,
            check=False,
            timeout=10,
        )
        passed = completed.returncode == 2 and not completed.stdout and bool(completed.stderr.strip())
        results.append({
            "name": case["name"], "kind": "invalid", "status": "passed" if passed else "failed",
            "input_bytes": len(request_bytes), "input_sha256": _sha256(request_bytes),
            "leading_whitespace_bytes": 0,
            "expected_exit_code": 2, "exit_code": completed.returncode,
            "stdout_sha256": _sha256(completed.stdout.encode("utf-8")),
            "expected_stdout_sha256": _sha256(b""),
            "stderr_nonempty": bool(completed.stderr.strip()),
        })
        if not passed:
            failures.append(
                f"{case['name']}: expected status 2, empty stdout, and a diagnostic; "
                f"got exit={completed.returncode}, stdout={completed.stdout[:200]!r}, "
                f"stderr={completed.stderr[:200]!r}"
            )
    if report_json is not None:
        report = {
            "schema": "semaprax.event-sim.acceptance-report.v1",
            "corpus_sha256": _sha256(corpus_bytes),
            "status": "passed" if not failures else "failed",
            "valid_cases": len(corpus["valid"]), "invalid_cases": len(corpus["invalid"]),
            "cases": results,
        }
        report_json.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if failures:
        print("ShiftSim acceptance failed:", file=sys.stderr)
        print("\n".join(f"- {failure}" for failure in failures), file=sys.stderr)
        return 1
    print(f"ShiftSim acceptance passed: {len(corpus['valid'])} valid, {len(corpus['invalid'])} invalid")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--command-json", required=True, help="JSON array containing executable and arguments")
    parser.add_argument("--report-json", type=Path, help="write per-case results for qualification evidence")
    args = parser.parse_args()
    try:
        command = json.loads(args.command_json)
        if not isinstance(command, list) or not command or not all(isinstance(arg, str) for arg in command):
            raise ValueError("must be a non-empty JSON array of strings")
        return run(command, args.report_json)
    except (json.JSONDecodeError, ValueError, OSError, subprocess.TimeoutExpired) as error:
        print(f"acceptance runner error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
