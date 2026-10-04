#!/usr/bin/env python3
"""Run one preregistered LAW-16 review trial through ``codex exec``.

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


SCHEMA = "semaprax.bend2-law-benchmark.codex-agent-trial.v1"
MAX_EVENT_BYTES = 32 * 1024 * 1024


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
    completed = [event for event in events if event.get("type") == "turn.completed"]
    if len(completed) != 1 or not isinstance(completed[0].get("usage"), dict):
        return None
    raw = completed[0]["usage"]
    fields = ("input_tokens", "cached_input_tokens", "output_tokens")
    if any(not isinstance(raw.get(field), int) or raw[field] < 0 for field in fields):
        return None
    return {field: raw[field] for field in fields} | {"total_tokens": sum(raw[field] for field in fields)}


def prompt(trial: dict) -> str:
    return (
        "You are participating in a LAW-16 evidence trial. Work only from this prompt; do not "
        "attempt to inspect files, repositories, network resources, credentials, or tools. "
        "State whether the following preregistered acceptance rule is logically complete, then "
        "give one concise reason. This is a review response, not a code edit or proof execution.\n\n"
        + json.dumps({"trial_id": trial["id"], "language": trial["language"], "acceptance": trial["acceptance"]}, sort_keys=True)
    )


def run(plan_path: pathlib.Path, trial_id: str, evidence_dir: pathlib.Path, executable: str = "codex") -> dict:
    plan = PLAN.object_json(plan_path, PLAN.SCHEMA)
    trial = selected_trial(plan, trial_id)
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
    return {
        "schema": SCHEMA,
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "status": "executed_unassessed" if completed.returncode == 0 and within_budget else "ineligible",
        "plan": {"path": str(plan_path.resolve()), "sha256": PLAN.digest(plan_path)},
        "trial": {"id": trial["id"], "language": trial["language"]},
        "execution": {"command": command[:-1] + ["<preregistered-prompt>"], "sandbox": "read-only",
                      "working_directory": "fresh-empty-directory", "repository_access": "none",
                      "exit_code": completed.returncode, "wall_ms": elapsed_ms},
        "artifacts": {"events": artifact(events_path, evidence_dir), "stderr": artifact(stderr_path, evidence_dir),
                      "codex_version": artifact(version_path, evidence_dir)},
        "telemetry": {"token_usage": observed_usage, "within_fixed_token_budget": within_budget,
                      "cost_usage": {"status": "unavailable", "reason": "codex_exec_json_events_do_not_supply_monetary_usage"}},
        "phase_measurements": {phase: {"status": "unavailable", "reason": "one Codex turn does not isolate this phase"}
                               for phase in PLAN.MEASUREMENT_PHASES},
        "nonclaims": ["this record does not assess acceptance witnesses or attacks", "this record is not a completed LAW-16 agent trial", "no monetary cost observation was available"],
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
    return 0 if document["status"] == "executed_unassessed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
