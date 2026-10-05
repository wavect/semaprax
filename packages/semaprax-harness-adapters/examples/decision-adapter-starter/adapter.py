#!/usr/bin/env python3
"""Copyable minimal decision.evaluate adapter (v1, v2 and v3). Deliberately not SystemOne.

v3 is the finite runtime choice task choice-select/v1 (MR-11): the same scorer
over the host-rendered option descriptions, answered in the v2 result shape.

A deterministic keyword scorer: each candidate scores 1 + the number of its
label words found in STARTER_KEYWORDS (comma list, default "frontier,tools").
No network, no model, no ambient authority. Copy this directory, replace
`score_options`, keep the validation and the result shape.

  STARTER_SCORELESS=1   choose without scores (score_kind "none")
  STARTER_ABSTAIN=1     abstain natively (choice null, abstention_reason "native")
  STARTER_FAULT=<name>  TEST HOOK for the conformance runner (slow|crash|oversize|leak);
                        delete `_fault` when you copy this file.
"""

import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "sdk", "python"))
from semaprax_harness_adapter import AdapterError, serve_cancellable  # noqa: E402
from decision_fixtures import CHOICE_TASK, CHOICE_VERSION, Invalid, canonical_json, rendered_digest, validate_choice_request  # noqa: E402

PROVIDER_ID = "org.example/keyword-decision"
ADAPTER_VERSION = "0.1.0"
SECRET_ENV = "SEMAPRAX_HARNESS_SECRET_STARTER"
MODEL = "keyword-scorer"
CHECKPOINT = "keyword-v1"
ACCEPTED = [{"kind": "decision.evaluate", "version": v, "operations": ["evaluate"]} for v in (1, 2, 3)]
PROVENANCE = {"provider_id": PROVIDER_ID, "adapter_version": ADAPTER_VERSION, "upstream_version": "builtin-0.1.0"}


def _refuse(code, message, status="refused"):
    return AdapterError(status, code, message)


def _fault(name, cancelled):
    """Test hook only (see module docstring)."""
    if name == "slow":
        cancelled.wait(30)
    elif name == "crash":
        os._exit(3)
    elif name == "leak":
        return os.environ.get(SECRET_ENV, "")
    return None


def score_options(labels, keywords, scoreless):
    """Deterministic: weight = 1 + matching label words; first option wins ties."""
    weights = {o: 1 + sum(1 for w in labels[o].lower().replace(",", " ").split() if w in keywords) for o in labels}
    total = sum(weights.values())
    choice = max(weights, key=lambda o: (weights[o], -list(weights).index(o)))
    return choice, None if scoreless else {o: round(w / total, 6) for o, w in weights.items()}


def _validate_v1(p):
    opts = p.get("options")
    if not isinstance(opts, list) or not opts or len(set(opts)) != len(opts) or any(not isinstance(o, str) or not o for o in opts):
        raise _refuse("SPX-HPK006", "options must be a non-empty list of unique strings")
    if len(opts) > 16:
        raise _refuse("SPX-HPK005", "too many options")
    return {o: o for o in opts}


def _validate_v2(p):
    cands = p.get("candidates")
    if not isinstance(cands, list) or not cands or len(cands) > 16:
        raise _refuse("SPX-HPK005", "candidates must be 1..16")
    ids = [c.get("id") if isinstance(c, dict) else None for c in cands]
    if p.get("options") != ids or ids != [f"m{i}" for i in range(len(ids))]:
        raise _refuse("SPX-HPK010", "options must equal candidate ids m0..m{n-1}")
    r = p.get("rendered")
    if not isinstance(r, dict) or r.get("digest") != rendered_digest(r) or list(r.get("option_labels", {})) != ids:
        raise _refuse("SPX-HPK006", "rendered digest or option labels do not match")
    if "image" in p.get("features", {}).get("input_modalities", []):
        raise _refuse("SPX-HPK004", "image input is not supported", "unsupported")
    return r["option_labels"], r


def handle(req, cancelled):
    p = req.get("payload")
    task = p.get("task") if isinstance(p, dict) else None
    if task not in ("model-route/v1", "model-route/v2", CHOICE_TASK):
        raise _refuse("SPX-HPK004", "only model-route/v1, v2 and choice-select/v1 are supported", "unsupported")
    if (task == CHOICE_TASK) != (req["capability"]["version"] == CHOICE_VERSION):
        raise _refuse("SPX-HPK004", "choice-select/v1 travels only as decision.evaluate v3", "unsupported")
    leaked = _fault(os.environ.get("STARTER_FAULT"), cancelled)
    if os.environ.get("STARTER_FAULT") == "slow":
        raise _refuse("SPX-HPK013", "cancelled")
    keywords = set(os.environ.get("STARTER_KEYWORDS", "frontier,tools").lower().split(","))
    scoreless = os.environ.get("STARTER_SCORELESS") == "1"
    diags = [{"code": "SPX-HPK100", "message": "keyword scorer" + (f" ({leaked})" if leaked else "")}]
    abstain = os.environ.get("STARTER_ABSTAIN") == "1"
    if task == "model-route/v1":
        if scoreless:
            raise _refuse("SPX-HPK004", "v1 requires scores; this scoreless profile needs v2", "unsupported")
        labels = _validate_v1(p)
        choice, scores = score_options(labels, keywords, False)
        return "complete", {"choice": None if abstain else choice, "scores": scores, "abstain": abstain}, diags
    if task == CHOICE_TASK:
        try:
            labels, rendered = validate_choice_request(p)
        except Invalid as err:
            raise _refuse("SPX-HPK006", f"malformed choice request: {err}")
    else:
        labels, rendered = _validate_v2(p)
    choice, scores = score_options(labels, keywords, scoreless)
    wire = len(canonical_json(rendered))  # no upstream request: the rendered block is the "wire"
    if wire > p["max_wire_bytes"]:
        raise _refuse("SPX-HPK005", "rendered request exceeds max_wire_bytes")
    out = {
        "choice": None if abstain else choice, "abstain": abstain, "abstention_reason": "native" if abstain else "none", "scores": scores,
        "score_kind": "none" if scoreless else "candidate_relative",
        "native_confidence": None, "native_confidence_kind": None, "calibration_id": None,
        "call": {
            "adapter": f"{PROVIDER_ID}@{ADAPTER_VERSION}", "requested_model": MODEL, "answering_model": MODEL,
            "checkpoint": CHECKPOINT, "identity_kind": "local_declared", "rendered_digest": rendered["digest"],
            "wire_bytes": wire,
            "usage": {"input_tokens": None, "output_tokens": None, "basis": "unknown"}, "billing": "local",
        },
    }
    if os.environ.get("STARTER_FAULT") == "oversize":
        out["pad"] = "x" * 200000
    return "complete", out, diags


if __name__ == "__main__":
    serve_cancellable(ACCEPTED, {("decision.evaluate", "evaluate"): handle}, PROVENANCE, secret_env=(SECRET_ENV,))
