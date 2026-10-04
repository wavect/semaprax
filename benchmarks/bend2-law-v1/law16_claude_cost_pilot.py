#!/usr/bin/env python3
"""Sanitize one bounded Claude stream into LAW-16 cost-pilot provenance.

The collector never starts Claude.  It accepts a retained JSONL stream from a
separately bounded invocation and records only provider telemetry required to
decide whether a source/outcome pilot may advance to a campaign.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib


SCHEMA = "semaprax.bend2-law-benchmark.claude-cost-pilot.v1"
PLAN_SCHEMA = "semaprax.bend2-law-benchmark.claude-cost-pilot-plan.v1"


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def capture(plan_path: pathlib.Path, events_path: pathlib.Path) -> dict:
    plan = json.loads(plan_path.read_text())
    if plan.get("schema") != PLAN_SCHEMA:
        raise ValueError("pilot plan has unsupported schema")
    raw = events_path.read_bytes()
    try:
        rows = [json.loads(line) for line in raw.splitlines() if line]
    except json.JSONDecodeError as error:
        raise ValueError("Claude stream is not JSONL") from error
    init = [row for row in rows if row.get("type") == "system" and row.get("subtype") == "init"]
    result = [row for row in rows if row.get("type") == "result"]
    if len(init) != 1 or len(result) != 1:
        raise ValueError("Claude stream lacks one init and one result event")
    expected_model = plan["provider"]["model_id"]
    if init[0].get("model") != expected_model:
        raise ValueError("Claude stream model differs from the pinned plan")
    completed = result[0]
    cost = completed.get("total_cost_usd")
    if not isinstance(cost, (int, float)) or isinstance(cost, bool) or cost < 0:
        raise ValueError("Claude result lacks a nonnegative monetary cost")
    usage = completed.get("usage")
    if not isinstance(usage, dict):
        raise ValueError("Claude result lacks usage telemetry")
    fields = ("input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")
    if any(not isinstance(usage.get(field), int) or usage[field] < 0 for field in fields):
        raise ValueError("Claude usage telemetry is invalid")
    max_cost = float(plan["budget"]["max_cost_usd"])
    accepted = completed.get("is_error") is False and cost <= max_cost
    return {
        "schema": SCHEMA,
        "status": "eligible_for_source_outcome_review" if accepted else "blocked_before_source_outcome_review",
        "plan": {"path": plan_path.name, "sha256": digest(plan_path.read_bytes())},
        "provider": {"id": plan["provider"]["id"], "model_id": expected_model, "cli_version": init[0].get("claude_code_version")},
        "trial": plan["trial"],
        "budget": plan["budget"],
        "provider_stream": {"bytes": len(raw), "sha256": digest(raw)},
        "provider_usage": {field: usage[field] for field in fields},
        "provider_cost_usd": cost,
        "result": {"is_error": completed.get("is_error"), "subtype": completed.get("subtype"), "duration_ms": completed.get("duration_ms")},
        # A provider bill alone never establishes the required source and
        # independent candidate/attack outcomes, so this collector cannot
        # admit a campaign even after a successful, in-budget inference.
        "campaign_admission": False,
        "nonclaims": [
            "the collector does not retain assistant text or claim a source outcome",
            "a provider cost event without a successful source and independent route outcome cannot start a campaign",
            "no currency value is derived from token counters",
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", required=True, type=pathlib.Path)
    parser.add_argument("--events", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.output.exists() or not args.output.parent.is_dir():
        parser.error("output must be a new file beneath an existing directory")
    try:
        value = capture(args.plan, args.events)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        parser.error(str(error))
    args.output.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
