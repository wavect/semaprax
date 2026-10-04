#!/usr/bin/env python3
"""Run one preregistered LAW-16 Boolean source/proof edit through ``codex exec``.

The runner intentionally gives the model no checkout.  It starts Codex in a
fresh empty directory with its read-only sandbox, retains its complete JSONL
event stream and stderr, and derives only the token counters Codex actually
reports.  Codex JSON events have no monetary charge field and one agent turn
does not separate proof synthesis from law-kernel or runtime time, so this
record is raw trial provenance, not an accepted LAW-16 comparison observation.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import pathlib
import subprocess
import tempfile
import time
from datetime import datetime, timezone


SCHEMA = "semaprax.bend2-law-benchmark.codex-agent-trial.v2"
EDIT_RESPONSE_SCHEMA = "semaprax.bend2-law-benchmark.boolean-edit-response.v1"
MAX_EVENT_BYTES = 32 * 1024 * 1024
TASK = "scalar-contract-bug-v1"


def _plan_module():
    path = pathlib.Path(__file__).with_name("agent_trial_plan.py")
    spec = importlib.util.spec_from_file_location("bend2_agent_trial_plan", path)
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    spec.loader.exec_module(module)
    return module


PLAN = _plan_module()


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def sha256(body: bytes) -> str:
    return "sha256:" + hashlib.sha256(body).hexdigest()


def artifact(path: pathlib.Path, root: pathlib.Path) -> dict:
    body = path.read_bytes()
    return {"path": path.relative_to(root).as_posix(), "sha256": sha256(body), "bytes": len(body)}


def selected_trial(plan: dict, trial_id: str) -> dict:
    if plan.get("schema") != PLAN.SCHEMA:
        raise ValueError("trial plan has unsupported schema")
    admitted = [trial for cell in plan.get("cells", []) if cell.get("status") == "preregistered"
               for trial in cell.get("trials", []) if trial.get("id") == trial_id]
    if len(admitted) != 1:
        raise ValueError("trial identity is not a unique preregistered trial")
    trial = admitted[0]
    if trial.get("status") != "not_run" or trial.get("execution", {}).get("repository_access") != "none":
        raise ValueError("trial is not admitted for the isolated Codex route")
    return trial


def usage(events: list[dict]) -> dict | None:
    """Keep Codex cache telemetry, without charging its input-token subset twice."""
    completed = [event for event in events if event.get("type") == "turn.completed"]
    if len(completed) != 1 or not isinstance(completed[0].get("usage"), dict):
        return None
    raw = completed[0]["usage"]
    fields = ("input_tokens", "cached_input_tokens", "output_tokens")
    if any(not isinstance(raw.get(field), int) or raw[field] < 0 for field in fields):
        return None
    # Codex reports cached input as the reusable subset of input_tokens.  The
    # fixed token budget charges the full input once plus generated output;
    # cached_input_tokens is retained as provenance for later cost analysis.
    return {field: raw[field] for field in fields} | {"total_tokens": raw["input_tokens"] + raw["output_tokens"]}


def edit_context(trial: dict) -> dict:
    if not trial["id"].startswith(TASK + ":"):
        raise ValueError("isolated Codex edit route admits only the pinned Boolean task")
    root = pathlib.Path(__file__).with_name("fixtures")
    if trial["language"] == "bend2":
        extension, attack, success = ".bend", "bend-two-value-boolean-law-gaming-v1.bend", "bend-two-value-boolean-v1.bend"
    elif trial["language"] == "semaprax-scalar-v1":
        extension, attack, success = ".spx", "semaprax-two-value-boolean-law-gaming-v1.spx", "semaprax-two-value-boolean-v1.spx"
    else:
        raise ValueError("isolated Codex edit route has an unsupported language")
    return {"extension": extension, "attack_source": (root / attack).read_text(), "success_source": (root / success).read_text()}


def prompt(trial: dict) -> str:
    context = edit_context(trial)
    request = {
        "task": TASK, "trial_id": trial["id"], "language": trial["language"], "laws": trial["acceptance"],
        "seeded_law_gaming_source": context["attack_source"], "required_success_case": trial["acceptance"]["success_witnesses"],
        "response_schema": EDIT_RESPONSE_SCHEMA,
        "response_fields": ["schema", "trial_id", "language", "final_source", "proof_explanation", "seeded_attack"],
        "seeded_attack_fields": ["source_sha256", "decision", "reason"],
    }
    return (
        "Perform the bounded Boolean law repair from the embedded seeded law-gaming source. Do not inspect files, "
        "repositories, network resources, credentials, or tools. Return only one compact JSON object with the requested "
        "schema. `final_source` must contain the complete repaired source, preserving the stated law and any Bend proof body. "
        "`seeded_attack.decision` must be `reject`. Your explanation is an agent claim, not proof execution.\n\n"
        + json.dumps(request, sort_keys=True)
    )


def final_response(events: list[dict]) -> str | None:
    messages = []
    for event in events:
        item = event.get("item") if event.get("type") == "item.completed" else None
        if isinstance(item, dict) and item.get("type") == "agent_message" and isinstance(item.get("text"), str):
            messages.append(item["text"])
    return messages[-1] if len(messages) == 1 else None


def edit_response(text: str, trial: dict, context: dict) -> dict:
    try:
        value = json.loads(text)
    except json.JSONDecodeError as error:
        raise ValueError("Codex final response is not one JSON edit artifact") from error
    if not isinstance(value, dict) or set(value) != {"schema", "trial_id", "language", "final_source", "proof_explanation", "seeded_attack"}:
        raise ValueError("Codex final response has unexpected edit artifact fields")
    if value["schema"] != EDIT_RESPONSE_SCHEMA or value["trial_id"] != trial["id"] or value["language"] != trial["language"]:
        raise ValueError("Codex final response is not bound to the selected trial")
    if not isinstance(value["final_source"], str) or not value["final_source"] or not isinstance(value["proof_explanation"], str) or not value["proof_explanation"]:
        raise ValueError("Codex final response lacks source or proof explanation")
    attack = value["seeded_attack"]
    if not isinstance(attack, dict) or set(attack) != {"source_sha256", "decision", "reason"}:
        raise ValueError("Codex final response lacks seeded-attack evidence")
    if attack["source_sha256"] != sha256(context["attack_source"].encode()) or attack["decision"] != "reject" or not isinstance(attack["reason"], str) or not attack["reason"]:
        raise ValueError("Codex seeded-attack evidence is not bound to the embedded control")
    return value


def run(plan_path: pathlib.Path, trial_id: str, evidence_dir: pathlib.Path, executable: str = "codex") -> dict:
    plan = PLAN.object_json(plan_path, PLAN.SCHEMA)
    trial = selected_trial(plan, trial_id)
    context = edit_context(trial)
    configuration = plan.get("agent_configuration", {}).get("value")
    if not isinstance(configuration, dict):
        raise ValueError("trial plan lacks agent configuration")
    PLAN.require_config(configuration)
    model = configuration["model"]
    execution = configuration["execution"]
    if model["provider"] != "openai-codex-cli":
        raise ValueError("isolated runner requires the openai-codex-cli provider")
    if evidence_dir.exists() or not evidence_dir.parent.is_dir():
        raise ValueError("evidence directory must be a new child of an existing directory")
    evidence_dir.mkdir()
    workspace = evidence_dir / "workspace"
    workspace.mkdir()
    events_path, stderr_path, version_path = (evidence_dir / name for name in ("events.jsonl", "stderr.txt", "codex-version.txt"))
    try:
        version = subprocess.run([executable, "--version"], capture_output=True, timeout=15, check=False)
        version_path.write_bytes(version.stdout + version.stderr)
        command = [executable, "exec", "--skip-git-repo-check", "--ephemeral", "--ignore-user-config",
                   "-s", "read-only", "-m", model["name"], "--json", prompt(trial)]
        started = time.monotonic_ns()
        completed = subprocess.run(command, cwd=workspace, capture_output=True,
                                   timeout=execution["max_wall_seconds"], check=False)
        elapsed_ms = round((time.monotonic_ns() - started) / 1_000_000, 3)
        if len(completed.stdout) > MAX_EVENT_BYTES or len(completed.stderr) > MAX_EVENT_BYTES:
            raise ValueError("Codex trial stream exceeds its byte bound")
        events_path.write_bytes(completed.stdout)
        stderr_path.write_bytes(completed.stderr)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ValueError("Codex trial could not be completed") from error
    try:
        events = [json.loads(line) for line in completed.stdout.splitlines() if line]
    except json.JSONDecodeError as error:
        raise ValueError("Codex trial did not emit valid JSONL events") from error
    observed_usage = usage(events)
    within_budget = observed_usage is not None and observed_usage["total_tokens"] <= configuration["fixed_budget"]["max_tokens"]
    response = None
    if completed.returncode == 0 and within_budget:
        response = edit_response(final_response(events) or "", trial, context)
        response_path = evidence_dir / "model-response.json"
        final_source_path = evidence_dir / ("final-source" + context["extension"])
        attack_path = evidence_dir / "seeded-attack-claim.json"
        response_path.write_text(canonical(response))
        final_source_path.write_text(response["final_source"])
        attack_path.write_text(canonical(response["seeded_attack"]))
    return {
        "schema": SCHEMA,
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "status": "edit_artifacts_captured" if response is not None else "ineligible",
        "plan": {"path": str(plan_path.resolve()), "sha256": PLAN.digest(plan_path)},
        "trial": {"id": trial["id"], "language": trial["language"]},
        "execution": {"command": command[:-1] + ["<preregistered-prompt>"], "sandbox": "read-only",
                      "working_directory": "fresh-empty-directory", "repository_access": "none",
                      "exit_code": completed.returncode, "wall_ms": elapsed_ms},
        "artifacts": {"events": artifact(events_path, evidence_dir), "stderr": artifact(stderr_path, evidence_dir),
                      "codex_version": artifact(version_path, evidence_dir)},
        "edit_artifacts": None if response is None else {"model_response": artifact(response_path, evidence_dir),
            "final_source": artifact(final_source_path, evidence_dir), "seeded_attack_claim": artifact(attack_path, evidence_dir)},
        "telemetry": {"token_usage": observed_usage, "within_fixed_token_budget": within_budget,
                      "cost_usage": {"status": "unavailable", "reason": "codex_exec_json_events_do_not_supply_monetary_usage"}},
        "phase_measurements": {phase: {"status": "unavailable", "reason": "one Codex turn does not independently execute this phase"}
                               for phase in PLAN.MEASUREMENT_PHASES},
        "nonclaims": ["captured edit bytes are not a successful law repair", "the seeded-attack claim is not independent rejection evidence", "this record is not a completed LAW-16 agent trial", "no monetary cost observation was available"],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", required=True, type=pathlib.Path)
    parser.add_argument("--trial", required=True)
    parser.add_argument("--evidence-dir", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--codex", default="codex")
    args = parser.parse_args(argv)
    try:
        document = run(args.plan, args.trial, args.evidence_dir, args.codex)
    except ValueError as error:
        parser.error(str(error))
    if args.output.exists() or not args.output.parent.is_dir():
        parser.error("output must be a new file beneath an existing directory")
    args.output.write_text(canonical(document))
    return 0 if document["status"] == "edit_artifacts_captured" else 1


if __name__ == "__main__":
    raise SystemExit(main())
