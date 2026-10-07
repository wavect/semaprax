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
from statistics import mean, median
from typing import Any

from oracle import analyse as oracle_analyse
from oracle import json_line as oracle_json_line
from oracle import text as oracle_text

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
SPARSE_FILES = (
    "/benchmarks/cli-tokens-v1/SPEC.md",
    "/benchmarks/cli-tokens-v1/sample.log",
)
USAGE_FIELDS = (
    "input_tokens",
    "cache_creation_input_tokens",
    "cache_read_input_tokens",
    "output_tokens",
)


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

Read the specification and public sample input. The trial checkout contains
only those two benchmark files; the independent acceptance corpus and oracle
are not present in your filesystem. Run your own relevant checks and finish
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
        "trial_checkout": {
            "mode": "non-cone sparse checkout",
            "included_files": list(SPARSE_FILES),
            "candidate_path": "benchmarks/cli-tokens-v1/candidate/",
            "excluded": "all other repository paths",
        },
    }


def _usage_values(usage: Any) -> dict[str, int | None]:
    if not isinstance(usage, dict):
        return {name: None for name in USAGE_FIELDS}
    aliases = {
        "input_tokens": ("input_tokens", "inputTokens"),
        "cache_creation_input_tokens": ("cache_creation_input_tokens", "cacheCreationInputTokens"),
        "cache_read_input_tokens": ("cache_read_input_tokens", "cacheReadInputTokens"),
        "output_tokens": ("output_tokens", "outputTokens"),
    }
    values = {field: next((usage[key] for key in keys if key in usage), None) for field, keys in aliases.items()}
    return {
        name: int(values[name])
        if isinstance(values[name], (int, float)) and not isinstance(values[name], bool)
        else None
        for name in USAGE_FIELDS
    }


def _sum_usage(rows: list[dict[str, int | None]]) -> dict[str, int | None]:
    result: dict[str, int | None] = {}
    for name in USAGE_FIELDS:
        values = [row[name] for row in rows if row.get(name) is not None]
        result[name] = sum(values) if values else None
    return result


def stream_usage(path: Path) -> dict[str, Any]:
    usage_by_id: dict[str, dict[str, int | None]] = {}
    updates_per_id: dict[str, int] = {}
    text_by_id: dict[str, str] = {}
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
                    parsed = _usage_values(usage)
                    previous = usage_by_id.get(identity, {name: None for name in USAGE_FIELDS})
                    usage_by_id[identity] = {
                        name: parsed[name] if parsed[name] is not None else previous[name]
                        for name in USAGE_FIELDS
                    }
                    updates_per_id[identity] = updates_per_id.get(identity, 0) + 1
                message_text = ""
                for block in message.get("content", []):
                    if isinstance(block, dict) and block.get("type") == "text":
                        message_text += str(block.get("text", "")) + "\n"
                if isinstance(identity, str):
                    text_by_id[identity] = message_text
                else:
                    visible_output += message_text
        if event.get("type") == "result":
            observed_result = event
            model_usage = event.get("modelUsage")
            if isinstance(model_usage, dict):
                models.update(str(model) for model in model_usage)

    totals = _sum_usage(list(usage_by_id.values()))
    first = next(iter(usage_by_id.values()), {name: None for name in USAGE_FIELDS})
    final_usage: dict[str, int | None] | None = None
    if isinstance(observed_result, dict):
        parsed = _usage_values(observed_result.get("usage"))
        if any(value is not None for value in parsed.values()):
            final_usage = parsed
        model_usage = observed_result.get("modelUsage")
        if isinstance(model_usage, dict):
            for concrete_model in (MODEL, *sorted(model_usage)):
                candidate = model_usage.get(concrete_model)
                parsed = _usage_values(candidate)
                if any(value is not None for value in parsed.values()):
                    final_usage = parsed
                    break
    discrepancies = {}
    if final_usage is not None:
        for name in USAGE_FIELDS:
            turn_sum = totals[name]
            final_value = final_usage[name]
            if turn_sum is not None and final_value is not None and turn_sum != final_value:
                discrepancies[name] = {"per_turn_sum": turn_sum, "final_result": final_value}
        totals = {
            name: final_usage[name] if final_usage[name] is not None else totals[name]
            for name in USAGE_FIELDS
        }
    return {
        "models_observed": sorted(models),
        "turns_with_usage": len(usage_by_id),
        "usage": totals,
        "usage_totals_source": "final_result_with_per_turn_fallback" if final_usage else "per_turn_deduplicated",
        "usage_updates_per_message": updates_per_id,
        "usage_discrepancies": discrepancies,
        "first_turn_usage": first,
        "inherited_context_tokens": None,
        "inherited_context_note": "Unmeasured; first-turn usage includes task prompt and provider-managed context.",
        "visible_output_bytes": len((visible_output + "".join(text_by_id.values())).encode("utf-8")),
        "provider_output_tokens": totals["output_tokens"],
        "result_event": observed_result,
        "invalid_stream_lines": invalid_lines,
    }


