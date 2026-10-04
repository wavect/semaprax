#!/usr/bin/env python3
"""Run the exact Boolean witness and law-gaming mutants as separate local paths."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import subprocess
from datetime import datetime, timezone


SCHEMA = "semaprax.bend2-law-benchmark.boolean-negative-controls.v1"
PINNED_BEND_COMMIT = "947db722640c86247849343657bf2f7ef01cb7f1"


def digest(path: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def git_head(root: pathlib.Path) -> str:
    return subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True, stderr=subprocess.DEVNULL).strip()


def store_artifact(directory: pathlib.Path, name: str, content: str) -> dict:
    path = directory / name
    path.write_text(content)
    return {"path": name, "sha256": digest(path)}


def invoke(directory: pathlib.Path, label: str, command: list[str], environment: dict[str, str]) -> dict:
    try:
        result = subprocess.run(command, capture_output=True, text=True, env=environment)
    except FileNotFoundError:
        return {"status": "unavailable", "reason": "tool not found"}
    return {
        "status": "accepted" if result.returncode == 0 else "rejected",
        "exit_code": result.returncode,
        "stdout": store_artifact(directory, f"{label}.stdout", result.stdout),
        "stderr": store_artifact(directory, f"{label}.stderr", result.stderr),
    }


def expected_accept(row: dict, stdout: str) -> dict:
    if row["status"] != "accepted" or stdout != "0\n":
        row["status"] = "failed"
        row["reason"] = "Boolean success witness was not accepted with exact result 0"
    return row


def expected_reject(row: dict, stderr: str, marker: str) -> dict:
    if row["status"] != "rejected" or marker not in stderr:
        row["status"] = "failed"
        row["reason"] = "law-gaming mutant was not rejected by its expected checked path"
    return row


def main(argv: list[str] | None = None) -> int:
    root = pathlib.Path(__file__).parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bend-root", required=True, type=pathlib.Path)
    parser.add_argument("--bun", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax-root", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax-commit", required=True)
    parser.add_argument("--artifact-dir", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    document = {
        "schema": SCHEMA,
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "environment": {"BEND_NO_TELEMETRY": "1"},
        "inputs": {},
        "identities": {"bend": {"expected": PINNED_BEND_COMMIT}, "semaprax": {"expected": args.semaprax_commit}},
        "paths": {},
        "nonclaims": [
            "this is a Boolean negative-control receipt, not a checked-u32 benchmark cell",
            "ordinary Bend checking and Bend verdict are retained as distinct paths",
            "no timing, agent, comparison, or superiority result",
        ],
    }
    try:
        document["identities"]["bend"]["observed"] = git_head(args.bend_root)
        document["identities"]["semaprax"]["observed"] = git_head(args.semaprax_root)
    except (OSError, subprocess.CalledProcessError):
        document.update({"status": "unavailable", "reason": "a subject root is not an accessible Git checkout"})
    else:
        if document["identities"]["bend"]["observed"] != PINNED_BEND_COMMIT or document["identities"]["semaprax"]["observed"] != args.semaprax_commit:
            document.update({"status": "unavailable", "reason": "a subject root differs from its pinned commit"})
        elif not args.bun.is_file() or not args.semaprax.is_file():
            document.update({"status": "unavailable", "reason": "a pinned executable is unavailable"})
        else:
            fixtures = {
                "bend_success": root / "fixtures/bend-two-value-boolean-v1.bend",
                "bend_attack": root / "fixtures/bend-two-value-boolean-law-gaming-v1.bend",
                "semaprax_success": root / "fixtures/semaprax-two-value-boolean-v1.spx",
                "semaprax_attack": root / "fixtures/semaprax-two-value-boolean-law-gaming-v1.spx",
            }
            if not all(path.is_file() for path in fixtures.values()):
                document.update({"status": "unavailable", "reason": "a committed Boolean control fixture is unavailable"})
            else:
                args.artifact_dir.mkdir(parents=True, exist_ok=True)
                document["inputs"] = {name: {"path": str(path.resolve()), "sha256": digest(path)} for name, path in fixtures.items()}
                document["executables"] = {"bun_sha256": digest(args.bun), "semaprax_sha256": digest(args.semaprax)}
                environment = dict(os.environ, BEND_NO_TELEMETRY="1")
                bend_base = [str(args.bun), str(args.bend_root / "bend2/main.ts")]
                normal = invoke(args.artifact_dir, "bend_normal_attack", [*bend_base, str(fixtures["bend_attack"])], environment)
                normal_stderr = (args.artifact_dir / "bend_normal_attack.stderr").read_text() if "stderr" in normal else ""
                document["paths"]["bend_normal_attack"] = expected_reject(normal, normal_stderr, "SOME PROOFS FAIL")
                verdict = invoke(args.artifact_dir, "bend_verdict_attack", [*bend_base, str(fixtures["bend_attack"]), "--verdict"], environment)
                verdict_stderr = (args.artifact_dir / "bend_verdict_attack.stderr").read_text() if "stderr" in verdict else ""
                document["paths"]["bend_verdict_attack"] = expected_reject(verdict, verdict_stderr, "SOME PROOFS FAIL")
                success = invoke(args.artifact_dir, "semaprax_runtime_success", [str(args.semaprax), "run", str(fixtures["semaprax_success"])], environment)
                success_stdout = (args.artifact_dir / "semaprax_runtime_success.stdout").read_text() if "stdout" in success else ""
                document["paths"]["semaprax_runtime_success"] = expected_accept(success, success_stdout)
                attack = invoke(args.artifact_dir, "semaprax_runtime_attack", [str(args.semaprax), "run", str(fixtures["semaprax_attack"])], environment)
                attack_stderr = (args.artifact_dir / "semaprax_runtime_attack.stderr").read_text() if "stderr" in attack else ""
                document["paths"]["semaprax_runtime_attack"] = expected_reject(attack, attack_stderr, "language status")
                document["status"] = "completed" if all(row["status"] in {"accepted", "rejected"} for row in document["paths"].values()) else "failed"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    return 0 if document["status"] == "completed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
