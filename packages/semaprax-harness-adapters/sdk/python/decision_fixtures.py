"""Typed decision.evaluate fixtures and result validators (v1 and v2).

Standard library only and independent of any adapter: these are the host-side
checks a decision adapter's output must survive. `v2_request` renders its own
`rendered` block, so the digest canonicalization below is the wire definition:
sha256 over compact, key-sorted, non-ASCII-escaped UTF-8 JSON of
{renderer, instructions, state, option_labels}, prefixed `sha256:`.
"""

import hashlib
import json
import math

PROJECT = {"id": "p" * 64, "worktree": "w" * 64, "revision": "r" * 64}
RENDERER = "semaprax.route-render.v2"
INSTRUCTIONS = "Which candidate model should handle this task step?"

V1_FEATURES = {
    "task_family": "localized_debug", "estimated_context_tokens": 12000,
    "requires_structured_output": False, "requires_tools": True,
    "confidentiality": "project", "latency_class": "interactive",
}
V2_FEATURES = {
    "execution_domain": "development", "task_profile": "mechanical", "phase": "implement", "attempt_index": 0,
    "previous_failure": "none", "verified_progress": 0, "no_progress": 0, "estimated_context_tokens": 1200,
    "requires_structured_output": True, "requires_tools": False, "input_modalities": ["text"],
    "confidentiality": "project", "latency_class": "interactive", "remaining_budget_micros": None,
}
TIERS = ("economy", "standard", "frontier")


def canonical_json(obj):
    return json.dumps(obj, separators=(",", ":"), sort_keys=True, ensure_ascii=False).encode("utf-8")


def rendered_digest(rendered):
    core = {k: rendered[k] for k in ("renderer", "instructions", "state", "option_labels")}
    return "sha256:" + hashlib.sha256(canonical_json(core)).hexdigest()


def _envelope(payload, version, inv, deadline_ms, max_bytes):
    return {
        "schema": "semaprax.harness-request.v1", "invocation_id": inv, "project": PROJECT, "lock_digest": "l" * 64,
        "capability": {"kind": "decision.evaluate", "version": version}, "operation": "evaluate",
        "deadline_ms": deadline_ms, "budget": {"max_result_bytes": max_bytes, "remaining_calls": 1}, "lineage": [],
        "payload": payload,
    }


def v1_request(features=None, options=None, inv="inv-000001", deadline_ms=5000, max_bytes=65536):
    options = ["m-cheap", "m-mid", "m-strong"] if options is None else options
    payload = {"task": "model-route/v1", "features": dict(V1_FEATURES if features is None else features), "options": list(options)}
    return _envelope(payload, 1, inv, deadline_ms, max_bytes)


def v2_payload(features=None, n=3, disclosure="metadata_only", max_wire_bytes=8192):
    cands, labels = [], {}
    for i in range(n):
        cid = f"m{i}"
        label = f"{TIERS[i % 3]} tier, candidate {i}"
        cands.append({
            "id": cid, "label": label, "capabilities": ["structured_output"], "max_context": 100000 * (i + 1),
            "quality_tier": TIERS[i % 3], "est_cost_micros": {"value": 100 * (i + 1), "basis": "configured"},
            "est_latency_ms": {"value": None, "basis": "unknown"},
        })
        labels[cid] = f"{cid}: {label}"
    f = dict(V2_FEATURES if features is None else features)
    state = "\n".join(f"{k}: {json.dumps(f[k])}" for k in sorted(f)) + "\n" + "\n".join(labels.values())
    rendered = {"renderer": RENDERER, "instructions": INSTRUCTIONS, "state": state, "option_labels": labels}
    rendered["digest"] = rendered_digest(rendered)
    payload = {"task": "model-route/v2", "features": f, "candidates": cands, "options": [c["id"] for c in cands],
               "disclosure": disclosure, "rendered": rendered, "max_wire_bytes": max_wire_bytes}
    if disclosure == "excerpt":
        payload["excerpt"] = "short excerpt"
    return payload


