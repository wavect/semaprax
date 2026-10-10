"""Shared offline helpers for isolated, matched agent benchmark campaigns.

This module deliberately has no benchmark-oracle imports.  Each benchmark
supplies its own public seed-file list and independently owned acceptance
adapter, so loading one campaign cannot shadow another campaign's ``oracle``.
"""

from __future__ import annotations

import hashlib
import json
import math
import shutil
import subprocess
import time
from pathlib import Path
from typing import Any, Iterable

USAGE_FIELDS = (
    "input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens", "output_tokens",
)
CACHE_TTL_USAGE_FIELDS = (
    "cache_creation_ephemeral_5m_input_tokens", "cache_creation_ephemeral_1h_input_tokens",
)
ALL_USAGE_FIELDS = (*USAGE_FIELDS, *CACHE_TTL_USAGE_FIELDS)
PRICE_USD_PER_MTOK = {
    "input": 2.0, "cache_write_5m": 2.5, "cache_write_1h": 4.0,
    "cache_read": 0.2, "output": 10.0,
}
ARCHIVE_EXCLUDED_DIRS = {"node_modules", ".cache", "__pycache__", ".pytest_cache"}
MAX_LOG_BYTES = 1_000_000
TOKENIZER_PACKAGE = "@anthropic-ai/tokenizer"
TOKENIZER_VERSION = "0.0.4"
TOKENIZER_ENCODING = "package-bundled Claude BPE (`claude.json`), NFKC-normalized by countTokens"
AUTHORED_SUFFIXES = {
    ".spx", ".ts", ".tsx", ".js", ".mjs", ".cjs", ".py", ".sh", ".rs", ".json", ".jsonc",
    ".toml", ".yaml", ".yml", ".md", ".txt", ".lock", ".ini", ".cfg", ".conf", ".xml",
    ".properties", ".html", ".css",
}
AUTHORED_SPECIAL_NAMES = {".gitignore", "Makefile"}
AUTHORED_EXCLUDED_DIRS = {
    ".git", ".cache", ".mypy_cache", ".pytest_cache", ".ruff_cache", ".semaprax", ".spx-cache",
    "__pycache__", "acceptance-fixtures", "build", "coverage", "dist", "generated", "gen",
    "node_modules", "out", "target",
}
TOKENIZE_SCRIPT = r"""
const fs = require('node:fs');
const path = require('node:path');
const { createRequire } = require('node:module');
const root = process.argv[1];
const fromRoot = createRequire(path.join(root, '__codex_tokenizer__.js'));
const pkg = fromRoot('@anthropic-ai/tokenizer/package.json');
const { countTokens } = fromRoot('@anthropic-ai/tokenizer');
const request = JSON.parse(fs.readFileSync(0, 'utf8'));
const counts = request.map(file => ({path: file.path, tokens: countTokens(file.content)}));
process.stdout.write(JSON.stringify({version: pkg.version, counts}) + '\n');
"""


def bounded_text(value: bytes | str) -> str:
    if isinstance(value, bytes):
        value = value.decode("utf-8", errors="replace")
    if len(value.encode("utf-8")) > MAX_LOG_BYTES:
        value = value[:MAX_LOG_BYTES] + "\n[truncated by campaign harness]\n"
    return value


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def require_compiler_binding(settings: dict[str, Any], semaprax_bin: Path) -> None:
    """Recheck the planned compiler before preparing or dispatching paid work."""
    expected = settings.get("source_binary_sha256")
    if (not isinstance(expected, str) or len(expected) != 64
            or any(character not in "0123456789abcdef" for character in expected)):
        raise ValueError("campaign plan must bind a lowercase SHA-256 compiler binary hash")
    binary = semaprax_bin.expanduser().resolve(strict=True)
    if not binary.is_file() or digest(binary) != expected:
        raise ValueError("compiler binary differs from the immutable campaign plan")
    qualification = settings.get("qualification", {})
    if not isinstance(qualification, dict):
        raise ValueError("campaign qualification metadata must be an object")
    if any(key in qualification for key in ("compiler_source_commit", "compiler_binary_sha256")):
        source = qualification.get("compiler_source_commit")
        if (not isinstance(source, str) or not source
                or source != settings.get("compiler_source_commit")
                or qualification.get("compiler_binary_sha256") != expected):
            raise ValueError("requested compiler source or binary differs from qualification evidence")


