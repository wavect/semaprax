#!/usr/bin/env python3
"""Prepare, launch, and independently validate matched LogLens live trials."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from statistics import mean, median
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from oracle import analyse as oracle_analyse
from oracle import json_line as oracle_json_line
from oracle import text as oracle_text

import live_campaign_common as shared

BENCHMARK = Path(__file__).resolve().parent
REPO = BENCHMARK.parents[1]
MODEL = "claude-sonnet-5-5"
EFFORT = "medium"
PRICE_BOOK_DATE = "2026-10-07"
PRICE_BOOK_SOURCE = "https://platform.claude.com/docs/en/models/sonnet-5-5/overview"
PRICE_USD_PER_MTOK = {
    "input": 2.0,
    "cache_write_5m": 2.5,
    "cache_write_1h": 4.0,
    "cache_read": 0.2,
    "output": 10.0,
}
ARMS = ("semaprax", "typescript")
MIN_TRIALS_PER_ARM = 5
MAX_LOG_BYTES = 1_000_000
TOKENIZER_PACKAGE = "@anthropic-ai/tokenizer"
TOKENIZER_VERSION = "0.0.4"
TOKENIZER_ENCODING = "package-bundled Claude BPE (`claude.json`), NFKC-normalized by countTokens"
AUTHORED_SUFFIXES = {
    ".spx", ".ts", ".tsx", ".js", ".mjs", ".cjs", ".py", ".sh", ".rs",
    ".json", ".jsonc", ".toml", ".yaml", ".yml", ".md", ".txt", ".lock",
    ".ini", ".cfg", ".conf", ".xml", ".properties", ".html", ".css",
}
AUTHORED_SPECIAL_NAMES = {".gitignore", "Makefile"}
AUTHORED_EXCLUDED_DIRS = {
    ".git", ".cache", ".mypy_cache", ".pytest_cache", ".ruff_cache",
    ".semaprax", ".spx-cache", "__pycache__", "acceptance-fixtures", "build",
    "coverage", "dist", "generated", "gen", "node_modules", "out", "target",
}
ARCHIVE_EXCLUDED_DIRS = {
    "node_modules", ".cache", "__pycache__", ".pytest_cache",
}
SEED_FILES = (
    "/benchmarks/cli-tokens-v1/SPEC.md",
    "/benchmarks/cli-tokens-v1/sample.log",
)
USAGE_FIELDS = (
    "input_tokens",
    "cache_creation_input_tokens",
    "cache_read_input_tokens",
    "output_tokens",
)
CACHE_TTL_USAGE_FIELDS = (
    "cache_creation_ephemeral_5m_input_tokens",
    "cache_creation_ephemeral_1h_input_tokens",
)
ALL_USAGE_FIELDS = (*USAGE_FIELDS, *CACHE_TTL_USAGE_FIELDS)
CALIBRATION_PROMPT = (
    "This is a context calibration request. Reply with exactly READY; "
    "do not use tools or read files."
)


def digest(path: Path) -> str:
    return shared.digest(path)


def archive_candidate(candidate: Path, archive: Path) -> tuple[dict[str, str], list[str]]:
    return shared.archive_candidate(candidate, archive)


def tokenizer_metadata(tokenizer_dir: str | Path | None) -> dict[str, Any] | None:
    return shared.tokenizer_metadata(tokenizer_dir)


TOKENIZE_SCRIPT = shared.TOKENIZE_SCRIPT


def tokenize_texts(texts: list[dict[str, str]], metadata: dict[str, Any]) -> list[int]:
    return shared.tokenize_texts(texts, metadata)


def authored_source_metrics(candidate: Path, metadata: dict[str, Any] | None) -> dict[str, Any]:
    return shared.authored_source_metrics(candidate, metadata)


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


def create_seed_repository(source_repo: Path, source_commit: str, seed_repo: Path) -> dict[str, Any]:
    return shared.create_seed_repository(source_repo, source_commit, seed_repo, SEED_FILES)


def sha_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def prompt_for(arm: str, candidate: Path, semaprax_bin: Path) -> str:
    language = "SEMAPRAX" if arm == "semaprax" else "TypeScript with Node.js"
    return f"""Implement an idiomatic, complete LogLens command-line application specified in
`benchmarks/cli-tokens-v1/SPEC.md` using {language}. The entire implementation
must live under `{candidate}`. Do not change compiler or harness files.

Provide executable `candidate/build.sh`, `candidate/run.sh`, and
`candidate/test.sh` files. The build script must compile or validate the
implementation. The run script must implement
`loglens <file> [--top N] [--json]`. The test script must run your automated
tests, including the two exact sample golden cases in the specification and
checks for exit statuses 0, 1, and 2; it must exit nonzero if any test fails.
Use the public sample input at `../sample.log` from the candidate directory.
For SEMAPRAX, use the verified compiler executable at `{semaprax_bin}` (also
available as `$SEMAPRAX_BIN`). For TypeScript, use Node from `PATH`.