def v2_request(inv="inv-000001", deadline_ms=5000, max_bytes=65536, **kw):
    return _envelope(v2_payload(**kw), 2, inv, deadline_ms, max_bytes)


def retag_digest(payload):
    """Recompute rendered.digest after a test mutated rendered content (keeps it self-consistent)."""
    payload["rendered"]["digest"] = rendered_digest(payload["rendered"])
    return payload


def _prob(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x) and 0.0 <= x <= 1.0


class Invalid(AssertionError):
    pass


def _need(cond, msg):
    if not cond:
        raise Invalid(msg)


def validate_v1_result(p, options):
    """Strict v1 shape: {choice, scores, abstain}; distribution required."""
    _need(isinstance(p, dict) and set(p) == {"choice", "scores", "abstain"}, "v1 result members")
    _need(isinstance(p["abstain"], bool), "abstain is boolean")
    _need(set(p["scores"] or {}) == set(options) and all(_prob(v) for v in p["scores"].values()), "scores cover options, finite [0,1]")
    _need(abs(sum(p["scores"].values()) - 1.0) <= 0.05, "scores sum to ~1")
    _check_choice(p, p["scores"], options)


def _check_choice(p, scores, options):
    if p["abstain"]:
        _need(p["choice"] is None, "abstention carries no choice")
    else:
        _need(p["choice"] in options, "choice is an option")
        if scores is not None:
            _need(scores[p["choice"]] >= max(scores.values()) - 1e-3, "choice is argmax")


def validate_v2_result(p, request_payload, scoreless=False):
    """Host-side acceptance of a model-route/v2 result against its request."""
    options = request_payload["options"]
    keys = {"choice", "abstain", "abstention_reason", "scores", "score_kind", "native_confidence",
            "native_confidence_kind", "calibration_id", "call"}
    _need(isinstance(p, dict) and set(p) == keys, "v2 result members")
    _need(p["abstention_reason"] in ("none", "native", "host_threshold", "unsupported_input"), "abstention_reason")
    _need((p["abstention_reason"] == "none") == (not p["abstain"]), "reason agrees with abstain")
    _need(p["score_kind"] in ("option_distribution", "candidate_relative", "none"), "score_kind")
    if p["score_kind"] == "none":
        _need(p["scores"] is None, "scoreless result carries no scores")
        _need(p["choice"] is None or not p["abstain"], "scoreless choice")
    else:
        _need(not scoreless, "a scoreless adapter must not fabricate scores")
        _need(isinstance(p["scores"], dict) and set(p["scores"]) == set(options) and all(_prob(v) for v in p["scores"].values()),
              "scores cover options, finite [0,1]")
        if p["score_kind"] == "option_distribution":
            _need(abs(sum(p["scores"].values()) - 1.0) <= 0.05, "distribution sums to ~1")
    _check_choice(p, p["scores"], options)
    _need((p["native_confidence"] is None) == (p["native_confidence_kind"] is None), "native confidence is labelled")
    _need(p["native_confidence"] is None or _prob(p["native_confidence"]), "native confidence finite [0,1]")
    c = p["call"]
    _need(isinstance(c, dict) and set(c) == {"adapter", "requested_model", "answering_model", "checkpoint", "identity_kind",
                                             "rendered_digest", "wire_bytes", "usage", "billing"}, "call members")
    _need(c["rendered_digest"] == request_payload["rendered"]["digest"], "call binds the request digest")
    _need(isinstance(c["wire_bytes"], int) and 0 < c["wire_bytes"] <= request_payload["max_wire_bytes"], "wire_bytes within bound")
    _need(c["identity_kind"] in ("immutable_checkpoint", "mutable_service", "local_declared", "unknown"), "identity_kind")
    _need(c["usage"]["basis"] in ("provider_reported", "local_measured", "unknown"), "usage basis")
    _need(c["billing"] in ("api", "local", "unknown"), "billing")