def rate_card_estimate(usage: dict[str, int | None]) -> float | None:
    if not all(usage.get(key) is not None for key in USAGE_FIELDS):
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
    added = subprocess.run(
        ["git", "worktree", "add", "--detach", "--no-checkout", str(workspace), commit],
        cwd=repo,
        text=True,
        capture_output=True,
        check=False,
    )
    row: dict[str, Any] = {**trial, "workspace": str(workspace), "status": "failed", "failure": None}
    if added.returncode:
        row["failure"] = f"worktree creation failed: {bounded_text(added.stderr)}"
        return row

    sparse = subprocess.run(
        ["git", "sparse-checkout", "init", "--no-cone"], cwd=workspace,
        text=True, capture_output=True, check=False,
    )
    if sparse.returncode == 0:
        sparse = subprocess.run(
            ["git", "sparse-checkout", "set", "--no-cone", *SPARSE_FILES], cwd=workspace,
            text=True, capture_output=True, check=False,
        )
    if sparse.returncode == 0:
        sparse = subprocess.run(
            ["git", "read-tree", "-mu", "HEAD"], cwd=workspace,
            text=True, capture_output=True, check=False,
        )
    if sparse.returncode:
        row["failure"] = f"sparse trial checkout failed: {bounded_text(sparse.stderr)}"
        row["workspace_retained_for_review"] = True
        return row

    visible = sorted(
        str(path.relative_to(workspace))
        for path in workspace.rglob("*")
        if path.is_file() and ".git" not in path.parts
    )
    expected_visible = [file.lstrip("/") for file in SPARSE_FILES]
    if visible != expected_visible:
        row["failure"] = f"sparse checkout exposed unexpected files: {visible}"
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
        "--restricted",
        "--strict-mcp-config",
        "--tools",
        "Bash,Read,Edit,Write,Glob,Grep",
        "--allowedTools",
        "Bash,Read,Edit,Write,Glob,Grep",
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
    row["list_price_estimate_usd"] = rate_card_estimate(usage["usage"])
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
        acceptance_started = time.monotonic()
        row["acceptance"] = check_program(candidate, settings["timeout_seconds"], env)
        row["acceptance_elapsed_seconds"] = round(time.monotonic() - acceptance_started, 3)
        row["status"] = "accepted" if row["acceptance"]["accepted"] else "not_accepted"
        if row["status"] != "accepted":
            row["failure"] = "candidate failed independent build or acceptance checks"
    # Archive candidate source and generated outputs before deleting the
    # disposable sparse checkout. Any write outside the advertised candidate
    # directory keeps the checkout for inspection.
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
    if candidate.exists():
        shutil.copytree(candidate, archive)
    else:
        archive.mkdir()
    archived_files = {}
    for path in sorted(archive.rglob("*")):
        if path.is_file():
            archived_files[str(path.relative_to(archive))] = digest(path)
    save_json(archive.parent / f"{label}.manifest.json", {
        "trial": label,
        "files_sha256": archived_files,
        "candidate_archive": str(archive),
    })
    row["candidate_archive"] = str(archive)
    row["candidate_files_sha256"] = archived_files
    removed = subprocess.run(
        ["git", "worktree", "remove", "--force", str(workspace)],
        cwd=repo, text=True, capture_output=True, check=False,
    )
    row["workspace_removed"] = removed.returncode == 0
    if removed.returncode:
        row["workspace_retained_for_review"] = True
        row["failure"] = row.get("failure") or f"worktree cleanup failed: {bounded_text(removed.stderr)}"
    return row


def summarize(results: list[dict[str, Any]]) -> dict[str, Any]:
    rows = []
    for arm in ARMS:
        selected = [row for row in results if row["arm"] == arm]
        accepted = sum(row.get("status") == "accepted" for row in selected)
        per_trial_costs = [row.get("list_price_estimate_usd") for row in selected]
        known_cost = sum(value for value in per_trial_costs if value is not None)
        usage_totals: dict[str, int | None] = {}
        usage_missing: dict[str, int] = {}
        for field in USAGE_FIELDS:
            values = [row.get("observed", {}).get("usage", {}).get(field) for row in selected]
            known = [value for value in values if value is not None]
            usage_totals[field] = sum(known) if known else None
            usage_missing[field] = len(values) - len(known)
        elapsed = [row.get("elapsed_seconds") for row in selected if row.get("elapsed_seconds") is not None]
        complete_cost = len(per_trial_costs) == len(selected) and all(value is not None for value in per_trial_costs)
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
            "turns": sum(row.get("observed", {}).get("turns_with_usage", 0) for row in selected),
            "per_trial_model_session_wall_seconds": elapsed,
            "aggregate_model_session_wall_seconds": round(sum(elapsed), 3),
            "mean_model_session_wall_seconds": round(mean(elapsed), 3) if elapsed else None,
            "median_model_session_wall_seconds": round(median(elapsed), 3) if elapsed else None,
            "list_price_estimate_per_attempt_usd": per_trial_costs,
            "list_price_estimate_known_subtotal_usd": round(known_cost, 6),
            "list_price_estimate_complete": complete_cost,
            "list_price_estimate_total_usd": round(known_cost, 6) if complete_cost else None,
            "estimated_cost_per_accepted_task_usd": (
                round(known_cost / accepted, 6) if complete_cost and accepted else None
            ),
            "accepted_task_cost_note": "All attempts, including failed and rejected trials, are included in the numerator.",
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
        campaign_started = time.monotonic()
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
            save_json(artifacts / "results.json", {
                "campaign": settings,
                "trials": rows,
                "summary": summarize(rows),
                "campaign_elapsed_wall_seconds": round(time.monotonic() - campaign_started, 3),
            })
            print(f"{arm} {trial_number[arm]}/{settings['trials_per_arm']}: {row.get('status', 'failed')}", flush=True)
        return 0
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"live campaign error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
