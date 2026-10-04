#!/usr/bin/env python3
"""Authenticate retained two-input SEMAPRAX Boolean runtime evidence.

This checker never invokes a compiler.  It only validates exact retained
source, command receipts, and stdout/stderr bytes from a separately performed
compiler/runtime invocation.  Passing is a runtime-evidence result; it is not
a formal proof or a claim that an agent repaired a law.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import pathlib
import re
import stat


SCHEMA = "semaprax.bend2-law-benchmark.semaprax-boolean-runtime-acceptance.v1"
INPUT_SCHEMA = "semaprax.bend2-law-benchmark.semaprax-boolean-two-input-evidence.v1"
RECEIPT_SCHEMA = "semaprax.bend2-law-benchmark.raw-cli-receipt.v1"
EDIT_SCHEMA = "semaprax.bend2-law-benchmark.boolean-edit-response.v1"
TASK = "scalar-contract-bug-v1"
LANE = "semaprax-scalar-v1"
MAX_ARTIFACT_BYTES = 32 * 1024 * 1024


def _plan_module():
    path = pathlib.Path(__file__).with_name("agent_trial_plan.py")
    spec = importlib.util.spec_from_file_location("semaprax_boolean_runtime_plan", path)
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    spec.loader.exec_module(module)
    return module


PLAN = _plan_module()


def digest(body: bytes) -> str:
    return "sha256:" + hashlib.sha256(body).hexdigest()


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def raw_file(reference: object, root: pathlib.Path, label: str) -> tuple[dict, bytes]:
    if not isinstance(reference, dict) or set(reference) != {"path", "sha256", "bytes"}:
        raise ValueError(f"{label} reference is malformed")
    name, expected_digest, expected_bytes = reference["path"], reference["sha256"], reference["bytes"]
    if (not isinstance(name, str) or not name or not isinstance(expected_digest, str)
            or not isinstance(expected_bytes, int) or not 0 <= expected_bytes <= MAX_ARTIFACT_BYTES):
        raise ValueError(f"{label} reference is invalid")
    relative = pathlib.PurePosixPath(name)
    if relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts):
        raise ValueError(f"{label} path is unsafe")
    path = root
    try:
        for part in relative.parts:
            path /= part
            details = path.lstat()
            if stat.S_ISLNK(details.st_mode):
                raise ValueError(f"{label} path crosses a symbolic link")
        if not stat.S_ISREG(details.st_mode) or details.st_size != expected_bytes:
            raise ValueError(f"{label} bytes disagree")
        body = path.read_bytes()
        after = path.lstat()
    except OSError as error:
        raise ValueError(f"{label} is unavailable") from error
    if (len(body) != expected_bytes or digest(body) != expected_digest or after.st_ino != details.st_ino
            or after.st_dev != details.st_dev or after.st_size != details.st_size):
        raise ValueError(f"{label} changed or digest disagrees")
    return ({"path": name, "sha256": digest(body), "bytes": len(body)}, body)


def root_directory(path: pathlib.Path) -> pathlib.Path:
    try:
        details = path.lstat()
    except OSError as error:
        raise ValueError("raw artifact root is unavailable") from error
    if not stat.S_ISDIR(details.st_mode) or stat.S_ISLNK(details.st_mode):
        raise ValueError("raw artifact root must be a directory, not a link")
    return path.resolve(strict=True)


def selected_trial(plan: dict, trial_id: str) -> dict:
    if plan.get("schema") != PLAN.SCHEMA:
        raise ValueError("plan has unsupported schema")
    rows = [trial for cell in plan.get("cells", []) if cell.get("id") == TASK and cell.get("status") == "preregistered"
            for trial in cell.get("trials", []) if trial.get("id") == trial_id]
    if len(rows) != 1 or rows[0].get("language") != LANE or rows[0].get("status") != "not_run":
        raise ValueError("evidence does not select one preregistered SEMAPRAX Boolean trial")
    return rows[0]


def source_contract(source: bytes) -> None:
    try:
        text = source.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError("final source is not UTF-8") from error
    patterns = (
        r"(?m)^module app;$",
        r'(?ms)@id\("app\.negate"\)\s+fn negate\(value: bool\) -> bool\s+ensures result == !value\s*\{',
        r'(?ms)@id\("app\.main"\)\s+fn main\(\) -> i64\s*\{',
    )
    if not all(re.search(pattern, text) for pattern in patterns):
        raise ValueError("final source does not retain the pinned Boolean module, identities, and contract")


def receipt(body: bytes, expected_source: bytes, identity: dict, kind: str) -> dict:
    try:
        value = json.loads(body)
    except json.JSONDecodeError as error:
        raise ValueError(f"{kind} receipt is not JSON") from error
    if not isinstance(value, dict) or set(value) != {"schema", "argv", "exit_code", "source_sha256", "compiler_identity"}:
        raise ValueError(f"{kind} receipt fields are not exact")
    if value["schema"] != RECEIPT_SCHEMA or value["source_sha256"] != digest(expected_source):
        raise ValueError(f"{kind} receipt is not bound to its retained source")
    if value["compiler_identity"] != identity:
        raise ValueError(f"{kind} receipt compiler identity disagrees")
    argv = value["argv"]
    if (not isinstance(argv, list) or len(argv) != 3 or not all(isinstance(part, str) and part for part in argv)
            or argv[1] != "run" or not isinstance(value["exit_code"], int)):
        raise ValueError(f"{kind} receipt is not a bounded SEMAPRAX run command")
    return value


def read_input(path: pathlib.Path) -> dict:
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError("cannot read two-input evidence") from error
    keys = {"schema", "plan_sha256", "trial_id", "compiler_identity", "model_response", "final_source", "seeded_attack_source", "candidate", "seeded_attack"}
    if not isinstance(value, dict) or value.get("schema") != INPUT_SCHEMA or set(value) != keys:
        raise ValueError("two-input evidence fields are not exact")
    identity = value["compiler_identity"]
    if (not isinstance(identity, dict) or set(identity) != {"commit", "executable_sha256"}
            or not all(isinstance(identity[key], str) and identity[key] for key in identity)):
        raise ValueError("compiler identity is incomplete")
    return value


def route(row: object, root: pathlib.Path, source: bytes, identity: dict, kind: str) -> dict:
    if not isinstance(row, dict) or set(row) != {"receipt", "stdout", "stderr"}:
        raise ValueError(f"{kind} route fields are not exact")
    receipt_ref, receipt_body = raw_file(row["receipt"], root, f"{kind} receipt")
    stdout_ref, stdout = raw_file(row["stdout"], root, f"{kind} stdout")
    stderr_ref, stderr = raw_file(row["stderr"], root, f"{kind} stderr")
    return {"receipt": receipt_ref, "value": receipt(receipt_body, source, identity, kind),
            "stdout": stdout_ref, "stderr": stderr_ref, "stdout_bytes": stdout, "stderr_bytes": stderr}


def evaluate(plan_path: pathlib.Path, input_path: pathlib.Path, artifact_root: pathlib.Path, expected_commit: str) -> dict:
    plan = PLAN.object_json(plan_path, PLAN.SCHEMA)
    evidence = read_input(input_path)
    if evidence["plan_sha256"] != PLAN.digest(plan_path):
        raise ValueError("two-input evidence is not bound to this exact preregistration")
    trial = selected_trial(plan, evidence["trial_id"])
    identity = evidence["compiler_identity"]
    if identity["commit"] != expected_commit:
        raise ValueError("compiler identity is not the pinned SEMAPRAX commit")
    root = root_directory(artifact_root)
    response_ref, response_body = raw_file(evidence["model_response"], root, "model response")
    source_ref, source = raw_file(evidence["final_source"], root, "final source")
    attack_ref, attack = raw_file(evidence["seeded_attack_source"], root, "seeded attack source")
    source_contract(source)
    expected_attack = (pathlib.Path(__file__).with_name("fixtures") / "semaprax-two-value-boolean-law-gaming-v1.spx").read_bytes()
    if attack != expected_attack:
        raise ValueError("seeded attack source is not the pinned law-gaming control")
    try:
        response = json.loads(response_body)
    except json.JSONDecodeError as error:
        raise ValueError("model response is not JSON") from error
    if (not isinstance(response, dict) or response.get("schema") != EDIT_SCHEMA or response.get("trial_id") != trial["id"]
            or response.get("language") != LANE or response.get("final_source") != source.decode("utf-8")):
        raise ValueError("model response does not bind the retained final source")
    claim = response.get("seeded_attack")
    if not isinstance(claim, dict) or claim.get("source_sha256") != digest(attack) or claim.get("decision") != "reject":
        raise ValueError("model response does not bind the seeded attack claim")
    candidate = route(evidence["candidate"], root, source, identity, "candidate")
    seeded_attack = route(evidence["seeded_attack"], root, attack, identity, "seeded attack")
    if candidate["value"]["exit_code"] != 0 or candidate["stdout_bytes"] != b"0\n" or candidate["stderr_bytes"]:
        raise ValueError("candidate compiler/runtime evidence lacks the exact Boolean witness")
    if seeded_attack["value"]["exit_code"] == 0 or seeded_attack["stdout_bytes"] or b"language status" not in seeded_attack["stderr_bytes"]:
        raise ValueError("seeded attack compiler/runtime evidence does not show the required rejection")
    def public(route_value: dict) -> dict:
        return {key: route_value[key] for key in ("receipt", "stdout", "stderr")}
    return {
        "schema": SCHEMA,
        "status": "two_input_runtime_authenticated",
        "plan": {"path": str(plan_path.resolve()), "sha256": PLAN.digest(plan_path)},
        "trial": {"id": trial["id"], "language": LANE},
        "compiler_identity": identity,
        "raw_artifact_root": str(root),
        "final_source": source_ref,
        "model_response": response_ref,
        "seeded_attack_source": attack_ref,
        "candidate": public(candidate),
        "seeded_attack": public(seeded_attack),
        "proof_phase": {"status": "unavailable", "reason": "the scalar runtime route supplies no admitted formal Boolean proof phase"},
        "cost_usage": {"status": "unavailable", "reason": "raw compiler/runtime evidence has no provider billing record"},
        "nonclaims": [
            "two-input runtime authentication is not a successful law repair",
            "this evaluator does not execute a compiler or interpret an unretained exit code",
            "the runtime witness is not a formal proof",
            "the model claim is authenticated only as provenance and does not decide this result",
            "no comparative, timing, or agent-authorship result",
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", required=True, type=pathlib.Path)
    parser.add_argument("--evidence", required=True, type=pathlib.Path)
    parser.add_argument("--artifact-root", required=True, type=pathlib.Path)
    parser.add_argument("--expected-semaprax-commit", required=True)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        value = evaluate(args.plan, args.evidence, args.artifact_root, args.expected_semaprax_commit)
    except ValueError as error:
        parser.error(str(error))
    if args.output.exists() or not args.output.parent.is_dir():
        parser.error("output must be a new file beneath an existing directory")
    args.output.write_text(canonical(value))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
