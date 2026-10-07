"""Strict, optional import of per-trial billing and request-composition evidence.

Sidecar hashes and bindings authenticate the bytes and their association with a
retained campaign trial. They do not authenticate provider origin. Caller-supplied
provider exports therefore remain reported receipt amounts, never verified
account charges.
"""

from __future__ import annotations

from decimal import Decimal, InvalidOperation
import hashlib
import json
from pathlib import Path
import re
from typing import Any

RECEIPT_SCHEMA = "semaprax.cli-tokens.provider-receipt-binding.v1"
TRACE_SCHEMA = "semaprax.cli-tokens.request-context-trace.v1"
HEX_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
DECIMAL_USD = re.compile(r"(?:0|[1-9][0-9]*)(?:\.[0-9]{1,12})?\Z")
USAGE_FIELDS = (
    "input_tokens",
    "cache_creation_input_tokens",
    "cache_read_input_tokens",
    "output_tokens",
)
RAW_USAGE_FIELDS = (*USAGE_FIELDS, "thinking_tokens")
CONTEXT_BUCKETS = (
    "system_tokens",
    "tool_schema_tokens",
    "task_prompt_tokens",
    "conversation_history_tokens",
)
CONTEXT_TOTALS = (*CONTEXT_BUCKETS, "fixed_harness_context_tokens")


def _sha(value: Any, label: str) -> str:
    if not isinstance(value, str) or not HEX_SHA256.fullmatch(value):
        raise ValueError(f"{label} must be a lowercase SHA-256 digest")
    return value


