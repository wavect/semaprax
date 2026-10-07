#!/usr/bin/env python3
"""Prepare or run a matched ShiftSim campaign (currently preflight-only)."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
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
MODEL = "claude-sonnet-5-5"
EFFORT = "medium"
ARMS = ("semaprax", "typescript")
MIN_TRIALS_PER_ARM = 5
SEED_FILES = ("/benchmarks/event-sim-tokens-v1/SPEC.md",)
CALIBRATION_PROMPT = "This is a context calibration request. Reply with exactly READY; do not use tools or read files."
PRICE_BOOK_DATE = "2026-10-07"
PRICE_BOOK_SOURCE = "https://platform.claude.com/docs/en/models/sonnet-5-5/overview"


def sha_text(value: str) -> str:
    import hashlib
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def resolve_commit(repo: Path, ref: str) -> str:
    result = subprocess.run(["git", "rev-parse", "--verify", f"{ref}^{{commit}}"], cwd=repo,
                            text=True, capture_output=True, check=False)
    if result.returncode:
        raise ValueError(f"base ref does not resolve to a commit: {ref}")
    return result.stdout.strip()


def trial_environment(semaprax_bin: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["SEMAPRAX_BIN"] = str(semaprax_bin)
    env["PATH"] = str(semaprax_bin.parent) + os.pathsep + env.get("PATH", "")
    return env


def prompt_for(arm: str, candidate: Path, semaprax_bin: Path) -> str:
    language = "SEMAPRAX native Project" if arm == "semaprax" else "TypeScript on Node.js"
    return f"""Implement a complete, idiomatic ShiftSim application in {language} according to
`benchmarks/event-sim-tokens-v1/SPEC.md`. The whole implementation belongs under
`{candidate}`. Do not change compiler or benchmark files. The checkout contains
only the public specification; the independent acceptance corpus and oracle are
not in your workspace.

Create executable `build.sh`, `run.sh`, and `test.sh`. `build.sh` must compile
or validate the implementation without network access. `run.sh` must accept
one JSON request on stdin, emit the exact report plus one newline on stdout,
and produce the specified status-2 diagnostic for invalid requests. `test.sh`
must run your own automated tests and fail nonzero on errors. The SEMAPRAX arm
must use a native Project manifest and the compiler at `{semaprax_bin}` (also
available as `$SEMAPRAX_BIN`), with the required stdin capability. The
TypeScript arm must use Node from `PATH` and provide the same stdin interface.

