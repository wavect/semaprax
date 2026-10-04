#!/usr/bin/env python3
"""Authenticate one pinned Boolean LAW-16 final-source pair without running tools.

This evaluator is deliberately a source/proof-artifact reviewer.  It reads
retained raw files below one supplied root and compares both final sources to
the pinned Boolean success fixtures.  It never reads an exit code and cannot
turn an agent response, a runtime witness, or a successful command into a law
repair claim.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import pathlib
import stat


SCHEMA = "semaprax.bend2-law-benchmark.boolean-pair-acceptance.v1"
INPUT_SCHEMA = "semaprax.bend2-law-benchmark.boolean-final-artifacts.v1"
MAX_ARTIFACT_BYTES = 32 * 1024 * 1024
TASK = "scalar-contract-bug-v1"
LANES = ("bend2", "semaprax-scalar-v1")


def _plan_module():
    path = pathlib.Path(__file__).with_name("agent_trial_plan.py")
    spec = importlib.util.spec_from_file_location("bend2_boolean_pair_plan", path)
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    spec.loader.exec_module(module)
    return module


PLAN = _plan_module()


def digest(body: bytes) -> str:
    return "sha256:" + hashlib.sha256(body).hexdigest()


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def read_input(path: pathlib.Path) -> dict:
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError("cannot read final-artifact input") from error
    if not isinstance(value, dict) or value.get("schema") != INPUT_SCHEMA:
        raise ValueError("final-artifact input has unsupported schema")
    if set(value) != {"schema", "plan_sha256", "task", "ordinal", "lanes"}:
        raise ValueError("final-artifact input keys are not exact")
    if value["task"] != TASK or not isinstance(value["ordinal"], int) or value["ordinal"] < 1:
        raise ValueError("final-artifact input does not select the admitted Boolean task")
    if not isinstance(value["plan_sha256"], str) or not value["plan_sha256"].startswith("sha256:"):
        raise ValueError("final-artifact input does not bind a plan digest")
    if not isinstance(value["lanes"], dict) or set(value["lanes"]) != set(LANES):
        raise ValueError("final-artifact input does not contain the matched lane pair")
    return value


def root_directory(root: pathlib.Path) -> pathlib.Path:
    try:
        details = root.lstat()
    except OSError as error:
        raise ValueError("raw artifact root is unavailable") from error
    if not stat.S_ISDIR(details.st_mode) or stat.S_ISLNK(details.st_mode):
        raise ValueError("raw artifact root must be a directory, not a link")
    return root.resolve(strict=True)


def raw_file(reference: object, root: pathlib.Path, label: str) -> tuple[dict, bytes]:
    if not isinstance(reference, dict) or set(reference) != {"path", "sha256", "bytes"}:
        raise ValueError(f"{label} reference is malformed")
    name, expected_digest, expected_bytes = reference["path"], reference["sha256"], reference["bytes"]
    if (not isinstance(name, str) or not name or not isinstance(expected_digest, str)
            or not isinstance(expected_bytes, int) or expected_bytes < 0 or expected_bytes > MAX_ARTIFACT_BYTES):
        raise ValueError(f"{label} reference is invalid")
    relative = pathlib.PurePosixPath(name)
    if relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts):
        raise ValueError(f"{label} path is not a safe relative path")
    path = root
    try:
        for part in relative.parts:
            path = path / part
            details = path.lstat()
            if stat.S_ISLNK(details.st_mode):
                raise ValueError(f"{label} path crosses a symbolic link")
        if not stat.S_ISREG(details.st_mode) or details.st_size != expected_bytes:
            raise ValueError(f"{label} bytes disagree")
        body = path.read_bytes()
        after = path.lstat()
    except OSError as error:
        raise ValueError(f"{label} is unavailable") from error
    if (len(body) != expected_bytes or len(body) > MAX_ARTIFACT_BYTES
            or after.st_ino != details.st_ino or after.st_dev != details.st_dev
            or after.st_size != details.st_size or digest(body) != expected_digest):
        raise ValueError(f"{label} changed or digest disagrees")
    return ({"path": name, "sha256": digest(body), "bytes": len(body)}, body)


def require_preregistered_pair(plan: dict, ordinal: int) -> dict:
    if plan.get("schema") != PLAN.SCHEMA:
        raise ValueError("plan has unsupported schema")
    cells = [cell for cell in plan.get("cells", []) if cell.get("id") == TASK and cell.get("status") == "preregistered"]
    if len(cells) != 1:
        raise ValueError("plan does not preregister the Boolean task")
    trials = cells[0].get("trials")
    by_lane = {lane: [trial for trial in trials if trial.get("language") == lane and trial.get("ordinal") == ordinal]
               for lane in LANES}
    if any(len(rows) != 1 or rows[0].get("status") != "not_run" for rows in by_lane.values()):
        raise ValueError("plan does not contain the selected unmatched Boolean trial pair")
    return {lane: by_lane[lane][0]["id"] for lane in LANES}


def lane_input(row: object, lane: str, root: pathlib.Path, expected_source: bytes) -> dict:
    if not isinstance(row, dict) or set(row) != {"trial_id", "final_source", "verification_artifact"}:
        raise ValueError(f"{lane} final-artifact lane is malformed")
    source, source_body = raw_file(row["final_source"], root, f"{lane} final source")
    verification, verification_body = raw_file(row["verification_artifact"], root, f"{lane} verification artifact")
    if source_body != expected_source:
        raise ValueError(f"{lane} final source is not the pinned Boolean success source")
    if lane == "bend2":
        if b"ALL PROOFS CHECK" not in verification_body:
            raise ValueError("Bend verification artifact lacks its proof-kernel success marker")
        verification_status = {"status": "observed_raw_kernel_output", "marker": "ALL PROOFS CHECK"}
    else:
        if verification_body != b"0\n":
            raise ValueError("SEMAPRAX runtime witness does not have the exact Boolean result")
        verification_status = {"status": "runtime_witness_only", "reason": "runtime output is not a formal proof artifact"}
    return {"trial_id": row["trial_id"], "final_source": source, "verification_artifact": verification,
            "verification": verification_status}


def evaluate(plan_path: pathlib.Path, input_path: pathlib.Path, artifact_root: pathlib.Path) -> dict:
    plan = PLAN.object_json(plan_path, PLAN.SCHEMA)
    source = read_input(input_path)
    if source["plan_sha256"] != PLAN.digest(plan_path):
        raise ValueError("final-artifact input is not bound to this exact preregistration")
    trials = require_preregistered_pair(plan, source["ordinal"])
    root = root_directory(artifact_root)
    fixture_root = pathlib.Path(__file__).parent / "fixtures"
    expected = {
        "bend2": (fixture_root / "bend-two-value-boolean-v1.bend").read_bytes(),
        "semaprax-scalar-v1": (fixture_root / "semaprax-two-value-boolean-v1.spx").read_bytes(),
    }
    lanes = {lane: lane_input(source["lanes"][lane], lane, root, expected[lane]) for lane in LANES}
    if any(lanes[lane]["trial_id"] != trials[lane] for lane in LANES):
        raise ValueError("final-artifact lane is not bound to the selected preregistered trial")
    return {
        "schema": SCHEMA,
        "status": "source_pair_authenticated",
        "plan": {"path": str(plan_path.resolve()), "sha256": PLAN.digest(plan_path)},
        "task": TASK,
        "ordinal": source["ordinal"],
        "raw_artifact_root": str(root),
        "lanes": lanes,
        "proof_phases": {
            "bend_verdict": {"status": "observed_raw_kernel_output", "limitations": "the raw marker is not an independently replayed proof"},
            "semaprax_scalar": {"status": "unavailable", "reason": "no admitted formal Boolean proof phase for the scalar runtime route"},
        },
        "cost_usage": {"status": "unavailable", "reason": "final source and proof artifacts contain no provider billing record"},
        "nonclaims": [
            "source-pair authentication is not a successful law repair",
            "this evaluator does not execute a CLI or interpret an exit code",
            "the SEMAPRAX runtime witness is not a formal proof",
            "no agent authorship, comparative, timing, or superiority result",
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", required=True, type=pathlib.Path)
    parser.add_argument("--final-artifacts", required=True, type=pathlib.Path)
    parser.add_argument("--artifact-root", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        document = evaluate(args.plan, args.final_artifacts, args.artifact_root)
    except ValueError as error:
        parser.error(str(error))
    if args.output.exists() or not args.output.parent.is_dir():
        parser.error("output must be a new file beneath an existing directory")
    args.output.write_text(canonical(document))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
