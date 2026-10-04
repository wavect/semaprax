#!/usr/bin/env python3
"""Authenticate retained two-input Bend Boolean runtime and verdict evidence."""
from __future__ import annotations

import argparse
import importlib.util
import json
import pathlib
import re


SCHEMA = "semaprax.bend2-law-benchmark.bend-boolean-runtime-acceptance.v1"
INPUT_SCHEMA = "semaprax.bend2-law-benchmark.bend-boolean-two-input-evidence.v1"
RECEIPT_SCHEMA = "semaprax.bend2-law-benchmark.raw-bend-cli-receipt.v1"
EDIT_SCHEMA = "semaprax.bend2-law-benchmark.boolean-edit-response.v1"
TASK, LANE = "scalar-contract-bug-v1", "bend2"
NORMAL = b"ALL PROOFS CHECK\nUse --verdict for mathematical validity.\n"
VERDICT = b"ALL PROOFS CHECK\n"


def _runtime_module():
    path = pathlib.Path(__file__).with_name("semaprax_boolean_runtime_acceptance.py")
    spec = importlib.util.spec_from_file_location("bend_boolean_runtime_shared", path)
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    spec.loader.exec_module(module)
    return module


SHARED = _runtime_module()
PLAN = SHARED.PLAN
digest, canonical, raw_file, root_directory = SHARED.digest, SHARED.canonical, SHARED.raw_file, SHARED.root_directory


def selected_trial(plan: dict, trial_id: str) -> dict:
    rows = [trial for cell in plan.get("cells", []) if cell.get("id") == TASK and cell.get("status") == "preregistered"
            for trial in cell.get("trials", []) if trial.get("id") == trial_id]
    if len(rows) != 1 or rows[0].get("language") != LANE or rows[0].get("status") != "not_run":
        raise ValueError("evidence does not select one preregistered Bend Boolean trial")
    return rows[0]


def source_contract(source: bytes) -> None:
    try:
        text = source.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError("final Bend source is not UTF-8") from error
    patterns = (r"(?m)^import Base$", r"(?ms)^def boolean_score\(value: Bool\) -> U32:",
                r"(?ms)^law boolean_score_matches_base:\s+for value: Bool\s+\{boolean_score\(value\) == Bool\.to_u32\(value\) : U32\}",
                r"(?ms)^def boolean_score_matches_base\(value\):")
    if not all(re.search(pattern, text) for pattern in patterns):
        raise ValueError("final Bend source does not retain the pinned Boolean function and law")


def read_input(path: pathlib.Path) -> dict:
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError("cannot read Bend two-input evidence") from error
    keys = {"schema", "plan_sha256", "trial_id", "bend_identity", "model_response", "final_source", "seeded_attack_source", "candidate", "seeded_attack"}
    if not isinstance(value, dict) or value.get("schema") != INPUT_SCHEMA or set(value) != keys:
        raise ValueError("Bend two-input evidence fields are not exact")
    identity = value["bend_identity"]
    if (not isinstance(identity, dict) or set(identity) != {"commit", "bun_sha256"}
            or not all(isinstance(identity[key], str) and identity[key] for key in identity)):
        raise ValueError("Bend identity is incomplete")
    return value


def receipt(body: bytes, source: bytes, identity: dict, verdict: bool, kind: str) -> dict:
    try:
        value = json.loads(body)
    except json.JSONDecodeError as error:
        raise ValueError(f"{kind} receipt is not JSON") from error
    keys = {"schema", "argv", "exit_code", "source_sha256", "bend_identity", "environment"}
    if not isinstance(value, dict) or set(value) != keys or value["schema"] != RECEIPT_SCHEMA:
        raise ValueError(f"{kind} receipt fields are not exact")
    if value["source_sha256"] != digest(source) or value["bend_identity"] != identity or value["environment"] != {"BEND_NO_TELEMETRY": "1"}:
        raise ValueError(f"{kind} receipt is not bound to source, identity, and environment")
    argv = value["argv"]
    expected_length = 4 if verdict else 3
    if (not isinstance(argv, list) or len(argv) != expected_length or not all(isinstance(part, str) and part for part in argv)
            or (verdict and argv[-1] != "--verdict") or (not verdict and argv[-1] == "--verdict") or not isinstance(value["exit_code"], int)):
        raise ValueError(f"{kind} receipt is not a bounded Bend command")
    return value


def route(row: object, root: pathlib.Path, source: bytes, identity: dict, kind: str) -> dict:
    if not isinstance(row, dict) or set(row) != {"normal", "verdict"}:
        raise ValueError(f"{kind} route fields are not exact")
    result = {}
    for route_name, is_verdict in (("normal", False), ("verdict", True)):
        entry = row[route_name]
        if not isinstance(entry, dict) or set(entry) != {"receipt", "stdout", "stderr"}:
            raise ValueError(f"{kind} {route_name} fields are not exact")
        receipt_ref, receipt_body = raw_file(entry["receipt"], root, f"{kind} {route_name} receipt")
        stdout_ref, stdout = raw_file(entry["stdout"], root, f"{kind} {route_name} stdout")
        stderr_ref, stderr = raw_file(entry["stderr"], root, f"{kind} {route_name} stderr")
        result[route_name] = {"receipt": receipt_ref, "value": receipt(receipt_body, source, identity, is_verdict, f"{kind} {route_name}"),
                              "stdout": stdout_ref, "stderr": stderr_ref, "stdout_bytes": stdout, "stderr_bytes": stderr}
    return result


