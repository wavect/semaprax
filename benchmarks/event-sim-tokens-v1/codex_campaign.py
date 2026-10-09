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
import os
import shutil
import subprocess
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
HARNESS_SOURCE_FILES = (*codex.HARNESS_SOURCE_FILES,
    "benchmarks/event-sim-tokens-v1/SPEC.md",
    "benchmarks/event-sim-tokens-v1/acceptance/corpus.json",
    "benchmarks/event-sim-tokens-v1/acceptance/run.py",
    "benchmarks/event-sim-tokens-v1/campaign.py",
    "benchmarks/event-sim-tokens-v1/codex_campaign.py",
    "benchmarks/event-sim-tokens-v1/codex_report.py",
    "benchmarks/event-sim-tokens-v1/corpus_io.py",
    "benchmarks/event-sim-tokens-v1/oracle.py",
)



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
    if args.max_budget_usd is not None:
        raise ValueError("Codex does not supply a strict per-attempt monetary cap; --max-budget-usd is unsupported")
    # Reuse ShiftSim's frozen-input and qualification validator, whose legacy
    # model check belongs to the Claude adapter rather than this Codex arm.
    inherited = argparse.Namespace(**vars(args))
    inherited.model, inherited.effort = shiftsim.MODEL, shiftsim.EFFORT
    base = shiftsim.plan(inherited, shiftsim.common.digest(binary))
    qualification = base["qualification"]
    if qualification.get("scored_trials_allowed") is not True:
        raise ValueError("qualification evidence does not admit scored ShiftSim trials")
    compiler_commit = shiftsim.resolve_commit(Path(args.repo).resolve(), args.compiler_source_ref)
    if compiler_commit != qualification["compiler_source_commit"]:
        raise ValueError("compiler-source-ref differs from the qualified compiler source commit")
    base.update({
        "schema": shiftsim.AUTHORING_PROFILES[base["authoring_profile"]]["codex_campaign_schema"],
        "adapter": "codex-matched-shiftsim-v2" if base["round"] == 3 else "codex-matched-shiftsim-v1",
        "resource_policy": codex.resources.policy(),
        "harness_source_files_sha256": codex.harness_source_inventory(REPO, HARNESS_SOURCE_FILES),
        "model_requested": MODEL,
        "effort_requested": EFFORT,
        "model": MODEL,
        "effort": EFFORT,
        "codex_binary": args.codex_binary,
        "codex_version": subprocess.run([args.codex_binary, "--version"], capture_output=True,
                                         text=True, check=True).stdout.strip(),
        "price_book": {"date": "2026-10-08", "source": "https://developers.openai.com/api/docs/models/gpt-6.1-sol",
                       "standard_short_context_usd_per_million": dict(codex.PRICE_USD_PER_MTOK),
                       "conditional": True, "actual_billed_usd": None},
        "compiler_source_commit": compiler_commit,
        "source_binary_sha256": shiftsim.common.digest(binary),
        "codex_capabilities": codex.capabilities(args.codex_binary),
        "codex_execution": {
            "command_configuration": command_for({"codex_binary": args.codex_binary}, "<benchmark prompt>")[:-1],
            "usage": "task-owned rollout request records must reconcile final turn.completed usage",
            "stable_context_tokens": None,
            "calibration": "one separate empty-task request; never subtract from trial usage",
        },
    })
    if base["codex_capabilities"]["status"] != "ready":
        raise ValueError("installed Codex CLI lacks required isolation controls")
    return base


def command_for(settings: dict[str, Any], prompt: str) -> list[str]:
    command = codex.codex_command(prompt, settings.get("model", MODEL), settings.get("effort", EFFORT))
    command[0] = settings["codex_binary"]
    return command


