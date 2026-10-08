#!/usr/bin/env python3
"""Matched LogLens pilot for Codex CLI, with trace-backed token accounting.

This adapter deliberately does not reuse the Claude JSONL accounting: Codex's
outer CLI turn is not a count of model requests.  Per-request usage is taken
only from the task-owned rollout emitted by the local CLI and must reconcile
with the final ``turn.completed`` total before a trial can be accepted.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import signal
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import live_campaign as legacy

BENCHMARK = Path(__file__).resolve().parent
REPO = BENCHMARK.parents[1]
MODEL = "gpt-6.1-sol"
EFFORT = "medium"
TIMEOUT_SECONDS = 1800
ARMS = legacy.ARMS
MIN_TRIALS_PER_ARM = legacy.MIN_TRIALS_PER_ARM
SEED_FILES = legacy.SEED_FILES
ROUND = 5
PRICE_USD_PER_MTOK = {
    "input": 2.0, "cache_read": 0.1, "cache_write": 2.5, "output": 10.0,
}
SHORT_CONTEXT_LIMIT = 272_000
CALIBRATION_PROMPT = "This is a context calibration request. Reply with exactly READY; do not use tools or read files."


def _usage(value: Any) -> dict[str, int | None]:
    names = ("input_tokens", "cached_input_tokens", "cache_write_input_tokens", "output_tokens", "reasoning_output_tokens")
    if not isinstance(value, dict):
        return {name: None for name in names}
    return {
        name: value.get(name) if isinstance(value.get(name), int) and value[name] >= 0 else None
        for name in names
    }


def _same_usage(left: dict[str, int | None], right: dict[str, int | None]) -> bool:
    return all(left.get(name) == right.get(name) for name in left)


def codex_command(prompt: str, model: str = MODEL, effort: str = EFFORT) -> list[str]:
    """Use only explicit, symmetric configuration; do not replace HOME/CODEX_HOME."""
    return [
        "codex", "exec", "--json", "--model", model, "--sandbox", "workspace-write",
        "--ignore-user-config", "--ignore-rules",
        "--disable", "apps", "--disable", "plugins", "--disable", "memories",
        "--disable", "multi_agent", "--disable", "skill_search",
        "--config", f'model_reasoning_effort="{effort}"',
        "--config", "web_search='disabled'", "--", prompt,
    ]


def capabilities(binary: str = "codex") -> dict[str, Any]:
    """A no-model-call preflight of the installed CLI's advertised exec flags."""
    completed = subprocess.run([binary, "exec", "--help"], text=True, capture_output=True, check=False)
    required = ("--json", "--model", "--sandbox", "--ignore-user-config", "--ignore-rules", "--disable", "--config")
    help_text = completed.stdout + completed.stderr
    return {
        "status": "ready" if completed.returncode == 0 and all(flag in help_text for flag in required) else "unsupported",
        "binary": binary, "exit_code": completed.returncode,
        "required_flags": {flag: flag in help_text for flag in required},
        "configuration": {
            "user_config": "ignored", "rules": "ignored",
            "disabled_features": ["apps", "plugins", "memories", "multi_agent", "skill_search"],
            "web_search": "disabled", "home_overridden": False, "codex_home_overridden": False,
        },
    }


def parse_exec_jsonl(path: Path) -> dict[str, Any]:
    """Read the final outer-turn total without treating tool items as turns."""
    completed: list[dict[str, int | None]] = []
    tool_counts: dict[str, int] = {}
    invalid = 0
    thread_ids: list[str] = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        try:
            event = json.loads(raw)
        except json.JSONDecodeError:
            invalid += 1
            continue
        if not isinstance(event, dict):
            invalid += 1
            continue
        if event.get("type") == "turn.completed":
            completed.append(_usage(event.get("usage")))
        if event.get("type") == "item.completed":
            item = event.get("item")
            kind = item.get("type") if isinstance(item, dict) else None
            if isinstance(kind, str):
                tool_counts[kind] = tool_counts.get(kind, 0) + 1
        thread = event.get("thread_id") or event.get("threadId")
        if isinstance(thread, str) and thread not in thread_ids:
            thread_ids.append(thread)
    return {
        "final_turn_usage": completed[-1] if len(completed) == 1 else None,
        "turn_completed_events": len(completed),
        "codex_outer_turns": len(completed),
        "tool_item_counts": tool_counts,
        "thread_ids": thread_ids,
        "invalid_stream_lines": invalid,
        "model_observed": None,
        "model_identity_note": "The exec JSONL does not expose an observed model identity.",
        "stable_context_tokens": None,
    }


