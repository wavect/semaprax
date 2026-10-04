#!/usr/bin/env python3
"""Exercise the pinned Bend two-Boolean fixture without claiming a u32 match."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import time
from datetime import datetime, timezone


SCHEMA = "semaprax.bend2-law-benchmark.boolean-driver.v1"
PINNED_BEND_COMMIT = "947db722640c86247849343657bf2f7ef01cb7f1"
EXPECTED_NORMAL_LINES = ["0", "1"]
EXPECTED_VERDICT = "ALL PROOFS CHECK"


def digest(path: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def git_head(root: pathlib.Path) -> str:
    return subprocess.check_output(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        text=True,
        stderr=subprocess.DEVNULL,
    ).strip()


def invoke(command: list[str], environment: dict[str, str], timeout: float) -> dict:
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            command, capture_output=True, text=True, env=environment, timeout=timeout
        )
    except FileNotFoundError:
        return {"status": "unavailable", "reason": "Bun executable is unavailable"}
    except subprocess.TimeoutExpired:
        return {"status": "timed_out", "reason": f"timeout after {timeout} seconds"}
    return {
        "status": "accepted" if completed.returncode == 0 else "rejected",
        "exit_code": completed.returncode,
        "wall_ms": round((time.perf_counter() - started) * 1000, 3),
        "stdout": completed.stdout,
        "stderr_sha256": "sha256:"
        + hashlib.sha256(completed.stderr.encode()).hexdigest(),
    }


def classify_normal(result: dict) -> dict:
    if result["status"] != "accepted":
        return result
    if result["stdout"].splitlines() != EXPECTED_NORMAL_LINES:
        result["status"] = "failed"
        result["reason"] = "two-Boolean fixture did not emit exact False/True values"
    return result


def classify_verdict(result: dict) -> dict:
    if result["status"] != "accepted":
        return result
    if result["stdout"].strip() != EXPECTED_VERDICT:
        result["status"] = "failed"
        result["reason"] = "Bend verdict kernel did not report ALL PROOFS CHECK"
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bend-root", required=True, type=pathlib.Path)
    parser.add_argument("--bun", required=True, type=pathlib.Path)
    parser.add_argument(
        "--fixture", type=pathlib.Path, default=pathlib.Path(__file__).with_name("fixtures") / "bend-two-value-boolean-v1.bend"
    )
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--timeout-seconds", type=float, default=120)
    args = parser.parse_args(argv)

    document = {
        "schema": SCHEMA,
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "fixture_sha256": None,
        "bend": {"expected_commit": PINNED_BEND_COMMIT, "observed_commit": None},
        "environment": {"BEND_NO_TELEMETRY": "1"},
        "nonclaims": [
            "no checked-u32 benchmark cell",
            "no Bend versus SEMAPRAX comparison",
            "no timing or superiority claim",
        ],
        "paths": {},
    }
    try:
        observed = git_head(args.bend_root)
    except (OSError, subprocess.CalledProcessError):
        document["status"] = "unavailable"
        document["reason"] = "Bend root is not an accessible Git checkout"
    else:
        document["bend"]["observed_commit"] = observed
        if observed != PINNED_BEND_COMMIT:
            document["status"] = "unavailable"
            document["reason"] = "Bend root differs from the pinned benchmark revision"
        elif not args.fixture.is_file() or not (args.bend_root / "bend2/main.ts").is_file():
            document["status"] = "unavailable"
            document["reason"] = "fixture or pinned Bend entrypoint is unavailable"
        else:
            document["fixture_sha256"] = digest(args.fixture)
            environment = dict(os.environ, BEND_NO_TELEMETRY="1")
            base = [str(args.bun), str(args.bend_root / "bend2/main.ts"), str(args.fixture)]
            document["paths"] = {
                "bend_normal": classify_normal(invoke(base, environment, args.timeout_seconds)),
                "bend_verdict": classify_verdict(
                    invoke([*base, "--verdict"], environment, args.timeout_seconds)
                ),
            }
            statuses = [row["status"] for row in document["paths"].values()]
            document["status"] = (
                "completed" if statuses == ["accepted", "accepted"] else "failed"
            )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, sort_keys=True, indent=2) + "\n")
    return 0 if document["status"] == "completed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