def workspace_guard(workspace: Path, settings: dict[str, Any]) -> dict[str, Any]:
    """Recheck immutable SPEC and the candidate-only write boundary at each phase."""
    candidate = Path("benchmarks/event-sim-tokens-v1/candidate")
    public = Path(shiftsim.SPEC_RELATIVE)
    allowed_dirs = set(public.parents) | set(candidate.parents)
    expected = settings.get("seed_files_sha256", {}).get(public.as_posix())
    unexpected = []
    try:
        spec_path = workspace / public
        spec_ok = (expected is not None and not spec_path.is_symlink() and spec_path.is_file()
                   and shiftsim.common.digest(spec_path) == expected)
        for path in workspace.rglob("*"):
            relative = path.relative_to(workspace)
            if relative.parts[0] == ".git":
                continue
            if relative == candidate:
                if path.is_symlink() or not path.is_dir():
                    unexpected.append(relative.as_posix())
            elif candidate in relative.parents:
                continue
            elif relative == public:
                continue
            elif relative in allowed_dirs and path.is_dir() and not path.is_symlink():
                continue
            else:
                unexpected.append(relative.as_posix())
        return {"status": "passed" if spec_ok and not unexpected else "failed",
                "spec_integrity": "passed" if spec_ok else "failed", "outside_candidate_paths": sorted(unexpected)}
    except OSError as error:
        return {"status": "failed", "error": str(error)}


def cleanup_workspace(seed_repo: Path, workspace: Path, settings: dict[str, Any], row: dict[str, Any]) -> None:
    guard = workspace_guard(workspace, settings)
    row["workspace_integrity_before_cleanup"] = guard
    if guard["status"] != "passed":
        invalidate(row, "workspace integrity failed before cleanup")
        return
    removed = subprocess.run(["git", "worktree", "remove", "--force", str(workspace)],
                             cwd=seed_repo, capture_output=True, text=True, check=False)
    row["worktree_removed_after_archive"] = removed.returncode == 0
    if removed.returncode:
        row["workspace_retained_for_review"] = True
        row["worktree_cleanup_error"] = shiftsim.common.bounded_text(removed.stderr)


def invalidate(row: dict[str, Any], reason: str) -> None:
    row.update({"status": "not_accepted", "failure": reason, "workspace_retained_for_review": True,
                "worktree_removed_after_archive": False})
    if isinstance(row.get("acceptance"), dict):
        row["acceptance"]["accepted"] = False
        row["acceptance"]["invalidated"] = reason


def observe(workspace: Path, artifacts: Path, label: str, stream: Path) -> dict[str, Any]:
    usage = codex.parse_exec_jsonl(stream)
    copied = codex.copy_task_rollout(usage["thread_ids"], workspace,
                                     artifacts / "transcripts" / f"{label}.rollout.jsonl")
    trace = codex.trace_usage(usage, copied) if copied else {"reconciled": False, "model_requests": []}
    return {"rollout_trace": str(copied) if copied else None,
            "observed": {**usage, **trace, "stable_context_tokens": None},
            "list_price": codex.list_price_estimate(trace.get("model_requests", [])),
            "provider_receipt_actual_usd": None}