def _rollout_records(path: Path) -> tuple[list[dict[str, Any]], list[str], list[str], int | None]:
    requests: list[dict[str, Any]] = []
    models: list[str] = []
    efforts: list[str] = []
    context_window: int | None = None
    seen: set[str] = set()
    for raw in path.read_text(encoding="utf-8").splitlines():
        try:
            event = json.loads(raw)
        except json.JSONDecodeError:
            continue
        payload = event.get("payload") if isinstance(event, dict) else None
        if not isinstance(payload, dict):
            continue
        if event.get("type") == "turn_context":
            for value, sink in ((payload.get("model"), models), (payload.get("effort"), efforts)):
                if isinstance(value, str) and value not in sink:
                    sink.append(value)
        if event.get("type") == "token_usage_record":
            usage = _usage(payload.get("turn_token_usage"))
            identity = payload.get("response_id") or payload.get("turn_id")
            key = json.dumps([identity, usage], sort_keys=True)
            if identity is not None and key not in seen:
                seen.add(key)
                requests.append({"request_id": identity, "usage": usage})
        if event.get("type") == "event_msg" and payload.get("type") == "token_count":
            info = payload.get("info")
            if isinstance(info, dict) and isinstance(info.get("model_context_window"), int):
                context_window = info["model_context_window"]
    return requests, models, efforts, context_window


def trace_usage(exec_usage: dict[str, Any], rollout: Path) -> dict[str, Any]:
    requests, models, efforts, context_window = _rollout_records(rollout)
    final = exec_usage.get("final_turn_usage")
    summed: dict[str, int | None] = {}
    for name in _usage({}):
        values = [request["usage"][name] for request in requests]
        summed[name] = sum(values) if values and all(value is not None for value in values) else None
    reconciled = isinstance(final, dict) and _same_usage(final, summed)
    return {
        "model_requests": requests if reconciled else [],
        "model_request_count": len(requests) if reconciled else None,
        "model_observed": models[0] if reconciled and len(models) == 1 else None,
        "effort_observed": efforts[0] if reconciled and len(efforts) == 1 else None,
        "model_context_window": context_window,
        "request_usage_sum": summed,
        "final_turn_usage": final,
        "reconciled": reconciled,
        "reconciliation_note": "Per-request trace sum must equal final turn.completed usage; tool item counts are separate.",
    }


def list_price_estimate(requests: list[dict[str, Any]]) -> dict[str, Any]:
    """Conditional standard-price estimate; actual billing and cache-write amount remain unknown."""
    amount, writes = 0.0, 0
    valid = bool(requests)
    for request in requests:
        known = _usage(request.get("usage"))
        if not all(known[key] is not None for key in ("input_tokens", "cached_input_tokens", "cache_write_input_tokens", "output_tokens")):
            valid = False; break
        uncached = known["input_tokens"] - known["cached_input_tokens"] - known["cache_write_input_tokens"]
        if uncached < 0 or known["input_tokens"] > SHORT_CONTEXT_LIMIT:
            valid = False; break
        writes += known["cache_write_input_tokens"]
        amount += (uncached * PRICE_USD_PER_MTOK["input"] + known["cached_input_tokens"] * PRICE_USD_PER_MTOK["cache_read"] + known["cache_write_input_tokens"] * PRICE_USD_PER_MTOK["cache_write"] + known["output_tokens"] * PRICE_USD_PER_MTOK["output"]) / 1_000_000
    return {
        "standard_short_context_api_equivalent_usd": round(amount, 6) if valid else None,
        "assumptions": "each request input <=272k; input/cache-read/cache-write/output use published standard rates",
        "actual_billed_usd": None, "cache_write_tokens": writes if valid else None,
        "cache_write_cost_usd": round(writes * PRICE_USD_PER_MTOK["cache_write"] / 1_000_000, 6) if valid else None,
        "reason": None if valid else "missing per-request usage, invalid bucket subset, or a request exceeds the short-context threshold",
    }