Read the specification and public sample input. The trial checkout contains
only those two benchmark files; the independent acceptance corpus and oracle
are not provided in the trial checkout. Do not inspect other repository
checkouts, benchmark results, or oracles. The verified compiler executable
and its embedded help are permitted. The golden outputs are included inline in
the specification; its linked `expected.txt` and `expected.json` files are not
provided. Run your own relevant checks and finish
by stating which files you wrote and whether the program is complete.
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
    if args.timeout_seconds <= 0:
        raise ValueError("timeout-seconds must be positive")
    if args.max_budget_usd is not None and args.max_budget_usd <= 0:
        raise ValueError("max-budget-usd must be positive when supplied")
    if args.model != MODEL:
        raise ValueError(f"this matched campaign pins --model {MODEL}")
    if args.effort != EFFORT:
        raise ValueError(f"this matched campaign pins --effort {EFFORT}")
    tokenizer = tokenizer_metadata(getattr(args, "tokenizer_dir", None))
    return {
        "schema": "semaprax.cli-tokens.campaign.v1",
        "created_at_utc": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "benchmark": "cli-tokens-v1",
        "round": 3,
        "repository_commit": commit,
        "artifacts": str(artifacts),
        "model": args.model,
        "effort": args.effort,
        "trials_per_arm": args.trials_per_arm,
        "attempt_denominator": 2 * args.trials_per_arm,
        "timeout_seconds": args.timeout_seconds,
        "max_budget_usd": args.max_budget_usd,
        "authored_source_tokenizer": tokenizer,
        "calibration": {
            "mode": "matched empty-task calibration session before trials",
            "prompt_sha256": sha_text(CALIBRATION_PROMPT),
            "prompt": CALIBRATION_PROMPT,
            "usage_convention": "calibration is reported separately as a one-turn diagnostic; no calibration proxy is subtracted from trial usage",
        },
        "price_book": {
            "date": PRICE_BOOK_DATE,
            "currency": "USD",
            "source_url": PRICE_BOOK_SOURCE,
            "per_million_tokens": PRICE_USD_PER_MTOK,
            "claim": "list-price estimate only; provider-reported API-equivalent total is separate and is not a billing receipt",
            "cache_write_fallback": "when provider TTL breakdown is unavailable, price all cache_creation_input_tokens at the 5-minute rate",
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
        "trial_checkout": {
            "mode": "fresh minimal seed Git repository created from pinned source commit; one parentless commit",
            "included_files": list(SEED_FILES),
            "candidate_path": "benchmarks/cli-tokens-v1/candidate/",
            "excluded": "all other repository files and all original repository history/objects",
        },
    }


def _usage_values(usage: Any) -> dict[str, int | None]:
    return shared._usage_values(usage)


def _sum_usage(rows: list[dict[str, int | None]]) -> dict[str, int | None]:
    return shared._sum_usage(rows)


def stream_usage(path: Path) -> dict[str, Any]:
    return shared.stream_usage(path, MODEL)


def cache_write_pricing(usage: dict[str, int | None]) -> dict[str, Any]:
    return shared.cache_write_pricing(usage)


def rate_card_estimate_details(usage: dict[str, int | None]) -> dict[str, Any]:
    return shared.rate_card_estimate_details(usage)


def rate_card_estimate(usage: dict[str, int | None]) -> float | None:
    return rate_card_estimate_details(usage)["usd"]


def save_json(path: Path, value: Any) -> None:
    shared.save_json(path, value)


def input_tokens_total(usage: dict[str, int | None]) -> int | None:
    return shared.input_tokens_total(usage)


def legacy_net_input_metrics(turn_usage: list[dict[str, int | None]]) -> dict[str, int | None]:
    return shared.legacy_net_input_metrics(turn_usage)


def observed_model_matches(observed: Any, expected_observed_id: Any) -> bool:
    return shared.observed_model_matches(observed, expected_observed_id)


def one_turn_context_proxy(
    first_turn_usage: dict[str, int | None], calibration_prompt_tokens: int | None,
) -> int | None:
    baseline = input_tokens_total(first_turn_usage)
    if baseline is None or calibration_prompt_tokens is None or baseline < calibration_prompt_tokens:
        return None
    return baseline - calibration_prompt_tokens


def claude_command(settings: dict[str, Any], prompt: str) -> list[str]:
    return shared.claude_command(settings, prompt)


def trial_environment(semaprax_bin: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["SEMAPRAX_BIN"] = str(semaprax_bin)
    env["PATH"] = str(semaprax_bin.parent) + os.pathsep + env.get("PATH", "")
    return env


def add_seed_worktree(seed_repo: Path, workspace: Path, seed_commit: str) -> str | None:
    return shared.add_seed_worktree(seed_repo, workspace, seed_commit, SEED_FILES)


def run_claude(command: list[str], workspace: Path, env: dict[str, str], stream_path: Path, stderr_path: Path, timeout_seconds: int) -> dict[str, Any]:
    return shared.run_claude(command, workspace, env, stream_path, stderr_path, timeout_seconds)


def launch_calibration(
    repo: Path,
    artifacts: Path,
    commit: str,
    settings: dict[str, Any],
    semaprax_bin: Path,
) -> dict[str, Any]:
    workspace = artifacts / "worktrees" / "calibration"
    workspace.parent.mkdir(parents=True, exist_ok=True)
    row: dict[str, Any] = {"name": "matched-empty-task-context", "workspace": str(workspace)}
    error = add_seed_worktree(repo, workspace, commit)
    if error:
        row.update({"status": "failed", "failure": error, "workspace_retained_for_review": True})
        return row
    prompt_path = artifacts / "prompts" / "calibration.txt"
    prompt_path.parent.mkdir(parents=True, exist_ok=True)
    prompt_path.write_text(CALIBRATION_PROMPT, encoding="utf-8")
    stream_path = artifacts / "transcripts" / "calibration.jsonl"
    stderr_path = artifacts / "transcripts" / "calibration.stderr.txt"
    stream_path.parent.mkdir(parents=True, exist_ok=True)
    process = run_claude(
        claude_command(settings, CALIBRATION_PROMPT),
        workspace,
        trial_environment(semaprax_bin),
        stream_path,
        stderr_path,
        settings["timeout_seconds"],
    )
    row.update(process)
    row["transcript"] = str(stream_path)
    row["stderr_path"] = str(stderr_path)
    row["prompt_sha256"] = sha_text(CALIBRATION_PROMPT)
    observed = stream_usage(stream_path) if stream_path.exists() else {
        "models_observed": [], "turns_with_usage": 0,
        "usage": {name: None for name in ALL_USAGE_FIELDS},
        "first_turn_usage": {name: None for name in ALL_USAGE_FIELDS},
    }
    row["observed"] = observed
    tokenizer = settings.get("authored_source_tokenizer")
    prompt_proxy = None
    if tokenizer is not None:
        try:
            prompt_proxy = tokenize_texts(
                [{"path": "calibration-prompt", "content": CALIBRATION_PROMPT}], tokenizer
            )[0]
        except (OSError, RuntimeError, UnicodeError, json.JSONDecodeError, subprocess.SubprocessError) as error:
            row["tokenizer_error"] = str(error)
    row["calibration_prompt_tokens_legacy_proxy"] = prompt_proxy
    baseline = input_tokens_total(observed.get("first_turn_usage", {}))
    row["first_turn_provider_input_plus_cache_tokens"] = baseline
    row["one_turn_context_input_tokens_proxy"] = one_turn_context_proxy(
        observed.get("first_turn_usage", {}), prompt_proxy
    )
    row["context_diagnostic_note"] = (
        "One-turn diagnostic only: provider first-turn input plus cache minus this fixed calibration prompt's legacy tokenizer proxy. "
        "It is not task-only or exact net input; trial contexts repeat across turns and grow with task/tool history. No value is subtracted from trial totals."
    )
    estimate = rate_card_estimate_details(observed.get("usage", {}))
    row["list_price_estimate_usd"] = estimate["usd"]
    row["list_price_cache_write_pricing"] = estimate["cache_write_pricing"]
    row["provider_reported_api_equivalent_total_cost_usd"] = observed.get(
        "provider_reported_api_equivalent_total_cost_usd"
    )
    row["provider_receipt_actual_usd"] = None
    observed_models = observed.get("models_observed", [])
    model_matches = len(observed_models) == 1
    if model_matches:
        row["observed_model_id"] = observed_models[0]
    tokenizer_ready = tokenizer is None or row["one_turn_context_input_tokens_proxy"] is not None
    no_tool_use = observed.get("tool_use_events", 0) == 0
    row["status"] = (
        "ready"
        if process["process_exit_code"] == 0 and not process["timed_out"] and model_matches and tokenizer_ready and no_tool_use
        else "failed"
    )
    if row["status"] != "ready":
        row["failure"] = row.get("failure") or "calibration tool-use, counters, tokenizer proxy, exit, or unique observed model did not match the pinned configuration"
    status = subprocess.run(
        ["git", "status", "--porcelain", "--untracked-files=all"],
        cwd=workspace, text=True, capture_output=True, check=False,
    )
    if status.returncode or status.stdout.strip():
        row["workspace_retained_for_review"] = True
        row["unexpected_workspace_changes"] = status.stdout.splitlines()
    else:
        removed = subprocess.run(
            ["git", "worktree", "remove", "--force", str(workspace)],
            cwd=repo, text=True, capture_output=True, check=False,
        )
        row["workspace_removed"] = removed.returncode == 0
        if removed.returncode:
            row["workspace_retained_for_review"] = True
            row["failure"] = row.get("failure") or f"calibration worktree cleanup failed: {bounded_text(removed.stderr)}"
    save_json(artifacts / "calibration.json", row)
    return row


def bounded_text(value: bytes | str) -> str:
    return shared.bounded_text(value)


def check_program(
    candidate: Path,
    timeout: float,
    env: dict[str, str],
) -> dict[str, Any]:
    build = candidate / "build.sh"
    program = candidate / "run.sh"
    result: dict[str, Any] = {
        "build": {"status": "missing"},
        "candidate_tests": {"status": "not_run"},
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
        result["candidate_tests"] = {"status": "not_run", "reason": "build failed"}
        return result

    candidate_test = candidate / "test.sh"
    if not candidate_test.is_file():
        result["candidate_tests"] = {"status": "missing", "path": str(candidate_test)}
        return result
    started = time.monotonic()
    try:
        tested = subprocess.run(
            ["/bin/sh", str(candidate_test)],
            cwd=candidate,
            text=True,
            capture_output=True,
            check=False,
            timeout=timeout,
            env=env,
        )
    except subprocess.TimeoutExpired as error:
        result["candidate_tests"] = {
            "status": "timeout",
            "seconds": round(time.monotonic() - started, 3),
            "stdout": bounded_text(error.stdout or b""),
            "stderr": bounded_text(error.stderr or b""),
        }
        return result
    result["candidate_tests"] = {
        "status": "passed" if tested.returncode == 0 else "failed",
        "exit_code": tested.returncode,
        "seconds": round(time.monotonic() - started, 3),
        "stdout": bounded_text(tested.stdout),
        "stderr": bounded_text(tested.stderr),
    }
    if tested.returncode:
        return result

    fixtures = candidate / "acceptance-fixtures"
    fixtures.mkdir(exist_ok=True)
    def write_fixture(name: str, content: str) -> str:
        (fixtures / name).write_bytes(content.encode("utf-8"))
        return f"acceptance-fixtures/{name}"

    sample_rel = write_fixture("sample.log", (BENCHMARK / "sample.log").read_text(encoding="utf-8"))
    def record(path: str, status: int = 200, size: str = "10", hour: str = "10") -> str:
        return f'203.0.113.1 - - [10/Oct/2026:{hour}:00:00 +0000] "GET {path} HTTP/1.1" {status} {size}'

    ties_rel = write_fixture("ties.log", "\n".join([
        record("/b"), record("/a"), record("/b"), record("/a"),
        record("/one", status=101), record("/two", status=199),
    ]) + "\n")
    malformed_rel = write_fixture("malformed.log", "not a log line\n\n" + record("/bad", status=600) + "\n")
    rounding_rel = write_fixture("rounding.log", "\n".join(
        [record("/rounded", status=(404 if index == 0 else 200), size=("-" if index == 1 else "1"))
         for index in range(8)]
    ) + "\n")
    empty_rel = write_fixture("empty.log", "")
    blank_rel = write_fixture("blanks.log", "\n\n" + record("/blank") + "\n\n")
    crlf_rel = write_fixture("crlf.log", (record("/crlf") + "\n" + record("/crlf", hour="09") + "\n").replace("\n", "\r\n"))
    cr_rel = write_fixture("cr.log", (record("/cr") + "\n" + record("/cr", hour="09") + "\n").replace("\n", "\r"))

    def expected(path: str, top: int, as_json: bool = False) -> bytes:
        content = (candidate / path).read_text(encoding="utf-8")
        report = oracle_analyse(content, top)
        return (oracle_json_line(report) if as_json else oracle_text(report)).encode("utf-8")

    cases: list[tuple[str, list[str], int, bytes, bool]] = []
    for label, relative, top in (
        ("checked-in-sample", sample_rel, 5),
        ("empty-input", empty_rel, 5),
        ("malformed-input", malformed_rel, 5),
        ("half-up-rounding-and-dash-size", rounding_rel, 5),
        ("ties-and-1xx", ties_rel, 5),
        ("blank-lines", blank_rel, 5),
        ("crlf-line-endings", crlf_rel, 5),
        ("cr-line-endings", cr_rel, 5),
    ):
        cases.append((f"{label}-text", [relative], 0, expected(relative, top), False))
        cases.append((f"{label}-json", [relative, "--json"], 0, expected(relative, top, True), False))
    for top in (1, 2, 3, 5, 50):
        relative = ties_rel if top in (1, 2, 50) else sample_rel
        cases.append((f"top-boundary-{top}", [relative, "--top", str(top)], 0, expected(relative, top), False))
    cases.extend([
        ("options-json-before-top", [ties_rel, "--json", "--top", "1"], 0, expected(ties_rel, 1, True), False),
        ("options-top-before-json", [ties_rel, "--top", "1", "--json"], 0, expected(ties_rel, 1, True), False),
        ("spec-sample-top3-json", [sample_rel, "--top", "3", "--json"], 0, expected(sample_rel, 3, True), False),
        ("missing-file", ["missing-loglens-input.log"], 1, b"", True),
        ("missing-file-argument", [], 2, b"", True),
        ("unknown-flag", [sample_rel, "--unknown"], 2, b"", True),
        ("missing-top-value", [sample_rel, "--top"], 2, b"", True),
        ("top-zero", [sample_rel, "--top", "0"], 2, b"", True),
        ("top-too-large", [sample_rel, "--top", "51"], 2, b"", True),
        ("top-negative", [sample_rel, "--top", "-1"], 2, b"", True),
        ("top-noninteger", [sample_rel, "--top", "abc"], 2, b"", True),
        ("top-decimal", [sample_rel, "--top", "1.0"], 2, b"", True),
    ])
    for name, arguments, expected_code, expected_stdout, stderr_required in cases:
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
            if stderr_required:
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
    row: dict[str, Any] = {**trial, "workspace": str(workspace), "status": "failed", "failure": None}
    checkout_error = add_seed_worktree(repo, workspace, commit)
    if checkout_error:
        row["failure"] = checkout_error
        row["workspace_retained_for_review"] = True
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
    env = trial_environment(semaprax_bin)
    process = run_claude(
        claude_command(settings, prompt), workspace, env,
        stream_path, stderr_path, settings["timeout_seconds"],
    )
    row.update(process)
    row["transcript"] = str(stream_path)
    row["stderr_path"] = str(stderr_path)
    usage = stream_usage(stream_path) if stream_path.exists() else {
        "models_observed": [], "turns_with_usage": 0, "usage": {}, "first_turn_usage": {},
        "visible_output_bytes": 0, "result_event": None, "invalid_stream_lines": 0,
    }
    row["observed"] = usage
    estimate = rate_card_estimate_details(usage["usage"])
    row["list_price_estimate_usd"] = estimate["usd"]
    row["list_price_cache_write_pricing"] = estimate["cache_write_pricing"]
    row["provider_reported_api_equivalent_total_cost_usd"] = usage.get(
        "provider_reported_api_equivalent_total_cost_usd"
    )
    row["provider_receipt_actual_usd"] = None
    expected_model = settings.get("observed_model_id")
    model_matches = observed_model_matches(usage.get("models_observed"), expected_model)
    if process["timed_out"]:
        row["failure"] = row.get("failure") or "trial hit wall-clock timeout"
    elif process["process_exit_code"] != 0:
        row["failure"] = row.get("failure") or f"Claude Code exited with {process['process_exit_code']}"
    elif not model_matches:
        row["failure"] = "observed model missing or differs from calibration model"
    else:
        acceptance_started = time.monotonic()
        row["acceptance"] = check_program(candidate, settings["timeout_seconds"], env)
        row["acceptance_elapsed_seconds"] = round(time.monotonic() - acceptance_started, 3)
        row["status"] = "accepted" if row["acceptance"]["accepted"] else "not_accepted"
        if row["status"] != "accepted":
            row["failure"] = "candidate failed independent build or acceptance checks"
    try:
        row["final_candidate_source_metrics"] = authored_source_metrics(
            candidate, settings.get("authored_source_tokenizer")
        )
    except (OSError, RuntimeError, UnicodeError, json.JSONDecodeError) as error:
        row["final_candidate_source_metrics"] = {
            "status": "measurement_failed", "total_tokens": None,
            "files": [], "tokenizer": settings.get("authored_source_tokenizer"),
            "error": str(error),
        }
    row["provider_input_plus_cache_tokens_raw"] = input_tokens_total(usage.get("usage", {}))
    legacy_net = usage.get("legacy_net_input", {})
    row["legacy_net_input_tokens"] = legacy_net.get("net_input_tokens")
    row["legacy_net_first_turn_input_plus_cache_tokens"] = legacy_net.get(
        "first_turn_input_plus_cache_tokens"
    )
    row["legacy_net_baseline_tokens_subtracted"] = legacy_net.get("baseline_tokens_subtracted")
    row["input_usage_note"] = (
        "Raw provider-reported input and cache counters. These include repeated system/tool context on every turn "
        "and task/tool history; the separate one-turn calibration diagnostic is not subtracted."
    )
    # Archive candidate files, excluding only known dependency and cache dirs,
    # before deleting the disposable isolated checkout. Any write outside the
    # advertised candidate directory keeps the checkout for inspection.
    status = subprocess.run(
        ["git", "status", "--porcelain", "--untracked-files=all"],
        cwd=workspace, text=True, capture_output=True, check=False,
    )
    changed = [line[3:] for line in status.stdout.splitlines() if len(line) >= 4]
    candidate_prefix = "benchmarks/cli-tokens-v1/candidate/"
    if status.returncode or any(not path.startswith(candidate_prefix) for path in changed):
        row["workspace_retained_for_review"] = True
        row["unexpected_workspace_changes"] = changed
        row["failure"] = row.get("failure") or "trial modified files outside candidate/"
        return row
    archive = artifacts / "candidates" / label
    archive.parent.mkdir(parents=True, exist_ok=True)
    archived_files, excluded_paths = archive_candidate(candidate, archive)
    save_json(archive.parent / f"{label}.manifest.json", {
        "trial": label,
        "archive_kind": "rebuildable candidate archive with dependency/cache directories excluded",
        "runnable_without_build": False,
        "excluded_directory_names": sorted(ARCHIVE_EXCLUDED_DIRS),
        "excluded_paths": excluded_paths,
        "files_sha256": archived_files,
        "candidate_archive": str(archive),
    })
    row["candidate_archive"] = str(archive)
    row["candidate_files_sha256"] = archived_files
    row["candidate_archive_excluded_paths"] = excluded_paths
    removed = subprocess.run(
        ["git", "worktree", "remove", "--force", str(workspace)],
        cwd=repo, text=True, capture_output=True, check=False,
    )
    row["workspace_removed"] = removed.returncode == 0
    if removed.returncode:
        row["workspace_retained_for_review"] = True
        row["failure"] = row.get("failure") or f"worktree cleanup failed: {bounded_text(removed.stderr)}"
    return row


def summarize(
    results: list[dict[str, Any]], calibration_result: dict[str, Any] | None = None,
) -> dict[str, Any]:
    rows = []
    for arm in ARMS:
        selected = [row for row in results if row["arm"] == arm]
        accepted = sum(row.get("status") == "accepted" for row in selected)
        per_trial_costs = [row.get("list_price_estimate_usd") for row in selected]
        cache_write_pricing = [row.get("list_price_cache_write_pricing") for row in selected]
        provider_costs = [row.get("provider_reported_api_equivalent_total_cost_usd") for row in selected]
        known_provider_costs = [value for value in provider_costs if value is not None]
        known_costs = [value for value in per_trial_costs if value is not None]
        known_cost = sum(known_costs) if known_costs else None
        usage_totals: dict[str, int | None] = {}
        usage_missing: dict[str, int] = {}
        for field in ALL_USAGE_FIELDS:
            values = [row.get("observed", {}).get("usage", {}).get(field) for row in selected]
            known = [value for value in values if value is not None]
            usage_totals[field] = sum(known) if known else None
            usage_missing[field] = len(values) - len(known)
        elapsed = [row.get("elapsed_seconds") for row in selected if row.get("elapsed_seconds") is not None]
        gross_input = [row.get("provider_input_plus_cache_tokens_raw") for row in selected]
        legacy_net = [row.get("legacy_net_input_tokens") for row in selected]
        legacy_baseline = [
            row.get("legacy_net_first_turn_input_plus_cache_tokens") for row in selected
        ]
        legacy_subtracted = [row.get("legacy_net_baseline_tokens_subtracted") for row in selected]
        authored = [row.get("final_candidate_source_metrics", {}).get("total_tokens") for row in selected]
        complete_cost = len(per_trial_costs) == len(selected) and all(value is not None for value in per_trial_costs)
        calibration_tokens = (
            calibration_result.get("one_turn_context_input_tokens_proxy")
            if calibration_result else None
        )
        rows.append({
            "arm": arm,
            "attempts": len(selected),
            "accepted": accepted,
            "acceptance_rate": (
                accepted / len(selected)
                if selected else None
            ),
            "provider_usage_totals_known_subtotal": usage_totals,
            "provider_usage_missing_trial_counts": usage_missing,
            "input_and_cache_are_separate_buckets": True,
            "provider_input_plus_cache_tokens_raw_per_trial": gross_input,
            "provider_input_plus_cache_tokens_raw_known_subtotal": (
                sum(value for value in gross_input if value is not None)
                if any(value is not None for value in gross_input) else None
            ),
            "provider_input_plus_cache_tokens_raw_incomplete_trials": sum(value is None for value in gross_input),
            "legacy_net_input_tokens_per_trial": legacy_net,
            "legacy_net_input_tokens_known_subtotal": (
                sum(value for value in legacy_net if value is not None)
                if any(value is not None for value in legacy_net) else None
            ),
            "legacy_net_input_tokens_incomplete_trials": sum(value is None for value in legacy_net),
            "legacy_net_first_turn_input_plus_cache_tokens_per_trial": legacy_baseline,
            "legacy_net_baseline_tokens_subtracted_per_trial": legacy_subtracted,
            "legacy_net_input_definition": (
                "deduplicated per-turn input + cache-write + cache-read sum minus first-turn input + cache total times turn count; "
                "first-turn baseline includes the task prompt and harness context"
            ),
            "one_turn_calibration_context_input_tokens_proxy": calibration_tokens,
            "context_baseline_applied_to_trial_totals": False,
            "context_accounting_note": (
                "Raw provider counters include repeated fixed context on every turn plus task and tool history. "
                "The separate single-turn calibration is diagnostic only and is not a task-only or net-input estimate."
            ),
            "final_candidate_source_token_proxy_per_trial": authored,
            "final_candidate_source_token_proxy_known_subtotal": (
                sum(value for value in authored if value is not None)
                if any(value is not None for value in authored) else None
            ),
            "final_candidate_source_token_proxy_incomplete_trials": sum(value is None for value in authored),
            "authored_source_token_claim": "legacy-Claude tokenizer proxy over final candidate source inventory only; excludes rewritten/deleted text and is not cumulative authored generation, provider output, current-model tokens, or billing tokens",
            "turns": sum(row.get("observed", {}).get("turns_with_usage", 0) for row in selected),
            "per_trial_model_session_wall_seconds": elapsed,
            "aggregate_model_session_wall_seconds": round(sum(elapsed), 3),
            "mean_model_session_wall_seconds": round(mean(elapsed), 3) if elapsed else None,
            "median_model_session_wall_seconds": round(median(elapsed), 3) if elapsed else None,
            "list_price_estimate_per_attempt_usd": per_trial_costs,
            "list_price_cache_write_pricing_per_attempt": cache_write_pricing,
            "list_price_estimate_known_subtotal_usd": round(known_cost, 6) if known_cost is not None else None,
            "list_price_estimate_complete": complete_cost,
            "list_price_estimate_total_usd": round(known_cost, 6) if complete_cost and known_cost is not None else None,
            "estimated_cost_per_accepted_task_usd": (
                round(known_cost / accepted, 6) if complete_cost and accepted and known_cost is not None else None
            ),
            "accepted_task_cost_note": "All attempts, including failed and rejected trials, are included in the numerator.",
            "provider_receipt_actual_usd": None,
            "provider_reported_api_equivalent_total_cost_usd_per_attempt": provider_costs,
            "provider_reported_api_equivalent_cost_known_subtotal_usd": (
                round(sum(known_provider_costs), 6) if known_provider_costs else None
            ),
            "failures": [
                {"trial": row["number"], "reason": row.get("failure")}
                for row in selected if row.get("status") != "accepted"
            ],
        })
    calibration_cost = calibration_result.get("list_price_estimate_usd") if calibration_result else None
    attempt_costs = [row.get("list_price_estimate_usd") for row in results]
    accepted = sum(row.get("status") == "accepted" for row in results)
    campaign_cost_complete = (
        calibration_cost is not None
        and len(attempt_costs) == len(results)
        and all(value is not None for value in attempt_costs)
    )
    total_estimated_cost = calibration_cost + sum(attempt_costs) if campaign_cost_complete else None
    return {
        "arms": rows,
        "attempt_denominator": sum(row["attempts"] for row in rows),
        "shared_calibration": {
            "list_price_estimate_usd": calibration_cost,
            "provider_receipt_actual_usd": None,
            "provider_reported_api_equivalent_total_cost_usd": (
                calibration_result.get("provider_reported_api_equivalent_total_cost_usd")
                if calibration_result else None
            ),
            "included_in_combined_campaign_cost": True,
        },
        "campaign_list_price_estimate_complete": campaign_cost_complete,
        "campaign_list_price_estimate_including_calibration_usd": (
            round(total_estimated_cost, 6) if total_estimated_cost is not None else None
        ),
        "campaign_estimated_cost_per_accepted_task_including_calibration_usd": (
            round(total_estimated_cost / accepted, 6)
            if total_estimated_cost is not None and accepted else None
        ),
    }


def _recount_usage_row(row: dict[str, Any], transcript: Path | None, label: str) -> None:
    if transcript is None:
        observed = {
            "models_observed": [], "turns_with_usage": 0,
            "usage": {name: None for name in ALL_USAGE_FIELDS},
            "first_turn_usage": {name: None for name in ALL_USAGE_FIELDS},
            "legacy_net_input": {
                "first_turn_input_plus_cache_tokens": None,
                "per_turn_input_plus_cache_tokens_sum": None,
                "baseline_tokens_subtracted": None,
                "net_input_tokens": None,
            },
            "provider_reported_api_equivalent_total_cost_usd": None,
            "provider_output_tokens": None,
        }
        row["accounting_status"] = "transcript_missing"
    else:
        observed = stream_usage(transcript)
        row["accounting_status"] = "recounted"
    estimate = rate_card_estimate_details(observed["usage"])
    row["observed"] = observed
    row["list_price_estimate_usd"] = estimate["usd"]
    row["list_price_cache_write_pricing"] = estimate["cache_write_pricing"]
    row["provider_reported_api_equivalent_total_cost_usd"] = observed.get(
        "provider_reported_api_equivalent_total_cost_usd"
    )
    row["provider_receipt_actual_usd"] = None
    row["provider_output_tokens"] = observed.get("provider_output_tokens")
    row["provider_input_plus_cache_tokens_raw"] = input_tokens_total(observed.get("usage", {}))
    legacy_net = observed.get("legacy_net_input", {})
    row["legacy_net_input_tokens"] = legacy_net.get("net_input_tokens")
    row["legacy_net_first_turn_input_plus_cache_tokens"] = legacy_net.get(
        "first_turn_input_plus_cache_tokens"
    )
    row["legacy_net_baseline_tokens_subtracted"] = legacy_net.get("baseline_tokens_subtracted")
    row["accounting_transcript"] = label


ROBUSTNESS_ONLY_CHECKS = {
    "crlf-line-endings-text",
    "crlf-line-endings-json",
    "cr-line-endings-text",
    "cr-line-endings-json",
    "options-json-before-top",
}


def acceptance_scope_assessment(row: dict[str, Any]) -> dict[str, Any]:
    """Report a post-run SPEC-scope view without changing full-corpus scoring."""
    acceptance = row.get("acceptance")
    checks = acceptance.get("checks", []) if isinstance(acceptance, dict) else []
    if not isinstance(checks, list):
        checks = []
    spec_checks = [check for check in checks if isinstance(check, dict)
                   and check.get("name") not in ROBUSTNESS_ONLY_CHECKS]
    robustness_checks = [check for check in checks if isinstance(check, dict)
                         and check.get("name") in ROBUSTNESS_ONLY_CHECKS]

    def status(rows: list[dict[str, Any]], absent: str) -> str:
        if not rows:
            return absent
        return "passed" if all(check.get("status") == "passed" for check in rows) else "failed"

    return {
        "original_full_corpus_accepted": acceptance.get("accepted") if isinstance(acceptance, dict) else None,
        "explicit_spec_checks": {
            "status": status(spec_checks, "not_assessed"),
            "passed": sum(check.get("status") == "passed" for check in spec_checks),
            "failed": [check.get("name") for check in spec_checks if check.get("status") != "passed"],
        },
        "additional_robustness_checks": {
            "status": status(robustness_checks, "not_tested"),
            "passed": sum(check.get("status") == "passed" for check in robustness_checks),
            "failed": [check.get("name") for check in robustness_checks if check.get("status") != "passed"],
        },
    }


def recount_results(artifacts: Path) -> Path:
    """Recompute usage and cost from saved provider JSONL without rerunning trials."""
    artifacts = artifacts.expanduser().resolve(strict=True)
    source_path = artifacts / "results.json"
    calibration_path = artifacts / "calibration.json"
    if not source_path.is_file() or not calibration_path.is_file():
        raise ValueError("recount requires existing results.json and calibration.json")
    source_results = json.loads(source_path.read_text(encoding="utf-8"))
    calibration = json.loads(calibration_path.read_text(encoding="utf-8"))
    trials = source_results.get("trials")
    if not isinstance(trials, list) or not isinstance(calibration, dict):
        raise ValueError("campaign results or calibration file has an invalid shape")

    def transcript_for(value: Any, fallback: str) -> tuple[Path | None, str]:
        raw = value if isinstance(value, str) and value else fallback
        path = Path(raw)
        if not path.is_absolute():
            path = artifacts / path
        path = path.resolve()
        try:
            label = str(path.relative_to(artifacts))
        except ValueError as error:
            raise ValueError(f"transcript is outside artifact directory: {path}") from error
        if not path.exists():
            return None, label
        if not path.is_file():
            raise ValueError(f"transcript is not a regular file: {path}")
        return path, label

    report = json.loads(json.dumps(source_results))
    report_calibration = report["calibration"] = calibration
    calibration_stream, calibration_label = transcript_for(
        calibration.get("transcript"), "transcripts/calibration.jsonl"
    )
    _recount_usage_row(report_calibration, calibration_stream, calibration_label)
    first = report_calibration["observed"].get("first_turn_usage", {})
    report_calibration["first_turn_provider_input_plus_cache_tokens"] = input_tokens_total(first)
    prompt_proxy = report_calibration.get("calibration_prompt_tokens_legacy_proxy")
    report_calibration["one_turn_context_input_tokens_proxy"] = one_turn_context_proxy(first, prompt_proxy)

    report_trials = report["trials"]
    scope_assessments = []
    transcript_records = [{
        "path": calibration_label,
        "sha256": digest(calibration_stream) if calibration_stream else None,
        "status": "recounted" if calibration_stream else "transcript_missing",
    }]
    for index, row in enumerate(report_trials):
        if not isinstance(row, dict):
            raise ValueError(f"trial row {index} is not an object")
        fallback = f"transcripts/{row.get('arm', 'unknown')}-{int(row.get('number', index + 1)):02d}.jsonl"
        transcript, label = transcript_for(row.get("transcript"), fallback)
        _recount_usage_row(row, transcript, label)
        row["post_run_acceptance_scope_assessment"] = acceptance_scope_assessment(row)
        scope_assessments.append({
            "arm": row.get("arm"), "number": row.get("number"),
            **row["post_run_acceptance_scope_assessment"],
        })
        transcript_records.append({
            "path": label,
            "sha256": digest(transcript) if transcript else None,
            "status": "recounted" if transcript else "transcript_missing",
        })

    campaign = report.get("campaign")
    if isinstance(campaign, dict) and isinstance(campaign.get("calibration_result"), dict):
        campaign_calibration = campaign["calibration_result"]
        campaign_calibration["list_price_estimate_usd"] = report_calibration.get("list_price_estimate_usd")
        campaign_calibration["observed_usage"] = report_calibration.get("observed", {}).get("usage")
        campaign_calibration["provider_reported_api_equivalent_total_cost_usd"] = report_calibration.get(
            "provider_reported_api_equivalent_total_cost_usd"
        )

    try:
        revision = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=REPO, text=True, capture_output=True, check=True
        ).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        revision = None
    report["summary"] = summarize(report_trials, report_calibration)
    campaign_settings = report.get("campaign") if isinstance(report.get("campaign"), dict) else {}
    planned_attempts = campaign_settings.get("attempt_denominator")
    robustness_failed = [
        {"arm": item["arm"], "number": item["number"], "check": name}
        for item in scope_assessments
        for name in item["additional_robustness_checks"]["failed"]
    ]
    spec_failed = [
        {"arm": item["arm"], "number": item["number"], "check": name}
        for item in scope_assessments
        for name in item["explicit_spec_checks"]["failed"]
    ]
    report["acceptance_scope_assessment"] = {
        "classification_timing": "post-run diagnostic; does not alter original scoring",
        "frozen_spec_sha256": campaign_settings.get("seed_files_sha256", {}).get(
            "benchmarks/cli-tokens-v1/SPEC.md"
        ),
        "full_corpus_acceptance": "original results and per-trial status remain authoritative",
        "explicit_spec_checks": {
            "failed_cases": spec_failed,
            "trial_assessments": sum(item["explicit_spec_checks"]["status"] != "not_assessed"
                                      for item in scope_assessments),
        },
        "additional_robustness_checks": {
            "not_specified_by_frozen_spec": sorted(ROBUSTNESS_ONLY_CHECKS),
            "failed_cases": robustness_failed,
            "trial_assessments": sum(item["additional_robustness_checks"]["status"] != "not_tested"
                                      for item in scope_assessments),
        },
        "per_trial": scope_assessments,
    }
    report["accounting"] = {
        "schema": "semaprax.cli-tokens.accounting.v1",
        "accounting_revision": revision,
        "parser_source_sha256": digest(Path(__file__).resolve()),
        "source_results_sha256": digest(source_path),
        "recounted_at_utc": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "usage_source": "saved Claude Code stream-json transcripts",
        "provider_receipt_actual_usd": None,
        "transcripts": transcript_records,
        "original_results_path": str(source_path),
        "recorded_attempts": len(report_trials),
        "planned_attempt_denominator": planned_attempts,
        "campaign_complete": isinstance(planned_attempts, int) and len(report_trials) == planned_attempts,
        "scope_assessment_is_post_run_and_not_original_scoring": True,
        "recount_does_not_rerun_models_builds_or_acceptance": True,
    }
    output = artifacts / "accounted-results.json"
    save_json(output, report)
    return output


def add_common_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--repo", default=str(REPO))
    parser.add_argument("--base-ref", required=True, help="verified compiler commit used for every isolated trial")
    parser.add_argument("--artifacts", required=True, help="new absolute or relative directory for this campaign")
    parser.add_argument("--trials-per-arm", type=int, default=MIN_TRIALS_PER_ARM)
    parser.add_argument("--model", default=MODEL)
    parser.add_argument("--effort", default=EFFORT)
    parser.add_argument("--timeout-seconds", type=int, default=1800)
    parser.add_argument("--max-budget-usd", type=float, default=None)
    parser.add_argument(
        "--tokenizer-dir",
        default=None,
        help="offline npm prefix containing @anthropic-ai/tokenizer; enables final-source and calibration-prompt proxy counts",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="action", required=True)
    recount_parser = subparsers.add_parser("recount", help="recompute accounting from saved campaign transcripts")
    recount_parser.add_argument("--artifacts", required=True, help="completed campaign artifact directory")
    for action in ("plan", "run"):
        sub = subparsers.add_parser(action)
        add_common_arguments(sub)
        if action == "run":
            sub.add_argument("--semaprax-bin", required=True, help="verified compiler executable shared across trials")
    args = parser.parse_args()
    try:
        if args.action == "recount":
            print(recount_results(Path(args.artifacts)))
            return 0
        settings = plan(args)
        if args.action == "plan":
            print(json.dumps(settings, indent=2))
            return 0
        semaprax_bin = Path(args.semaprax_bin).expanduser().resolve(strict=True)
        if not semaprax_bin.is_file():
            raise ValueError("semaprax-bin must be a regular executable file")
        artifacts = Path(settings["artifacts"])
        artifacts.mkdir(parents=True)
        seed_repo = artifacts / "seed-repository"
        seed_info = create_seed_repository(
            Path(args.repo).expanduser().resolve(strict=True),
            settings["repository_commit"],
            seed_repo,
        )
        settings.update(seed_info)
        settings["trial_checkout"]["seed_repository_commit"] = seed_info["seed_repository_commit"]
        settings["trial_checkout"]["source_files_sha256"] = seed_info["source_files_sha256"]
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
        campaign_started = time.monotonic()
        calibration_result = launch_calibration(
            seed_repo, artifacts, settings["seed_repository_commit"],
            settings, semaprax_bin,
        )
        settings["calibration_result"] = {
            "status": calibration_result.get("status"),
            "observed_model_id": calibration_result.get("observed_model_id"),
            "one_turn_context_input_tokens_proxy": calibration_result.get("one_turn_context_input_tokens_proxy"),
            "calibration_prompt_tokens_legacy_proxy": calibration_result.get("calibration_prompt_tokens_legacy_proxy"),
            "list_price_estimate_usd": calibration_result.get("list_price_estimate_usd"),
            "observed_usage": calibration_result.get("observed", {}).get("usage"),
        }
        settings["observed_model_id"] = calibration_result.get("observed_model_id")
        settings["calibration"]["result_file"] = str(artifacts / "calibration.json")
        save_json(artifacts / "campaign.json", settings)
        if calibration_result.get("status") != "ready":
            save_json(artifacts / "results.json", {
                "campaign": settings,
                "calibration": calibration_result,
                "trials": [],
                "summary": summarize([], calibration_result),
                "campaign_elapsed_wall_seconds": round(time.monotonic() - campaign_started, 3),
            })
            print("calibration failed; no benchmark trials were launched", file=sys.stderr)
            return 2
        rows = []
        trial_number = {arm: 0 for arm in ARMS}
        for arm in settings["trial_order"]:
            trial_number[arm] += 1
            row = launch_trial(
                seed_repo,
                artifacts,
                settings["seed_repository_commit"],
                {"arm": arm, "number": trial_number[arm]},
                settings,
                semaprax_bin,
            )
            rows.append(row)
            save_json(artifacts / "results.json", {
                "campaign": settings,
                "calibration": calibration_result,
                "trials": rows,
                "summary": summarize(rows, calibration_result),
                "campaign_elapsed_wall_seconds": round(time.monotonic() - campaign_started, 3),
            })
            print(f"{arm} {trial_number[arm]}/{settings['trials_per_arm']}: {row.get('status', 'failed')}", flush=True)
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"live campaign error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
