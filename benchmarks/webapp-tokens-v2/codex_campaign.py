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
import campaign_resources as resources

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
FROZEN_SPEC = "benchmarks/webapp-tokens-v2/SPEC.md"
QUALIFICATION_RECEIPT = "benchmarks/webapp-tokens-v2/acceptance/evidence/reference-r6-summary.json"
ACCEPTANCE_SOURCE_FILES = (
    FROZEN_SPEC,
    "benchmarks/webapp-tokens-v2/acceptance/package.json",
    "benchmarks/webapp-tokens-v2/acceptance/package-lock.json",
    "benchmarks/webapp-tokens-v2/acceptance/run.mjs",
    "benchmarks/webapp-tokens-v2/acceptance/api.mjs",
    "benchmarks/webapp-tokens-v2/acceptance/browser.mjs",
    "benchmarks/webapp-tokens-v2/acceptance/browser-support.mjs",
    "benchmarks/webapp-tokens-v2/acceptance/client.mjs",
    "benchmarks/webapp-tokens-v2/acceptance/contract.mjs",
    "benchmarks/webapp-tokens-v2/acceptance/kdf-observer.mjs",
    "benchmarks/webapp-tokens-v2/acceptance/ordering.mjs",
    "benchmarks/webapp-tokens-v2/acceptance/process.mjs",
    "benchmarks/webapp-tokens-v2/acceptance/qualification.mjs",
)
HARNESS_SOURCE_FILES = (
    "benchmarks/webapp-tokens-v2/codex_campaign.py",
    "benchmarks/live_campaign_common.py",
    "benchmarks/campaign_resources.py",
    QUALIFICATION_RECEIPT,
    *ACCEPTANCE_SOURCE_FILES,
)
ARCHIVE_EXCLUDED_DIRS = {
    ".cache", ".mypy_cache", ".pytest_cache", "__pycache__", "dist", "node_modules", "out", "target",
}
PRICE_USD_PER_MTOK = {
    "input": 2.0, "cache_read": 0.1, "cache_write": 2.5, "output": 10.0,
}
SHORT_CONTEXT_LIMIT = 272_000
CALIBRATION_PROMPT = "This is a context calibration request. Reply with exactly READY; do not use tools or read files."


def _git_file(repo: Path, commit: str, relative: str) -> bytes:
    exported = subprocess.run(["git", "show", f"{commit}:{relative}"], cwd=repo,
                              capture_output=True, check=False)
    if exported.returncode:
        raise ValueError(f"pinned commit does not contain required benchmark file: {relative}")
    return exported.stdout


def pinned_seed_hashes(repo: Path, commit: str) -> dict[str, str]:
    return {relative.lstrip("/"): hashlib.sha256(_git_file(repo, commit, relative.lstrip("/"))).hexdigest()
            for relative in SEED_FILES}


def harness_source_inventory(repo: Path = REPO, frozen_spec_sha256: str | None = None) -> dict[str, str]:
    """Hash the complete local source closure used by the matched campaign."""
    root = repo.resolve(strict=True)
    inventory: dict[str, str] = {}
    for relative in HARNESS_SOURCE_FILES:
        if relative == FROZEN_SPEC and frozen_spec_sha256 is not None:
            inventory[relative] = frozen_spec_sha256
            continue
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
                             source_commit: str, frozen_spec_sha256: str) -> dict[str, Any]:
    """Copy only the pinned harness closure and verify every retained byte."""
    current = harness_source_inventory(repo, frozen_spec_sha256)
    if current != expected:
        raise ValueError("campaign harness source changed after the immutable plan")
    destination = artifacts / "harness-source"
    if destination.exists():
        raise ValueError("campaign harness source snapshot must be new")
    destination.mkdir(parents=True)
    for relative, wanted in current.items():
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        contents = (_git_file(repo, source_commit, relative) if relative == FROZEN_SPEC
                    else (repo / relative).read_bytes())
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