@codex.resources.guarded_attempt
def launch_trial(seed_repo: Path, artifacts: Path, seed_commit: str, trial: dict[str, Any],
                 settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    """Persist one attempt, reconcile exact task telemetry, and recheck each boundary."""
    shiftsim.common.require_compiler_binding(settings, semaprax_bin)
    label = f"{trial['arm']}-{trial['number']:02d}"
    workspace = artifacts / "worktrees" / label
    row: dict[str, Any] = {**trial, "workspace": str(workspace), "status": "failed", "failure": None,
                           "qualification_mode": "evidence_gated_scored"}
    error = shiftsim.common.add_seed_worktree(seed_repo, workspace, seed_commit, shiftsim.SEED_FILES)
    if error:
        row.update({"failure": error, "runner_error": True, "workspace_retained_for_review": True})
        return row
    candidate = workspace / "benchmarks/event-sim-tokens-v1/candidate"
    tooling = settings.get("typescript_bootstrap") if trial["arm"] == "typescript" else None
    env = shiftsim.trial_environment(semaprax_bin)
    receipt = None
    if tooling:
        setup_started = time.monotonic()
        try:
            receipt = shiftsim.ts_bootstrap.verify_plan(tooling)
            row["supplied_tooling"] = shiftsim.ts_bootstrap.stage(Path(tooling["receipt_path"]), candidate,
                                                                  tooling["node_binary"], tooling["npm_binary"])
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            row.update({"failure": f"TypeScript bootstrap refused before prompt: {error}",
                        "typescript_setup": {"status": "failed", "elapsed_seconds": round(time.monotonic() - setup_started, 3),
                                              "context_tokens": None},
                        "workspace_retained_for_review": True})
            return row
        row["typescript_setup"] = {"status": "ready", "elapsed_seconds": round(time.monotonic() - setup_started, 3),
                                   "context_tokens": None}
        node_path = Path(shutil.which(tooling["node_binary"]) or tooling["node_binary"]).resolve(strict=True)
        env["PATH"] = os.pathsep.join((str(candidate / "node_modules/.bin"), str(node_path.parent), env.get("PATH", "")))
    prompt = shiftsim.prompt_for(trial["arm"], candidate, semaprax_bin,
                                 settings.get("authoring_profile", shiftsim.AUTHORING_PROFILE_V24))
    if tooling:
        prompt += "\n\n" + shiftsim.ts_bootstrap.prompt_note(receipt)
    (artifacts / "prompts").mkdir(exist_ok=True)
    (artifacts / "prompts" / f"{label}.txt").write_text(prompt, encoding="utf-8")
    row["prompt_sha256"] = shiftsim.sha_text(prompt)
    stream, stderr = (artifacts / "transcripts" / f"{label}.{suffix}" for suffix in ("jsonl", "stderr.txt"))
    stream.parent.mkdir(exist_ok=True)
    row.update({"transcript": str(stream), "stderr_path": str(stderr)})
    try:
        shiftsim.common.require_compiler_binding(settings, semaprax_bin)
        row.update(codex.run_codex(command_for(settings, prompt), workspace,
                                   env, stream, stderr, settings["timeout_seconds"]))
        row.update(observe(workspace, artifacts, label, stream))
        observed = row["observed"]
        row["telemetry_valid"] = (observed.get("reconciled") is True
            and observed.get("model_observed") == MODEL and observed.get("effort_observed") == EFFORT
            and observed.get("invalid_stream_lines") == 0)
        guard = workspace_guard(workspace, settings)
        row["workspace_integrity_before_acceptance"] = guard
        if tooling:
            intact, evidence = shiftsim.ts_bootstrap.verify_staged(
                candidate, receipt["inventory"], evidence_path=artifacts / "dependency-evidence" / label)
            row["dependency_tree_integrity"] = evidence
            if not intact:
                invalidate(row, "staged TypeScript dependency tree changed")
                row["dependency_tree_integrity"] = evidence
                row["final_candidate_source_metrics"] = {"status": "measurement_failed", "total_tokens": None,
                    "files": [], "tokenizer": settings.get("authored_source_tokenizer")}
                return row
        if settings.get("authoring_profile") == shiftsim.AUTHORING_PROFILE_V27:
            try:
                row["closed_authored_inventory_after_model"] = shiftsim.closed_authored_inventory(
                    candidate, exclude_verified_node_modules=bool(tooling))
            except (OSError, ValueError) as error:
                invalidate(row, str(error))
                return row
        if row["timed_out"]:
            row["failure"] = "trial hit wall-clock timeout"
        elif row["process_exit_code"] != 0:
            row["failure"] = f"Codex exited with {row['process_exit_code']}"
        elif not observed.get("reconciled"):
            row["failure"] = "missing or unreconciled task-owned rollout usage"
        elif observed.get("model_observed") != MODEL or observed.get("effort_observed") != EFFORT:
            row["failure"] = "observed rollout model or effort differs from pinned request"
        elif not row["telemetry_valid"]:
            row["failure"] = "invalid Codex event stream"
        elif guard["status"] != "passed":
            invalidate(row, "workspace integrity failed before acceptance")
        else:
            admission = shiftsim.candidate_authoring_admission(
                candidate, trial["arm"], settings.get("authoring_profile", shiftsim.AUTHORING_PROFILE_V24))
            row["authoring_admission"] = admission
            if admission["status"] == "failed":
                row.update({"status": "not_accepted",
                            "failure": "candidate failed exact authoring-profile admission"})
            else:
                started = time.monotonic()
                if settings.get("authoring_profile") == shiftsim.AUTHORING_PROFILE_V27:
                    row["acceptance"] = shiftsim.check_program(candidate, settings["timeout_seconds"],
                        env, "evidence_gated_scored",
                        shiftsim.AUTHORING_PROFILE_V27,
                        artifacts / "harness-native" / label / "shiftsim",
                        settings["qualification"].get("compiler_binary_sha256"),
                        row.get("closed_authored_inventory_after_model"), trial["arm"],
                        exclude_verified_node_modules=bool(tooling))
                else:
                    row["acceptance"] = shiftsim.check_program(candidate, settings["timeout_seconds"],
                        env, "evidence_gated_scored", exclude_verified_node_modules=bool(tooling))
                row["acceptance_elapsed_seconds"] = round(time.monotonic() - started, 3)
                row["status"] = "accepted" if row["acceptance"]["accepted"] else "not_accepted"
                if row["status"] != "accepted":
                    row["failure"] = "candidate failed build or acceptance checks"
    except (OSError, RuntimeError, ValueError, UnicodeError) as error:
        row.update({"failure": str(error), "runner_error": True, "status": "failed"})
    try:
        if tooling:
            intact, evidence = shiftsim.ts_bootstrap.verify_staged(
                candidate, receipt["inventory"], evidence_path=artifacts / "dependency-evidence" / f"{label}-before-metrics")
            row["dependency_tree_integrity_before_metrics"] = evidence
            if not intact:
                invalidate(row, "staged TypeScript dependency tree changed before source measurement")
                row["dependency_tree_integrity_before_metrics"] = evidence
                return row
        row["final_candidate_source_metrics"] = shiftsim.common.authored_source_metrics(
            candidate, settings.get("authored_source_tokenizer"),
            exclude_verified_node_modules=bool(tooling))
    except (OSError, RuntimeError, ValueError, UnicodeError) as error:
        row["final_candidate_source_metrics"] = {"status": "measurement_failed", "total_tokens": None,
            "files": [], "tokenizer": settings.get("authored_source_tokenizer"), "error": str(error)}
    if settings.get("authoring_profile") == shiftsim.AUTHORING_PROFILE_V27:
        acceptance = row.get("acceptance", {})
        native = acceptance.get("native_binary", {})
        native_path = Path(native["path"]) if native.get("path") else None
        pinned = acceptance.get("pinned_compiler", {})
        compiler_path = Path(pinned["path"]) if pinned.get("path") else None
        expected_inventory = acceptance.get(
            "closed_authored_inventory", row.get("closed_authored_inventory_after_model"))
        passed, phase_guard = shiftsim._phase_source_and_binary_guard(
            candidate, expected_inventory, native_path, native.get("sha256"),
            compiler_path, pinned.get("sha256"), exclude_verified_node_modules=bool(tooling))
        row["source_consistency_after_metrics"] = phase_guard
        if not passed:
            invalidate(row, "candidate source, compiler, or harness native binary changed")
            return row
    guard = workspace_guard(workspace, settings)
    row["workspace_integrity_before_archive"] = guard
    if guard["status"] != "passed":
        invalidate(row, "workspace integrity failed before archive")
        return row
    if tooling:
        intact, evidence = shiftsim.ts_bootstrap.verify_staged(
            candidate, receipt["inventory"], evidence_path=artifacts / "dependency-evidence" / f"{label}-before-archive")
        row["dependency_tree_integrity_before_archive"] = evidence
        if not intact:
            invalidate(row, "staged TypeScript dependency tree changed before archive")
            row["dependency_tree_integrity_before_archive"] = evidence
            row["final_candidate_source_metrics"] = {"status": "measurement_failed", "total_tokens": None,
                "files": [], "tokenizer": settings.get("authored_source_tokenizer")}
            return row
    archive = artifacts / "candidates" / label
    archive.parent.mkdir(parents=True, exist_ok=True)
    try:
        hashes, omitted = shiftsim.common.archive_candidate(
            candidate, archive, exclude_verified_node_modules=bool(tooling))
        row.update({"candidate_archive": str(archive), "candidate_source_sha256": hashes,
                    "candidate_archive_excluded_paths": omitted})
    except (OSError, RuntimeError, ValueError) as error:
        row.update({"runner_error": True, "workspace_retained_for_review": True,
                    "failure": f"candidate archive failed: {error}", "status": "failed"})
        if isinstance(row.get("acceptance"), dict):
            row["acceptance"]["accepted"] = False
        return row
    if settings.get("authoring_profile") == shiftsim.AUTHORING_PROFILE_V27:
        acceptance = row.get("acceptance", {})
        native = acceptance.get("native_binary", {})
        native_path = Path(native["path"]) if native.get("path") else None
        pinned = acceptance.get("pinned_compiler", {})
        compiler_path = Path(pinned["path"]) if pinned.get("path") else None
        expected_inventory = acceptance.get(
            "closed_authored_inventory", row.get("closed_authored_inventory_after_model"))
        passed, phase_guard = shiftsim._phase_source_and_binary_guard(
            candidate, expected_inventory, native_path, native.get("sha256"),
            compiler_path, pinned.get("sha256"), exclude_verified_node_modules=bool(tooling))
        try:
            archived_inventory = shiftsim.closed_authored_inventory(archive)
            archive_matches = archived_inventory == expected_inventory
        except (OSError, ValueError) as error:
            archived_inventory = {"status": "failed", "error": str(error)}
            archive_matches = False
        phase_guard["archived_inventory"] = archived_inventory
        phase_guard["archive_matches"] = archive_matches
        passed = passed and archive_matches
        row["source_consistency_after_archive"] = phase_guard
        if not passed:
            invalidate(row, "candidate source, compiler, or harness native binary changed")
            return row
    cleanup_workspace(seed_repo, workspace, settings, row)
    return row


@codex.resources.guarded_attempt
def launch_calibration(seed_repo: Path, artifacts: Path, seed_commit: str,
                       settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    """A separate tool-free READY request, with the same isolation and cleanup."""
    shiftsim.common.require_compiler_binding(settings, semaprax_bin)
    workspace = artifacts / "worktrees" / "calibration"
    row: dict[str, Any] = {"status": "failed", "failure": None, "separate_from_trials": True,
                           "subtracted_from_trials": False}
    error = shiftsim.common.add_seed_worktree(seed_repo, workspace, seed_commit, shiftsim.SEED_FILES)
    if error:
        row["failure"] = error
        return row
    stream, stderr = (artifacts / "transcripts" / f"calibration.{suffix}" for suffix in ("jsonl", "stderr.txt"))
    stream.parent.mkdir(parents=True, exist_ok=True)
    row.update({"transcript": str(stream), "stderr_path": str(stderr)})
    try:
        shiftsim.common.require_compiler_binding(settings, semaprax_bin)
        row.update(codex.run_codex(command_for(settings, shiftsim.CALIBRATION_PROMPT), workspace,
                                   shiftsim.trial_environment(semaprax_bin), stream, stderr, settings["timeout_seconds"]))
        row.update(observe(workspace, artifacts, "calibration", stream))
        observed = row["observed"]
        events = [json.loads(line) for line in stream.read_text().splitlines() if line]
        messages = [event.get("item") for event in events if isinstance(event, dict)]
        replies = [item.get("text", "").strip() for item in messages
                   if isinstance(item, dict) and item.get("type") == "agent_message"]
        tools = observed.get("tool_item_counts", {})
        row["status"] = "ready" if (not row["timed_out"] and row["process_exit_code"] == 0
            and observed.get("reconciled") and observed.get("model_observed") == MODEL
            and observed.get("effort_observed") == EFFORT and replies == ["READY"]
            and observed.get("invalid_stream_lines") == 0
            and all(kind == "agent_message" or not count for kind, count in tools.items())) else "failed"
        if row["status"] == "failed":
            row["failure"] = "calibration failed tool-free READY, model/effort, or telemetry checks"
    except (OSError, RuntimeError, ValueError, UnicodeError) as error:
        row["failure"] = str(error)
    cleanup_workspace(seed_repo, workspace, settings, row)
    return row


def run_campaign(args: argparse.Namespace) -> dict[str, Any]:
    settings = plan(args)
    shiftsim.common.require_compiler_binding(settings, Path(args.semaprax_bin))
    shiftsim.ts_bootstrap.verify_plan(settings.get("typescript_bootstrap"))
    artifacts, binary = Path(settings["artifacts"]), Path(args.semaprax_bin).expanduser().resolve()
    artifacts.mkdir(parents=True)
    snapshot = codex.snapshot_harness_sources(REPO, artifacts, settings["harness_source_files_sha256"],
        settings["seed_files_sha256"]["benchmarks/event-sim-tokens-v1/SPEC.md"],
        relative_files=HARNESS_SOURCE_FILES, spec_path="benchmarks/event-sim-tokens-v1/SPEC.md")
    settings["harness_source_snapshot"] = snapshot
    qualification = settings["qualification"]
    shiftsim.copy_qualification_artifacts(qualification, artifacts)
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
            (artifacts / "attempts").mkdir(exist_ok=True)
            (artifacts / "attempts" / f"{arm}-{numbers[arm]:02d}.json").write_text(
                json.dumps(row, indent=2, sort_keys=True) + "\n")
            # A nonzero Codex exit can include quota exhaustion; do not retry or launch later paid attempts.
            if (row.get("resource_assessment", {}).get("contaminated")
                    or row.get("runner_error") or row.get("workspace_retained_for_review")
                    or (not row.get("timed_out") and (
                        row.get("process_exit_code") not in (0, None)
                        or row.get("telemetry_valid") is False))):
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
        p.add_argument("--round", type=int, choices=(1, 2, 3), default=2)
        p.add_argument("--authoring-profile", choices=tuple(shiftsim.AUTHORING_PROFILES), default=None)
        p.add_argument("--trials-per-arm", type=int, default=5)
        p.add_argument("--model", default=MODEL); p.add_argument("--effort", default=EFFORT)
        p.add_argument("--timeout-seconds", type=int, default=TIMEOUT_SECONDS); p.add_argument("--max-budget-usd", type=float, default=None)
        p.add_argument("--tokenizer-dir", default=None); p.add_argument("--codex-binary", default="codex")
        p.add_argument("--typescript-bootstrap-receipt", default=None)
        p.add_argument("--node-binary", default="node"); p.add_argument("--npm-binary", default="npm")
        if action == "run": p.add_argument("--acknowledge-paid-attempts", action="store_true")
    args = parser.parse_args()
    try:
        result = codex.capabilities(args.codex_binary) if args.action == "capabilities" else plan(args)
        if args.action == "run":
            if not args.acknowledge_paid_attempts: raise ValueError("run requires --acknowledge-paid-attempts")
            result = run_campaign(args)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if result.get("campaign_status", result.get("status", "ready")) in {"ready", "complete"} else 2
    except (OSError, ValueError, RuntimeError) as error:
        print(f"Codex ShiftSim campaign error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
