#!/usr/bin/env python3
"""Authenticate the retained Codex token events and monetary-receipt absence.

This is deliberately an offline adapter.  It reads the exact JSONL streams
already named by the retained trial records and never estimates a charge from
tokens or a public price list.  A provider invoice or a provider-reported
monetary field is required before a numeric cost can be recorded.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import stat


SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-cost-provenance.v1"
PROVIDER = "openai-codex-cli"
TOKEN_FIELDS = ("input_tokens", "cached_input_tokens", "output_tokens")
MONETARY_FIELD_NAMES = frozenset((
    "amount", "amount_usd", "billing", "charge", "charge_usd", "cost",
    "cost_micros", "cost_usd", "price", "price_usd",
))


def sha256(body: bytes) -> str:
    return "sha256:" + hashlib.sha256(body).hexdigest()


def reference(path: pathlib.Path, root: pathlib.Path) -> dict:
    body = path.read_bytes()
    return {"path": path.relative_to(root).as_posix(), "bytes": len(body), "sha256": sha256(body)}


def read_regular(path: pathlib.Path, label: str) -> bytes:
    information = path.lstat()
    if not stat.S_ISREG(information.st_mode) or stat.S_ISLNK(information.st_mode):
        raise ValueError(f"{label} is not a regular file")
    return path.read_bytes()


def record_events(record_path: pathlib.Path) -> tuple[dict, pathlib.Path, bytes]:
    record = json.loads(read_regular(record_path, "trial record"))
    if record.get("schema") != "semaprax.bend2-law-benchmark.codex-agent-trial.v2":
        raise ValueError("trial record has an unsupported schema")
    event_ref = record.get("artifacts", {}).get("events")
    if not isinstance(event_ref, dict) or set(event_ref) != {"path", "bytes", "sha256"}:
        raise ValueError("trial record does not name its event stream")
    name = event_ref["path"]
    relative = pathlib.PurePosixPath(name) if isinstance(name, str) else None
    if relative is None or relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts):
        raise ValueError("trial event stream path is unsafe")
    # The runner writes artifacts beneath a fresh evidence directory.  The
    # checked-in campaign keeps its summary record beside that directory, so
    # reconstruct the retained child from the record stem instead of assuming
    # that the summary itself lives in the runner's evidence directory.
    events_path = record_path.parent / record_path.stem / pathlib.Path(*relative.parts)
    body = read_regular(events_path, "trial event stream")
    if event_ref["bytes"] != len(body) or event_ref["sha256"] != sha256(body):
        raise ValueError("trial event stream disagrees with its retained record")
    return record, events_path, body


def provider_monetary_paths(value: object, path: str = "") -> list[str]:
    """Find monetary fields in provider event metadata, excluding agent text."""
    if isinstance(value, dict):
        if value.get("type") == "agent_message":
            return []
        found = []
        for key, child in value.items():
            child_path = f"{path}.{key}" if path else key
            if key in MONETARY_FIELD_NAMES:
                found.append(child_path)
            found.extend(provider_monetary_paths(child, child_path))
        return found
    if isinstance(value, list):
        return [item for index, child in enumerate(value) for item in provider_monetary_paths(child, f"{path}[{index}]")]
    return []


def usage_and_absence(events: bytes) -> tuple[dict, dict]:
    try:
        rows = [json.loads(line) for line in events.splitlines() if line]
    except json.JSONDecodeError as error:
        raise ValueError("trial event stream is not JSONL") from error
    completed = [row for row in rows if isinstance(row, dict) and row.get("type") == "turn.completed"]
    if len(completed) != 1 or not isinstance(completed[0].get("usage"), dict):
        raise ValueError("trial has no unique provider token usage event")
    usage = completed[0]["usage"]
    if any(not isinstance(usage.get(field), int) or usage[field] < 0 for field in TOKEN_FIELDS):
        raise ValueError("provider token usage fields are invalid")
    if usage["cached_input_tokens"] > usage["input_tokens"]:
        raise ValueError("provider cached input exceeds input tokens")
    monetary_paths = sorted(set(provider_monetary_paths(rows)))
    if monetary_paths:
        raise ValueError("provider monetary fields were observed; retain an authenticated provider receipt instead")
    return (
        {field: usage[field] for field in TOKEN_FIELDS} | {"total_tokens": usage["input_tokens"] + usage["output_tokens"]},
        {
            "status": "unavailable",
            "provider": PROVIDER,
            "receipt_kind": "provider_monetary_usage_event",
            "reason": "retained Codex JSONL contains no provider monetary usage field or invoice reference",
            "observed_monetary_field_paths": monetary_paths,
        },
    )


def trial_paths(root: pathlib.Path) -> list[pathlib.Path]:
    paths = [
        root / "law16-boolean-negation-agent-pilot-v1" / f"{lane}-ordinal-1.json"
        for lane in ("bend", "semaprax")
    ]
    for ordinal in range(2, 11):
        paths.extend(root / "law16-boolean-negation-agent-campaign-v1" / f"ordinal-{ordinal}" / f"{lane}.json" for lane in ("bend", "semaprax"))
    return paths


def capture(evidence_root: pathlib.Path) -> dict:
    evidence_root = evidence_root.resolve(strict=True)
    rows = []
    for record_path in trial_paths(evidence_root):
        record, events_path, events = record_events(record_path)
        trial = record.get("trial", {})
        if not isinstance(trial.get("id"), str) or trial.get("language") not in {"bend2", "semaprax-scalar-v1"}:
            raise ValueError("trial record identity is invalid")
        tokens, cost = usage_and_absence(events)
        recorded = record.get("telemetry", {}).get("token_usage")
        if recorded != tokens:
            raise ValueError("trial record token usage disagrees with its provider event")
        rows.append({
            "trial_id": trial["id"],
            "language": trial["language"],
            "record": reference(record_path, evidence_root),
            "provider_event_stream": reference(events_path, evidence_root),
            "token_usage": tokens,
            "cost_usage": cost,
        })
    if len(rows) != 20 or len({row["trial_id"] for row in rows}) != 20:
        raise ValueError("retained Boolean campaign is not ten distinct matched pairs")
    totals = {field: sum(row["token_usage"][field] for row in rows) for field in (*TOKEN_FIELDS, "total_tokens")}
    return {
        "schema": SCHEMA,
        "status": "provider_monetary_receipts_unavailable",
        "scope": {"task": "boolean-negation-pair-v1", "matched_pairs": 10, "trials": 20, "provider": PROVIDER},
        "trials": rows,
        "aggregate_token_usage": totals,
        "cost_usage": {
            "status": "unavailable",
            "provider": PROVIDER,
            "receipt_kind": "provider_monetary_usage_event",
            "reason": "none of the 20 retained Codex JSONL streams contains a provider monetary usage field or invoice reference",
            "observed_monetary_field_paths": [],
        },
        "nonclaims": [
            "token counts are not converted to currency",
            "this receipt does not authenticate an external provider invoice",
            "this Boolean-only provenance does not complete LAW-16 or an agent-trial comparison",
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence-root", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.output.exists() or not args.output.parent.is_dir():
        parser.error("output must be a new file beneath an existing directory")
    try:
        value = capture(args.evidence_root)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        parser.error(str(error))
    args.output.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