def tokenizer_metadata(tokenizer_dir: str | Path | None) -> dict[str, Any] | None:
    if tokenizer_dir is None:
        return None
    root = Path(tokenizer_dir).expanduser().resolve(strict=True)
    package_root = root / "node_modules" / "@anthropic-ai" / "tokenizer"
    tiktoken_root = root / "node_modules" / "tiktoken"
    package_json, tiktoken_json = package_root / "package.json", tiktoken_root / "package.json"
    if not root.is_dir() or not package_json.is_file() or not tiktoken_json.is_file():
        raise ValueError("tokenizer-dir must contain @anthropic-ai/tokenizer and its tiktoken dependency")
    package = json.loads(package_json.read_text(encoding="utf-8"))
    dependency = json.loads(tiktoken_json.read_text(encoding="utf-8"))
    if package.get("name") != TOKENIZER_PACKAGE or package.get("version") != TOKENIZER_VERSION:
        raise ValueError(f"tokenizer-dir must contain {TOKENIZER_PACKAGE}@{TOKENIZER_VERSION}")
    node = shutil.which("node")
    if not node:
        raise ValueError("Node.js is required to use tokenizer-dir")
    node_version = subprocess.run([node, "--version"], text=True, capture_output=True, check=False)
    fingerprint = hashlib.sha256()
    for name, directory in (("anthropic-tokenizer", package_root), ("tiktoken", tiktoken_root)):
        for path in sorted(p for p in directory.rglob("*") if p.is_file() and not p.is_symlink()):
            relative = f"{name}/{path.relative_to(directory).as_posix()}"
            fingerprint.update(relative.encode("utf-8") + b"\0")
            with path.open("rb") as source:
                for chunk in iter(lambda: source.read(1024 * 1024), b""):
                    fingerprint.update(chunk)
    return {
        "package": TOKENIZER_PACKAGE, "version": package.get("version"), "dependency": "tiktoken",
        "dependency_version": dependency.get("version"), "encoding_identity": TOKENIZER_ENCODING,
        "claim": "legacy-Claude tokenizer proxy; not exact current-model or billing tokenization",
        "runtime": "node", "runtime_version": node_version.stdout.strip() if node_version.returncode == 0 else None,
        "tokenizer_dir": str(root), "fingerprint_sha256": fingerprint.hexdigest(),
    }


def tokenize_texts(texts: list[dict[str, str]], metadata: dict[str, Any]) -> list[int]:
    completed = subprocess.run(
        [shutil.which("node") or "node", "-e", TOKENIZE_SCRIPT, metadata["tokenizer_dir"]],
        input=json.dumps(texts, ensure_ascii=False), text=True, capture_output=True, check=False,
        env={"PATH": __import__("os").environ.get("PATH", "")}, timeout=60,
    )
    if completed.returncode:
        raise RuntimeError(f"offline tokenizer failed: {bounded_text(completed.stderr)}")
    response = json.loads(completed.stdout)
    counts = response.get("counts")
    values = [item.get("tokens") for item in counts] if isinstance(counts, list) else []
    if response.get("version") != metadata["version"]:
        raise RuntimeError("tokenizer runtime version differs from pinned metadata")
    if len(values) != len(texts) or not all(isinstance(value, int) and value >= 0 for value in values):
        raise RuntimeError("offline tokenizer returned invalid token counts")
    return values