def _integer(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ValueError(f"{label} must be a nonnegative integer")
    return value


def _read_json(path: Path, label: str) -> tuple[dict[str, Any], str]:
    try:
        raw = path.read_bytes()
        value = json.loads(raw)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError(f"{label} is not readable JSON: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a JSON object")
    return value, hashlib.sha256(raw).hexdigest()


def _strict_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    if set(value) != expected:
        raise ValueError(f"{label} fields differ from the closed v1 schema")


def _binding(expected: dict[str, Any], supplied: Any, label: str) -> None:
    fields = {
        "campaign_sha256", "results_sha256", "trial_id", "arm", "number",
        "model_id", "prompt_sha256", "transcript_sha256",
    }
    if not isinstance(supplied, dict):
        raise ValueError(f"{label} binding must be an object")
    _strict_keys(supplied, fields, f"{label} binding")
    normalized = dict(supplied)
    if (not isinstance(expected.get("number"), int)
            or isinstance(expected.get("number"), bool)
            or expected["number"] < 1):
        raise ValueError(f"{label} expected trial number is invalid")
    for key in ("campaign_sha256", "results_sha256", "prompt_sha256", "transcript_sha256"):
        _sha(normalized[key], f"{label} binding {key}")
    for key in ("trial_id", "arm", "model_id"):
        if not isinstance(normalized[key], str) or not normalized[key]:
            raise ValueError(f"{label} binding {key} must be a nonempty string")
    if (not isinstance(normalized["number"], int)
            or isinstance(normalized["number"], bool)
            or normalized["number"] < 1):
        raise ValueError(f"{label} binding number must be a positive integer")
    if normalized["trial_id"] != f"{normalized['arm']}-{normalized['number']:02d}":
        raise ValueError(f"{label} trial_id does not match arm and number")
    if normalized != expected:
        raise ValueError(f"{label} binding does not match the retained campaign trial")


def _artifact_path(root: Path, relative: Any, label: str) -> Path:
    candidate = Path(relative) if isinstance(relative, str) else None
    if (candidate is None or not relative or candidate.is_absolute()
            or ".." in candidate.parts):
        raise ValueError(f"{label} path must be a nonempty relative path")
    try:
        path = (root / candidate).resolve(strict=True)
    except (OSError, RuntimeError) as error:
        raise ValueError(f"{label} path cannot be resolved inside the artifact directory") from error
    try:
        path.relative_to(root.resolve(strict=True))
    except ValueError as error:
        raise ValueError(f"{label} path escapes the artifact directory") from error
    if not path.is_file():
        raise ValueError(f"{label} path is not a regular file")
    return path


def _receipt(root: Path, sidecar: Path, expected: dict[str, Any]) -> dict[str, Any]:
    sidecar = _artifact_path(root, str(sidecar.relative_to(root)), "provider receipt sidecar")
    value, sidecar_hash = _read_json(sidecar, "provider receipt sidecar")
    _strict_keys(value, {"schema", "binding", "provenance", "billed"}, "provider receipt")
    if value.get("schema") != RECEIPT_SCHEMA:
        raise ValueError("provider receipt schema is unsupported")
    _binding(expected, value.get("binding"), "provider receipt")
    provenance = value.get("provenance")
    if not isinstance(provenance, dict):
        raise ValueError("provider receipt provenance must be an object")
    _strict_keys(provenance, {"provider", "source_kind", "reference", "document_path", "document_sha256"},
                 "provider receipt provenance")
    if (provenance.get("provider") != "anthropic"
            or provenance.get("source_kind") != "caller_supplied_provider_export"
            or not isinstance(provenance.get("reference"), str)
            or not provenance["reference"].strip()):
        raise ValueError("provider receipt provenance is incomplete or unsupported")
    if (not isinstance(provenance.get("document_path"), str)
            or not provenance["document_path"].startswith("provider-receipts/")):
        raise ValueError("provider receipt document must be retained under provider-receipts/")
    document_path = _artifact_path(root, provenance.get("document_path"), "provider receipt document")
    document_hash = hashlib.sha256(document_path.read_bytes()).hexdigest()
    if document_hash != _sha(provenance.get("document_sha256"), "provider receipt document_sha256"):
        raise ValueError("provider receipt document digest does not match its bytes")
    billed = value.get("billed")
    if not isinstance(billed, dict):
        raise ValueError("provider receipt billed amount must be an object")
    _strict_keys(billed, {"currency", "amount_decimal"}, "provider receipt billed amount")
    amount = billed.get("amount_decimal")
    if billed.get("currency") != "USD" or not isinstance(amount, str) or not DECIMAL_USD.fullmatch(amount):
        raise ValueError("provider receipt amount must be a nonnegative decimal USD string")
    try:
        parsed = Decimal(amount)
    except InvalidOperation as error:
        raise ValueError("provider receipt amount is invalid") from error
    if not parsed.is_finite() or parsed < 0:
        raise ValueError("provider receipt amount is invalid")
    return {
        "status": "bound_caller_supplied_origin_unverified",
        "sidecar_path": str(sidecar.relative_to(root)),
        "sidecar_sha256": sidecar_hash,
        "provider": "anthropic",
        "reference": provenance["reference"],
        "document_path": str(document_path.relative_to(root.resolve())),
        "document_sha256": document_hash,
        "provider_origin_verified": False,
        "reported_billed_usd": amount,
        "actual_billed_usd": None,
    }


def _trace(
    root: Path,
    sidecar: Path,
    expected: dict[str, Any],
    turn_usage: Any,
) -> dict[str, Any]:
    sidecar = _artifact_path(root, str(sidecar.relative_to(root)), "request-context trace sidecar")
    value, sidecar_hash = _read_json(sidecar, "request-context trace sidecar")
    _strict_keys(value, {"schema", "binding", "provenance", "turns"}, "request-context trace")
    if value.get("schema") != TRACE_SCHEMA:
        raise ValueError("request-context trace schema is unsupported")
    _binding(expected, value.get("binding"), "request-context trace")
    provenance = value.get("provenance")
    if not isinstance(provenance, dict):
        raise ValueError("request-context trace provenance must be an object")
    _strict_keys(provenance, {"provider", "source_kind", "reference"}, "request-context trace provenance")
    if (provenance.get("provider") != "anthropic"
            or provenance.get("source_kind") != "caller_supplied_request_trace"
            or not isinstance(provenance.get("reference"), str)
            or not provenance["reference"].strip()):
        raise ValueError("request-context trace provenance is incomplete or unsupported")
    if not isinstance(turn_usage, list) or not turn_usage:
        raise ValueError("request-context trace requires retained per-message usage records")
    supplied_turns = value.get("turns")
    if not isinstance(supplied_turns, list) or len(supplied_turns) != len(turn_usage):
        raise ValueError("request-context trace turn inventory differs from the transcript")

    rows: list[dict[str, Any]] = []
    fixed_identities: list[tuple[str, str]] = []
    task_hashes: set[str] = set()
    request_ids: set[str] = set()
    complete = True
    for index, (supplied, observed) in enumerate(zip(supplied_turns, turn_usage, strict=True)):
        label = f"request-context trace turn {index}"
        if not isinstance(supplied, dict):
            raise ValueError(f"{label} must be an object")
        _strict_keys(supplied, {
            "message_id", "request_id", "system_prompt_sha256", "tool_schema_sha256",
            "task_prompt_sha256", "composition", "thinking_output_tokens",
        }, label)
        if (not isinstance(observed, dict)
                or supplied.get("message_id") != observed.get("message_id")
                or not isinstance(supplied.get("request_id"), str)
                or not supplied["request_id"].strip()):
            raise ValueError(f"{label} identity does not match the transcript message")
        if supplied["request_id"] in request_ids:
            raise ValueError(f"{label} repeats a provider request_id")
        request_ids.add(supplied["request_id"])
        system_hash = _sha(supplied.get("system_prompt_sha256"), f"{label} system hash")
        tool_hash = _sha(supplied.get("tool_schema_sha256"), f"{label} tool-schema hash")
        task_hash = _sha(supplied.get("task_prompt_sha256"), f"{label} task-prompt hash")
        fixed_identities.append((system_hash, tool_hash))
        task_hashes.add(task_hash)
        if task_hash != expected["prompt_sha256"]:
            raise ValueError(f"{label} task-prompt digest differs from the retained prompt")
        composition = supplied.get("composition")
        if not isinstance(composition, dict):
            raise ValueError(f"{label} composition must be an object")
        _strict_keys(composition, set(CONTEXT_BUCKETS), f"{label} composition")
        buckets: dict[str, int | None] = {}
        for field in CONTEXT_BUCKETS:
            raw = composition[field]
            if raw is None:
                buckets[field] = None
                complete = False
            else:
                buckets[field] = _integer(raw, f"{label} {field}")
        raw_usage = observed.get("usage") if isinstance(observed, dict) else None
        if not isinstance(raw_usage, dict):
            raise ValueError(f"{label} has no retained raw provider counters")
        retained = {field: raw_usage.get(field) for field in RAW_USAGE_FIELDS}
        input_fields = USAGE_FIELDS[:3]
        if any(retained.get(field) is None for field in input_fields):
            complete = False
            bucket_total = None
        elif all(buckets[field] is not None for field in CONTEXT_BUCKETS):
            bucket_total = sum(buckets.values())
            raw_total = sum(retained[field] for field in input_fields)
            if bucket_total != raw_total:
                raise ValueError(f"{label} composition does not equal retained input plus cache counters")
        else:
            bucket_total = None
        thinking = supplied.get("thinking_output_tokens")
        if thinking is not None:
            thinking = _integer(thinking, f"{label} thinking_output_tokens")
            retained_thinking = retained.get("thinking_tokens")
            if retained_thinking is not None and thinking != retained_thinking:
                raise ValueError(f"{label} thinking count disagrees with retained provider usage")
        rows.append({
            "message_id": supplied["message_id"],
            "request_id": supplied["request_id"],
            "raw_provider_usage": retained,
            "composition": buckets,
            "composition_input_plus_cache_total": bucket_total,
            "fixed_harness_context_tokens": (
                buckets["system_tokens"] + buckets["tool_schema_tokens"]
                if buckets["system_tokens"] is not None and buckets["tool_schema_tokens"] is not None
                else None
            ),
            "thinking_output_tokens_reported_by_trace": thinking,
        })

    schema_stable = len(set(fixed_identities)) == 1 and len(task_hashes) == 1
    if not schema_stable:
        complete = False
    status = "complete" if complete else ("schema_drift" if len(set(fixed_identities)) != 1 else "incomplete")
    complete_totals = {
        field: sum(row["composition"][field] for row in rows)
        if complete else None
        for field in CONTEXT_BUCKETS
    }
    complete_totals["fixed_harness_context_tokens"] = (
        complete_totals["system_tokens"] + complete_totals["tool_schema_tokens"]
        if complete else None
    )
    return {
        "status": status,
        "sidecar_path": str(sidecar.relative_to(root)),
        "sidecar_sha256": sidecar_hash,
        "provider": "anthropic",
        "source_kind": "caller_supplied_request_trace",
        "provider_origin_verified": False,
        "fixed_context_identity_stable": schema_stable,
        "turns": rows,
        "complete_composition_totals": complete_totals,
        "task_prompt_token_claim": "explicit trace bucket; no baseline subtraction or residual inference",
        "thinking_token_claim": "trace-reported subset; kept separate from output tokens and not added to them",
    }


def attach_trial(
    row: dict[str, Any],
    artifacts: Path,
    expected: dict[str, Any],
) -> None:
    """Attach bound sidecars if present; absent sidecars remain explicitly null."""
    label = expected["trial_id"]
    receipt_path = artifacts / "provider-receipts" / f"{label}.json"
    trace_path = artifacts / "request-context-traces" / f"{label}.json"
    receipt = None
    if receipt_path.exists() or receipt_path.is_symlink():
        receipt = _receipt(artifacts, receipt_path, expected)
    trace = None
    if trace_path.exists() or trace_path.is_symlink():
        trace = _trace(artifacts, trace_path, expected, row.get("observed", {}).get("turn_usage_by_message"))
    row["provider_receipt_evidence"] = receipt or {
        "status": "missing",
        "reported_billed_usd": None,
        "actual_billed_usd": None,
        "provider_origin_verified": False,
    }
    row["provider_receipt_reported_billed_usd"] = receipt["reported_billed_usd"] if receipt else None
    row["provider_receipt_actual_usd"] = None
    row["request_context_trace"] = trace or {
        "status": "missing",
        "provider_origin_verified": False,
        "turns": [],
        "complete_composition_totals": {field: None for field in CONTEXT_BUCKETS},
    }


def arm_summary(rows: list[dict[str, Any]], min_trials: int = 5) -> dict[str, Any]:
    """Report caller-supplied receipt subtotals and complete trace subtotals."""
    receipt_values = [row.get("provider_receipt_reported_billed_usd") for row in rows]
    valid_receipts = [Decimal(value) for value in receipt_values if isinstance(value, str)]
    receipt_complete = len(rows) >= min_trials and len(valid_receipts) == len(rows)
    receipt_subtotal = sum(valid_receipts, Decimal(0)) if valid_receipts else None
    accepted = sum(row.get("status") == "accepted" for row in rows)
    trace_rows = [row.get("request_context_trace") or {"status": "missing"} for row in rows]
    complete_traces = [row for row in trace_rows if row.get("status") == "complete"]
    trace_totals = {
        field: sum(row["complete_composition_totals"][field] for row in complete_traces)
        if complete_traces else None
        for field in CONTEXT_TOTALS
    }
    return {
        "provider_receipt_reported_billed_usd_per_trial": receipt_values,
        "provider_receipt_reported_billed_usd_known_subtotal": (
            format(receipt_subtotal, "f") if receipt_subtotal is not None else None
        ),
        "provider_receipt_reported_billed_usd_complete": receipt_complete,
        "provider_receipt_origin_verified_trials": sum(
            row.get("provider_receipt_evidence", {}).get("provider_origin_verified") is True for row in rows
        ),
        "reported_receipt_cost_per_accepted_task_usd": (
            format(receipt_subtotal / accepted, "f")
            if receipt_complete and accepted and receipt_subtotal is not None else None
        ),
        "provider_receipt_actual_usd": None,
        "provider_receipt_claim": (
            "caller-supplied receipt amount bound to exact campaign, trial, prompt, transcript, and receipt bytes; "
            "provider origin is unverified, so this is not a verified account charge"
        ),
        "request_context_trace_status_per_trial": [row.get("status") or "missing" for row in trace_rows],
        "request_context_trace_complete_trials": len(complete_traces),
        "request_context_trace_incomplete_trials": len(rows) - len(complete_traces),
        "request_context_composition_tokens_known_subtotal": trace_totals,
        "request_context_composition_incomplete": len(complete_traces) != len(rows),
        "request_context_trace_claim": (
            "caller-supplied per-turn request composition bound to transcript messages; no inferred buckets, "
            "baseline subtraction, provider-origin verification, or ratio headline"
        ),
    }