Do not access material outside the public specification and your candidate
directory. Finish by listing files written and stating whether the implementation
is complete.
"""


def plan(args: argparse.Namespace) -> dict[str, Any]:
    repo = Path(args.repo).resolve(strict=True)
    commit = resolve_commit(repo, args.base_ref)
    artifacts = Path(args.artifacts).expanduser().resolve()
    try:
        artifacts.relative_to(repo)
    except ValueError:
        pass
    else:
        raise ValueError("artifact directory must be outside the repository")
    if artifacts.exists():
        raise ValueError(f"artifact path must not already exist: {artifacts}")
    if args.trials_per_arm < MIN_TRIALS_PER_ARM:
        raise ValueError(f"at least {MIN_TRIALS_PER_ARM} trials per arm are required")
    if args.timeout_seconds <= 0 or (args.max_budget_usd is not None and args.max_budget_usd <= 0):
        raise ValueError("timeout and optional max budget must be positive")
    if args.model != MODEL or args.effort != EFFORT:
        raise ValueError(f"matched ShiftSim pins --model {MODEL} and --effort {EFFORT}")
    tokenizer = common.tokenizer_metadata(getattr(args, "tokenizer_dir", None))
    rounds = [arm for i in range(args.trials_per_arm)
              for arm in (ARMS if i % 2 == 0 else tuple(reversed(ARMS)))]
    return {
        "schema": "semaprax.event-sim-campaign.v1",
        "created_at_utc": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "benchmark": "event-sim-tokens-v1",
        "repository_commit": commit,
        "artifacts": str(artifacts),
        "model": args.model,
        "effort": args.effort,
        "trials_per_arm": args.trials_per_arm,
        "attempt_denominator": 2 * args.trials_per_arm,
        "timeout_seconds": args.timeout_seconds,
        "max_budget_usd": args.max_budget_usd,
        "authored_source_tokenizer": tokenizer,
        "arms": list(ARMS),
        "trial_order": rounds,
        "qualification": {
            "status": "preflight_not_qualified",
            "blocking_issue": 611,
            "reason": "stdin transport capacity and legal-input bounds are not reconciled; no scored comparison is qualified",
        },
        "price_book": {
            "date": PRICE_BOOK_DATE,
            "source_url": PRICE_BOOK_SOURCE,
            "per_million_tokens": common.PRICE_USD_PER_MTOK,
            "claim": "list-price estimate; provider result cost is reported separately and is not a billing receipt",
        },
        "calibration": {
            "mode": "matched empty-task one-turn calibration before trials",
            "prompt": CALIBRATION_PROMPT,
            "prompt_sha256": sha_text(CALIBRATION_PROMPT),
            "usage_convention": "diagnostic only; never subtracted from raw trial usage",
        },
        "seed_checkout": {
            "mode": "new parentless one-commit repository and detached worktree per calibration/trial",
            "included_files": list(SEED_FILES),
            "candidate_path": "benchmarks/event-sim-tokens-v1/candidate/",
            "excluded": "all other source files, hidden acceptance corpus/oracle, and original repository history/objects",
        },
    }


def run_process(command: list[str], cwd: Path, env: dict[str, str], stdout: Path, stderr: Path,
                timeout: int) -> dict[str, Any]:
    return common.run_claude(command, cwd, env, stdout, stderr, timeout)


def launch_calibration(seed_repo: Path, artifacts: Path, seed_commit: str,
                       settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    workspace = artifacts / "worktrees" / "calibration"
    error = common.add_seed_worktree(seed_repo, workspace, seed_commit, SEED_FILES)
    row: dict[str, Any] = {"status": "failed", "failure": error}
    if error:
        return row
    prompt = CALIBRATION_PROMPT
    stream, stderr = artifacts / "calibration.jsonl", artifacts / "calibration.stderr.txt"
    process = run_process(common.claude_command(settings, prompt), workspace,
                          trial_environment(semaprax_bin), stream, stderr, settings["timeout_seconds"])
    usage = common.stream_usage(stream, settings["model"]) if stream.exists() else {"usage": {}, "models_observed": []}
    row.update(process)
    row["observed"] = usage
    row["observed_model_id"] = usage["models_observed"][0] if len(usage["models_observed"]) == 1 else None
    row["status"] = "ready" if process["process_exit_code"] == 0 and row["observed_model_id"] else "failed"
    row["list_price_estimate_usd"] = common.rate_card_estimate_details(usage.get("usage", {}))["usd"]
    common.save_json(artifacts / "calibration.json", row)
    return row


def check_program(candidate: Path, timeout: int, env: dict[str, str]) -> dict[str, Any]:
    result: dict[str, Any] = {"build": {"status": "missing"}, "candidate_tests": {"status": "not_run"},
                              "independent_acceptance": {"status": "not_run"}, "accepted": False}
    for key, script in (("build", "build.sh"), ("candidate_tests", "test.sh")):
        path = candidate / script
        if not path.is_file():
            result[key] = {"status": "missing", "path": str(path)}
            return result
        started = time.monotonic()
        try:
            proc = subprocess.run(["/bin/sh", str(path)], cwd=candidate, capture_output=True,
                                  check=False, timeout=timeout, env=env)
            row = {"status": "passed" if proc.returncode == 0 else "failed",
                   "exit_code": proc.returncode, "seconds": round(time.monotonic() - started, 3),
                   "stdout": common.bounded_text(proc.stdout), "stderr": common.bounded_text(proc.stderr)}
        except subprocess.TimeoutExpired as exc:
            row = {"status": "timeout", "seconds": round(time.monotonic() - started, 3),
                   "stdout": common.bounded_text(exc.stdout or b""), "stderr": common.bounded_text(exc.stderr or b"")}
        result[key] = row
        if row["status"] != "passed":
            return result
    runner = BENCHMARK / "acceptance" / "run.py"
    command = [sys.executable, str(runner), "--command-json", json.dumps(["/bin/sh", str(candidate / "run.sh")])]
    started = time.monotonic()
    try:
        proc = subprocess.run(command, cwd=candidate, capture_output=True, check=False, timeout=timeout, env=env)
        row = {"status": "passed" if proc.returncode == 0 else "failed", "exit_code": proc.returncode,
               "seconds": round(time.monotonic() - started, 3), "stdout": common.bounded_text(proc.stdout),
               "stderr": common.bounded_text(proc.stderr)}
    except subprocess.TimeoutExpired as exc:
        row = {"status": "timeout", "stdout": common.bounded_text(exc.stdout or b""),
               "stderr": common.bounded_text(exc.stderr or b"")}
    result["independent_acceptance"] = row
    result["accepted"] = row["status"] == "passed"
    result["qualification"] = "preflight_only_issue_611_open"
    return result


def launch_trial(seed_repo: Path, artifacts: Path, seed_commit: str, trial: dict[str, Any],
                 settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    arm, number = trial["arm"], trial["number"]
    label = f"{arm}-{number:02d}"
    workspace = artifacts / "worktrees" / label
    error = common.add_seed_worktree(seed_repo, workspace, seed_commit, SEED_FILES)
    row: dict[str, Any] = {**trial, "workspace": str(workspace), "status": "failed", "failure": error}
    if error:
        return row
    candidate = workspace / "benchmarks" / "event-sim-tokens-v1" / "candidate"
    candidate.mkdir(parents=True, exist_ok=True)
    prompt = prompt_for(arm, candidate, semaprax_bin)
    prompts = artifacts / "prompts"
    prompts.mkdir(exist_ok=True)
    (prompts / f"{label}.txt").write_text(prompt, encoding="utf-8")
    row["prompt_sha256"] = sha_text(prompt)
    transcripts = artifacts / "transcripts"
    transcripts.mkdir(exist_ok=True)
    stream, stderr = transcripts / f"{label}.jsonl", transcripts / f"{label}.stderr.txt"
    process = run_process(common.claude_command(settings, prompt), workspace,
                          trial_environment(semaprax_bin), stream, stderr, settings["timeout_seconds"])
    row.update(process)
    row["transcript"], row["stderr_path"] = str(stream), str(stderr)
    usage = common.stream_usage(stream, settings["model"]) if stream.exists() else {
        "models_observed": [], "usage": {field: None for field in common.ALL_USAGE_FIELDS},
        "turns_with_usage": 0, "legacy_net_input": {"net_input_tokens": None},
    }
    row["observed"] = usage
    row["provider_input_plus_cache_tokens_raw"] = common.input_tokens_total(usage["usage"])
    row["list_price_estimate_usd"] = common.rate_card_estimate_details(usage["usage"])["usd"]
    row["provider_reported_api_equivalent_total_cost_usd"] = usage.get("provider_reported_api_equivalent_total_cost_usd")
    row["provider_receipt_actual_usd"] = None
    model_ok = common.observed_model_matches(usage.get("models_observed"), settings.get("observed_model_id"))
    if process["timed_out"]:
        row["failure"] = "trial hit wall-clock timeout"
    elif process["process_exit_code"] != 0:
        row["failure"] = f"Claude Code exited with {process['process_exit_code']}"
    elif not model_ok:
        row["failure"] = "observed model missing or differs from calibration model"
    else:
        started = time.monotonic()
        row["acceptance"] = check_program(candidate, settings["timeout_seconds"], trial_environment(semaprax_bin))
        row["acceptance_elapsed_seconds"] = round(time.monotonic() - started, 3)
        row["status"] = "accepted" if row["acceptance"]["accepted"] else "not_accepted"
        row["qualification"] = "preflight_only_issue_611_open"
        if row["status"] != "accepted":
            row["failure"] = "candidate failed build or acceptance checks"
    try:
        row["final_candidate_source_metrics"] = common.authored_source_metrics(
            candidate, settings.get("authored_source_tokenizer")
        )
    except (OSError, RuntimeError, UnicodeError, json.JSONDecodeError) as error:
        row["final_candidate_source_metrics"] = {
            "status": "measurement_failed", "total_tokens": None, "files": [],
            "tokenizer": settings.get("authored_source_tokenizer"), "error": str(error),
        }
    archive = artifacts / "candidates" / label
    archive.parent.mkdir(exist_ok=True)
    hashes, omitted = common.archive_candidate(candidate, archive)
    row["candidate_archive"] = str(archive)
    row["candidate_source_sha256"] = hashes
    row["candidate_archive_excluded_paths"] = omitted
    # Preserve the complete authored candidate before deleting its modified worktree.
    removed = subprocess.run(["git", "worktree", "remove", "--force", str(workspace)],
                             cwd=seed_repo, text=True, capture_output=True, check=False)
    row["worktree_removed_after_archive"] = removed.returncode == 0
    if removed.returncode:
        row["worktree_cleanup_error"] = common.bounded_text(removed.stderr)
    return row


def summarize(rows: list[dict[str, Any]], calibration: dict[str, Any] | None = None) -> dict[str, Any]:
    arms: dict[str, Any] = {}
    for arm in ARMS:
        selected = [row for row in rows if row.get("arm") == arm]
        accepted = sum(row.get("status") == "accepted" for row in selected)
        costs = [row.get("list_price_estimate_usd") for row in selected]
        complete_cost_total = sum(costs) if costs and all(isinstance(value, (int, float)) for value in costs) else None
        provider_costs = [row.get("provider_reported_api_equivalent_total_cost_usd") for row in selected]
        provider_cost_total = (
            sum(provider_costs) if provider_costs and all(isinstance(value, (int, float)) for value in provider_costs)
            else None
        )
        usage_rows = [row.get("observed", {}).get("usage", {}) for row in selected]
        raw_usage_totals = {
            field: sum(usage.get(field) for usage in usage_rows if isinstance(usage.get(field), int))
            for field in common.ALL_USAGE_FIELDS
        }
        raw_usage_incomplete = {
            field: sum(not isinstance(usage.get(field), int) for usage in usage_rows)
            for field in common.ALL_USAGE_FIELDS
        }
        arms[arm] = {
            "attempts": len(selected),
            "accepted_preflight_trials": accepted,
            "accepted_per_attempt": accepted,
            "all_attempt_wall_seconds": [row.get("elapsed_seconds") for row in selected],
            "aggregate_attempt_wall_seconds": sum(row.get("elapsed_seconds", 0) or 0 for row in selected),
            "list_price_estimate_usd_per_attempt": [row.get("list_price_estimate_usd") for row in selected],
            "list_price_estimate_total_usd_including_failures": complete_cost_total,
            "list_price_estimate_usd_per_accepted_task_including_failures": (
                complete_cost_total / accepted if complete_cost_total is not None and accepted else None
            ),
            "provider_cost_per_attempt": [row.get("provider_reported_api_equivalent_total_cost_usd") for row in selected],
            "provider_reported_api_equivalent_total_cost_known_subtotal_usd": provider_cost_total,
            "provider_reported_api_equivalent_usd_per_accepted_task_including_failures": (
                provider_cost_total / accepted if provider_cost_total is not None and accepted else None
            ),
            "raw_usage_per_attempt": [row.get("observed", {}).get("usage") for row in selected],
            "raw_usage_known_subtotal_by_bucket": raw_usage_totals,
            "raw_usage_attempts_missing_bucket": raw_usage_incomplete,
            "raw_input_plus_cache_tokens_per_attempt": [row.get("provider_input_plus_cache_tokens_raw") for row in selected],
            "legacy_net_input_tokens_per_attempt": [row.get("observed", {}).get("legacy_net_input", {}).get("net_input_tokens") for row in selected],
            "final_authored_source_token_proxy_per_attempt": [
                row.get("final_candidate_source_metrics", {}).get("total_tokens") for row in selected
            ],
        }
    return {
        "qualification": "preflight_not_qualified_open_issue_611",
        "qualification_note": "Preflight acceptance outcomes are descriptive only and must not be presented as a scored live comparison.",
        "attempt_denominator": len(rows),
        "arms": arms,
        "calibration": calibration,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    for action in ("plan", "run"):
        p = sub.add_parser(action)
        p.add_argument("--repo", default=str(REPO))
        p.add_argument("--base-ref", required=True)
        p.add_argument("--artifacts", required=True)
        p.add_argument("--trials-per-arm", type=int, default=MIN_TRIALS_PER_ARM)
        p.add_argument("--model", default=MODEL)
        p.add_argument("--effort", default=EFFORT)
        p.add_argument("--timeout-seconds", type=int, default=1800)
        p.add_argument("--max-budget-usd", type=float, default=None)
        p.add_argument("--tokenizer-dir", default=None)
        if action == "run":
            p.add_argument("--semaprax-bin", required=True)
    args = parser.parse_args()
    try:
        settings = plan(args)
        if args.action == "plan":
            print(json.dumps(settings, indent=2))
            return 0
        semaprax_bin = Path(args.semaprax_bin).expanduser().resolve(strict=True)
        if not semaprax_bin.is_file():
            raise ValueError("semaprax-bin must be a regular file")
        artifacts = Path(settings["artifacts"])
        artifacts.mkdir(parents=True)
        seed_repo = artifacts / "seed-repository"
        seed = common.create_seed_repository(Path(args.repo).resolve(strict=True), settings["repository_commit"],
                                             seed_repo, SEED_FILES)
        settings.update(seed)
        settings["semaprax_binary"] = str(semaprax_bin)
        settings["semaprax_binary_sha256"] = common.digest(semaprax_bin)
        version = subprocess.run(["claude", "--version"], text=True, capture_output=True, check=False)
        settings["claude_version"] = version.stdout.strip() if version.returncode == 0 else None
        node = subprocess.run(["node", "--version"], text=True, capture_output=True, check=False)
        settings["node_version"] = node.stdout.strip() if node.returncode == 0 else None
        common.save_json(artifacts / "campaign.json", settings)
        campaign_started = time.monotonic()
        calibration = launch_calibration(seed_repo, artifacts, seed["seed_repository_commit"], settings, semaprax_bin)
        settings["observed_model_id"] = calibration.get("observed_model_id")
        settings["calibration_result"] = calibration
        common.save_json(artifacts / "campaign.json", settings)
        rows = []
        if calibration.get("status") == "ready":
            numbers = {arm: 0 for arm in ARMS}
            for arm in settings["trial_order"]:
                numbers[arm] += 1
                row = launch_trial(seed_repo, artifacts, seed["seed_repository_commit"],
                                   {"arm": arm, "number": numbers[arm]}, settings, semaprax_bin)
                rows.append(row)
                common.save_json(artifacts / "results.json", {
                    "campaign": settings, "calibration": calibration, "trials": rows,
                    "summary": summarize(rows, calibration),
                    "campaign_elapsed_wall_seconds": round(time.monotonic() - campaign_started, 3),
                })
                print(f"{arm} {numbers[arm]}/{settings['trials_per_arm']}: {row.get('status', 'failed')}", flush=True)
        else:
            common.save_json(artifacts / "results.json", {
                "campaign": settings, "calibration": calibration, "trials": [], "summary": summarize([], calibration),
                "campaign_elapsed_wall_seconds": round(time.monotonic() - campaign_started, 3),
            })
            print("calibration failed; no trials launched", file=sys.stderr)
            return 2
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"ShiftSim campaign error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
