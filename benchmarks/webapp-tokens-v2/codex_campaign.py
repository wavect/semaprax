#!/usr/bin/env python3
"""Matched TeamDesk webapp campaign for Codex CLI, with trace-backed accounting.

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
import re
import signal
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import live_campaign_common as common

BENCHMARK = Path(__file__).resolve().parent
REPO = BENCHMARK.parents[1]
MODEL = "gpt-6.1-sol"
EFFORT = "medium"
TIMEOUT_SECONDS = 1800
ARMS = ("semaprax", "typescript")
MIN_TRIALS_PER_ARM = 5
SEED_FILES = (
    "/benchmarks/webapp-tokens-v2/SPEC.md",
    "/benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md",
)
ROUND = 1
HARNESS_SOURCE_FILES = ("benchmarks/webapp-tokens-v2/codex_campaign.py", "benchmarks/live_campaign_common.py")
PRICE_USD_PER_MTOK = {
    "input": 2.0, "cache_read": 0.1, "cache_write": 2.5, "output": 10.0,
}
SHORT_CONTEXT_LIMIT = 272_000
CALIBRATION_PROMPT = "This is a context calibration request. Reply with exactly READY; do not use tools or read files."


def harness_source_inventory(repo: Path = REPO) -> dict[str, str]:
    """Hash the complete local source closure used by the matched campaign."""
    root = repo.resolve(strict=True)
    inventory: dict[str, str] = {}
    for relative in HARNESS_SOURCE_FILES:
        source = root / relative
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"campaign harness source is not a regular file: {relative}")
        try:
            source.resolve(strict=True).relative_to(root)
        except ValueError as error:
            raise ValueError(f"campaign harness source escapes repository: {relative}") from error
        inventory[relative] = hashlib.sha256(source.read_bytes()).hexdigest()
    return inventory


def snapshot_harness_sources(repo: Path, artifacts: Path, expected: dict[str, str],
                             frozen_spec_sha256: str) -> dict[str, Any]:
    """Copy only the pinned harness closure and verify every retained byte."""
    current = harness_source_inventory(repo)
    if current != expected:
        raise ValueError("campaign harness source changed after the immutable plan")
    destination = artifacts / "harness-source"
    if destination.exists():
        raise ValueError("campaign harness source snapshot must be new")
    destination.mkdir(parents=True)
    for relative, wanted in current.items():
        source = repo / relative
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        contents = source.read_bytes()
        if hashlib.sha256(contents).hexdigest() != wanted:
            raise ValueError(f"campaign harness source changed while snapshotting: {relative}")
        target.write_bytes(contents)
    for relative, wanted in current.items():
        copied = destination / relative
        if hashlib.sha256(copied.read_bytes()).hexdigest() != wanted:
            raise ValueError(f"campaign harness source snapshot hash differs: {relative}")
    snapshot = {
        "schema": "semaprax.codex-harness-source-snapshot.v1",
        "path": "harness-source",
        "files_sha256": current,
        "frozen_spec": {
            "path": "benchmarks/webapp-tokens-v2/SPEC.md",
            "sha256": frozen_spec_sha256,
            "source": "seed_files_sha256",
        },
    }
    (destination / "manifest.json").write_text(json.dumps(snapshot, indent=2, sort_keys=True) + "\n")
    return snapshot


def _usage(value: Any) -> dict[str, int | None]:
    names = ("input_tokens", "cached_input_tokens", "cache_write_input_tokens", "output_tokens", "reasoning_output_tokens")
    if not isinstance(value, dict):
        return {name: None for name in names}
    return {
        name: value.get(name) if type(value.get(name)) is int and value[name] >= 0 else None
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


def resolve_commit(repo: Path, ref: str) -> str:
    result = subprocess.run(["git", "rev-parse", "--verify", f"{ref}^{{commit}}"], cwd=repo,
                            text=True, capture_output=True, check=False)
    if result.returncode or not result.stdout.strip():
        raise ValueError(f"ref does not resolve to a commit: {ref}")
    return result.stdout.strip()


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
            usage = _usage(payload.get("usage"))
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
    reconciled = isinstance(final, dict) and all(value is not None for value in final.values()) and _same_usage(final, summed)
    return {
        "model_requests": requests if reconciled else [],
        "model_request_count": len(requests) if reconciled else None,
        "model_observed": models[0] if reconciled and len(models) == 1 else None,
        "effort_observed": efforts[0] if reconciled and len(efforts) == 1 else None,
        "model_context_window": context_window,
        "model_identity_source": "client turn_context; provider-resolved model is unavailable",
        "provider_resolved_model": None,
        "legacy_net_input_tokens": (sum(r["usage"]["input_tokens"] for r in requests) - requests[0]["usage"]["input_tokens"] * len(requests)) if reconciled and requests and all(r["usage"]["input_tokens"] is not None for r in requests) else None,
        "legacy_net_note": "Codex input includes cache subsets; subtract first request total once per model request for historical comparability only, never task-only input.",
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
    commit = resolve_commit(repo, args.base_ref)
    compiler_source_commit = resolve_commit(repo, args.compiler_source_ref)
    hashes = {path.lstrip("/"): common.digest(repo / path.lstrip("/")) for path in SEED_FILES}
    frozen_spec_hash = hashes["benchmarks/webapp-tokens-v2/SPEC.md"]
    receipt = json.loads((repo / "benchmarks/webapp-tokens-v2/acceptance/evidence/reference-r5-summary.json").read_text())
    if not receipt.get("qualified") or any(arm["qualification"]["cases"] != 912 for arm in receipt["arms"].values()):
        raise ValueError("both frozen TeamDesk references must have independent 912/912 qualification")
    harness_sources = harness_source_inventory()
    artifacts = Path(args.artifacts).expanduser().resolve()
    try:
        artifacts.relative_to(repo)
    except ValueError:
        pass
    else:
        raise ValueError("artifact directory must be outside the repository")
    if artifacts.exists():
        raise ValueError("artifact path must not already exist")
    if args.model != MODEL or args.effort != EFFORT:
        raise ValueError("matched campaign pins gpt-6.1-sol at medium effort")
    if args.trials_per_arm < MIN_TRIALS_PER_ARM or args.timeout_seconds != TIMEOUT_SECONDS:
        raise ValueError("matched campaign requires five trials per arm and a 1800-second timeout")
    return {
        "adapter": "codex-matched-teamdesk-webapp-v1", "round": ROUND, "repository_commit": commit,
        "repository_root": str(repo),
        "compiler_source_commit": compiler_source_commit,
        "seed_files_sha256": hashes, "artifacts": str(artifacts), "model_requested": args.model,
        "harness_source_snapshot": {
            "schema": "semaprax.codex-harness-source-snapshot.v1",
            "path": "harness-source",
            "files_sha256": harness_sources,
            "frozen_spec": {
            "path": "benchmarks/webapp-tokens-v2/SPEC.md",
                "sha256": frozen_spec_hash,
                "source": "seed_files_sha256",
            },
        },
        "qualification": {"receipt": "benchmarks/webapp-tokens-v2/acceptance/evidence/reference-r5-summary.json",
                          "receipt_sha256": common.digest(repo / "benchmarks/webapp-tokens-v2/acceptance/evidence/reference-r5-summary.json"),
                          "spec_sha256": frozen_spec_hash, "required_cases": 912, "required_arms": list(ARMS)},
        "effort_requested": args.effort, "timeout_seconds": args.timeout_seconds,
        "acceptance_timeout_seconds": 2700,
        "trial_order": [arm for index in range(args.trials_per_arm) for arm in (ARMS if index % 2 == 0 else tuple(reversed(ARMS)))],
        "authored_source_tokenizer": common.tokenizer_metadata(args.tokenizer_dir),
        "codex_binary": args.codex_binary,
        "codex_version": subprocess.run([args.codex_binary, "--version"], capture_output=True, text=True, check=True).stdout.strip(),
        "price_book": {"date": "2026-10-08", "source": "https://developers.openai.com/api/docs/models/gpt-6.1-sol", "standard_short_context_usd_per_million": PRICE_USD_PER_MTOK, "conditional": True},
        "attempt_denominator": args.trials_per_arm * len(ARMS),
        "source_binary_sha256": common.digest(Path(args.semaprax_bin).resolve(strict=True)),
        "calibration": {"prompt": CALIBRATION_PROMPT, "separate": True, "subtracted_from_trials": False},
        "measurement": {"stable_context_tokens": None, "legacy_net_input_tokens": None,
                        "model_request_count": "trace-backed only; never inferred from item counts"},
        "capabilities": capabilities(args.codex_binary),
        "acceptance": {"runner": "benchmarks/webapp-tokens-v2/acceptance/run.mjs", "node_major": 24,
                       "playwright": "1.62.0", "required_cases": 912, "fresh_evidence": True},
    }


def copy_task_rollout(thread_ids: list[str], workspace: Path, destination: Path,
                      sessions_root: Path | None = None) -> Path | None:
    """Read only the exact CLI-emitted thread, authenticated by ID and cwd."""
    if len(thread_ids) != 1 or not re.fullmatch(r"[0-9a-f-]{36}", thread_ids[0]):
        return None
    root = sessions_root or (Path.home() / ".codex" / "sessions")
    paths = list(root.rglob(f"*{thread_ids[0]}.jsonl"))
    if len(paths) != 1:
        return None
    raw_lines = paths[0].read_text(encoding="utf-8").splitlines()
    try:
        records = [json.loads(raw) for raw in raw_lines]
    except json.JSONDecodeError:
        return None
    metas = [r.get("payload", {}) for r in records if isinstance(r, dict) and r.get("type") == "session_meta"]
    if len(metas) != 1 or metas[0].get("id") != thread_ids[0] or not isinstance(metas[0].get("cwd"), str):
        return None
    if Path(metas[0]["cwd"]).resolve() != workspace.resolve():
        return None
    def redact(value: Any) -> Any:
        if isinstance(value, dict):
            return {key: redact(item) for key, item in value.items()
                    if key.lower() not in {"creator_user", "creator_user_id", "creator_account_id", "account_id", "user_id", "email"}}
        if isinstance(value, list):
            return [redact(item) for item in value]
        return value
    destination.write_text("\n".join(json.dumps(redact(r), sort_keys=True) for r in records) + "\n", encoding="utf-8")
    return destination


def _command(settings: dict[str, Any], prompt: str) -> list[str]:
    command = codex_command(prompt)
    command[0] = settings["codex_binary"]
    return command


def cleanup_trial(repo: Path, workspace: Path, settings: dict[str, Any], row: dict[str, Any]) -> None:
    removed = subprocess.run(["git", "worktree", "remove", "--force", str(workspace)], cwd=repo,
                             capture_output=True, text=True, check=False)
    row["workspace_removed"] = removed.returncode == 0
    if removed.returncode:
        row["workspace_retained_for_review"] = True
        row["cleanup_error"] = common.bounded_text(removed.stderr)


def create_seed_repository(source_repo: Path, source_commit: str, seed_repo: Path) -> dict[str, Any]:
    return common.create_seed_repository(source_repo, source_commit, seed_repo, SEED_FILES)


def add_seed_worktree(seed_repo: Path, workspace: Path, seed_commit: str) -> str | None:
    return common.add_seed_worktree(seed_repo, workspace, seed_commit, SEED_FILES)


def trial_environment(compiler: Path | None) -> dict[str, str]:
    env = dict(os.environ)
    env["SEMAPRAX_BIN"] = str(compiler) if compiler is not None else ""
    env["TEAMDESK_BENCHMARK_ARM"] = ""
    return env


def prompt_for(arm: str, candidate: Path, compiler: Path) -> str:
    language = "TypeScript/React" if arm == "typescript" else "SEMAPRAX"
    compiler_note = (f"Use the supplied compiler at {compiler} for the webapp projection; do not build or fetch a compiler."
                     if arm == "semaprax" else "Do not use SEMAPRAX or substitute another language.")
    return f"""Build the complete TeamDesk Enterprise web application described by the supplied SPEC.md and acceptance CONTRACT.md.

