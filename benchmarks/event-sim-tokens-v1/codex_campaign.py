#!/usr/bin/env python3
"""No-model planning adapter for the matched Codex ShiftSim campaign.

Execution is deliberately delegated to the existing ShiftSim campaign callbacks;
this module binds Codex-specific configuration/accounting without duplicating
the frozen-task, qualification, or acceptance contract.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import sys
import time
from pathlib import Path
from typing import Any

BENCHMARK = Path(__file__).resolve().parent
REPO = BENCHMARK.parents[1]
CLI_ADAPTER = REPO / "benchmarks" / "cli-tokens-v1" / "codex_campaign.py"

sys.path.insert(0, str(CLI_ADAPTER.parent))
spec = importlib.util.spec_from_file_location("loglens_codex_campaign", CLI_ADAPTER)
if spec is None or spec.loader is None:
    raise RuntimeError("cannot load shared Codex CLI accounting adapter")
codex = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = codex
spec.loader.exec_module(codex)

sys.path.insert(0, str(BENCHMARK))
import campaign as shiftsim

MODEL, EFFORT, TIMEOUT_SECONDS = codex.MODEL, codex.EFFORT, codex.TIMEOUT_SECONDS
ARMS, MIN_TRIALS_PER_ARM = shiftsim.ARMS, shiftsim.MIN_TRIALS_PER_ARM


def plan(args: argparse.Namespace) -> dict[str, Any]:
    """Bind every scored campaign to qualification and frozen ShiftSim bytes."""
    binary = Path(args.semaprax_bin).expanduser().resolve(strict=True)
    if not binary.is_file():
        raise ValueError("semaprax-bin must be a regular file")
    if args.model != MODEL or args.effort != EFFORT or args.timeout_seconds != TIMEOUT_SECONDS:
        raise ValueError("matched Codex ShiftSim pins gpt-6.1-sol/medium and 1800 seconds")
    if args.trials_per_arm < MIN_TRIALS_PER_ARM:
        raise ValueError("scored ShiftSim requires at least five trials per arm")
    if not args.qualification_evidence:
        raise ValueError("scored ShiftSim requires --qualification-evidence")
    # Reuse ShiftSim's frozen-input and qualification validator, whose legacy
    # model check belongs to the Claude adapter rather than this Codex arm.
    inherited = argparse.Namespace(**vars(args))
    inherited.model, inherited.effort = shiftsim.MODEL, shiftsim.EFFORT
    base = shiftsim.plan(inherited, shiftsim.common.digest(binary))
    qualification = base["qualification"]
    if qualification.get("scored_trials_allowed") is not True:
        raise ValueError("qualification evidence does not admit scored ShiftSim trials")
    base.update({
        "schema": "semaprax.event-sim-codex-campaign.v1",
        "adapter": "codex-matched-shiftsim-v1",
        "model_requested": MODEL,
        "effort_requested": EFFORT,
        "compiler_source_commit": shiftsim.resolve_commit(Path(args.repo).resolve(), args.compiler_source_ref),
        "source_binary_sha256": shiftsim.common.digest(binary),
        "codex_capabilities": codex.capabilities(args.codex_binary),
        "codex_execution": {
            "command_configuration": codex.codex_command("<benchmark prompt>")[:-1],
            "usage": "task-owned rollout request records must reconcile final turn.completed usage",
            "stable_context_tokens": None,
            "calibration": "one separate empty-task request; never subtract from trial usage",
        },
    })
    if base["codex_capabilities"]["status"] != "ready":
        raise ValueError("installed Codex CLI lacks required isolation controls")
    return base


def launch_trial(seed_repo: Path, artifacts: Path, seed_commit: str, trial: dict[str, Any],
                 settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    """ShiftSim callback around shared Codex process/trace accounting helpers."""
    arm, number = trial["arm"], trial["number"]
    label, workspace = f"{arm}-{number:02d}", artifacts / "worktrees" / f"{arm}-{number:02d}"
    error = shiftsim.common.add_seed_worktree(seed_repo, workspace, seed_commit, shiftsim.SEED_FILES)
    row: dict[str, Any] = {**trial, "workspace": str(workspace), "status": "failed", "failure": error,
                           "qualification_mode": "evidence_gated_scored"}
    if error: return row
    candidate = workspace / "benchmarks/event-sim-tokens-v1/candidate"; candidate.mkdir(parents=True)
    prompt = shiftsim.prompt_for(arm, candidate, semaprax_bin)
    (artifacts / "prompts").mkdir(exist_ok=True); (artifacts / "prompts" / f"{label}.txt").write_text(prompt)
    transcript, stderr = artifacts / "transcripts" / f"{label}.jsonl", artifacts / "transcripts" / f"{label}.stderr.txt"
    transcript.parent.mkdir(exist_ok=True)
    row.update(codex.run_codex(codex.codex_command(prompt), workspace, shiftsim.trial_environment(semaprax_bin), transcript, stderr, settings["timeout_seconds"]))
    row["prompt_sha256"] = shiftsim.sha_text(prompt)
    usage = codex.parse_exec_jsonl(transcript); trace = codex.copy_task_rollout(usage["thread_ids"], workspace, artifacts / "transcripts" / f"{label}.rollout.jsonl")
    observed = codex.trace_usage(usage, trace) if trace else {"reconciled": False, "model_observed": None, "effort_observed": None, "model_requests": []}
    row.update({"transcript": str(transcript), "stderr_path": str(stderr), "rollout_trace": str(trace) if trace else None, "observed": {**usage, **observed, "stable_context_tokens": None},
                "list_price": codex.list_price_estimate(observed.get("model_requests", [])), "provider_receipt_actual_usd": None})
    if row["timed_out"]: row["failure"] = "trial hit wall-clock timeout"
    elif row["process_exit_code"] != 0: row["failure"] = f"Codex exited with {row['process_exit_code']}"
    elif not observed.get("reconciled"): row["failure"] = "missing or unreconciled task-owned rollout usage"
    elif observed.get("model_observed") != MODEL or observed.get("effort_observed") != EFFORT: row["failure"] = "observed rollout model or effort differs from pinned request"
    elif shiftsim.seeded_spec_integrity(workspace, settings)["status"] != "passed": row["failure"] = "trial changed frozen public SPEC"
    else:
        row["acceptance"] = shiftsim.check_program(candidate, settings["timeout_seconds"], shiftsim.trial_environment(semaprax_bin), "evidence_gated_scored")
        row["status"] = "accepted" if row["acceptance"]["accepted"] else "not_accepted"
        if row["status"] != "accepted": row["failure"] = "candidate failed build or acceptance checks"
    row["final_candidate_source_metrics"] = shiftsim.common.authored_source_metrics(candidate, settings.get("authored_source_tokenizer"))
    before_archive = shiftsim.seeded_spec_integrity(workspace, settings); row["seeded_spec_integrity_before_archive"] = before_archive
    if before_archive["status"] != "passed":
        row["status"] = "not_accepted"; row["failure"] = "trial changed frozen public SPEC"; row["workspace_retained_for_review"] = True
        return row
    hashes, omitted = shiftsim.common.archive_candidate(candidate, artifacts / "candidates" / label)
    row.update({"candidate_archive": str(artifacts / "candidates" / label), "candidate_source_sha256": hashes, "candidate_archive_excluded_paths": omitted})
    removed = __import__("subprocess").run(["git", "worktree", "remove", "--force", str(workspace)], cwd=seed_repo, capture_output=True, text=True, check=False)
    row["worktree_removed_after_archive"] = removed.returncode == 0
    return row


def launch_calibration(seed_repo: Path, artifacts: Path, seed_commit: str,
                       settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    """A separate, exact READY request that is never subtracted from trials."""
    workspace = artifacts / "worktrees" / "calibration"
    error = shiftsim.common.add_seed_worktree(seed_repo, workspace, seed_commit, shiftsim.SEED_FILES)
    row: dict[str, Any] = {"status": "failed", "failure": error, "separate_from_trials": True,
                           "subtracted_from_trials": False}
    if error: return row
    stream, stderr = artifacts / "calibration.jsonl", artifacts / "calibration.stderr.txt"
    row.update(codex.run_codex(codex.codex_command(shiftsim.CALIBRATION_PROMPT), workspace,
                               shiftsim.trial_environment(semaprax_bin), stream, stderr, settings["timeout_seconds"]))
    usage = codex.parse_exec_jsonl(stream); trace = codex.copy_task_rollout(usage["thread_ids"], workspace, artifacts / "calibration.rollout.jsonl")
    observed = codex.trace_usage(usage, trace) if trace else {"reconciled": False}
    row.update({"transcript": str(stream), "stderr_path": str(stderr), "observed": {**usage, **observed, "stable_context_tokens": None},
                "list_price": codex.list_price_estimate(observed.get("model_requests", [])), "provider_receipt_actual_usd": None})
    messages = [json.loads(line).get("item", {}) for line in stream.read_text().splitlines() if line]
    replies = [item.get("text", "").strip() for item in messages if item.get("type") == "agent_message"]
    tools = usage.get("tool_item_counts", {})
    row["status"] = "ready" if (row["process_exit_code"] == 0 and observed.get("reconciled")
                                  and observed.get("model_observed") == MODEL and observed.get("effort_observed") == EFFORT) else "failed"
    if replies != ["READY"] or any(kind != "agent_message" and count for kind, count in tools.items()): row["status"] = "failed"
    return row


def run_campaign(args: argparse.Namespace) -> dict[str, Any]:
    settings = plan(args)
    artifacts, binary = Path(settings["artifacts"]), Path(args.semaprax_bin).expanduser().resolve()
    artifacts.mkdir(parents=True)
    seed = shiftsim.common.create_seed_repository(Path(args.repo).resolve(), settings["repository_commit"],
                                                   artifacts / "seed-repository", shiftsim.SEED_FILES)
    settings.update(seed); settings["semaprax_binary"] = str(binary)
    (artifacts / "campaign.json").write_text(json.dumps(settings, indent=2, sort_keys=True) + "\n")
    calibration = launch_calibration(artifacts / "seed-repository", artifacts, seed["seed_repository_commit"], settings, binary)
    (artifacts / "calibration.json").write_text(json.dumps(calibration, indent=2, sort_keys=True) + "\n")
    rows: list[dict[str, Any]] = []; remaining = list(settings["trial_order"])
    if calibration["status"] == "ready":
        numbers = {arm: 0 for arm in ARMS}
        while remaining:
            arm = remaining.pop(0); numbers[arm] += 1
            row = launch_trial(artifacts / "seed-repository", artifacts, seed["seed_repository_commit"],
                               {"arm": arm, "number": numbers[arm]}, settings, binary)
            rows.append(row)
            # A nonzero Codex exit can include quota exhaustion; do not retry or launch later paid attempts.
            if row.get("process_exit_code") not in (0, None) or row.get("timed_out"):
                break
    report = {"campaign": settings, "calibration": calibration, "trials": rows,
              "attempt_denominator": settings["attempt_denominator"], "recorded_attempts": len(rows),
              "unlaunched_trial_order": remaining,
              "campaign_status": "complete" if not remaining and calibration["status"] == "ready" else "interrupted"}
    (artifacts / "results.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    caps = sub.add_parser("capabilities")
    caps.add_argument("--codex-binary", default="codex")
    for action in ("plan", "run"):
        p = sub.add_parser(action, help="validate or explicitly launch the evidence-gated campaign")
        p.add_argument("--repo", default=str(REPO)); p.add_argument("--base-ref", required=True)
        p.add_argument("--compiler-source-ref", required=True); p.add_argument("--semaprax-bin", required=True)
        p.add_argument("--qualification-evidence", required=True); p.add_argument("--artifacts", required=True)
        p.add_argument("--round", type=int, choices=(1, 2), default=2); p.add_argument("--trials-per-arm", type=int, default=5)
        p.add_argument("--model", default=MODEL); p.add_argument("--effort", default=EFFORT)
        p.add_argument("--timeout-seconds", type=int, default=TIMEOUT_SECONDS); p.add_argument("--max-budget-usd", type=float, default=None)
        p.add_argument("--tokenizer-dir", default=None); p.add_argument("--codex-binary", default="codex")
        if action == "run": p.add_argument("--acknowledge-paid-attempts", action="store_true")
    args = parser.parse_args()
    try:
        result = codex.capabilities(args.codex_binary) if args.action == "capabilities" else plan(args)
        if args.action == "run":
            if not args.acknowledge_paid_attempts: raise ValueError("run requires --acknowledge-paid-attempts")
            result = run_campaign(args)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if result.get("status", "ready") in {"ready", "complete"} else 2
    except (OSError, ValueError, RuntimeError) as error:
        print(f"Codex ShiftSim campaign error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