def evaluate(plan_path: pathlib.Path, input_path: pathlib.Path, artifact_root: pathlib.Path, expected_commit: str) -> dict:
    plan = PLAN.object_json(plan_path, PLAN.SCHEMA)
    evidence = read_input(input_path)
    if evidence["plan_sha256"] != PLAN.digest(plan_path):
        raise ValueError("Bend evidence is not bound to this exact preregistration")
    trial = selected_trial(plan, evidence["trial_id"])
    identity = evidence["bend_identity"]
    if identity["commit"] != expected_commit:
        raise ValueError("Bend identity is not the pinned commit")
    root = root_directory(artifact_root)
    response_ref, response_body = raw_file(evidence["model_response"], root, "model response")
    source_ref, source = raw_file(evidence["final_source"], root, "final source")
    attack_ref, attack = raw_file(evidence["seeded_attack_source"], root, "seeded attack source")
    source_contract(source)
    expected_attack = (pathlib.Path(__file__).with_name("fixtures") / "bend-two-value-boolean-law-gaming-v1.bend").read_bytes()
    if attack != expected_attack:
        raise ValueError("seeded attack source is not the pinned law-gaming control")
    try:
        response = json.loads(response_body)
    except json.JSONDecodeError as error:
        raise ValueError("model response is not JSON") from error
    claim = response.get("seeded_attack") if isinstance(response, dict) else None
    if (not isinstance(response, dict) or response.get("schema") != EDIT_SCHEMA or response.get("trial_id") != trial["id"]
            or response.get("language") != LANE or response.get("final_source") != source.decode("utf-8")
            or not isinstance(claim, dict) or claim.get("source_sha256") != digest(attack) or claim.get("decision") != "reject"):
        raise ValueError("model response does not bind final source and seeded attack claim")
    candidate, seeded_attack = route(evidence["candidate"], root, source, identity, "candidate"), route(evidence["seeded_attack"], root, attack, identity, "seeded attack")
    if (candidate["normal"]["value"]["exit_code"] != 0 or candidate["normal"]["stdout_bytes"] != NORMAL or candidate["normal"]["stderr_bytes"]
            or candidate["verdict"]["value"]["exit_code"] != 0 or candidate["verdict"]["stdout_bytes"] != VERDICT or candidate["verdict"]["stderr_bytes"]):
        raise ValueError("candidate evidence lacks exact ordinary-check and verdict outputs")
    if any(value["value"]["exit_code"] == 0 or b"SOME PROOFS FAIL" not in value["stdout_bytes"] + value["stderr_bytes"] for value in seeded_attack.values()):
        raise ValueError("seeded attack evidence does not reject on both Bend routes")
    def public(value: dict) -> dict:
        return {name: {key: value[name][key] for key in ("receipt", "stdout", "stderr")} for name in ("normal", "verdict")}
    return {"schema": SCHEMA, "status": "two_input_bend_runtime_authenticated", "plan": {"path": str(plan_path.resolve()), "sha256": PLAN.digest(plan_path)},
            "trial": {"id": trial["id"], "language": LANE}, "bend_identity": identity, "numeric_domain": {"status": "bool_exact_only", "caveat": "this Boolean microcell does not establish matched checked-u32 support"},
            "raw_artifact_root": str(root), "final_source": source_ref, "model_response": response_ref, "seeded_attack_source": attack_ref,
            "candidate": public(candidate), "seeded_attack": public(seeded_attack),
            "proof_phase": {"status": "observed_raw_kernel_output", "limitations": "the retained Bend verdict marker is not independently replayed proof"},
            "cost_usage": {"status": "unavailable", "reason": "raw Bend CLI evidence has no provider billing record"},
            "nonclaims": ["two-input Bend check authentication is not a successful law repair", "ordinary Bend output is checker evidence, not a runtime input witness", "this evaluator does not execute Bend", "the Boolean microcell is not checked-u32 evidence", "no matched-language, comparative, timing, or agent-authorship result"]}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", required=True, type=pathlib.Path); parser.add_argument("--evidence", required=True, type=pathlib.Path)
    parser.add_argument("--artifact-root", required=True, type=pathlib.Path); parser.add_argument("--expected-bend-commit", required=True)
    parser.add_argument("--output", required=True, type=pathlib.Path); args = parser.parse_args(argv)
    try: value = evaluate(args.plan, args.evidence, args.artifact_root, args.expected_bend_commit)
    except ValueError as error: parser.error(str(error))
    if args.output.exists() or not args.output.parent.is_dir(): parser.error("output must be a new file beneath an existing directory")
    args.output.write_text(canonical(value)); return 0


if __name__ == "__main__":
    raise SystemExit(main())
