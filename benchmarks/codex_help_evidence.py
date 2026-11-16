"""Count direct compiler help calls represented by completed Codex exec items."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shlex
from typing import Any


SHELL_CONTROL = ";&|<>()"


def _words(command: str) -> list[str]:
    if "\n" in command or "\r" in command or "`" in command or "$(" in command:
        return []
    lexer = shlex.shlex(command, posix=True, punctuation_chars=SHELL_CONTROL)
    lexer.whitespace_split = True
    try:
        return list(lexer)
    except ValueError:
        return []


def _help_arguments(command: str) -> list[str] | None:
    words = _words(command)
    if len(words) == 3 and Path(words[0]).name in {"bash", "sh", "zsh"} and words[1] in {"-c", "-lc"}:
        words = _words(words[2])
    if not words or any(word and all(char in SHELL_CONTROL for char in word) for word in words):
        return None
    if words[0] not in {"$SEMAPRAX_BIN", "${SEMAPRAX_BIN}"} and Path(words[0]).name not in {
            "semaprax", "semaprax-full"}:
        return None
    arguments = words[1:]
    flags = arguments[:arguments.index("--")] if "--" in arguments else arguments
    if not arguments or (arguments[0] != "help" and not {"-h", "--help"}.intersection(flags)):
        return None
    return arguments


def capture(path: Path) -> dict[str, Any] | None:
    """Bind captured UTF-8 bytes to exec record locations, within the direct-call scope."""
    if not path.is_file():
        return None
    rows = []
    seen = set()
    incomplete = False
    transcript_hash = hashlib.sha256()
    valid_events = 0
    with path.open("rb") as stream:
        for line_number, line in enumerate(stream, 1):
            transcript_hash.update(line)
            try:
                event = json.loads(line.decode("utf-8"))
            except (json.JSONDecodeError, UnicodeDecodeError):
                incomplete = True
                continue
            if not isinstance(event, dict) or not isinstance(event.get("type"), str):
                incomplete = True
                continue
            valid_events += 1
            if event["type"] != "item.completed":
                continue
            item = event.get("item")
            if not isinstance(item, dict) or not isinstance(item.get("type"), str):
                incomplete = True
                continue
            if item["type"] != "command_execution":
                continue
            command = item.get("command")
            if not isinstance(command, str):
                incomplete = True
                continue
            arguments = _help_arguments(command)
            if arguments is None:
                continue
            item_id = item.get("id")
            if not isinstance(item_id, str) or not item_id or item_id in seen:
                incomplete = True
                continue
            seen.add(item_id)
            captured = item.get("aggregated_output")
            try:
                output = captured.encode("utf-8") if isinstance(captured, str) else None
            except UnicodeEncodeError:
                output = None
            incomplete |= output is None
            rows.append({"line": line_number, "item_id": item_id, "arguments": arguments,
                         "captured_utf8_bytes": len(output) if output is not None else None,
                         "captured_sha256": hashlib.sha256(output).hexdigest() if output is not None else None})
    incomplete |= valid_events == 0
    return {
        "transcript_sha256": transcript_hash.hexdigest(),
        "scope": "direct compiler help commands in completed exec items; compound shell commands are outside this count",
        "complete_within_scope": not incomplete,
        "captured_help_calls": len(rows) if not incomplete else None,
        "captured_help_utf8_bytes": sum(row["captured_utf8_bytes"] for row in rows) if not incomplete else None,
        "delivered_help_utf8_bytes": None,
        "delivered_help_tokens": None,
        "delivery_evidence": "requires model request evidence linked to completed exec items",
        "items": rows,
    }


def summarize(trials: list[dict[str, Any]], accepted: int) -> dict[str, Any]:
    """Use all attempts in the numerator, including failed or rejected trials."""
    attempts = [trial.get("help_output") for trial in trials]
    metrics = {}
    for key in ("captured_help_calls", "captured_help_utf8_bytes",
                "delivered_help_utf8_bytes", "delivered_help_tokens"):
        values = [attempt.get(key) if isinstance(attempt, dict) else None for attempt in attempts]
        total = sum(values) if values and all(type(value) is int and value >= 0 for value in values) else None
        metrics[key] = {
            "total_all_attempts": total,
            "per_accepted_task_including_failures": total / accepted if total is not None and accepted else None,
        }
    return metrics