def authored_source_metrics(candidate: Path, metadata: dict[str, Any] | None, *,
                            exclude_verified_node_modules: bool = False,
                            additional_suffixes: frozenset[str] = frozenset()) -> dict[str, Any]:
    if metadata is None:
        return {"status": "unmeasured", "total_tokens": None, "files": [], "tokenizer": None}
    files = []
    for path in sorted(candidate.rglob("*")):
        relative = path.relative_to(candidate)
        excluded = AUTHORED_EXCLUDED_DIRS - ({"node_modules", ".cache"} if exclude_verified_node_modules else set())
        if (not path.is_file() or path.is_symlink()
                or any(part in excluded for part in relative.parts[:-1])
                or (exclude_verified_node_modules and relative.parts[0] == "node_modules")):
            continue
        if path.suffix.lower() not in AUTHORED_SUFFIXES | additional_suffixes and path.name not in AUTHORED_SPECIAL_NAMES:
            continue
        if path.suffix.lower() in {".c", ".h"}:
            continue
        if path.suffix.lower() in {".js", ".mjs", ".cjs"} and any(
            path.with_suffix(suffix).is_file() for suffix in (".ts", ".tsx", *sorted(additional_suffixes))
        ):
            continue
        content = path.read_text(encoding="utf-8")
        files.append({"path": relative.as_posix(), "content": content, "bytes": path.stat().st_size,
                      "sha256": digest(path)})
    texts = [{"path": row["path"], "content": row.pop("content")} for row in files]
    counts = tokenize_texts(texts, metadata)
    for row, count in zip(files, counts):
        row["tokens"] = count
    excluded_directories = AUTHORED_EXCLUDED_DIRS - ({"node_modules", ".cache"} if exclude_verified_node_modules else set())
    if exclude_verified_node_modules:
        excluded_directories = set(excluded_directories) | {"node_modules (receipt-verified root only)"}
    return {
        "status": "measured_proxy", "scope": "final_candidate_source_inventory_only; not cumulative authored edits or provider output",
        "total_tokens": sum(counts), "files": files, "excluded_directories": sorted(excluded_directories),
        "excluded_generated_extensions": [".c", ".h", "non-text/binary files"], "tokenizer": metadata,
    }


def archive_candidate(candidate: Path, archive: Path, *,
                      exclude_verified_node_modules: bool = False) -> tuple[dict[str, str], list[str]]:
    """Copy authored candidate files, excluding only dependency/cache directories."""
    excluded_paths: list[str] = []

    def ignore(directory: str, names: list[str]) -> set[str]:
        ignored = set()
        for name in names:
            path = Path(directory) / name
            relative = path.relative_to(candidate).as_posix()
            excluded = ARCHIVE_EXCLUDED_DIRS - ({"node_modules", ".cache"} if exclude_verified_node_modules else set())
            if ((name in excluded or (exclude_verified_node_modules and relative == "node_modules"))
                    and (path.is_dir() or path.is_symlink())):
                excluded_paths.append(path.relative_to(candidate).as_posix())
                ignored.add(name)
        return ignored

    if candidate.exists():
        shutil.copytree(candidate, archive, ignore=ignore)
    else:
        archive.mkdir(parents=True)
    hashes = {
        str(path.relative_to(archive)): digest(path)
        for path in sorted(archive.rglob("*")) if path.is_file()
    }
    return hashes, sorted(excluded_paths)


