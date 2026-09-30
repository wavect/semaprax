#!/usr/bin/env python3
"""Validate a closed, read-only Kernel-0 accepted-revision record for #328.

The gate never executes receipt commands. It only validates one canonical JSON
record and compares declared tracked source blobs at immutable Git commits.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

SCHEMA = "semaprax.kernel-zero-accepted-revision.v1"
HEX = re.compile(r"[0-9a-f]{40}")
RECEIPTS = ("lean-proof", "renderer-authority", "bootstrap-artifact", "scalar-targets-recovery", "differential-corpus", "owned-handoff", "baseline-preservation")
INVENTORY_FIELDS = ("selector", "harness", "profile_sources", "fixtures", "generated_inputs", "cargo_lock")


class Refusal(Exception):
    def __init__(self, code: str, detail: str) -> None:
        super().__init__(f"{code}: {detail}")


def fail(code: str, detail: str) -> None:
    raise Refusal(code, detail)


def exact_keys(value: dict[str, Any], keys: tuple[str, ...], where: str) -> None:
    if set(value) != set(keys):
        fail("SPX-K328-KEYS", f"{where} must contain exactly {', '.join(keys)}")


def text(value: Any, where: str) -> str:
    if not isinstance(value, str) or not value or any(ord(c) < 32 for c in value):
        fail("SPX-K328-TEXT", f"{where} must be nonempty control-free text")
    return value


def revision(value: Any, where: str) -> str:
    value = text(value, where)
    if not HEX.fullmatch(value):
        fail("SPX-K328-REVISION", f"{where} must be a 40-lower-hex commit")
    return value


def source_path(value: Any, where: str) -> str:
    value = text(value, where)
    candidate = Path(value)
    if candidate.is_absolute() or ".." in candidate.parts or value.startswith(".git/") or value == "." or value.endswith("/"):
        fail("SPX-K328-PATH", f"{where} must be a repository-relative file path")
    return value


def git(repository: Path, *arguments: str) -> str:
    completed = subprocess.run(("git", *arguments), cwd=repository, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if completed.returncode:
        fail("SPX-K328-GIT", completed.stderr.decode("utf-8", "replace").strip() or f"git {' '.join(arguments)} failed")
    return completed.stdout.decode("ascii", "strict").strip()


def commit(repository: Path, value: str, where: str) -> str:
    resolved = git(repository, "rev-parse", "--verify", f"{value}^{{commit}}")
    if resolved != value:
        fail("SPX-K328-REVISION", f"{where} did not resolve to its exact commit")
    return resolved


def tools(value: Any, where: str) -> None:
    if not isinstance(value, dict):
        fail("SPX-K328-TOOLS", f"{where} must be an object")
    for name, version in value.items():
        text(name, f"{where} tool")
        text(version, f"{where} version")


def inventory(value: Any, where: str) -> list[str]:
    if not isinstance(value, dict):
        fail("SPX-K328-INVENTORY", f"{where} must be an object")
    exact_keys(value, INVENTORY_FIELDS, where)
    paths: list[str] = []
    for field in INVENTORY_FIELDS:
        entries = value[field]
        if not isinstance(entries, list) or not entries:
            fail("SPX-K328-INVENTORY", f"{where}.{field} must be a nonempty path list")
        accepted = [source_path(item, f"{where}.{field}") for item in entries]
        if accepted != sorted(set(accepted)):
            fail("SPX-K328-INVENTORY", f"{where}.{field} paths must be sorted and unique")
        paths.extend(accepted)
    if value["cargo_lock"] != ["Cargo.lock"]:
        fail("SPX-K328-INVENTORY", f"{where}.cargo_lock must be exactly Cargo.lock")
    if len(paths) != len(set(paths)):
        fail("SPX-K328-INVENTORY", f"{where} paths must not overlap across categories")
    return paths


def tracked_blob(repository: Path, revision: str, path: str, where: str) -> None:
    if git(repository, "cat-file", "-t", f"{revision}:{path}") != "blob":
        fail("SPX-K328-SUBJECT", f"{where} is not a tracked file blob")


def validate_receipt(repository: Path, candidate: str, value: Any) -> str:
    if not isinstance(value, dict):
        fail("SPX-K328-RECEIPT", "receipt must be an object")
    identifier = text(value.get("id"), "receipt.id")
    state = text(value.get("state"), f"receipt {identifier}.state")
    if state == "pending":
        exact_keys(value, ("id", "state"), f"receipt {identifier}")
        return state
    common = ("command", "execution_revision", "id", "passed", "state", "tool_versions")
    if state == "executed":
        exact_keys(value, common, f"receipt {identifier}")
    elif state == "reconciled":
        exact_keys(value, ("command", "execution_revision", "id", "inventory", "passed", "state", "tool_versions"), f"receipt {identifier}")
    else:
        fail("SPX-K328-STATE", f"receipt {identifier} has unsupported state {state!r}")
    if value["passed"] is not True:
        fail("SPX-K328-RESULT", f"receipt {identifier} did not pass")
    execution = commit(repository, revision(value["execution_revision"], f"receipt {identifier}.execution_revision"), f"receipt {identifier}.execution_revision")
    text(value["command"], f"receipt {identifier}.command")
    tools(value["tool_versions"], f"receipt {identifier}.tool_versions")
    if state == "executed":
        if execution != candidate:
            fail("SPX-K328-BINDING", f"receipt {identifier} was not executed at candidate_revision")
        return state
    paths = inventory(value["inventory"], f"receipt {identifier}.inventory")
    for path in paths:
        tracked_blob(repository, execution, path, f"receipt {identifier} execution subject")
        tracked_blob(repository, candidate, path, f"receipt {identifier} candidate subject")
    compared = subprocess.run(("git", "diff", "--quiet", execution, candidate, "--", *paths), cwd=repository, stdin=subprocess.DEVNULL, check=False).returncode
    if compared == 1:
        fail("SPX-K328-DRIFT", f"receipt {identifier} declared subject changed")
    if compared != 0:
        fail("SPX-K328-GIT", f"receipt {identifier} subject comparison failed")
    return state


def validate(record: Any, repository: Path) -> dict[str, Any]:
    if not isinstance(record, dict):
        fail("SPX-K328-RECORD", "record must be an object")
    exact_keys(record, ("candidate_revision", "outcome", "reason", "receipts", "schema"), "record")
    if record["schema"] != SCHEMA:
        fail("SPX-K328-SCHEMA", "record schema is not accepted-revision v1")
    candidate = commit(repository, revision(record["candidate_revision"], "candidate_revision"), "candidate_revision")
    if not isinstance(record["receipts"], list) or len(record["receipts"]) != len(RECEIPTS):
        fail("SPX-K328-INVENTORY", "record must contain every required receipt once")
    identifiers = tuple(text(item.get("id"), "receipt.id") if isinstance(item, dict) else "" for item in record["receipts"])
    if identifiers != RECEIPTS:
        fail("SPX-K328-INVENTORY", "receipts must use the fixed ordered inventory")
    states = [validate_receipt(repository, candidate, item) for item in record["receipts"]]
    outcome, reason = text(record["outcome"], "outcome"), text(record["reason"], "reason")
    if outcome == "validation-incomplete" and "pending" in states:
        return {"candidate_revision": candidate, "outcome": outcome, "reason": reason, "states": states}
    if outcome == "rung-1-retained" and "pending" not in states:
        return {"candidate_revision": candidate, "outcome": outcome, "reason": reason, "states": states}
    fail("SPX-K328-OUTCOME", "outcome does not match the receipt states; rung-2 promotion needs separate review")


def read_record(path: Path) -> Any:
    try:
        raw = path.read_bytes()
        value = json.loads(raw)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("SPX-K328-RECORD", f"cannot parse record: {error}")
    canonical = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode() + b"\n"
    if raw != canonical:
        fail("SPX-K328-CANONICAL", "record must be canonical JSON with one trailing LF")
    return value


class GateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="semaprax-k328-")
        self.repo = Path(self.temp.name)
        for command in (("git", "init", "-q"), ("git", "config", "user.email", "gate@example.invalid"), ("git", "config", "user.name", "gate")):
            subprocess.run(command, cwd=self.repo, check=True)
        for path in ("Cargo.lock", "fixture.txt", "generated.txt", "harness.py", "profile.spx", "selector.rs"):
            (self.repo / path).write_text(path + "\n", encoding="utf-8")
        subprocess.run(("git", "add", "."), cwd=self.repo, check=True)
        subprocess.run(("git", "commit", "-qm", "subject"), cwd=self.repo, check=True)
        self.head = git(self.repo, "rev-parse", "HEAD")

    def tearDown(self) -> None:
        self.temp.cleanup()

    def record(self) -> dict[str, Any]:
        receipts: list[dict[str, Any]] = [{"id": identifier, "state": "pending"} for identifier in RECEIPTS]
        receipts[0] = {"command": "scripts/kernel0-lean-gate.py", "execution_revision": self.head, "id": "lean-proof", "inventory": {"cargo_lock": ["Cargo.lock"], "fixtures": ["fixture.txt"], "generated_inputs": ["generated.txt"], "harness": ["harness.py"], "profile_sources": ["profile.spx"], "selector": ["selector.rs"]}, "passed": True, "state": "reconciled", "tool_versions": {"lean": "4.34.0"}}
        return {"candidate_revision": self.head, "outcome": "validation-incomplete", "reason": "remaining receipts are pending", "receipts": receipts, "schema": SCHEMA}

    def test_accepts_exact_reconciliation_with_complete_pending_inventory(self) -> None:
        self.assertEqual(validate(self.record(), self.repo)["states"][0], "reconciled")

    def test_refuses_declared_subject_drift(self) -> None:
        record = self.record()
        (self.repo / "fixture.txt").write_text("changed\n", encoding="utf-8")
        subprocess.run(("git", "add", "fixture.txt"), cwd=self.repo, check=True)
        subprocess.run(("git", "commit", "-qm", "drift"), cwd=self.repo, check=True)
        record["candidate_revision"] = git(self.repo, "rev-parse", "HEAD")
        with self.assertRaisesRegex(Refusal, "SPX-K328-DRIFT"):
            validate(record, self.repo)

    def test_refuses_missing_required_receipt(self) -> None:
        record = self.record()
        record["receipts"].pop()
        with self.assertRaisesRegex(Refusal, "SPX-K328-INVENTORY"):
            validate(record, self.repo)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--record", type=Path)
    parser.add_argument("--repository", type=Path, default=Path.cwd())
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        if args.record is not None:
            parser.error("--self-test does not accept --record")
        return 0 if unittest.main(argv=[sys.argv[0]], exit=False).result.wasSuccessful() else 1
    if args.record is None:
        parser.error("--record is required unless --self-test is selected")
    try:
        report = validate(read_record(args.record), args.repository.resolve())
    except Refusal as error:
        print(error, file=sys.stderr)
        return 1
    print(json.dumps(report, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
