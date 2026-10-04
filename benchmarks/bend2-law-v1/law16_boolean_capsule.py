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
PROJECT_PROOF_REPLAY_SCHEMA = "semaprax.bend2-law-benchmark.project-z3-proof-replay-summary.v1"
PROJECT_PROOF_RECEIPT_SCHEMA = "semaprax.bend2-law-benchmark.project-z3-proof-replay-receipt.v1"
PROJECT_PROOF_OUTPUT_SCHEMA = "semaprax.installed-project-proof-check.v1"
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


def exact_digest(path: pathlib.Path, expected: object, label: str) -> None:
    if not isinstance(expected, str) or digest(path.read_bytes()) != expected:
        raise ValueError(f"{label} digest disagrees")


def project_proof_observation(root: pathlib.Path, replay: dict) -> dict:
    proof_root = root / "project-z3-replay"
    summary = read_json(proof_root / "summary.json", "project Z3 proof replay summary")
    if summary.get("schema") != PROJECT_PROOF_REPLAY_SCHEMA or summary.get("status") != "completed":
        raise ValueError("project Z3 proof replay summary has unsupported identity")
    solver, semaprax, rows = summary.get("solver"), summary.get("semaprax"), summary.get("rows")
    if not isinstance(solver, dict) or not isinstance(semaprax, dict) or not isinstance(rows, list):
        raise ValueError("project Z3 proof replay summary is malformed")
    solver_version = solver.get("version")
    if not isinstance(solver_version, str) or not isinstance(solver.get("sha256"), str) or not isinstance(semaprax.get("sha256"), str):
        raise ValueError("project Z3 proof replay identities are malformed")
    expected = {(ordinal, kind) for ordinal in range(1, 11) for kind in ("candidate", "attack")}
    actual = {(row.get("ordinal"), row.get("kind")) for row in rows if isinstance(row, dict)}
    if len(rows) != 20 or actual != expected:
        raise ValueError("project Z3 proof replay does not cover exactly ten candidate and attack pairs")
    for row in rows:
        if not isinstance(row, dict) or row.get("schema") != PROJECT_PROOF_RECEIPT_SCHEMA:
            raise ValueError("project Z3 proof receipt has unsupported identity")
        ordinal, kind = row["ordinal"], row["kind"]
        if row.get("semaprax_sha256") != semaprax["sha256"] or row.get("z3_sha256") != solver["sha256"] or row.get("z3_version") != solver_version:
            raise ValueError("project Z3 proof receipt identity drifted")
        raw = proof_root / f"ordinal-{ordinal}" / kind
        receipt = read_json(raw / "receipt.json", "project Z3 proof receipt")
        if receipt != row:
            raise ValueError("project Z3 proof receipt differs from its summary")
        source = raw / "src/app.spx"
        exact_digest(source, row.get("source_sha256"), "project Z3 proof source")
        exact_digest(raw / "semaprax.toml", row.get("manifest_sha256"), "project Z3 proof manifest")
        exact_digest(raw / "stdout.json", row.get("stdout_sha256"), "project Z3 proof stdout")
        exact_digest(raw / "stderr.txt", row.get("stderr_sha256"), "project Z3 proof stderr")
        source_from_replay = root / "replay" / f"ordinal-{ordinal}" / f"semaprax-{kind}.spx"
        exact_digest(source_from_replay, row.get("source_sha256"), "project Z3 proof source binding")
        if kind == "candidate":
            if row.get("exit_code") != 0:
                raise ValueError("project Z3 candidate was not discharged")
            output = read_json(raw / "stdout.json", "project Z3 candidate output")
            obligations = output.get("project_assurance", {}).get("payload", {}).get("obligations")
            if not isinstance(obligations, list) or not any(
                isinstance(obligation, dict)
                and obligation.get("declaration_id") == "app.negate"
                and obligation.get("kind") == "postcondition"
                and obligation.get("classification") == "smt_proved"
                and any(
                    isinstance(method, dict)
                    and method.get("class") == "smt_proved"
                    and method.get("tool") == "z3"
                    and method.get("tool_version") == solver_version
                    and isinstance(method.get("proof_ref"), str)
                    for method in obligation.get("methods", [])
                )
                for obligation in obligations
            ):
                raise ValueError("project Z3 candidate lacks the app.negate SMT discharge")
        elif row.get("exit_code") == 0:
            raise ValueError("project Z3 exact attack was unexpectedly discharged")
    return {
        "status": "observed_installed_z3_source_proof",
        "candidate_postcondition_discharges": 10,
        "exact_seeded_attack_rejections": 10,
        "solver": {"sha256": solver["sha256"], "version": solver_version},
        "semaprax": {"sha256": semaprax["sha256"]},
        "limitations": [
            "each receipt proves only app.negate ensures[0] in its retained Project revision",
            "the admitted SMT subset excludes calls, so app.main is outside this discharge",
            "trusted local Z3 and source translation do not prove lowering or execution",
            "an attack nonzero exit is route rejection; this retained output does not independently expose a solver counterexample",
        ],
    }


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
    project_proof = project_proof_observation(root, replay)
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
            "semaprax_project_z3_candidate_postcondition_discharges": project_proof["candidate_postcondition_discharges"],
            "semaprax_project_z3_exact_attack_rejections": project_proof["exact_seeded_attack_rejections"],
        },
        "check_phase": {"status": "observed_non_proof_check", "reason": "SEMAPRAX check accepts every exact law-gaming attack and is not the formal-discharge route"},
        "proof_phase": project_proof,
        "cost_usage": {"status": "unavailable", "reason": "the retained Codex JSON events have no provider monetary charge"},
        "remaining_acceptance_gaps": [
            "the five checked-u32 cells remain unsupported by the reviewed SEMAPRAX scalar profile",
            "the tool identities are local pinned observations rather than current-head evidence",
            "Bend verdict markers are retained output and were not independently replayed by a separate proof system",
            "the Boolean microcell cannot establish the full LAW-16 matrix",
            "a next scalar campaign needs a separately specified call-free contract with an identical Bend numeric domain and explicit overflow rules; the five checked-u32 cells cannot use an i32 substitute",
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