You are writing the {language} arm. Work only inside {candidate}. Create build.sh, test.sh, and run.sh at the candidate root. {compiler_note}
Implement every requirement: all 20 entities and fields, validation, references, unique keys, workflows, computed fields and rollups, accounts, sessions, role and row permissions, audit history, CSV export, REST CRUD API, persistence, and the browser UI. The independent gate is authoritative and covers all 912 obligations; do not weaken, edit, or bypass it. Keep the API and UI on loopback using TEAMDESK_HOST, TEAMDESK_PORT, TEAMDESK_UI_PORT, TEAMDESK_DATA_DIR, and make run.sh stay in the foreground while emitting the required readiness JSON line.

Use only the files and tools available in this isolated workspace. Run your own build and tests before reporting completion. Do not edit files outside the candidate directory. When finished, leave all implementation files in {candidate} and briefly report the commands and results."""


def check_candidate(candidate: Path, output: Path, arm: str, settings: dict[str, Any], compiler: Path) -> dict[str, Any]:
    """Run candidate checks, then the independent 912-case gate from the harness checkout."""
    env = trial_environment(compiler if arm == "semaprax" else None)
    env["TEAMDESK_BENCHMARK_ARM"] = arm
    steps: list[dict[str, Any]] = []
    for script in ("build.sh", "test.sh"):
        path = candidate / script
        if not path.is_file():
            return {"accepted": False, "failure": f"missing candidate {script}", "steps": steps}
        started = time.monotonic()
        result = subprocess.run(["/bin/sh", str(path)], cwd=candidate, env=env, text=True,
                                capture_output=True, timeout=settings["acceptance_timeout_seconds"])
        steps.append({"script": script, "exit_code": result.returncode,
                      "seconds": round(time.monotonic() - started, 3),
                      "stdout": common.bounded_text(result.stdout), "stderr": common.bounded_text(result.stderr)})
        if result.returncode != 0:
            return {"accepted": False, "failure": f"candidate {script} failed", "steps": steps}
    gate = Path(settings["repository_root"]) / "benchmarks/webapp-tokens-v2/acceptance/run.mjs"
    command = ["node", str(gate), "--arm", arm, "--candidate", str(candidate), "--output", str(output)]
    if arm == "semaprax":
        command.extend(["--compiler", str(compiler), "--compiler-source-sha", settings["compiler_source_commit"]])
    started = time.monotonic()
    result = subprocess.run(command, cwd=gate.parents[3], env=env, text=True, capture_output=True,
                            timeout=settings["acceptance_timeout_seconds"])
    report_path = output / "report.json"
    report: dict[str, Any] = {}
    if report_path.is_file():
        report = json.loads(report_path.read_text())
    qualification = report.get("qualification", {})
    accepted = (result.returncode == 0 and qualification.get("passed") is True
                and qualification.get("cases") == 912
                and not qualification.get("missingCases") and not qualification.get("missingGroups"))
    return {"accepted": accepted, "exit_code": result.returncode,
            "seconds": round(time.monotonic() - started, 3), "report": report,
            "stdout": common.bounded_text(result.stdout), "stderr": common.bounded_text(result.stderr),
            "steps": steps}


def launch_trial(repo: Path, artifacts: Path, commit: str, trial: dict[str, Any], settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    """Run one paid attempt. Every attempted trial remains in the result denominator."""
    arm, number = trial["arm"], trial["number"]
    label = f"{arm}-{number:02d}"
    workspace = artifacts / "worktrees" / label
    row: dict[str, Any] = {**trial, "workspace": str(workspace), "status": "failed", "failure": None}
    error = add_seed_worktree(repo, workspace, commit)
    if error:
        row.update({"failure": error, "workspace_retained_for_review": True})
        return row
    candidate = workspace / "candidate"
    candidate.mkdir(parents=True, exist_ok=True)
    prompt = prompt_for(arm, candidate, semaprax_bin)
    (artifacts / "prompts").mkdir(parents=True, exist_ok=True)
    (artifacts / "prompts" / f"{label}.txt").write_text(prompt, encoding="utf-8")
    row["prompt_sha256"] = hashlib.sha256(prompt.encode()).hexdigest()
    transcript, stderr, trace = (artifacts / "transcripts" / f"{label}.{suffix}" for suffix in ("jsonl", "stderr.txt", "rollout.jsonl"))
    transcript.parent.mkdir(parents=True, exist_ok=True)
    process = run_codex(_command(settings, prompt), workspace, trial_environment(semaprax_bin), transcript, stderr, settings["timeout_seconds"])
    row.update(process); row.update({"transcript": str(transcript), "stderr_path": str(stderr)})
    exec_usage = parse_exec_jsonl(transcript)
    copied = copy_task_rollout(exec_usage["thread_ids"], workspace, trace)
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
        row["acceptance"] = check_candidate(candidate, artifacts / "qualification" / label, arm, settings, semaprax_bin)
        row["status"] = "accepted" if row["acceptance"]["accepted"] else "not_accepted"
        if row["status"] != "accepted": row["failure"] = "candidate failed independent 912-case acceptance"
    row["final_candidate_source_metrics"] = common.authored_source_metrics(candidate, settings["authored_source_tokenizer"])
    archive = artifacts / "candidates" / label
    archive.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(candidate, archive, dirs_exist_ok=True,
                    ignore=shutil.ignore_patterns("node_modules", "dist", "out", ".cache"))
    row["candidate_archive"] = str(archive)
    row["candidate_files_sha256"] = {p.relative_to(archive).as_posix(): common.digest(p)
                                      for p in sorted(archive.rglob("*")) if p.is_file()}
    cleanup_trial(repo, workspace, settings, row)
    return row


def launch_calibration(repo: Path, artifacts: Path, commit: str, settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    """One separately reported empty-task request; it is never subtracted from trials."""
    workspace = artifacts / "worktrees" / "calibration"
    row: dict[str, Any] = {"status": "failed", "separate_from_trials": True, "subtracted_from_trials": False}
    error = add_seed_worktree(repo, workspace, commit)
    if error:
        row["failure"] = error; return row
    transcript, stderr, trace = (artifacts / "transcripts" / f"calibration.{suffix}" for suffix in ("jsonl", "stderr.txt", "rollout.jsonl"))
    transcript.parent.mkdir(parents=True, exist_ok=True)
    row.update(run_codex(_command(settings, CALIBRATION_PROMPT), workspace, trial_environment(semaprax_bin), transcript, stderr, settings["timeout_seconds"]))
    usage = parse_exec_jsonl(transcript); copied = copy_task_rollout(usage["thread_ids"], workspace, trace)
    trace_result = trace_usage(usage, copied) if copied else {"reconciled": False}
    row["observed"] = {**usage, **trace_result, "stable_context_tokens": None}
    row["rollout_trace"] = str(copied) if copied else None
    row["list_price"] = list_price_estimate(trace_result.get("model_requests", []))
    messages = [json.loads(line).get("item", {}) for line in transcript.read_text().splitlines() if line]
    replies = [item.get("text", "").strip() for item in messages if item.get("type") == "agent_message"]
    counts = usage.get("tool_item_counts", {})
    row["status"] = "ready" if (row["process_exit_code"] == 0 and trace_result.get("reconciled")
        and trace_result.get("model_observed") == MODEL and trace_result.get("effort_observed") == EFFORT
        and replies == ["READY"] and all(kind == "agent_message" or not count for kind, count in counts.items())) else "failed"
    cleanup_trial(repo, workspace, settings, row)
    return row


def summarize(rows: list[dict[str, Any]]) -> dict[str, Any]:
    prices = [row.get("list_price", {}).get("standard_short_context_api_equivalent_usd")
              for row in rows if isinstance(row.get("list_price"), dict)]
    known = [value for value in prices if isinstance(value, (int, float))]
    return {
        "attempt_denominator": len(ARMS) * MIN_TRIALS_PER_ARM,
        "recorded_attempts": len(rows),
        "accepted_attempts": sum(row.get("status") == "accepted" for row in rows),
        "failed_or_rejected_attempts": sum(row.get("status") != "accepted" for row in rows),
        "list_price_estimate_known_attempts": len(known),
        "list_price_estimate_known_subtotal_usd": round(sum(known), 6) if len(known) == len(rows) else None,
        "provider_receipt_actual_usd": None,
        "cost_claim": "conditional standard short-context API-equivalent estimate; no provider billing receipt",
        "calibration_separate": True,
    }


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
        current.add_argument("--compiler-source-ref", required=True,
                             help="commit that produced --semaprax-bin; distinct from the docs-only seed ref")
        current.add_argument("--tokenizer-dir", required=True)
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
            snapshot = snapshot_harness_sources(
                Path(args.repo).resolve(), artifacts,
                result["harness_source_snapshot"]["files_sha256"],
                result["seed_files_sha256"]["benchmarks/webapp-tokens-v2/SPEC.md"],
            )
            if snapshot != result["harness_source_snapshot"]:
                raise ValueError("campaign harness source snapshot differs from the immutable plan")
            seed = create_seed_repository(Path(args.repo).resolve(), result["repository_commit"], artifacts / "seed-repository")
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
                (artifacts / "results.json").write_text(json.dumps({"campaign": result, "calibration": calibration,
                    "trials": rows, "summary": summarize(rows)}, indent=2, sort_keys=True) + "\n")
            result = {"status": "completed", "artifacts": str(artifacts), **summarize(rows)}
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if result.get("status", "ready") in {"ready", "completed"} else 2
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"codex campaign error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
