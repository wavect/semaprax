#!/usr/bin/env python3
"""Offline, trace-backed report for a Codex ShiftSim campaign.

This command only reads a saved ``results.json`` and its evidence files.  It
never starts Codex, invokes the compiler, or changes campaign artifacts.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import statistics
from pathlib import Path
from typing import Any

import codex_campaign as campaign


METRICS = (
    "model_request_turns", "model_requests", "raw_input_tokens", "cached_input_tokens",
    "cache_write_input_tokens", "output_tokens", "legacy_net_input_tokens",
    "final_authored_tokens_proxy", "agent_wall_seconds",
    "acceptance_wall_seconds",
)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _number(value: Any) -> bool:
    return type(value) in (int, float) and math.isfinite(value)


def _metric(values: list[Any]) -> dict[str, Any] | None:
    if not values or not all(_number(value) for value in values):
        return None
    return {
        "total": sum(values),
        "mean_per_attempt": statistics.mean(values),
        "median_per_attempt": statistics.median(values),
    }


def summarize(data: dict[str, Any], rows: list[dict[str, Any]]) -> dict[str, Any]:
    """Summarize recorded attempts without treating missing telemetry as zero."""
    campaign_data = data.get("campaign", {})
    order = campaign_data.get("trial_order", [])
    require(isinstance(order, list), "campaign trial_order is missing or invalid")
    require(len(rows) <= len(order), "more attempts than planned")
    counts = {arm: 0 for arm in campaign.ARMS}
    for row, expected in zip(rows, order):
        arm = row.get("arm")
        require(arm in counts, f"unknown arm: {arm}")
        counts[expected] += 1
        require((arm, row.get("number")) == (expected, counts[expected]),
                "attempt order differs from plan")

    arms: dict[str, Any] = {}
    for arm in campaign.ARMS:
        selected = [row for row in rows if row.get("arm") == arm]
        accepted = sum(row.get("status") == "accepted" for row in selected)
        costs = [row.get("conditional_api_equivalent_usd") for row in selected]
        total_cost = sum(costs) if costs and all(_number(value) for value in costs) else None
        values = {key: [row.get(key) for row in selected] for key in METRICS}
        arms[arm] = {
            "planned": order.count(arm),
            "recorded": len(selected),
            "attempted": len(selected),
            "accepted": accepted,
            "failed_or_rejected": len(selected) - accepted,
            "conditional_short_context_api_equivalent_usd_all_recorded_attempts": total_cost,
            "conditional_short_context_api_equivalent_usd_per_accepted_task": (
                total_cost / accepted if total_cost is not None and accepted else None
            ),
            "actual_billed_usd": None,
            "actual_billed_usd_per_accepted_task": None,
            "metrics_all_recorded_attempts": {key: _metric(value) for key, value in values.items()},
            "metrics_per_attempt": values,
            "model_request_turns_per_attempt": values["model_request_turns"],
            "model_request_turns_total": (sum(values["model_request_turns"])
                                           if values["model_request_turns"] and
                                           all(_number(value) for value in values["model_request_turns"])
                                           else None),
            "raw_input_tokens_per_attempt": values["raw_input_tokens"],
            "cached_input_tokens_per_attempt": values["cached_input_tokens"],
            "cache_write_input_tokens_per_attempt": values["cache_write_input_tokens"],
            "output_tokens_per_attempt": values["output_tokens"],
            "legacy_net_input_tokens_per_attempt": values["legacy_net_input_tokens"],
            "final_authored_claude_bpe_proxy_per_attempt": values["final_authored_tokens_proxy"],
            "cost_includes_failed_attempts": True,
            "cost_note": "Conditional standard short-context API-equivalent estimate; actual billing is unavailable.",
        }

    complete = len(rows) == len(order)
    all_arms_complete = all(arms[arm]["recorded"] >= campaign.MIN_TRIALS_PER_ARM for arm in campaign.ARMS)
    comparison_ready = complete and all_arms_complete
    return {
        "complete": complete,
        "all_arms_have_minimum_trials": all_arms_complete,
        "comparison_status": (
            "complete; all arms have five recorded trials"
            if comparison_ready else "incomplete; no comparative headline is supported"
        ),
        "winner": None,
        "arms": arms,
        "planned_attempts": len(order),
        "recorded_attempts": len(rows),
        "unlaunched_order": order[len(rows):],
        "trials": rows,
    }


def _path(value: Any) -> Path | None:
    return Path(value) if isinstance(value, str) and value else None


def _acceptance_seconds(original: dict[str, Any]) -> float | None:
    value = original.get("acceptance_elapsed_seconds")
    if _number(value):
        return value
    acceptance = original.get("acceptance")
    if not isinstance(acceptance, dict):
        return None
    pieces = [acceptance.get(key, {}).get("seconds") for key in ("build", "candidate_tests")
              if isinstance(acceptance.get(key), dict)]
    checks = acceptance.get("checks", [])
    if isinstance(checks, list):
        pieces.extend(check.get("seconds") for check in checks if isinstance(check, dict))
    return sum(pieces) if pieces and all(_number(value) for value in pieces) else None


def _recount_trial(original: dict[str, Any]) -> dict[str, Any]:
    label = f"{original.get('arm')}-{original.get('number', 0):02d}"
    stream, trace = _path(original.get("transcript")), _path(original.get("rollout_trace"))
    evidence: dict[str, str] = {}
    if stream is not None and stream.is_file():
        evidence["exec"] = digest(stream)
    if trace is not None and trace.is_file():
        evidence["rollout"] = digest(trace)
    if isinstance(original.get("prompt_sha256"), str):
        evidence["prompt"] = original["prompt_sha256"]
    source_hashes = original.get("candidate_source_sha256")
    if isinstance(source_hashes, dict):
        evidence["candidate_manifest"] = hashlib.sha256(
            json.dumps(source_hashes, sort_keys=True, separators=(",", ":")).encode("utf-8")
        ).hexdigest()
    observed: dict[str, Any] = {}
    price: dict[str, Any] = {"standard_short_context_api_equivalent_usd": None,
                             "actual_billed_usd": None, "reason": "missing trace evidence"}
    if stream is not None and trace is not None and stream.is_file() and trace.is_file():
        observed = campaign.codex.trace_usage(campaign.codex.parse_exec_jsonl(stream), trace)
        if observed.get("reconciled"):
            price = campaign.codex.list_price_estimate(observed.get("model_requests", []))
        else:
            price["reason"] = "unreconciled task-owned rollout usage"
    usage = observed.get("request_usage_sum", {}) if observed.get("reconciled") else {}
    metrics = original.get("final_candidate_source_metrics", {})
    authored = metrics.get("total_tokens") if isinstance(metrics, dict) else None
    if not _number(authored):
        authored = None
    elapsed = original.get("elapsed_seconds") if _number(original.get("elapsed_seconds")) else None
    acceptance = _acceptance_seconds(original)
    return {
        "arm": original.get("arm"), "number": original.get("number"),
        "status": original.get("status"), "failure": original.get("failure"),
        "trace_status": "reconciled" if observed.get("reconciled") else "missing_or_unreconciled",
        "model_observed": observed.get("model_observed") if observed.get("reconciled") else None,
        "effort_observed": observed.get("effort_observed") if observed.get("reconciled") else None,
        "provider_resolved_model": None,
        "model_request_turns": observed.get("model_request_count") if observed.get("reconciled") else None,
        "model_requests": observed.get("model_request_count") if observed.get("reconciled") else None,
        "raw_input_tokens": usage.get("input_tokens"),
        "cached_input_tokens": usage.get("cached_input_tokens"),
        "cache_write_input_tokens": usage.get("cache_write_input_tokens"),
        "output_tokens": usage.get("output_tokens"),
        "legacy_net_input_tokens": observed.get("legacy_net_input_tokens") if observed.get("reconciled") else None,
        "legacy_net_input_is_proxy": True,
        "final_authored_tokens_proxy": authored,
        "agent_wall_seconds": elapsed,
        "acceptance_wall_seconds": acceptance,
        "conditional_api_equivalent_usd": price.get("standard_short_context_api_equivalent_usd"),
        "actual_billed_usd": None,
        "evidence_sha256": evidence,
    }


def recount(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(data.get("trials"), list), "results file has no trials array")
    require(all(isinstance(row, dict) for row in data["trials"]),
            "results file contains a malformed trial")
    rows = [_recount_trial(row) for row in data["trials"]]
    result = summarize(data, rows)
    campaign_data = data.get("campaign", {})
    calibration = data.get("calibration")
    result.update({
        "schema": "semaprax.event-sim-codex-report.v1",
        "results_sha256": digest(path),
        "campaign_status": data.get("campaign_status"),
        "recorded_attempts": len(rows),
        "actual_billed_usd": None,
        "provenance": {key: campaign_data.get(key) for key in (
            "adapter", "round", "repository_commit", "compiler_source_commit",
            "source_binary_sha256", "seed_files_sha256", "model_requested",
            "effort_requested", "codex_version", "price_book",
            "authored_source_tokenizer")},
        "measurement_notes": {
            "turns": "Model request turns come only from reconciled task-owned rollout records; outer CLI turns and tool items are separate.",
            "input": "Raw input, cache reads, cache writes, and output remain separate counters; missing buckets stay null.",
            "net_input": "Legacy net input is an explicit historical proxy: summed input minus first-request input times request count; it is not task-only input.",
            "authored": "Final authored source is a Claude BPE tokenizer proxy over the final candidate inventory, not generated output or billed tokens.",
            "cost": "Conditional Standard short-context API-equivalent estimate includes every recorded attempt, including failures, divided by accepted tasks; actual billed cost is unavailable.",
        },
        "fixed_harness_context_tokens": None,
        "fixed_harness_composition": None,
        "fixed_harness_context_note": "No per-request fixed system/tool/task/history composition is exposed; no context baseline is subtracted.",
        "calibration_separate": calibration,
        "calibration_fixed_composition_tokens": None,
        "calibration_note": "Calibration is a separate diagnostic request and is never subtracted from trial usage or cost.",
    })
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path)
    args = parser.parse_args()
    print(json.dumps(recount(args.results.resolve(strict=True)), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