def create_seed_repository(
    source_repo: Path, source_commit: str, seed_repo: Path, seed_files: Iterable[str],
) -> dict[str, Any]:
    """Export exactly pinned public inputs into a fresh parentless Git repository."""
    expected = [path.lstrip("/") for path in seed_files]
    seed_repo.mkdir(parents=True)
    source_bytes: dict[str, bytes] = {}
    for relative in expected:
        exported = subprocess.run(
            ["git", "show", f"{source_commit}:{relative}"], cwd=source_repo,
            capture_output=True, check=False,
        )
        if exported.returncode:
            raise ValueError(f"pinned commit does not contain required benchmark file: {relative}")
        source_bytes[relative] = exported.stdout

    def git(*arguments: str) -> str:
        result = subprocess.run(
            ["git", *arguments], cwd=seed_repo, text=True, capture_output=True, check=False,
        )
        if result.returncode:
            raise RuntimeError(f"seed repository git {arguments[0]} failed: {bounded_text(result.stderr)}")
        return result.stdout.strip()

    git("init", "--quiet", "--template=")
    source_hashes: dict[str, str] = {}
    for relative, content in source_bytes.items():
        path = seed_repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        source_hashes[relative] = hashlib.sha256(content).hexdigest()
    git("add", "--", *expected)
    commit = subprocess.run(
        ["git", "-c", "user.name=SEMAPRAX Benchmark", "-c", "user.email=benchmark@example.invalid",
         "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "Pinned public benchmark inputs"],
        cwd=seed_repo, text=True, capture_output=True, check=False,
    )
    if commit.returncode:
        raise RuntimeError(f"seed repository commit failed: {bounded_text(commit.stderr)}")
    seed_commit = git("rev-parse", "HEAD")
    tree = git("ls-tree", "-r", "--name-only", "HEAD").splitlines()
    parents = git("rev-list", "--parents", "-n", "1", "HEAD").split()
    if tree != expected or parents != [seed_commit]:
        raise RuntimeError("fresh seed repository contains unexpected files or commit history")
    return {
        "source_repository_commit": source_commit,
        "source_files_sha256": source_hashes,
        "seed_repository_commit": seed_commit,
        "seed_files_sha256": source_hashes,
        "seed_repository": str(seed_repo),
    }


def _usage_values(usage: Any) -> dict[str, int | None]:
    if not isinstance(usage, dict):
        return {name: None for name in ALL_USAGE_FIELDS}
    aliases = {
        "input_tokens": ("input_tokens", "inputTokens"),
        "cache_creation_input_tokens": ("cache_creation_input_tokens", "cacheCreationInputTokens"),
        "cache_read_input_tokens": ("cache_read_input_tokens", "cacheReadInputTokens"),
        "output_tokens": ("output_tokens", "outputTokens"),
        "cache_creation_ephemeral_5m_input_tokens": (
            "cache_creation_ephemeral_5m_input_tokens", "ephemeral_5m_input_tokens", "ephemeral5mInputTokens",
        ),
        "cache_creation_ephemeral_1h_input_tokens": (
            "cache_creation_ephemeral_1h_input_tokens", "ephemeral_1h_input_tokens", "ephemeral1hInputTokens",
        ),
    }
    values = {field: next((usage[key] for key in keys if key in usage), None) for field, keys in aliases.items()}
    nested = usage.get("cache_creation", usage.get("cacheCreation"))
    if isinstance(nested, dict):
        for field, key in (
            ("cache_creation_ephemeral_5m_input_tokens", "ephemeral_5m_input_tokens"),
            ("cache_creation_ephemeral_1h_input_tokens", "ephemeral_1h_input_tokens"),
        ):
            if values[field] is None and key in nested:
                values[field] = nested[key]
    return {
        name: int(values[name])
        if isinstance(values[name], int) and not isinstance(values[name], bool) and values[name] >= 0 else None
        for name in ALL_USAGE_FIELDS
    }


def _sum_usage(rows: list[dict[str, int | None]]) -> dict[str, int | None]:
    return {
        name: sum(value for row in rows if (value := row.get(name)) is not None) or 0
        if any(row.get(name) is not None for row in rows) else None
        for name in ALL_USAGE_FIELDS
    }


def input_tokens_total(usage: dict[str, int | None]) -> int | None:
    values = [usage.get(field) for field in USAGE_FIELDS[:3]]
    return sum(values) if all(value is not None for value in values) else None


def legacy_net_input_metrics(turn_usage: list[dict[str, int | None]]) -> dict[str, int | None]:
    baseline = input_tokens_total(turn_usage[0]) if turn_usage else None
    required = USAGE_FIELDS[:3]
    complete = bool(turn_usage) and all(all(turn.get(field) is not None for field in required) for turn in turn_usage)
    per_turn_sum = sum(sum(turn[field] for field in required) for turn in turn_usage) if complete else None
    subtracted = baseline * len(turn_usage) if baseline is not None else None
    return {
        "first_turn_input_plus_cache_tokens": baseline,
        "per_turn_input_plus_cache_tokens_sum": per_turn_sum,
        "baseline_tokens_subtracted": subtracted,
        "net_input_tokens": per_turn_sum - subtracted if per_turn_sum is not None and subtracted is not None else None,
    }


def stream_usage(path: Path, model_preference: str) -> dict[str, Any]:
    """Parse Claude stream JSONL, deduplicate message updates, preserve final totals."""
    usage_by_id: dict[str, dict[str, int | None]] = {}
    raw_usage_by_id: dict[str, dict[str, int | None]] = {}
    updates: dict[str, int] = {}
    texts: dict[str, str] = {}
    message_models: set[str] = set()
    model_usage_models: set[str] = set()
    visible = ""
    tool_use_ids: set[str] = set()
    tool_use_without_id = 0
    result_event: dict[str, Any] | None = None
    invalid = 0
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            invalid += 1
            continue
        if not isinstance(event, dict):
            continue
        if event.get("type") == "assistant":
            message = event.get("message")
            if isinstance(message, dict):
                model = message.get("model")
                if isinstance(model, str):
                    message_models.add(model)
                identity = message.get("id")
                usage = message.get("usage")
                if isinstance(identity, str) and isinstance(usage, dict):
                    parsed = _usage_values(usage)
                    raw_previous = raw_usage_by_id.get(
                        identity, {name: None for name in (*ALL_USAGE_FIELDS, "thinking_tokens")}
                    )
                    raw_usage_by_id[identity] = {
                        name: parsed[name] if parsed[name] is not None else raw_previous[name]
                        for name in ALL_USAGE_FIELDS
                    }
                    thinking = usage.get("thinking_tokens")
                    raw_usage_by_id[identity]["thinking_tokens"] = (
                        thinking if isinstance(thinking, int) and not isinstance(thinking, bool) and thinking >= 0
                        else raw_previous["thinking_tokens"]
                    )
                    previous = usage_by_id.get(identity, {name: None for name in USAGE_FIELDS})
                    usage_by_id[identity] = {name: parsed[name] if parsed[name] is not None else previous[name]
                                              for name in USAGE_FIELDS}
                    updates[identity] = updates.get(identity, 0) + 1
                message_text = ""
                for block in message.get("content", []):
                    if isinstance(block, dict):
                        if block.get("type") == "text":
                            message_text += str(block.get("text", "")) + "\n"
                        elif block.get("type") == "tool_use":
                            identity = block.get("id")
                            if isinstance(identity, str):
                                tool_use_ids.add(identity)
                            else:
                                tool_use_without_id += 1
                if isinstance(identity, str):
                    texts[identity] = message_text
                else:
                    visible += message_text
        if event.get("type") == "result":
            result_event = event
            model_usage = event.get("modelUsage")
            if isinstance(model_usage, dict):
                model_usage_models.update(str(model) for model in model_usage)
    turns = list(usage_by_id.values())
    totals = _sum_usage(turns)
    legacy_net = legacy_net_input_metrics(turns)
    first = next(iter(usage_by_id.values()), {name: None for name in ALL_USAGE_FIELDS})
    provider_usage = _usage_values(result_event.get("usage")) if isinstance(result_event, dict) else None
    final = provider_usage if provider_usage and any(value is not None for value in provider_usage.values()) else None
    if isinstance(result_event, dict):
        model_usage = result_event.get("modelUsage")
        if isinstance(model_usage, dict):
            for model in (model_preference, *sorted(model_usage)):
                parsed = _usage_values(model_usage.get(model))
                if any(value is not None for value in parsed.values()):
                    final = parsed
                    break
        if final is not None and provider_usage is not None:
            for field in CACHE_TTL_USAGE_FIELDS:
                if provider_usage[field] is not None:
                    final[field] = provider_usage[field]
    discrepancies: dict[str, Any] = {}
    if final is not None:
        for name in ALL_USAGE_FIELDS:
            if totals[name] is not None and final[name] is not None and totals[name] != final[name]:
                discrepancies[name] = {"per_turn_sum": totals[name], "final_result": final[name]}
        totals = {name: final[name] if final[name] is not None else totals[name] for name in ALL_USAGE_FIELDS}
    cost = result_event.get("total_cost_usd") if isinstance(result_event, dict) else None
    if isinstance(cost, bool) or not isinstance(cost, (int, float)) or not math.isfinite(cost) or cost < 0:
        cost = None
    provider_turns = result_event.get("num_turns") if isinstance(result_event, dict) else None
    if isinstance(provider_turns, bool) or not isinstance(provider_turns, int) or provider_turns < 0:
        provider_turns = None
    return {
        "models_observed": sorted(message_models or model_usage_models),
        "assistant_message_models_observed": sorted(message_models),
        "model_usage_keys_observed": sorted(model_usage_models),
        "turns_with_usage": len(usage_by_id),
        "turn_usage_by_message": [
            {"message_id": identity, "usage": values}
            for identity, values in raw_usage_by_id.items()
        ],
        "turns_with_usage_definition": "deduplicated assistant message IDs carrying usage; not tool calls or the provider's session turn count",
        "provider_reported_session_turns": provider_turns,
        "legacy_net_input": legacy_net,
        "legacy_net_input_definition": (
            "sum of deduplicated per-turn input/cache-write/cache-read minus first-turn input/cache total multiplied by turn count; "
            "reproduces historical operational convention, not task-only model input"
        ),
        "usage": totals,
        "usage_totals_source": "final_result_with_per_turn_fallback" if final else "per_turn_deduplicated",
        "provider_reported_api_equivalent_total_cost_usd": cost,
        "provider_reported_api_equivalent_cost_note": (
            "result.total_cost_usd from the provider stream, when supplied; not a receipt or account-billed amount"
        ),
        "usage_updates_per_message": updates,
        "usage_discrepancies": discrepancies,
        "first_turn_usage": first,
        "fixed_context_tokens_in_this_session": None,
        "fixed_context_note": "Not isolated within this transcript; calibration is a separate proxy.",
        "visible_output_bytes": len((visible + "".join(texts.values())).encode("utf-8")),
        "tool_use_events": len(tool_use_ids) + tool_use_without_id,
        "provider_output_tokens": totals["output_tokens"],
        "result_event": result_event,
        "invalid_stream_lines": invalid,
    }


def cache_write_pricing(usage: dict[str, int | None]) -> dict[str, Any]:
    total = usage.get("cache_creation_input_tokens")
    five, hour = (usage.get(field) for field in CACHE_TTL_USAGE_FIELDS)
    if total is None and five is not None and hour is not None:
        total = five + hour
    if total is None:
        return {"basis": "unavailable", "five_minute_tokens": None, "one_hour_tokens": None}
    if five is not None and hour is not None:
        if five + hour != total:
            return {"basis": "inconsistent_provider_ttl_breakdown", "five_minute_tokens": five,
                    "one_hour_tokens": hour}
        return {"basis": "provider_ttl_breakdown", "five_minute_tokens": five, "one_hour_tokens": hour}
    if five is not None:
        if five > total:
            return {"basis": "inconsistent_provider_ttl_breakdown", "five_minute_tokens": five,
                    "one_hour_tokens": None}
        return {"basis": "provider_ttl_breakdown_and_total", "five_minute_tokens": five,
                "one_hour_tokens": total-five}
    if hour is not None:
        if hour > total:
            return {"basis": "inconsistent_provider_ttl_breakdown", "five_minute_tokens": None,
                    "one_hour_tokens": hour}
        return {"basis": "provider_ttl_breakdown_and_total", "five_minute_tokens": total-hour,
                "one_hour_tokens": hour}
    return {"basis": "assumed_all_cache_writes_5m", "five_minute_tokens": total, "one_hour_tokens": 0}


def rate_card_estimate_details(
    usage: dict[str, int | None],
    per_million_prices: dict[str, float] | None = None,
) -> dict[str, Any]:
    prices = PRICE_USD_PER_MTOK if per_million_prices is None else per_million_prices
    if set(prices) != set(PRICE_USD_PER_MTOK) or any(
        isinstance(rate, bool) or not isinstance(rate, (int, float)) or not math.isfinite(rate) or rate < 0
        for rate in prices.values()
    ):
        raise ValueError("rate card must provide finite nonnegative prices for every usage bucket")
    cache = cache_write_pricing(usage)
    required = ("input_tokens", "cache_read_input_tokens", "output_tokens")
    known = cache["basis"] not in {"unavailable", "inconsistent_provider_ttl_breakdown"}
    if not all(usage.get(key) is not None for key in required) or not known:
        return {"usd": None, "cache_write_pricing": cache}
    amount = (
        usage["input_tokens"] * prices["input"]
        + cache["five_minute_tokens"] * prices["cache_write_5m"]
        + cache["one_hour_tokens"] * prices["cache_write_1h"]
        + usage["cache_read_input_tokens"] * prices["cache_read"]
        + usage["output_tokens"] * prices["output"]
    ) / 1_000_000
    return {"usd": round(amount, 6), "cache_write_pricing": cache}


def observed_model_matches(observed: Any, expected_observed_id: Any) -> bool:
    return isinstance(expected_observed_id, str) and observed == [expected_observed_id]


def claude_command(settings: dict[str, Any], prompt: str) -> list[str]:
    command = [
        "claude", "--print", "--output-format", "stream-json", "--verbose", "--model", settings["model"],
        "--effort", settings["effort"], "--no-session-persistence", "--permission-mode", "acceptEdits",
        "--permission-prompts", "none", "--restricted", "--strict-mcp-config", "--tools",
        "Bash,Read,Edit,Write,Glob,Grep", "--allowedTools", "Bash,Read,Edit,Write,Glob,Grep",
    ]
    if settings.get("max_budget_usd") is not None:
        command.extend(["--max-budget-usd", str(settings["max_budget_usd"])])
    return [*command, "--", prompt]


def add_seed_worktree(seed_repo: Path, workspace: Path, seed_commit: str, seed_files: Iterable[str]) -> str | None:
    added = subprocess.run(["git", "worktree", "add", "--detach", str(workspace), seed_commit],
                           cwd=seed_repo, text=True, capture_output=True, check=False)
    if added.returncode:
        return f"worktree creation failed: {bounded_text(added.stderr)}"
    tracked = subprocess.run(["git", "ls-files"], cwd=workspace, text=True, capture_output=True, check=False)
    if tracked.returncode:
        return f"minimal seed inventory check failed: {bounded_text(tracked.stderr)}"
    visible = sorted(str(path.relative_to(workspace)) for path in workspace.rglob("*")
                     if path.is_file() and ".git" not in path.parts)
    expected = sorted(path.lstrip("/") for path in seed_files)
    tracked_paths = sorted(tracked.stdout.splitlines())
    return None if visible == expected and tracked_paths == expected else (
        f"minimal seed checkout exposed unexpected files: visible={visible}, tracked={tracked_paths}"
    )


def run_claude(command: list[str], workspace: Path, env: dict[str, str], stream_path: Path,
               stderr_path: Path, timeout_seconds: int) -> dict[str, Any]:
    started, timed_out = time.monotonic(), False
    try:
        with stream_path.open("wb") as output, stderr_path.open("wb") as errors:
            process = subprocess.Popen(command, cwd=workspace, stdout=output, stderr=errors, env=env)
            try:
                exit_code = process.wait(timeout=timeout_seconds)
            except subprocess.TimeoutExpired:
                timed_out = True
                process.terminate()
                try:
                    exit_code = process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    exit_code = process.wait()
        failure = None
    except OSError as error:
        exit_code, failure = None, str(error)
    return {"process_exit_code": exit_code, "timed_out": timed_out,
            "elapsed_seconds": round(time.monotonic() - started, 3), "failure": failure}


def save_json(path: Path, value: Any) -> None:
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    temporary.replace(path)
