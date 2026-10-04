#!/usr/bin/env python3
"""Verify the bounded, local LAW-16 Boolean raw-evidence capsule without tools."""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import stat


SCHEMA = "semaprax.bend2-law-benchmark.boolean-agent-raw-capsule.v1"
REPLAY_SCHEMA = "semaprax.bend2-law-benchmark.ten-boolean-independent-replay.v1"
RESULT_SCHEMA = "semaprax.bend2-law-benchmark.boolean-agent-raw-capsule-review.v1"
MAX_BYTES = 2 * 1024 * 1024


def digest(body: bytes) -> str:
    return "sha256:" + hashlib.sha256(body).hexdigest()


def read_json(path: pathlib.Path, label: str) -> dict:
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {label}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} is not an object")
    return value


def raw_file(root: pathlib.Path, reference: object) -> None:
    if not isinstance(reference, dict) or set(reference) != {"path", "sha256", "bytes"}:
        raise ValueError("capsule reference is malformed")
    name, expected_digest, expected_bytes = reference["path"], reference["sha256"], reference["bytes"]
    if not isinstance(name, str) or not isinstance(expected_digest, str) or not isinstance(expected_bytes, int) or not 0 <= expected_bytes <= MAX_BYTES:
        raise ValueError("capsule reference is invalid")
    relative = pathlib.PurePosixPath(name)
    if relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts):
        raise ValueError("capsule reference path is unsafe")
    path = root
    try:
        for part in relative.parts:
            path /= part
            details = path.lstat()
            if stat.S_ISLNK(details.st_mode):
                raise ValueError("capsule reference crosses a symbolic link")
        if not stat.S_ISREG(details.st_mode) or details.st_size != expected_bytes:
            raise ValueError("capsule reference bytes disagree")
        body = path.read_bytes()
    except OSError as error:
        raise ValueError("capsule reference is unavailable") from error
    if digest(body) != expected_digest:
        raise ValueError("capsule reference digest disagrees")


def review(root: pathlib.Path) -> dict:
    root = root.resolve(strict=True)
    manifest = read_json(root / "manifest.json", "capsule manifest")
    if manifest.get("schema") != SCHEMA or manifest.get("status") != "local_unhosted_raw_evidence":
        raise ValueError("capsule manifest has unsupported identity")
    files = manifest.get("files")
    if not isinstance(files, list) or len(files) < 1:
        raise ValueError("capsule has no raw file inventory")
    for reference in files:
        raw_file(root, reference)
    replay = read_json(root / "replay/summary.json", "independent replay summary")
    if replay.get("schema") != REPLAY_SCHEMA or replay.get("status") != "completed_local_replay":
        raise ValueError("replay summary has unsupported identity")
    rows = replay.get("ordinals")
    if not isinstance(rows, list) or [row.get("ordinal") for row in rows] != list(range(1, 11)):
        raise ValueError("replay summary does not cover exactly ten ordinals")
    for row in rows:
        bend, semaprax = row.get("bend"), row.get("semaprax_check")
        if not isinstance(bend, dict) or not isinstance(semaprax, dict):
            raise ValueError("replay row lacks language routes")
        if any(bend.get(kind, {}).get(route, {}).get("exit_code") != expected for kind, expected in (("candidate", 0), ("attack", 1)) for route in ("normal", "verdict")):
            raise ValueError("Bend replay does not preserve candidate success and attack rejection")
        if any(semaprax.get(kind, {}).get("exit_code") != 0 for kind in ("candidate", "attack")):
            raise ValueError("SEMAPRAX check replay diverged from recorded behavior")
    return {
        "schema": RESULT_SCHEMA,
        "status": "local_boolean_replay_authenticated",
        "raw_capsule": {"path": str(root), "manifest_files": len(files)},
        "observations": {
            "ordinals": 10,
            "bend_candidate_check_and_verdict_successes": 10,
            "bend_exact_attack_check_and_verdict_rejections": 10,
            "semaprax_check_candidate_successes": 10,
            "semaprax_check_exact_attack_successes": 10,
        },
        "proof_phase": {"status": "unavailable", "reason": "the available SEMAPRAX check accepts every exact law-gaming attack, so it is not a Boolean proof/rejection phase"},
        "cost_usage": {"status": "unavailable", "reason": "the retained Codex JSON events have no provider monetary charge"},
        "remaining_acceptance_gaps": [
            "the five checked-u32 cells remain unsupported by the reviewed SEMAPRAX scalar profile",
            "the tool identities are local pinned observations rather than current-head evidence",
            "Bend verdict markers are retained output and were not independently replayed by a separate proof system",
            "the Boolean microcell cannot establish the full LAW-16 matrix",
        ],
        "nonclaims": [
            "raw local replay is not a completed LAW-16 repair",
            "no agent authorship, cross-language comparison, timing, or superiority result",
            "no checked-u32 result",
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capsule", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        value = review(args.capsule)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    if args.output.exists() or not args.output.parent.is_dir():
        parser.error("output must be a new file beneath an existing directory")
    args.output.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