def acceptance_capabilities(node_binary: str, playwright_root: Path) -> dict[str, Any]:
    """Check the exact local Node/Playwright runtime before any paid request."""
    script = r"""
const fs=require('node:fs');
const path=require('node:path');
const {createRequire}=require('node:module');
const root=process.argv[1];
const requireFromRoot=createRequire(path.join(root,'package.json'));
const version=requireFromRoot('@playwright/test/package.json').version;
const executable=requireFromRoot('@playwright/test').chromium.executablePath();
process.stdout.write(JSON.stringify({version,executable,executable_exists:fs.existsSync(executable)}));
    """
    root = playwright_root.expanduser().resolve()
    resolved_node = shutil.which(node_binary)
    if resolved_node is None:
        return {"status": "unsupported", "node_binary": node_binary,
                "playwright_root": str(root), "error": "Node binary is unavailable"}
    resolved_node = str(Path(resolved_node).resolve())
    if not root.is_dir() or not (root / "package.json").is_file():
        return {"status": "unsupported", "node_binary": resolved_node,
                "playwright_root": str(root), "error": "playwright root lacks package.json"}
    try:
        version = subprocess.run([resolved_node, "--version"], text=True, capture_output=True,
                                 check=False, timeout=15)
        match = re.fullmatch(r"v(\d+)\.\d+\.\d+", version.stdout.strip())
        package = subprocess.run([resolved_node, "-e", script, str(root)], text=True,
                                 capture_output=True, check=False, timeout=30)
        detail = json.loads(package.stdout) if package.returncode == 0 else {}
    except (OSError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        return {"status": "unsupported", "node_binary": resolved_node,
                "playwright_root": str(root), "error": str(error)}
    ready = (version.returncode == 0 and match is not None and int(match.group(1)) >= 24
             and package.returncode == 0 and detail.get("version") == "1.62.0"
             and detail.get("executable_exists") is True)
    return {
        "status": "ready" if ready else "unsupported",
        "node_binary": resolved_node,
        "node_version": version.stdout.strip() or None,
        "playwright_root": str(root),
        "playwright_version": detail.get("version"),
        "chromium_executable": detail.get("executable"),
        "chromium_executable_exists": detail.get("executable_exists", False),
        "probe_stderr": common.bounded_text(package.stderr),
    }


def resolve_commit(repo: Path, ref: str) -> str:
    result = subprocess.run(["git", "rev-parse", "--verify", f"{ref}^{{commit}}"], cwd=repo,
                            text=True, capture_output=True, check=False)
    if result.returncode or not result.stdout.strip():
        raise ValueError(f"ref does not resolve to a commit: {ref}")
    return result.stdout.strip()


def validate_qualification_receipt(repo: Path, receipt: dict[str, Any],
                                   frozen_spec_sha256: str) -> str:
    """Bind the 912/912 references to the exact gate and frozen SPEC bytes."""
    if (receipt.get("schema") != "semaprax.teamdesk.reference.qualification.v1"
            or receipt.get("qualified") is not True
            or receipt.get("spec_sha256") != frozen_spec_sha256
            or set(receipt.get("arms", {})) != set(ARMS)):
        raise ValueError("TeamDesk reference qualification identity differs from this campaign")
    for arm in ARMS:
        qualification = receipt["arms"][arm].get("qualification", {})
        if (qualification.get("passed") is not True or qualification.get("cases") != 912
                or qualification.get("missingCases") != [] or qualification.get("missingGroups") != []
                or qualification.get("failures") != []):
            raise ValueError(f"{arm} reference lacks independent 912/912 qualification")
    gate_commit = resolve_commit(repo, str(receipt.get("gate_source", "")))
    for relative in ACCEPTANCE_SOURCE_FILES:
        current = (repo / relative).read_bytes()
        qualified = _git_file(repo, gate_commit, relative)
        if current != qualified:
            raise ValueError(f"qualified acceptance source drifted: {relative}")
        if relative == FROZEN_SPEC and hashlib.sha256(qualified).hexdigest() != frozen_spec_sha256:
            raise ValueError("qualified gate SPEC differs from the campaign base-ref")
    return gate_commit


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


def _rollout_records_with_integrity(
    path: Path,
) -> tuple[list[dict[str, Any]], list[str], list[str], int | None, int, int]:
    requests: list[dict[str, Any]] = []
    models: list[str] = []
    efforts: list[str] = []
    context_window: int | None = None
    seen: set[str] = set()
    malformed_lines = 0
    orphan_usage_records = 0
    for raw in path.read_text(encoding="utf-8").splitlines():
        try:
            event = json.loads(raw)
        except json.JSONDecodeError:
            malformed_lines += 1
            continue
        if not isinstance(event, dict):
            malformed_lines += 1
            continue
        payload = event.get("payload")
        if not isinstance(payload, dict):
            if event.get("type") == "token_usage_record":
                orphan_usage_records += 1
            continue
        if event.get("type") == "turn_context":
            for value, sink in ((payload.get("model"), models), (payload.get("effort"), efforts)):
                if isinstance(value, str) and value not in sink:
                    sink.append(value)
        if event.get("type") == "token_usage_record":
            usage = _usage(payload.get("usage"))
            identity = payload.get("response_id") or payload.get("turn_id")
            if not isinstance(identity, str) or not identity.strip():
                orphan_usage_records += 1
                continue
            key = json.dumps([identity, usage], sort_keys=True)
            if key not in seen:
                seen.add(key)
                requests.append({"request_id": identity, "usage": usage})
        if event.get("type") == "event_msg" and payload.get("type") == "token_count":
            info = payload.get("info")
            if isinstance(info, dict) and isinstance(info.get("model_context_window"), int):
                context_window = info["model_context_window"]
    return requests, models, efforts, context_window, malformed_lines, orphan_usage_records


def _rollout_records(path: Path) -> tuple[list[dict[str, Any]], list[str], list[str], int | None]:
    """Compatibility projection of rollout records without integrity counters."""
    requests, models, efforts, context_window, _, _ = _rollout_records_with_integrity(path)
    return requests, models, efforts, context_window


def trace_usage(exec_usage: dict[str, Any], rollout: Path) -> dict[str, Any]:
    requests, models, efforts, context_window, malformed_lines, orphan_usage_records = (
        _rollout_records_with_integrity(rollout)
    )
    final = exec_usage.get("final_turn_usage")
    summed: dict[str, int | None] = {}
    for name in _usage({}):
        values = [request["usage"][name] for request in requests]
        summed[name] = sum(values) if values and all(value is not None for value in values) else None
    reconciled = (
        isinstance(final, dict)
        and all(value is not None for value in final.values())
        and _same_usage(final, summed)
        and malformed_lines == 0
        and orphan_usage_records == 0
    )
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
        "rollout_malformed_lines": malformed_lines,
        "rollout_orphan_usage_records": orphan_usage_records,
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
    hashes = pinned_seed_hashes(repo, commit)
    frozen_spec_hash = hashes[FROZEN_SPEC]
    receipt = json.loads((repo / QUALIFICATION_RECEIPT).read_text())
    gate_commit = validate_qualification_receipt(repo, receipt, frozen_spec_hash)
    harness_sources = harness_source_inventory(repo, frozen_spec_hash)
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
    if args.trials_per_arm != MIN_TRIALS_PER_ARM or args.timeout_seconds != TIMEOUT_SECONDS:
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
        "qualification": {"receipt": QUALIFICATION_RECEIPT,
                          "receipt_sha256": common.digest(repo / QUALIFICATION_RECEIPT),
                          "gate_source_commit": gate_commit, "spec_sha256": frozen_spec_hash,
                          "required_cases": 912, "required_arms": list(ARMS)},
        "effort_requested": args.effort, "timeout_seconds": args.timeout_seconds,
        "acceptance_timeout_seconds": 2700,
        "trial_order": [arm for index in range(args.trials_per_arm) for arm in (ARMS if index % 2 == 0 else tuple(reversed(ARMS)))],
        "authored_source_tokenizer": common.tokenizer_metadata(args.tokenizer_dir),
        "codex_binary": args.codex_binary,
        "codex_version": subprocess.run([args.codex_binary, "--version"], capture_output=True, text=True, check=True).stdout.strip(),
        "price_book": {"date": "2026-10-08", "source": "https://developers.openai.com/api/docs/models/gpt-6.1-sol", "standard_short_context_usd_per_million": PRICE_USD_PER_MTOK, "conditional": True},
        "attempt_denominator": args.trials_per_arm * len(ARMS),
        "resource_policy": resources.policy(),
        "source_binary_sha256": common.digest(Path(args.semaprax_bin).resolve(strict=True)),
        "calibration": {"prompt": CALIBRATION_PROMPT, "separate": True, "subtracted_from_trials": False},
        "measurement": {"stable_context_tokens": None, "legacy_net_input_tokens": None,
                        "model_request_count": "trace-backed only; never inferred from item counts"},
        "capabilities": capabilities(args.codex_binary),
        "acceptance": {"runner": "benchmarks/webapp-tokens-v2/acceptance/run.mjs", "node_major": 24,
                       "playwright": "1.62.0", "required_cases": 912, "fresh_evidence": True,
                       "capabilities": acceptance_capabilities(args.node_binary, Path(args.playwright_root))},
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
    guard = workspace_guard(workspace, settings, allow_candidate="arm" in row)
    row["workspace_integrity_before_cleanup"] = guard
    if guard["status"] != "passed":
        if "arm" in row:
            invalidate(row, "workspace integrity failed before cleanup")
        else:
            row.update({"status": "failed", "failure": "workspace integrity failed before cleanup",
                        "workspace_retained_for_review": True})
        return
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


def workspace_guard(workspace: Path, settings: dict[str, Any], allow_candidate: bool = True) -> dict[str, Any]:
    """Require exact seed bytes and confine trial writes to candidate/."""
    candidate = Path("candidate")
    public = {Path(path.lstrip("/")) for path in SEED_FILES}
    allowed_dirs = {candidate}
    for relative in public:
        allowed_dirs.update(relative.parents)
    unexpected: list[str] = []
    seed_status: dict[str, str] = {}
    try:
        for relative in public:
            path = workspace / relative
            expected = settings.get("seed_files_sha256", {}).get(relative.as_posix())
            okay = (expected is not None and path.is_file() and not path.is_symlink()
                    and common.digest(path) == expected)
            seed_status[relative.as_posix()] = "passed" if okay else "failed"
        for path in workspace.rglob("*"):
            relative = path.relative_to(workspace)
            if relative.parts[0] == ".git":
                continue
            if relative == candidate and allow_candidate:
                if path.is_symlink() or not path.is_dir():
                    unexpected.append(relative.as_posix())
            elif candidate in relative.parents and allow_candidate:
                continue
            elif relative in public:
                continue
            elif relative in allowed_dirs and path.is_dir() and not path.is_symlink():
                continue
            else:
                unexpected.append(relative.as_posix())
        passed = all(status == "passed" for status in seed_status.values()) and not unexpected
        return {"status": "passed" if passed else "failed", "seed_files": seed_status,
                "outside_candidate_paths": sorted(unexpected)}
    except OSError as error:
        return {"status": "failed", "seed_files": seed_status, "error": str(error)}


def invalidate(row: dict[str, Any], reason: str) -> None:
    row.update({"status": "not_accepted", "failure": reason, "workspace_retained_for_review": True})
    if isinstance(row.get("acceptance"), dict):
        row["acceptance"]["accepted"] = False
        row["acceptance"]["invalidated"] = reason


def candidate_inventory(candidate: Path) -> tuple[dict[str, str], list[str]]:
    """Hash the exact archive projection and reject symlink substitutions."""
    hashes: dict[str, str] = {}
    omitted: set[str] = set()
    if not candidate.exists():
        return hashes, []
    if not candidate.is_dir() or candidate.is_symlink():
        raise ValueError("candidate root is not a regular directory")
    for path in sorted(candidate.rglob("*")):
        relative = path.relative_to(candidate)
        excluded = next((part for part in relative.parts if part in ARCHIVE_EXCLUDED_DIRS), None)
        if excluded is not None:
            omitted.add(relative.parts[0] if relative.parts[0] in ARCHIVE_EXCLUDED_DIRS
                        else relative.as_posix().split(f"/{excluded}", 1)[0] + f"/{excluded}")
            continue
        if path.is_symlink():
            raise ValueError(f"candidate archive refuses symlink: {relative.as_posix()}")
        if path.is_file():
            hashes[relative.as_posix()] = common.digest(path)
        elif not path.is_dir():
            raise ValueError(f"candidate archive refuses special file: {relative.as_posix()}")
    return hashes, sorted(omitted)


def archive_candidate(candidate: Path, archive: Path) -> tuple[dict[str, str], list[str]]:
    expected, omitted = candidate_inventory(candidate)
    if candidate.exists():
        shutil.copytree(candidate, archive, ignore=shutil.ignore_patterns(*sorted(ARCHIVE_EXCLUDED_DIRS)))
    else:
        archive.mkdir(parents=True)
    actual, archive_omitted = candidate_inventory(archive)
    if actual != expected or archive_omitted:
        raise RuntimeError("candidate archive inventory differs from accepted workspace")
    return actual, omitted


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

You are writing the {language} arm. Create the new implementation root {candidate}, then work only inside it. Create build.sh, test.sh, and run.sh at the candidate root. {compiler_note}
Implement every requirement: all 20 entities and fields, validation, references, unique keys, workflows, computed fields and rollups, accounts, sessions, role and row permissions, audit history, CSV export, REST CRUD API, persistence, and the browser UI. The independent gate is authoritative and covers all 912 obligations; do not weaken, edit, or bypass it. Keep the API and UI on loopback using TEAMDESK_HOST, TEAMDESK_PORT, TEAMDESK_UI_PORT, TEAMDESK_DATA_DIR, and make run.sh stay in the foreground while emitting the required readiness JSON line.

Use only the files and tools available in this isolated workspace. Run your own build and tests before reporting completion. Do not edit files outside the candidate directory. When finished, leave all implementation files in {candidate} and briefly report the commands and results."""


def check_candidate(candidate: Path, output: Path, arm: str, settings: dict[str, Any], compiler: Path) -> dict[str, Any]:
    """Run the snapshotted independent gate and admit only its exact report contract."""
    env = trial_environment(compiler if arm == "semaprax" else None)
    env["TEAMDESK_BENCHMARK_ARM"] = arm
    gate_relative = settings["acceptance"]["runner"]
    gate = Path(settings["artifacts"]) / settings["harness_source_snapshot"]["path"] / gate_relative
    runtime = settings["acceptance"]["capabilities"]
    command = [runtime["node_binary"], str(gate), "--arm", arm, "--candidate", str(candidate),
               "--output", str(output), "--playwright-root", runtime["playwright_root"]]
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
    checks = report.get("checks") if isinstance(report.get("checks"), list) else []
    gate_prefix = "benchmarks/webapp-tokens-v2/acceptance/"
    expected_gate = {relative.removeprefix(gate_prefix): digest
                     for relative, digest in settings["harness_source_snapshot"]["files_sha256"].items()
                     if relative.startswith(gate_prefix)}
    reported_gate_rows = report.get("gate") if isinstance(report.get("gate"), list) else []
    reported_gate = {row.get("name"): row.get("sha256") for row in reported_gate_rows
                     if isinstance(row, dict) and isinstance(row.get("name"), str)}
    compiler_ok = True
    if arm == "semaprax":
        compiler_report = report.get("compiler", {})
        compiler_ok = (compiler_report.get("source_sha") == settings["compiler_source_commit"]
                       and compiler_report.get("sha256") == settings["source_binary_sha256"])
    contract_failures = []
    check_ids = [row.get("id") for row in checks if isinstance(row, dict)]
    candidate_after = report.get("candidate_after") if isinstance(report.get("candidate_after"), list) else []
    candidate_names = [row.get("name") for row in candidate_after if isinstance(row, dict)]
    for okay, label in (
        (result.returncode == 0, "gate exit status"),
        (report.get("schema") == "semaprax.teamdesk.acceptance.v1", "report schema"),
        (report.get("arm") == arm, "arm identity"),
        (report.get("spec_sha256") == settings["qualification"]["spec_sha256"], "SPEC identity"),
        (reported_gate == expected_gate and len(reported_gate_rows) == len(expected_gate), "gate inventory"),
        (qualification.get("passed") is True, "qualification status"),
        (qualification.get("cases") == 912 and len(checks) == 912, "912-case inventory"),
        (qualification.get("missingCases") == [], "missing cases"),
        (qualification.get("missingGroups") == [], "missing groups"),
        (qualification.get("failures") == [], "failed cases"),
        (all(isinstance(row, dict) and row.get("status") == "passed" for row in checks), "case statuses"),
        (len(check_ids) == 912 and len(set(check_ids)) == 912
         and all(isinstance(value, str) for value in check_ids), "case identities"),
        (compiler_ok, "compiler identity"),
        (isinstance(report.get("candidate_after"), list)
         and len(candidate_names) == len(candidate_after) == len(set(candidate_names))
         and all(isinstance(value, str) for value in candidate_names), "candidate-after inventory"),
    ):
        if not okay:
            contract_failures.append(label)
    accepted = not contract_failures
    return {"accepted": accepted, "exit_code": result.returncode,
            "seconds": round(time.monotonic() - started, 3), "report": report,
            "stdout": common.bounded_text(result.stdout), "stderr": common.bounded_text(result.stderr),
            "failure": None if accepted else "acceptance contract failed: " + ", ".join(contract_failures)}


@resources.guarded_attempt
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
    prompt = prompt_for(arm, candidate, semaprax_bin)
    (artifacts / "prompts").mkdir(parents=True, exist_ok=True)
    (artifacts / "prompts" / f"{label}.txt").write_text(prompt, encoding="utf-8")
    row["prompt_sha256"] = hashlib.sha256(prompt.encode()).hexdigest()
    transcript, stderr, trace = (artifacts / "transcripts" / f"{label}.{suffix}" for suffix in ("jsonl", "stderr.txt", "rollout.jsonl"))
    transcript.parent.mkdir(parents=True, exist_ok=True)
    row.update({"transcript": str(transcript), "stderr_path": str(stderr),
                "provider_receipt_actual_usd": None})
    try:
        process = run_codex(_command(settings, prompt), workspace, trial_environment(semaprax_bin),
                            transcript, stderr, settings["timeout_seconds"])
        row.update(process)
        exec_usage = parse_exec_jsonl(transcript)
        copied = copy_task_rollout(exec_usage["thread_ids"], workspace, trace)
        trace_result = (trace_usage(exec_usage, copied) if copied else
                        {"reconciled": False, "model_request_count": None,
                         "model_observed": None, "model_requests": []})
        observed = {**exec_usage, **trace_result, "stable_context_tokens": None}
        row["observed"] = observed
        row["rollout_trace"] = str(copied) if copied else None
        row["transcript_sha256"] = common.digest(transcript)
        row["rollout_trace_sha256"] = common.digest(copied) if copied else None
        row["list_price"] = list_price_estimate(trace_result.get("model_requests", []))
        row["telemetry_valid"] = (observed.get("reconciled") is True
            and observed.get("model_observed") == MODEL and observed.get("effort_observed") == EFFORT
            and observed.get("invalid_stream_lines") == 0)
        guard = workspace_guard(workspace, settings)
        row["workspace_integrity_before_acceptance"] = guard
        if process["timed_out"]:
            row["failure"] = "trial hit wall-clock timeout"
        elif process["process_exit_code"] != 0:
            row["failure"] = f"Codex exited with {process['process_exit_code']}"
        elif not row["telemetry_valid"]:
            row["failure"] = "missing, invalid, or mismatched task-owned rollout telemetry"
        elif guard["status"] != "passed":
            invalidate(row, "workspace integrity failed before acceptance")
        else:
            started = time.monotonic()
            try:
                row["acceptance"] = check_candidate(
                    candidate, artifacts / "qualification" / label, arm, settings, semaprax_bin)
            finally:
                row["acceptance_elapsed_seconds"] = round(time.monotonic() - started, 3)
            row["status"] = "accepted" if row["acceptance"]["accepted"] else "not_accepted"
            if row["status"] != "accepted":
                row["failure"] = "candidate failed independent 912-case acceptance"
    except (OSError, RuntimeError, ValueError, UnicodeError, json.JSONDecodeError,
            subprocess.SubprocessError) as error:
        row.update({"failure": str(error), "runner_error": True, "status": "failed"})
    try:
        row["final_candidate_source_metrics"] = common.authored_source_metrics(
            candidate, settings.get("authored_source_tokenizer"))
    except (OSError, RuntimeError, ValueError, UnicodeError, json.JSONDecodeError,
            subprocess.SubprocessError) as error:
        row["final_candidate_source_metrics"] = {
            "status": "measurement_failed", "total_tokens": None, "files": [],
            "tokenizer": settings.get("authored_source_tokenizer"), "error": str(error),
        }
    guard = workspace_guard(workspace, settings)
    row["workspace_integrity_before_archive"] = guard
    if guard["status"] != "passed":
        invalidate(row, "workspace integrity failed before archive")
        return row
    archive = artifacts / "candidates" / label
    archive.parent.mkdir(parents=True, exist_ok=True)
    try:
        source_hashes, omitted = candidate_inventory(candidate)
        if row.get("status") == "accepted":
            report_rows = row["acceptance"]["report"]["candidate_after"]
            report_hashes = {item.get("name"): item.get("sha256") for item in report_rows
                             if isinstance(item, dict) and isinstance(item.get("name"), str)}
            report_projection = {name: digest for name, digest in report_hashes.items()
                                 if not any(part in ARCHIVE_EXCLUDED_DIRS for part in Path(name).parts)}
            if report_projection != source_hashes:
                invalidate(row, "candidate changed after acceptance evidence")
                return row
        archived_hashes, archived_omitted = archive_candidate(candidate, archive)
        if archived_hashes != source_hashes or archived_omitted != omitted:
            raise RuntimeError("candidate archive closure differs from measured source")
        row.update({"candidate_archive": str(archive), "candidate_files_sha256": archived_hashes,
                    "candidate_archive_excluded_paths": omitted})
    except (OSError, RuntimeError, ValueError) as error:
        row.update({"failure": f"candidate archive failed: {error}", "runner_error": True,
                    "status": "failed", "workspace_retained_for_review": True})
        if isinstance(row.get("acceptance"), dict):
            row["acceptance"]["accepted"] = False
        return row
    cleanup_trial(repo, workspace, settings, row)
    return row


@resources.guarded_attempt
def launch_calibration(repo: Path, artifacts: Path, commit: str, settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    """One separately reported empty-task request; it is never subtracted from trials."""
    workspace = artifacts / "worktrees" / "calibration"
    row: dict[str, Any] = {"status": "failed", "separate_from_trials": True, "subtracted_from_trials": False}
    error = add_seed_worktree(repo, workspace, commit)
    if error:
        row["failure"] = error; return row
    transcript, stderr, trace = (artifacts / "transcripts" / f"calibration.{suffix}" for suffix in ("jsonl", "stderr.txt", "rollout.jsonl"))
    transcript.parent.mkdir(parents=True, exist_ok=True)
    row.update({"transcript": str(transcript), "stderr_path": str(stderr),
                "provider_receipt_actual_usd": None})
    try:
        row.update(run_codex(_command(settings, CALIBRATION_PROMPT), workspace,
                             trial_environment(semaprax_bin), transcript, stderr,
                             settings["timeout_seconds"]))
        usage = parse_exec_jsonl(transcript)
        copied = copy_task_rollout(usage["thread_ids"], workspace, trace)
        trace_result = trace_usage(usage, copied) if copied else {"reconciled": False, "model_requests": []}
        observed = {**usage, **trace_result, "stable_context_tokens": None}
        row["observed"] = observed
        row["rollout_trace"] = str(copied) if copied else None
        row["transcript_sha256"] = common.digest(transcript)
        row["rollout_trace_sha256"] = common.digest(copied) if copied else None
        row["list_price"] = list_price_estimate(trace_result.get("model_requests", []))
        messages = [json.loads(line).get("item", {}) for line in transcript.read_text().splitlines() if line]
        replies = [item.get("text", "").strip() for item in messages if item.get("type") == "agent_message"]
        counts = usage.get("tool_item_counts", {})
        guard = workspace_guard(workspace, settings, allow_candidate=False)
        row["workspace_integrity_before_cleanup"] = guard
        row["status"] = "ready" if (not row["timed_out"] and row["process_exit_code"] == 0
            and observed.get("reconciled") and observed.get("model_observed") == MODEL
            and observed.get("effort_observed") == EFFORT and observed.get("invalid_stream_lines") == 0
            and replies == ["READY"] and guard["status"] == "passed"
            and all(kind == "agent_message" or not count for kind, count in counts.items())) else "failed"
        if row["status"] != "ready":
            row["failure"] = "calibration failed tool-free READY, telemetry, or workspace-integrity checks"
    except (OSError, RuntimeError, ValueError, UnicodeError, json.JSONDecodeError,
            subprocess.SubprocessError) as error:
        row.update({"failure": str(error), "runner_error": True, "status": "failed"})
    cleanup_trial(repo, workspace, settings, row)
    return row


def summarize(rows: list[dict[str, Any]]) -> dict[str, Any]:
    def complete_sum(values: list[Any]) -> int | float | None:
        return sum(values) if len(values) == len(rows) and all(isinstance(value, (int, float)) for value in values) else None

    accepted_rows = [row for row in rows if row.get("status") == "accepted"]
    accepted = len(accepted_rows)
    prices = [row.get("list_price", {}).get("standard_short_context_api_equivalent_usd")
              for row in rows if isinstance(row.get("list_price"), dict)]
    price_total = complete_sum(prices)
    raw_input = complete_sum([row.get("observed", {}).get("request_usage_sum", {}).get("input_tokens") for row in rows])
    cached_input = complete_sum([row.get("observed", {}).get("request_usage_sum", {}).get("cached_input_tokens") for row in rows])
    cache_write_input = complete_sum([row.get("observed", {}).get("request_usage_sum", {}).get("cache_write_input_tokens") for row in rows])
    output = complete_sum([row.get("observed", {}).get("request_usage_sum", {}).get("output_tokens") for row in rows])
    legacy_net = complete_sum([row.get("observed", {}).get("legacy_net_input_tokens") for row in rows])
    agent_wall = complete_sum([row.get("elapsed_seconds") for row in rows])
    acceptance_wall_values = [row.get("acceptance_elapsed_seconds",
        row.get("acceptance", {}).get("seconds", None if "acceptance" in row else 0)) for row in rows]
    acceptance_wall = (sum(acceptance_wall_values)
                       if all(isinstance(value, (int, float)) for value in acceptance_wall_values) else None)
    authored_values = [row.get("final_candidate_source_metrics", {}).get("total_tokens") for row in accepted_rows]
    authored_total = (sum(authored_values)
                      if accepted and len(authored_values) == accepted and all(isinstance(value, int) for value in authored_values)
                      else None)
    return {
        "attempt_denominator": len(ARMS) * MIN_TRIALS_PER_ARM,
        "recorded_attempts": len(rows),
        "resource_contaminated_attempts": sum(bool(row.get("resource_assessment", {}).get("contaminated")) for row in rows),
        "clean_comparison_eligible": len(rows) >= len(ARMS) * MIN_TRIALS_PER_ARM and all(
            row.get("resource_assessment", {}).get("clean_comparison_eligible") is True for row in rows),
        "accepted_attempts": accepted,
        "failed_or_rejected_attempts": len(rows) - accepted,
        "usage_all_attempts": {
            "raw_input_tokens": raw_input, "cached_input_tokens_subset": cached_input,
            "cache_write_input_tokens_subset": cache_write_input, "output_tokens": output,
            "legacy_net_input_tokens": legacy_net,
        },
        "accepted_authored_source_tokens": authored_total,
        "authored_source_tokens_per_accepted_task": authored_total / accepted if authored_total is not None and accepted else None,
        "list_price_estimate_known_attempts": len([value for value in prices if isinstance(value, (int, float))]),
        "list_price_estimate_all_attempts_usd": round(price_total, 6) if price_total is not None else None,
        "list_price_estimate_per_accepted_task_usd": round(price_total / accepted, 6) if price_total is not None and accepted else None,
        "agent_wall_seconds_all_attempts": agent_wall,
        "agent_wall_seconds_per_accepted_task": agent_wall / accepted if agent_wall is not None and accepted else None,
        "acceptance_wall_seconds_all_attempts": acceptance_wall,
        "acceptance_wall_seconds_per_accepted_task": acceptance_wall / accepted if acceptance_wall is not None and accepted else None,
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
        current.add_argument("--node-binary", default="node")
        current.add_argument("--playwright-root", default=str(BENCHMARK / "acceptance"))
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
            if result["acceptance"]["capabilities"]["status"] != "ready":
                raise ValueError("Node 24, Playwright 1.62.0, and its Chromium are required before paid attempts")
            artifacts = Path(result["artifacts"]); artifacts.mkdir(parents=True)
            snapshot = snapshot_harness_sources(
                Path(args.repo).resolve(), artifacts,
                result["harness_source_snapshot"]["files_sha256"],
                result["repository_commit"],
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
                (artifacts / "results.json").write_text(json.dumps({"campaign": result, "calibration": calibration,
                    "trials": [], "unlaunched_trial_order": result["trial_order"],
                    "campaign_status": "calibration_failed", "summary": summarize([])}, indent=2, sort_keys=True) + "\n")
                print(json.dumps({"status": "calibration_failed", "artifacts": str(artifacts)}, indent=2))
                return 2
            rows = []; counters = {arm: 0 for arm in ARMS}; remaining = list(result["trial_order"])
            while remaining:
                arm = remaining.pop(0)
                counters[arm] += 1
                row = launch_trial(artifacts / "seed-repository", artifacts, seed["seed_repository_commit"],
                                   {"arm": arm, "number": counters[arm]}, result, Path(args.semaprax_bin).resolve())
                rows.append(row)
                # A bounded model task timeout is a scored failure, not a
                # reason to omit the other matched attempts. Missing final
                # telemetry after that timeout remains unknown in accounting.
                interrupted = (row.get("resource_assessment", {}).get("contaminated") is True
                    or row.get("runner_error") is True
                    or row.get("workspace_retained_for_review") is True
                    or (not row.get("timed_out") and (
                        row.get("process_exit_code") not in (0, None)
                        or row.get("telemetry_valid") is False)))
                (artifacts / "results.json").write_text(json.dumps({"campaign": result, "calibration": calibration,
                    "trials": rows, "unlaunched_trial_order": remaining,
                    "campaign_status": "interrupted" if interrupted else ("complete" if not remaining else "running"),
                    "summary": summarize(rows)}, indent=2, sort_keys=True) + "\n")
                if interrupted:
                    break
            status = "completed" if not remaining else "interrupted"
            result = {"status": status, "artifacts": str(artifacts),
                      "unlaunched_trial_order": remaining, **summarize(rows)}
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if result.get("status", "ready") in {"ready", "completed"} else 2
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"codex campaign error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