def run_codex(command: list[str], workspace: Path, env: dict[str, str], stream: Path, stderr: Path, timeout: int) -> dict[str, Any]:
    """Retain both logs and terminate the process group on timeout; never retry quota failures."""
    started = time.monotonic()
    with stream.open("wb") as output, stderr.open("wb") as errors:
        process = subprocess.Popen(command, cwd=workspace, env=env, stdout=output, stderr=errors, start_new_session=True)
        timed_out = False
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGTERM)
            try:
                code = process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                code = process.wait()
    return {"process_exit_code": code, "timed_out": timed_out, "elapsed_seconds": round(time.monotonic() - started, 3)}


def plan(args: argparse.Namespace) -> dict[str, Any]:
    repo = Path(args.repo).resolve(strict=True)
    commit = legacy.resolve_commit(repo, args.base_ref)
    hashes = legacy.pinned_seed_hashes(repo, commit)
    legacy.validate_round_identity(ROUND, hashes)
    artifacts = Path(args.artifacts).expanduser().resolve()
    if artifacts.exists():
        raise ValueError("artifact path must not already exist")
    if args.model != MODEL or args.effort != EFFORT:
        raise ValueError("matched campaign pins gpt-6.1-sol at medium effort")
    if args.trials_per_arm < MIN_TRIALS_PER_ARM or args.timeout_seconds != TIMEOUT_SECONDS:
        raise ValueError("matched campaign requires five trials per arm and a 1800-second timeout")
    return {
        "adapter": "codex-matched-loglens-v1", "round": ROUND, "repository_commit": commit,
        "seed_files_sha256": hashes, "artifacts": str(artifacts), "model_requested": args.model,
        "effort_requested": args.effort, "timeout_seconds": args.timeout_seconds,
        "trial_order": [arm for _ in range(args.trials_per_arm) for arm in ARMS],
        "attempt_denominator": args.trials_per_arm * len(ARMS),
        "source_binary_sha256": legacy.digest(Path(args.semaprax_bin).resolve(strict=True)),
        "calibration": {"prompt": CALIBRATION_PROMPT, "separate": True, "subtracted_from_trials": False},
        "measurement": {"stable_context_tokens": None, "legacy_net_input_tokens": None,
                        "model_request_count": "trace-backed only; never inferred from item counts"},
        "capabilities": capabilities(args.codex_binary),
    }


def _rollout_snapshot() -> set[Path]:
    root = Path.home() / ".codex" / "sessions"
    return set(root.rglob("*.jsonl")) if root.is_dir() else set()


def copy_new_rollout(before: set[Path], destination: Path) -> Path | None:
    """Copy exactly one newly-created task trace, removing account identifiers."""
    created = sorted(_rollout_snapshot() - before, key=lambda path: path.stat().st_mtime)
    if len(created) != 1:
        return None
    def redact(value: Any) -> Any:
        if isinstance(value, dict):
            return {key: redact(item) for key, item in value.items()
                    if key.lower() not in {"creator_user", "account_id", "user_id", "email"}}
        if isinstance(value, list):
            return [redact(item) for item in value]
        return value
    rows = []
    for raw in created[0].read_text(encoding="utf-8").splitlines():
        try:
            rows.append(json.dumps(redact(json.loads(raw)), sort_keys=True))
        except json.JSONDecodeError:
            continue
    destination.write_text("\n".join(rows) + ("\n" if rows else ""), encoding="utf-8")
    return destination


