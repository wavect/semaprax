#!/usr/bin/env python3
"""Prepare, launch, and independently validate matched LogLens live trials."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
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
MODEL = "claude-sonnet-5-5"
EFFORT = "medium"
PRICE_BOOK_DATE = "2026-09-25"
PRICE_USD_PER_MTOK = {
    "input": 2.0,
    "cache_write_5m": 2.5,
    "cache_read": 0.2,
    "output": 10.0,
}
ARMS = ("semaprax", "typescript")
MIN_TRIALS_PER_ARM = 5
MAX_LOG_BYTES = 1_000_000


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def resolve_commit(repo: Path, ref: str) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"{ref}^{{commit}}"],
        cwd=repo,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode:
        raise ValueError(f"base ref does not resolve to a commit: {ref}")
    return result.stdout.strip()


def sha_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def prompt_for(arm: str, candidate: Path, semaprax_bin: Path) -> str:
    language = "SEMAPRAX" if arm == "semaprax" else "TypeScript with Node.js"
    return f"""Implement the LogLens command-line application specified in
`benchmarks/cli-tokens-v1/SPEC.md` using {language}. The entire implementation
must live under `{candidate}`. Do not change compiler or harness files.

Provide executable `candidate/build.sh` and `candidate/run.sh` files. The
build script must compile or validate the implementation. The run script
must implement `loglens <file> [--top N] [--json]`. For SEMAPRAX, use the
verified compiler executable at `{semaprax_bin}` (also available as
`$SEMAPRAX_BIN`). For TypeScript, use Node from `PATH`.

