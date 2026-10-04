#!/usr/bin/env python3
"""Validate raw LAW-16 agent telemetry against a fixed trial preregistration.

This command does not invoke an agent.  It turns an existing agent telemetry
export plus raw case outcomes into a digest-bound report.  A report reaches
``completed`` only after every preregistered trial is present, its exact success
witnesses were accepted, every seeded law-gaming attack was rejected, both
telemetry events are present, and all three phase measurements completed.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
from collections import Counter
from datetime import datetime, timezone


PLAN_SCHEMA = "semaprax.bend2-law-benchmark.agent-trial-plan.v1"
RAW_SCHEMA = "semaprax.bend2-law-benchmark.agent-telemetry-export.v1"
SCHEMA = "semaprax.bend2-law-benchmark.agent-trial-capture.v1"
PHASES = ("proof_synthesis", "law_kernel_check", "compile_or_runtime")
SHA256 = re.compile(r"sha256:[0-9a-f]{64}\Z")
USD = re.compile(r"[0-9]+\.[0-9]{2}\Z")


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest(path: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def read_object(path: pathlib.Path, schema: str) -> dict:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {path}") from error
    if not isinstance(value, dict) or value.get("schema") != schema:
        raise ValueError(f"{path} has unsupported schema")
    return value


def admitted_trials(plan: dict) -> dict[str, dict]:
    if plan.get("schema") != PLAN_SCHEMA or not isinstance(plan.get("cells"), list):
        raise ValueError("trial plan is malformed")
    selected: dict[str, dict] = {}
    for cell in plan["cells"]:
        if cell.get("status") != "preregistered":
            continue
        for trial in cell.get("trials", []):
            trial_id = trial.get("id")
            if not isinstance(trial_id, str) or trial_id in selected:
                raise ValueError("trial plan has duplicate or invalid trial identity")
            if trial.get("status") != "not_run" or trial.get("language") not in {"bend2", "semaprax-scalar-v1"}:
                raise ValueError("trial plan has an unsupported admitted trial")
            selected[trial_id] = trial
    if not selected:
        raise ValueError("trial plan has no admitted trials")
    return selected


def require_event(event: object, kind: str) -> None:
    if not isinstance(event, dict) or set(event) != {"event_id", "kind", "value"}:
        raise ValueError(f"{kind} event is malformed")
    if not isinstance(event["event_id"], str) or not event["event_id"] or event["kind"] != kind:
        raise ValueError(f"{kind} event lacks existing telemetry provenance")
    if kind == "token_usage":
        valid = isinstance(event["value"], int) and event["value"] >= 0
    else:
        valid = isinstance(event["value"], str) and USD.fullmatch(event["value"])
    if not valid:
        raise ValueError(f"{kind} event value is invalid")


def case_outcomes(expected: object, observed: object, desired: str, label: str) -> list[str]:
    if not isinstance(expected, list) or not isinstance(observed, list) or len(expected) != len(observed):
        raise ValueError(f"{label} cases do not match preregistration")
    evidence: list[str] = []
    for row in observed:
        if not isinstance(row, dict) or set(row) != {"outcome", "evidence_sha256"}:
            raise ValueError(f"{label} observation is malformed")
        if row["outcome"] != desired or not isinstance(row["evidence_sha256"], str) or not SHA256.fullmatch(row["evidence_sha256"]):
            raise ValueError(f"{label} observation does not meet its fixed criterion")
        evidence.append(row["evidence_sha256"])
    return evidence


def validate_trial(expected: dict, observed: object) -> dict:
    if not isinstance(observed, dict) or set(observed) != {
        "id", "transcript_sha256", "telemetry_events", "phases", "success_witnesses", "attacks"
    }:
        raise ValueError("raw trial has unexpected fields")
    if observed["id"] != expected["id"] or not isinstance(observed["transcript_sha256"], str) or not SHA256.fullmatch(observed["transcript_sha256"]):
        raise ValueError("raw trial is not bound to its preregistered identity and transcript")
    events = observed["telemetry_events"]
    if not isinstance(events, list) or len(events) != 2:
        raise ValueError("raw trial must retain exactly token and cost telemetry events")
    require_event(events[0], "token_usage")
    require_event(events[1], "cost_usage")
    if events[0]["event_id"] == events[1]["event_id"]:
        raise ValueError("raw trial reuses one telemetry event for token and cost")
    phases = observed["phases"]
    if not isinstance(phases, dict) or set(phases) != set(PHASES):
        raise ValueError("raw trial lacks separate phase measurements")
    phase_values = {}
    for phase in PHASES:
        row = phases[phase]
        if not isinstance(row, dict) or set(row) != {"status", "wall_ms"} or row["status"] != "completed" or not isinstance(row["wall_ms"], (int, float)) or row["wall_ms"] < 0:
            raise ValueError(f"raw trial {phase} measurement is invalid")
        phase_values[phase] = row["wall_ms"]
    acceptance = expected.get("acceptance")
    if not isinstance(acceptance, dict):
        raise ValueError("preregistered trial lacks acceptance")
    success = case_outcomes(acceptance.get("success_witnesses"), observed["success_witnesses"], "accepted", "success witness")
    expected_attacks = acceptance.get("rejected_law_gaming_attacks")
    if not isinstance(expected_attacks, dict) or not isinstance(observed["attacks"], dict) or set(observed["attacks"]) != set(expected_attacks):
        raise ValueError("raw trial attacks do not match preregistration")
    attacks = {
        name: case_outcomes(expected_attacks[name], observed["attacks"][name], "rejected", f"attack {name}")
        for name in sorted(expected_attacks)
    }
    return {
        "id": expected["id"],
        "language": expected["language"],
        "status": "accepted",
        "transcript_sha256": observed["transcript_sha256"],
        "telemetry": {"token_usage": events[0], "cost_usage": events[1]},
        "phase_wall_ms": phase_values,
        "success_evidence_sha256": success,
        "attack_evidence_sha256": attacks,
    }


def capture(plan_path: pathlib.Path, raw_path: pathlib.Path) -> dict:
    plan = read_object(plan_path, PLAN_SCHEMA)
    expected = admitted_trials(plan)
    raw = read_object(raw_path, RAW_SCHEMA)
    if set(raw) != {"schema", "plan_sha256", "exporter", "trials"} or raw["plan_sha256"] != digest(plan_path):
        raise ValueError("raw export is not bound to this exact preregistration")
    if not isinstance(raw["exporter"], dict) or set(raw["exporter"]) != {"kind", "exported_at"} or raw["exporter"]["kind"] != "existing_agent_telemetry_export" or not isinstance(raw["exporter"]["exported_at"], str):
        raise ValueError("raw export lacks existing telemetry exporter provenance")
    if not isinstance(raw["trials"], list):
        raise ValueError("raw export trials are invalid")
    observed = {}
    for trial in raw["trials"]:
        if not isinstance(trial, dict) or not isinstance(trial.get("id"), str) or trial["id"] in observed:
            raise ValueError("raw export has duplicate or invalid trial identity")
        if trial["id"] not in expected:
            raise ValueError("raw export trial is absent from preregistration")
        observed[trial["id"]] = validate_trial(expected[trial["id"]], trial)
    counts = Counter(row["language"] for row in observed.values())
    expected_counts = Counter(row["language"] for row in expected.values())
    missing = sorted(set(expected) - set(observed))
    status = "completed" if not missing else "partial"
    return {
        "schema": SCHEMA,
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "status": status,
        "plan": {"path": str(plan_path.resolve()), "sha256": digest(plan_path)},
        "raw_export": {"path": str(raw_path.resolve()), "sha256": digest(raw_path), "exporter": raw["exporter"]},
        "trial_counts": {language: {"captured": counts[language], "required": expected_counts[language]} for language in sorted(expected_counts)},
        "captured_trials": [observed[key] for key in sorted(observed)],
        "missing_trial_ids": missing,
        "nonclaims": [
            "a digest binds retained raw exports and transcripts but does not independently authenticate an external telemetry provider",
            "partial capture is not an agent-trial result",
            "no comparative or superiority result",
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", required=True, type=pathlib.Path)
    parser.add_argument("--raw-export", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        document = capture(args.plan, args.raw_export)
    except ValueError as error:
        parser.error(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(canonical(document))
    return 0 if document["status"] == "completed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