def launch_trial(repo: Path, artifacts: Path, commit: str, trial: dict[str, Any], settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    """Run one paid attempt. Every attempted trial remains in the result denominator."""
    arm, number = trial["arm"], trial["number"]
    label = f"{arm}-{number:02d}"
    workspace = artifacts / "worktrees" / label
    row: dict[str, Any] = {**trial, "workspace": str(workspace), "status": "failed", "failure": None}
    error = legacy.add_seed_worktree(repo, workspace, commit)
    if error:
        row.update({"failure": error, "workspace_retained_for_review": True})
        return row
    candidate = workspace / "benchmarks" / "cli-tokens-v1" / "candidate"
    candidate.mkdir(parents=True, exist_ok=True)
    prompt = legacy.prompt_for(arm, candidate, semaprax_bin)
    (artifacts / "prompts").mkdir(parents=True, exist_ok=True)
    (artifacts / "prompts" / f"{label}.txt").write_text(prompt, encoding="utf-8")
    row["prompt_sha256"] = hashlib.sha256(prompt.encode()).hexdigest()
    transcript, stderr, trace = (artifacts / "transcripts" / f"{label}.{suffix}" for suffix in ("jsonl", "stderr.txt", "rollout.jsonl"))
    transcript.parent.mkdir(parents=True, exist_ok=True)
    before = _rollout_snapshot()
    process = run_codex(codex_command(prompt), workspace, legacy.trial_environment(semaprax_bin), transcript, stderr, settings["timeout_seconds"])
    row.update(process); row.update({"transcript": str(transcript), "stderr_path": str(stderr)})
    exec_usage = parse_exec_jsonl(transcript)
    copied = copy_new_rollout(before, trace)
    trace_result = trace_usage(exec_usage, copied) if copied else {"reconciled": False, "model_request_count": None, "model_observed": None}
    row["observed"] = {**exec_usage, **trace_result, "stable_context_tokens": None}
    row["rollout_trace"] = str(copied) if copied else None
    row["list_price"] = list_price_estimate(trace_result.get("model_requests", []))
    row["provider_receipt_actual_usd"] = None
    if process["timed_out"]:
        row["failure"] = "trial hit wall-clock timeout"
    elif process["process_exit_code"] != 0:
        row["failure"] = f"Codex exited with {process['process_exit_code']}"
    elif not trace_result.get("reconciled"):
        row["failure"] = "missing or unreconciled task-owned rollout usage"
    elif trace_result.get("model_observed") != MODEL or trace_result.get("effort_observed") != EFFORT:
        row["failure"] = "observed rollout model or effort differs from pinned request"
    else:
        guard, guard_error = legacy.trial_workspace_guard(workspace, settings)
        row["workspace_integrity_before_acceptance"] = guard
        if guard_error:
            legacy.invalidate_trial_acceptance(row, guard_error)
        else:
            row["acceptance"] = legacy.check_program(candidate, settings["timeout_seconds"], legacy.trial_environment(semaprax_bin))
            row["status"] = "accepted" if row["acceptance"]["accepted"] else "not_accepted"
            if row["status"] != "accepted": row["failure"] = "candidate failed independent acceptance"
    row["final_candidate_source_metrics"] = legacy.authored_source_metrics(candidate, None)
    legacy.preserve_candidate_source(row, artifacts, candidate, label)
    return row


def launch_calibration(repo: Path, artifacts: Path, commit: str, settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    """One separately reported empty-task request; it is never subtracted from trials."""
    workspace = artifacts / "worktrees" / "calibration"
    row: dict[str, Any] = {"status": "failed", "separate_from_trials": True, "subtracted_from_trials": False}
    error = legacy.add_seed_worktree(repo, workspace, commit)
    if error:
        row["failure"] = error; return row
    transcript, stderr, trace = (artifacts / "transcripts" / f"calibration.{suffix}" for suffix in ("jsonl", "stderr.txt", "rollout.jsonl"))
    transcript.parent.mkdir(parents=True, exist_ok=True)
    before = _rollout_snapshot()
    row.update(run_codex(codex_command(CALIBRATION_PROMPT), workspace, legacy.trial_environment(semaprax_bin), transcript, stderr, settings["timeout_seconds"]))
    usage = parse_exec_jsonl(transcript); copied = copy_new_rollout(before, trace)
    trace_result = trace_usage(usage, copied) if copied else {"reconciled": False}
    row["observed"] = {**usage, **trace_result, "stable_context_tokens": None}
    row["rollout_trace"] = str(copied) if copied else None
    row["list_price"] = list_price_estimate(trace_result.get("model_requests", []))
    row["status"] = "ready" if row["process_exit_code"] == 0 and trace_result.get("reconciled") else "failed"
    return row


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    preflight = sub.add_parser("capabilities", help="inspect CLI support without a model request")
    preflight.add_argument("--codex-binary", default="codex")
    plan_parser = sub.add_parser("plan", help="write no artifacts and make no model request")
    run_parser = sub.add_parser("run", help="launch the explicitly acknowledged matched campaign")
    for current in (plan_parser, run_parser):
        current.add_argument("--repo", default=str(REPO)); current.add_argument("--base-ref", required=True)
        current.add_argument("--artifacts", required=True); current.add_argument("--semaprax-bin", required=True)
        current.add_argument("--codex-binary", default="codex"); current.add_argument("--model", default=MODEL)
        current.add_argument("--effort", default=EFFORT); current.add_argument("--trials-per-arm", type=int, default=MIN_TRIALS_PER_ARM)
        current.add_argument("--timeout-seconds", type=int, default=TIMEOUT_SECONDS)
    run_parser.add_argument("--acknowledge-paid-attempts", action="store_true")
    args = parser.parse_args()
    try:
        result = capabilities(args.codex_binary) if args.action == "capabilities" else plan(args)
        if args.action == "run":
            if not args.acknowledge_paid_attempts:
                raise ValueError("run requires --acknowledge-paid-attempts")
            if result["capabilities"]["status"] != "ready":
                raise ValueError("installed Codex CLI lacks required isolated-execution controls")
            artifacts = Path(result["artifacts"]); artifacts.mkdir(parents=True)
            seed = legacy.create_seed_repository(Path(args.repo).resolve(), result["repository_commit"], artifacts / "seed-repository", SEED_FILES)
            result.update(seed); result["semaprax_binary"] = str(Path(args.semaprax_bin).resolve())
            (artifacts / "campaign.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
            calibration = launch_calibration(artifacts / "seed-repository", artifacts, seed["seed_repository_commit"], result, Path(args.semaprax_bin).resolve())
            (artifacts / "calibration.json").write_text(json.dumps(calibration, indent=2, sort_keys=True) + "\n")
            if calibration["status"] != "ready":
                print(json.dumps({"status": "calibration_failed", "artifacts": str(artifacts)}, indent=2))
                return 2
            rows = []; counters = {arm: 0 for arm in ARMS}
            for arm in result["trial_order"]:
                counters[arm] += 1
                rows.append(launch_trial(artifacts / "seed-repository", artifacts, seed["seed_repository_commit"],
                                         {"arm": arm, "number": counters[arm]}, result, Path(args.semaprax_bin).resolve()))
                (artifacts / "results.json").write_text(json.dumps({"campaign": result, "calibration": calibration, "trials": rows}, indent=2, sort_keys=True) + "\n")
            result = {"status": "completed", "artifacts": str(artifacts), "attempts": len(rows),
                      "accepted": sum(row.get("status") == "accepted" for row in rows),
                      "failed_or_rejected_attempts": sum(row.get("status") != "accepted" for row in rows)}
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if result.get("status", "ready") in {"ready", "completed"} else 2
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"codex campaign error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