Read the specification and public sample input. Do not inspect `oracle.py`,
`expected.txt`, or `expected.json`; the independent campaign runner validates
those after your session. Run your own relevant checks and finish by stating
which files you wrote and whether the program is complete.
"""


def plan(args: argparse.Namespace) -> dict[str, Any]:
    repo = Path(args.repo).resolve(strict=True)
    commit = resolve_commit(repo, args.base_ref)
    artifacts = Path(args.artifacts).expanduser().absolute()
    if artifacts.exists():
        raise ValueError(f"artifact path must not already exist: {artifacts}")
    if args.trials_per_arm < MIN_TRIALS_PER_ARM:
        raise ValueError(f"at least {MIN_TRIALS_PER_ARM} trials per arm are required")
    if args.timeout_seconds <= 0:
        raise ValueError("timeout-seconds must be positive")
    if args.max_budget_usd is not None and args.max_budget_usd <= 0:
        raise ValueError("max-budget-usd must be positive when supplied")
    if args.model != MODEL:
        raise ValueError(f"this matched campaign pins --model {MODEL}")
    if args.effort != EFFORT:
        raise ValueError(f"this matched campaign pins --effort {EFFORT}")
    return {
        "schema": "semaprax.cli-tokens.campaign.v1",
        "created_at_utc": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "benchmark": "cli-tokens-v1",
        "round": 3,
        "repository": str(repo),
        "repository_commit": commit,
        "artifacts": str(artifacts),
        "model": args.model,
        "effort": args.effort,
        "trials_per_arm": args.trials_per_arm,
        "attempt_denominator": 2 * args.trials_per_arm,
        "timeout_seconds": args.timeout_seconds,
        "max_budget_usd": args.max_budget_usd,
        "price_book": {
            "date": PRICE_BOOK_DATE,
            "currency": "USD",
            "per_million_tokens": PRICE_USD_PER_MTOK,
            "claim": "list-price estimate only; provider-billed cost requires a receipt",
        },
        "arms": list(ARMS),
        "trial_order": [
            arm
            for index in range(args.trials_per_arm)
            for arm in (ARMS if index % 2 == 0 else tuple(reversed(ARMS)))
        ],
        "claude_help_capabilities": [
            "--print",
            "--output-format stream-json",
            "--verbose",
            "--model",
            "--effort",
            "--max-budget-usd (optional)",
        ],
        "turn_limit": None,
    }


def stream_usage(path: Path) -> dict[str, Any]:
    usage_by_id: dict[str, dict[str, int]] = {}
    models: set[str] = set()
    visible_output = ""
    observed_result: dict[str, Any] | None = None
    invalid_lines = 0
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            invalid_lines += 1
            continue
        if not isinstance(event, dict):
            continue
        if event.get("type") == "assistant":
            message = event.get("message")
            if isinstance(message, dict):
                if isinstance(message.get("model"), str):
                    models.add(message["model"])
                usage = message.get("usage")
                identity = message.get("id")
                if isinstance(usage, dict) and isinstance(identity, str):
                    usage_by_id[identity] = {
                        name: int(usage.get(name, 0))
                        for name in (
                            "input_tokens",
                            "cache_creation_input_tokens",
                            "cache_read_input_tokens",
                            "output_tokens",
                        )
                        if isinstance(usage.get(name, 0), (int, float))
                    }
                for block in message.get("content", []):
                    if isinstance(block, dict) and block.get("type") == "text":
                        visible_output += str(block.get("text", "")) + "\n"
        if event.get("type") == "result":
            observed_result = event
            model_usage = event.get("modelUsage")
            if isinstance(model_usage, dict):
                models.update(str(model) for model in model_usage)

    names = (
        "input_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
        "output_tokens",
    )
    totals = {name: sum(usage.get(name, 0) for usage in usage_by_id.values()) for name in names}
    first = next(iter(usage_by_id.values()), {})
    return {
        "models_observed": sorted(models),
        "turns_with_usage": len(usage_by_id),
        "usage": totals,
        "first_turn_usage": first,
        "visible_output_bytes": len(visible_output.encode("utf-8")),
        "result_event": observed_result,
        "invalid_stream_lines": invalid_lines,
    }


def rate_card_estimate(usage: dict[str, int]) -> float | None:
    if not all(key in usage for key in (
        "input_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
        "output_tokens",
    )):
        return None
    amount = (
        usage["input_tokens"] * PRICE_USD_PER_MTOK["input"]
        + usage["cache_creation_input_tokens"] * PRICE_USD_PER_MTOK["cache_write_5m"]
        + usage["cache_read_input_tokens"] * PRICE_USD_PER_MTOK["cache_read"]
        + usage["output_tokens"] * PRICE_USD_PER_MTOK["output"]
    ) / 1_000_000
    return round(amount, 6)


def save_json(path: Path, value: Any) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    tmp.replace(path)


def bounded_text(value: bytes | str) -> str:
    if isinstance(value, bytes):
        value = value.decode("utf-8", errors="replace")
    if len(value.encode("utf-8")) > MAX_LOG_BYTES:
        value = value[:MAX_LOG_BYTES] + "\n[truncated by campaign harness]\n"
    return value


def check_program(
    candidate: Path,
    sample_path: Path,
    timeout: float,
    env: dict[str, str],
) -> dict[str, Any]:
    build = candidate / "build.sh"
    program = candidate / "run.sh"
    result: dict[str, Any] = {
        "build": {"status": "missing"},
        "checks": [],
        "accepted": False,
    }
    if not build.is_file() or not program.is_file():
        return result
    started = time.monotonic()
    try:
        compiled = subprocess.run(
            ["/bin/sh", str(build)],
            cwd=candidate,
            text=True,
            capture_output=True,
            check=False,
            timeout=timeout,
            env=env,
        )
    except subprocess.TimeoutExpired as error:
        result["build"] = {
            "status": "timeout",
            "seconds": round(time.monotonic() - started, 3),
            "stdout": bounded_text(error.stdout or b""),
            "stderr": bounded_text(error.stderr or b""),
        }
        return result
    result["build"] = {
        "status": "passed" if compiled.returncode == 0 else "failed",
        "exit_code": compiled.returncode,
        "seconds": round(time.monotonic() - started, 3),
        "stdout": bounded_text(compiled.stdout),
        "stderr": bounded_text(compiled.stderr),
    }
    if compiled.returncode:
        return result

    cases = [
        ("text-golden", [str(sample_path)], 0, (BENCHMARK / "expected.txt").read_bytes(), None),
        ("json-top-3-golden", [str(sample_path), "--top", "3", "--json"], 0, (BENCHMARK / "expected.json").read_bytes(), None),
        ("missing-file", ["missing-loglens-input.log"], 1, b"", "one-line-stderr"),
        ("missing-file-argument", [], 2, b"", "one-line-stderr"),
        ("unknown-flag", [str(sample_path), "--unknown"], 2, b"", "one-line-stderr"),
        ("missing-top-value", [str(sample_path), "--top"], 2, b"", "one-line-stderr"),
        ("invalid-top-value", [str(sample_path), "--top", "0"], 2, b"", "one-line-stderr"),
    ]
    for name, arguments, expected_code, expected_stdout, stderr_rule in cases:
        started = time.monotonic()
        try:
            completed = subprocess.run(
                ["/bin/sh", str(program), *arguments],
                cwd=candidate,
                capture_output=True,
                check=False,
                timeout=timeout,
                env=env,
            )
            stderr = bounded_text(completed.stderr)
            passed = completed.returncode == expected_code and completed.stdout == expected_stdout
            if stderr_rule == "one-line-stderr":
                passed = passed and len([line for line in stderr.splitlines() if line.strip()]) == 1
            row = {
                "name": name,
                "status": "passed" if passed else "failed",
                "exit_code": completed.returncode,
                "seconds": round(time.monotonic() - started, 3),
                "stdout_bytes": len(completed.stdout),
                "stderr": stderr,
            }
        except subprocess.TimeoutExpired:
            row = {"name": name, "status": "timeout"}
        result["checks"].append(row)
    result["accepted"] = all(row["status"] == "passed" for row in result["checks"])
    return result


def launch_trial(
    repo: Path,
    artifacts: Path,
    commit: str,
    trial: dict[str, Any],
    settings: dict[str, Any],
    semaprax_bin: Path,
) -> dict[str, Any]:
    arm = trial["arm"]
    number = trial["number"]
    label = f"{arm}-{number:02d}"
    workspace = artifacts / "worktrees" / label
    workspace.parent.mkdir(parents=True, exist_ok=True)
    added = subprocess.run(
        ["git", "worktree", "add", "--detach", str(workspace), commit],
        cwd=repo,
        text=True,
        capture_output=True,
        check=False,
    )
    row: dict[str, Any] = {**trial, "workspace": str(workspace), "status": "failed", "failure": None}
    if added.returncode:
        row["failure"] = f"worktree creation failed: {bounded_text(added.stderr)}"
        return row

    candidate = workspace / "benchmarks" / "cli-tokens-v1" / "candidate"
    candidate.mkdir(parents=True, exist_ok=True)
    prompt = prompt_for(arm, candidate, semaprax_bin)
    prompt_path = artifacts / "prompts" / f"{label}.txt"
    prompt_path.parent.mkdir(parents=True, exist_ok=True)
    prompt_path.write_text(prompt, encoding="utf-8")
    row["prompt_sha256"] = sha_text(prompt)
    stream_path = artifacts / "transcripts" / f"{label}.jsonl"
    stderr_path = artifacts / "transcripts" / f"{label}.stderr.txt"
    stream_path.parent.mkdir(parents=True, exist_ok=True)
    command = [
        "claude",
        "--print",
        "--output-format",
        "stream-json",
        "--verbose",
        "--model",
        settings["model"],
        "--effort",
        settings["effort"],
        "--no-session-persistence",
        "--permission-mode",
        "acceptEdits",
        "--permission-prompts",
        "none",
    ]
    if settings.get("max_budget_usd") is not None:
        command.extend(["--max-budget-usd", str(settings["max_budget_usd"])])
    command.append(prompt)
    env = os.environ.copy()
    env["SEMAPRAX_BIN"] = str(semaprax_bin)
    env["PATH"] = str(semaprax_bin.parent) + os.pathsep + env.get("PATH", "")

    started = time.monotonic()
    timed_out = False
    try:
        with stream_path.open("wb") as output, stderr_path.open("wb") as errors:
            process = subprocess.Popen(command, cwd=workspace, stdout=output, stderr=errors, env=env)
            try:
                exit_code = process.wait(timeout=settings["timeout_seconds"])
            except subprocess.TimeoutExpired:
                timed_out = True
                process.terminate()
                try:
                    exit_code = process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    exit_code = process.wait()
    except OSError as error:
        exit_code = None
        row["failure"] = str(error)
    row["elapsed_seconds"] = round(time.monotonic() - started, 3)
    row["process_exit_code"] = exit_code
    row["timed_out"] = timed_out
    row["transcript"] = str(stream_path)
    row["stderr_path"] = str(stderr_path)
    usage = stream_usage(stream_path) if stream_path.exists() else {
        "models_observed": [], "turns_with_usage": 0, "usage": {}, "first_turn_usage": {},
        "visible_output_bytes": 0, "result_event": None, "invalid_stream_lines": 0,
    }
    row["observed"] = usage
    row["list_price_estimate_usd"] = (
        rate_card_estimate(usage["usage"]) if usage["turns_with_usage"] else None
    )
    row["provider_receipt_actual_usd"] = None
    expected_model = settings["model"]
    model_matches = bool(usage["models_observed"]) and usage["models_observed"] == [expected_model]
    if timed_out:
        row["failure"] = row.get("failure") or "trial hit wall-clock timeout"
    elif exit_code != 0:
        row["failure"] = row.get("failure") or f"Claude Code exited with {exit_code}"
    elif not model_matches:
        row["failure"] = "observed model missing or differs from pinned model"
    else:
        row["acceptance"] = check_program(
            candidate,
            BENCHMARK / "sample.log",
            settings["timeout_seconds"],
            env,
        )
        row["status"] = "accepted" if row["acceptance"]["accepted"] else "not_accepted"
        if row["status"] != "accepted":
            row["failure"] = "candidate failed independent build or acceptance checks"
    return row


def summarize(results: list[dict[str, Any]]) -> dict[str, Any]:
    rows = []
    for arm in ARMS:
        selected = [row for row in results if row["arm"] == arm]
        rows.append({
            "arm": arm,
            "attempts": len(selected),
            "accepted": sum(row.get("status") == "accepted" for row in selected),
            "acceptance_rate": (
                sum(row.get("status") == "accepted" for row in selected) / len(selected)
                if selected else None
            ),
            "gross_input_tokens": sum(row.get("observed", {}).get("usage", {}).get("input_tokens", 0) for row in selected),
            "cache_write_input_tokens": sum(row.get("observed", {}).get("usage", {}).get("cache_creation_input_tokens", 0) for row in selected),
            "cache_read_input_tokens": sum(row.get("observed", {}).get("usage", {}).get("cache_read_input_tokens", 0) for row in selected),
            "output_tokens": sum(row.get("observed", {}).get("usage", {}).get("output_tokens", 0) for row in selected),
            "turns": sum(row.get("observed", {}).get("turns_with_usage", 0) for row in selected),
            "first_turn_input_tokens": [row.get("observed", {}).get("first_turn_usage", {}).get("input_tokens") for row in selected],
            "list_price_estimates_usd": [row.get("list_price_estimate_usd") for row in selected],
            "provider_receipt_actual_usd": None,
            "failures": [
                {"trial": row["number"], "reason": row.get("failure")}
                for row in selected if row.get("status") != "accepted"
            ],
        })
    return {"arms": rows, "attempt_denominator": sum(row["attempts"] for row in rows)}


def add_common_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--repo", default=str(REPO))
    parser.add_argument("--base-ref", required=True, help="verified compiler commit used for every isolated trial")
    parser.add_argument("--artifacts", required=True, help="new absolute or relative directory for this campaign")
    parser.add_argument("--trials-per-arm", type=int, default=MIN_TRIALS_PER_ARM)
    parser.add_argument("--model", default=MODEL)
    parser.add_argument("--effort", default=EFFORT)
    parser.add_argument("--timeout-seconds", type=int, default=1800)
    parser.add_argument("--max-budget-usd", type=float, default=None)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="action", required=True)
    for action in ("plan", "run"):
        sub = subparsers.add_parser(action)
        add_common_arguments(sub)
        if action == "run":
            sub.add_argument("--semaprax-bin", required=True, help="verified compiler executable shared across trials")
    args = parser.parse_args()
    try:
        settings = plan(args)
        if args.action == "plan":
            print(json.dumps(settings, indent=2))
            return 0
        semaprax_bin = Path(args.semaprax_bin).expanduser().resolve(strict=True)
        if not semaprax_bin.is_file():
            raise ValueError("semaprax-bin must be a regular executable file")
        artifacts = Path(settings["artifacts"])
        artifacts.mkdir(parents=True)
        settings["semaprax_binary"] = str(semaprax_bin)
        settings["semaprax_binary_sha256"] = digest(semaprax_bin)
        settings["claude_version"] = subprocess.run(
            ["claude", "--version"], text=True, capture_output=True, check=False
        ).stdout.strip()
        node_version = subprocess.run(
            ["node", "--version"], text=True, capture_output=True, check=False
        )
        settings["node_version"] = node_version.stdout.strip() if node_version.returncode == 0 else None
        settings["node_binary"] = shutil.which("node")
        save_json(artifacts / "campaign.json", settings)
        rows = []
        trial_number = {arm: 0 for arm in ARMS}
        for arm in settings["trial_order"]:
            trial_number[arm] += 1
            row = launch_trial(
                Path(settings["repository"]),
                artifacts,
                settings["repository_commit"],
                {"arm": arm, "number": trial_number[arm]},
                settings,
                semaprax_bin,
            )
            rows.append(row)
            save_json(artifacts / "results.json", {"campaign": settings, "trials": rows, "summary": summarize(rows)})
            print(f"{arm} {trial_number[arm]}/{settings['trials_per_arm']}: {row.get('status', 'failed')}", flush=True)
        return 0
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"live campaign error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
