#!/usr/bin/env python3
"""Run an arm against ShiftSim's frozen shared acceptance corpus."""

from __future__ import annotations

import argparse
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


def run(command: list[str]) -> int:
    corpus = json.loads(CORPUS.read_text(encoding="utf-8"))
    failures = []
    for case in corpus["valid"]:
        completed = subprocess.run(
            command,
            input=request_text(case),
            text=True,
            capture_output=True,
            check=False,
            timeout=10,
        )
        expected = json.dumps(case["expected"], ensure_ascii=False, separators=(",", ":")) + "\n"
        if completed.returncode != 0 or completed.stdout != expected:
            failures.append(
                f"{case['name']}: exit={completed.returncode}, "
                f"stdout={completed.stdout[:300]!r}, stderr={completed.stderr[:300]!r}"
            )
    for case in corpus["invalid"]:
        completed = subprocess.run(
            command,
            input=json.dumps(case["input"], separators=(",", ":")) + "\n",
            text=True,
            capture_output=True,
            check=False,
            timeout=10,
        )
        if completed.returncode != 2 or completed.stdout or not completed.stderr.strip():
            failures.append(
                f"{case['name']}: expected status 2, empty stdout, and a diagnostic; "
                f"got exit={completed.returncode}, stdout={completed.stdout[:200]!r}, "
                f"stderr={completed.stderr[:200]!r}"
            )
    if failures:
        print("ShiftSim acceptance failed:", file=sys.stderr)
        print("\n".join(f"- {failure}" for failure in failures), file=sys.stderr)
        return 1
    print(f"ShiftSim acceptance passed: {len(corpus['valid'])} valid, {len(corpus['invalid'])} invalid")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--command-json", required=True, help="JSON array containing executable and arguments")
    args = parser.parse_args()
    try:
        command = json.loads(args.command_json)
        if not isinstance(command, list) or not command or not all(isinstance(arg, str) for arg in command):
            raise ValueError("must be a non-empty JSON array of strings")
        return run(command)
    except (json.JSONDecodeError, ValueError, OSError, subprocess.TimeoutExpired) as error:
        print(f"acceptance runner error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
