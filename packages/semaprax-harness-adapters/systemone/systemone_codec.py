"""Shared SystemOne codec for decision.evaluate v1 (model-route/v1), v2 (model-route/v2) and v3 (choice-select/v1).

Standard library only, no I/O. Renders the bounded feature text, builds the
documented `POST /v1/systemone` choice question, and validates the documented
response (docs/HARNESS-LAYA-JEV-V1.md). Every refusal is a CodecError carrying
the harness result status and a stable SPX-HPK code. Nothing here truncates.
"""

import hashlib
import json
import math

TASK = "model-route/v1"

# Closed feature set of model-route/v1 (src/model_routing/engine/route.rs).
ENUMS = {
    "task_family": ("mechanical", "tests_docs", "localized_debug", "semantic_law"),
    "confidentiality": ("public", "project", "secret"),
    "latency_class": ("interactive", "batch"),
}
INTS = {"estimated_context_tokens": 1_000_000_000}
BOOLS = ("requires_structured_output", "requires_tools")
MANDATORY = (
    "task_family", "estimated_context_tokens", "requires_structured_output",
    "requires_tools", "confidentiality", "latency_class",
)
# Optional declared-input members. Anything else is an unsupported profile.
SUPPORTED_LANGUAGE = "en"
SUPPORTED_PROFILE = TASK
OPTIONAL = {"language": SUPPORTED_LANGUAGE, "profile": SUPPORTED_PROFILE}

INSTRUCTIONS = "Which logical model should handle this coding task?"

# Bounds. A larger input is refused, never cut down.
MAX_OPTIONS = 16
MAX_OPTION_ID_BYTES = 128
MAX_OPTION_BYTES_TOTAL = 640
MAX_STATE_BYTES = 512
PROB_SUM_TOLERANCE = 0.05
CHOICE_TIE_TOLERANCE = 1e-3


class CodecError(Exception):
    """A refusal. `status` is a harness result status, `code` an SPX-HPK code."""

    def __init__(self, status, code, message):
        super().__init__(message)
        self.status, self.code, self.message = status, code, message


def _refuse(code, message):
    return CodecError("refused", code, message)


def validate_request_payload(payload):
    """Return (features, options) after checking the model-route/v1 request shape."""
    if not isinstance(payload, dict) or payload.get("task") != TASK:
        raise CodecError("unsupported", "SPX-HPK004", "only task model-route/v1 is supported")
    extra = set(payload) - {"task", "features", "options"}
    if extra or not isinstance(payload.get("features"), dict):
        raise _refuse("SPX-HPK006", "malformed decision request payload")
    options = payload.get("options")
    if not isinstance(options, list) or not options:
        raise _refuse("SPX-HPK006", "`options` must be a non-empty list")
    seen = set()
    for o in options:
        if not isinstance(o, str) or not o or not o.isascii() or o != o.strip():
            raise _refuse("SPX-HPK006", "option ids must be non-empty ASCII strings")
        if len(o.encode()) > MAX_OPTION_ID_BYTES or o in seen:
            raise _refuse("SPX-HPK005" if o not in seen else "SPX-HPK006", "option id too long or duplicate")
        seen.add(o)
    if len(options) > MAX_OPTIONS or sum(len(o) for o in options) > MAX_OPTION_BYTES_TOTAL:
        raise _refuse("SPX-HPK005", f"option set exceeds the bound ({MAX_OPTIONS} options, {MAX_OPTION_BYTES_TOTAL} id bytes)")
    return payload["features"], list(options)


def render_features(features):
    """Deterministic bounded text rendering of the closed feature set."""
    for key, want in OPTIONAL.items():
        if key in features and features[key] != want:
            raise CodecError("unsupported", "SPX-HPK004", f"unsupported input {key} (only `{want}`)")
    unknown = set(features) - set(MANDATORY) - set(OPTIONAL)
    if unknown:
        raise CodecError("unsupported", "SPX-HPK004", "unsupported feature profile: unknown member")
    lines = []
    for key in MANDATORY:
        if key not in features:
            raise _refuse("SPX-HPK006", f"mandatory feature `{key}` is missing")
        v = features[key]
        if key in ENUMS:
            if not isinstance(v, str) or v not in ENUMS[key]:
                if isinstance(v, str) and len(v) > 64:
                    raise _refuse("SPX-HPK005", f"feature `{key}` is overlong")
                raise CodecError("unsupported", "SPX-HPK004", f"feature `{key}` outside the supported profile")
            lines.append(f"{key}: {v}")
        elif key in INTS:
            if isinstance(v, bool) or not isinstance(v, int) or not 0 <= v <= INTS[key]:
                raise _refuse("SPX-HPK006", f"feature `{key}` must be an integer in range")
            lines.append(f"{key}: {v}")
        else:
            if not isinstance(v, bool):
                raise _refuse("SPX-HPK006", f"feature `{key}` must be a boolean")
            lines.append(f"{key}: {'true' if v else 'false'}")
    text = "\n".join(lines)
    if len(text.encode()) > MAX_STATE_BYTES:
        raise _refuse("SPX-HPK005", "rendered features exceed the state bound")
    return text


def question_id(invocation_id, options):
    """Request binding: the question name the response must echo."""
    h = hashlib.sha256(("\n".join([invocation_id, TASK] + options)).encode()).hexdigest()
    return "route-" + h[:16]


def build_body(invocation_id, features, options, model, extra=None):
    """Documented SystemOne request: one `choice` question, options as criteria."""
    qid = question_id(invocation_id, options)
    body = {
        "state": render_features(features),
        "model": model,
        "questions": {qid: {"type": "choice", "instructions": INSTRUCTIONS, "criteria": {o: o for o in options}}},
    }
    body.update(extra or {})
    return qid, body


def _no_constant(name):
    raise ValueError(f"non-finite constant {name}")


def parse_json(raw, max_bytes):
    if len(raw) > max_bytes:
        raise _refuse("SPX-HPK007", f"response exceeds {max_bytes} bytes")
    try:
        return json.loads(raw.decode("utf-8"), parse_constant=_no_constant)
    except ValueError as err:
        text = str(err)
        if "non-finite" in text:
            raise _refuse("SPX-HPK009", "response carries a non-finite number")
        raise _refuse("SPX-HPK008", "response is not valid JSON")


def _prob(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x) and 0.0 <= x <= 1.0


def parse_response(doc, qid, options, min_score=None):
    """Validate a SystemOne response -> (payload, info).

    payload is {choice, scores, abstain}; info is non-authoritative metadata
    (answering model, token usage, routing checkpoint when the server gives it).
    """
    if not isinstance(doc, dict) or not isinstance(doc.get("answers"), dict):
        raise _refuse("SPX-HPK008", "response lacks `answers`")
    if qid not in doc["answers"]:
        raise _refuse("SPX-HPK008", "response does not echo the request question binding")
    if set(doc["answers"]) != {qid}:
        raise _refuse("SPX-HPK008", "response answers questions that were not asked")
    ans = doc["answers"][qid]
    if not isinstance(ans, dict) or ans.get("type") != "choice":
        raise _refuse("SPX-HPK008", "answer is not a choice answer")
    probs = ans.get("probabilities")
    if not isinstance(probs, dict):
        raise _refuse("SPX-HPK008", "answer lacks probabilities")
    for k, v in probs.items():
        if not _prob(v):
            raise _refuse("SPX-HPK009", f"probability for `{k}` is not finite in [0,1]")
    if set(probs) != set(options):
        raise _refuse("SPX-HPK010", "probabilities do not cover exactly the request options")
    if abs(sum(probs.values()) - 1.0) > PROB_SUM_TOLERANCE:
        raise _refuse("SPX-HPK009", "probabilities do not sum to approximately 1")
    choice = ans.get("choice")
    if not isinstance(choice, str) or choice not in options:
        raise _refuse("SPX-HPK010", "choice is not one of the request options")
    if probs[choice] < max(probs.values()) - CHOICE_TIE_TOLERANCE:
        raise _refuse("SPX-HPK010", "choice is not the highest-probability option")
    native_abstain = ans.get("abstention") == "abstained" or ans.get("low_confidence") is True
    host_abstain = min_score is not None and probs[choice] < min_score
    abstain = native_abstain or host_abstain
    info = {"model": doc.get("model") if isinstance(doc.get("model"), str) else None,
            "abstention_reason": "native" if native_abstain else "host_threshold" if host_abstain else "none"}
    conf = ans.get("confidence")
    if conf is not None:
        info["native_confidence"] = conf if _prob(conf) else "invalid"
    usage = doc.get("usage")
    if isinstance(usage, dict) and isinstance(usage.get("input_tokens"), int):
        info["input_tokens"] = usage["input_tokens"]
    if isinstance(usage, dict) and isinstance(usage.get("output_tokens"), int) and not isinstance(usage.get("output_tokens"), bool):
        info["output_tokens"] = usage["output_tokens"]
    routing = doc.get("routing")
    if isinstance(routing, dict) and isinstance(routing.get("model"), str):
        info["checkpoint"] = routing["model"]
    payload = {
        "choice": None if abstain else choice,
        "scores": {o: float(probs[o]) for o in options},
        "abstain": abstain,
    }
    return payload, info


# ---------------------------------------------------------------------------
# model-route/v2 (docs: MR-CONTRACT). The host renders; the adapter forwards.
# ---------------------------------------------------------------------------

TASK_V2 = "model-route/v2"
V2_ENUMS = {
    "execution_domain": ("development", "application"),
    "phase": ("plan", "implement", "review", "repair", "turn", "handoff", "unknown"),
    "previous_failure": ("none", "parse_schema", "semantic_law", "tool_transport", "acceptance", "budget", "unknown"),
    "confidentiality": ENUMS["confidentiality"],
    "latency_class": ENUMS["latency_class"],
}
V2_INT_RANGES = {"attempt_index": 64, "verified_progress": 1000, "no_progress": 1000, "estimated_context_tokens": 1_000_000_000}
V2_FEATURE_KEYS = (
    "execution_domain", "task_profile", "phase", "attempt_index", "previous_failure", "verified_progress", "no_progress",
    "estimated_context_tokens", "requires_structured_output", "requires_tools", "input_modalities", "confidentiality",
    "latency_class", "remaining_budget_micros",
)
V2_CANDIDATE_KEYS = {"id", "label", "capabilities", "max_context", "quality_tier", "est_cost_micros", "est_latency_ms"}
V2_TIERS = ("economy", "standard", "frontier", "unknown")
V2_BASES = ("measured", "configured", "unknown")
V2_RENDERED_KEYS = {"renderer", "instructions", "state", "option_labels", "digest"}
V2_DISCLOSURES = ("metadata_only", "excerpt")
V2_MAX_STATE_BYTES = 4096
V2_MAX_INSTRUCTIONS_BYTES = 1024
V2_MAX_LABEL_BYTES = 128
V2_MAX_EXCERPT_BYTES = 1024
MAX_U64 = 2**64 - 1


def canonical_json(obj):
    """The digest canonicalization: compact, key-sorted, UTF-8 (not ASCII-escaped)."""
    return json.dumps(obj, separators=(",", ":"), sort_keys=True, ensure_ascii=False).encode("utf-8")


def rendered_digest(rendered):
    """sha256 of canonical JSON of {renderer, instructions, state, option_labels}."""
    core = {k: rendered[k] for k in ("renderer", "instructions", "state", "option_labels")}
    return "sha256:" + hashlib.sha256(canonical_json(core)).hexdigest()


def _is_int(v, lo, hi):
    return isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi


def _metric(m, name):
    if not isinstance(m, dict) or set(m) != {"value", "basis"} or m["basis"] not in V2_BASES:
        raise _refuse("SPX-HPK006", f"candidate `{name}` is malformed")
    if (m["value"] is None) != (m["basis"] == "unknown") or (m["value"] is not None and not _is_int(m["value"], 0, MAX_U64)):
        raise _refuse("SPX-HPK006", f"candidate `{name}` value/basis disagree")


def _validate_features_v2(f):
    unknown = set(f) - set(V2_FEATURE_KEYS)
    if unknown:
        raise CodecError("unsupported", "SPX-HPK004", "unsupported feature profile: unknown member")
    for key in V2_FEATURE_KEYS:
        if key not in f:
            raise _refuse("SPX-HPK006", f"mandatory feature `{key}` is missing")
        v = f[key]
        if key in V2_ENUMS:
            if not isinstance(v, str) or v not in V2_ENUMS[key]:
                raise CodecError("unsupported", "SPX-HPK004", f"feature `{key}` outside the supported profile")
        elif key in V2_INT_RANGES:
            if not _is_int(v, 0, V2_INT_RANGES[key]):
                raise _refuse("SPX-HPK006", f"feature `{key}` must be an integer in range")
        elif key == "task_profile":
            if not isinstance(v, str) or not v.isascii() or not 1 <= len(v) <= 64:
                raise _refuse("SPX-HPK006", "feature `task_profile` must be ASCII of 1..64 bytes")
        elif key in BOOLS:
            if not isinstance(v, bool):
                raise _refuse("SPX-HPK006", f"feature `{key}` must be a boolean")
        elif key == "input_modalities":
            if not isinstance(v, list) or not v or v != sorted(set(v)) or any(m not in ("text", "image") for m in v):
                raise _refuse("SPX-HPK006", "`input_modalities` must be a sorted unique non-empty subset of text,image")
        elif key == "remaining_budget_micros":
            if v is not None and not _is_int(v, 0, MAX_U64):
                raise _refuse("SPX-HPK006", "`remaining_budget_micros` must be an integer or null")


def validate_request_v2(payload):
    """Return a dict {features, options, rendered, max_wire_bytes} after checking model-route/v2."""
    if not isinstance(payload, dict) or payload.get("task") != TASK_V2:
        raise CodecError("unsupported", "SPX-HPK004", "only task model-route/v2 is supported here")
    if set(payload) - {"task", "features", "candidates", "options", "disclosure", "rendered", "max_wire_bytes", "excerpt"}:
        raise _refuse("SPX-HPK006", "malformed decision request payload")
    for key in ("features", "candidates", "options", "disclosure", "rendered", "max_wire_bytes"):
        if key not in payload:
            raise _refuse("SPX-HPK006", f"`{key}` is missing")
    if not isinstance(payload["features"], dict):
        raise _refuse("SPX-HPK006", "`features` must be an object")
    _validate_features_v2(payload["features"])
    cands = payload["candidates"]
    if not isinstance(cands, list) or not cands:
        raise _refuse("SPX-HPK006", "`candidates` must be a non-empty list")
    if len(cands) > MAX_OPTIONS:
        raise _refuse("SPX-HPK005", f"candidate set exceeds the bound ({MAX_OPTIONS})")
    for i, c in enumerate(cands):
        if not isinstance(c, dict) or set(c) != V2_CANDIDATE_KEYS or c["id"] != f"m{i}":
            raise _refuse("SPX-HPK010", "candidates must be exactly m0..m{n-1} in order")
        if not isinstance(c["label"], str) or not c["label"].isascii() or not 1 <= len(c["label"]) <= 64:
            raise _refuse("SPX-HPK006", "candidate label must be ASCII of 1..64 bytes")
        if not isinstance(c["capabilities"], list) or any(not isinstance(x, str) for x in c["capabilities"]):
            raise _refuse("SPX-HPK006", "candidate capabilities must be a string list")
        if not _is_int(c["max_context"], 0, MAX_U64) or c["quality_tier"] not in V2_TIERS:
            raise _refuse("SPX-HPK006", "candidate max_context/quality_tier malformed")
        _metric(c["est_cost_micros"], "est_cost_micros")
        _metric(c["est_latency_ms"], "est_latency_ms")
    options = payload["options"]
    if options != [c["id"] for c in cands]:
        raise _refuse("SPX-HPK010", "`options` must equal the candidate ids in the same order")
    if payload["disclosure"] not in V2_DISCLOSURES:
        raise _refuse("SPX-HPK006", "`disclosure` outside the closed set")
    if (payload["disclosure"] == "excerpt") != ("excerpt" in payload):
        raise _refuse("SPX-HPK006", "`excerpt` is present exactly when disclosure is excerpt")
    if "excerpt" in payload and (not isinstance(payload["excerpt"], str) or len(payload["excerpt"].encode()) > V2_MAX_EXCERPT_BYTES):
        raise _refuse("SPX-HPK005", "excerpt exceeds its bound")
    if not _is_int(payload["max_wire_bytes"], 1, MAX_U64):
        raise _refuse("SPX-HPK006", "`max_wire_bytes` must be a positive integer")
    r = payload["rendered"]
    if not isinstance(r, dict) or set(r) != V2_RENDERED_KEYS:
        raise _refuse("SPX-HPK006", "`rendered` is malformed")
    for key in ("renderer", "instructions", "state", "digest"):
        if not isinstance(r[key], str) or (key != "state" and not r[key]):
            raise _refuse("SPX-HPK006", f"rendered `{key}` must be a string")
    if len(r["state"].encode()) > V2_MAX_STATE_BYTES or len(r["instructions"].encode()) > V2_MAX_INSTRUCTIONS_BYTES:
        raise _refuse("SPX-HPK005", "rendered state or instructions exceed their bound")
    labels = r["option_labels"]
    if not isinstance(labels, dict) or set(labels) != set(options) or len(labels) != len(options) or any(
            not isinstance(v, str) or not v or len(v.encode()) > V2_MAX_LABEL_BYTES for v in labels.values()):
        raise _refuse("SPX-HPK010", "`option_labels` must cover exactly the options")
    if r["digest"] != rendered_digest(r):
        raise _refuse("SPX-HPK006", "rendered digest does not match the rendered content")
    return {"features": payload["features"], "options": list(options), "rendered": r, "max_wire_bytes": payload["max_wire_bytes"]}


def question_id_v2(invocation_id, options, digest):
    h = hashlib.sha256(("\n".join([invocation_id, TASK_V2] + options + [digest])).encode()).hexdigest()
    return "route-" + h[:16]


def build_body_v2(invocation_id, req):
    """SystemOne `choice` question using the host-rendered content verbatim."""
    r = req["rendered"]
    qid = question_id_v2(invocation_id, req["options"], r["digest"])
    body = {
        "state": r["state"],
        "questions": {qid: {"type": "choice", "instructions": r["instructions"], "criteria": {o: r["option_labels"][o] for o in req["options"]}}},
    }
    return qid, body


def build_result_v2(payload, info, cfg, adapter_ref, digest, wire_bytes):
    """v1-shaped parse result + backend identity -> the model-route/v2 result payload."""
    conf = info.get("native_confidence")
    if conf == "invalid":
        raise _refuse("SPX-HPK009", "native confidence is not finite in [0,1]")
    profile, backend = cfg.profile, cfg.backend
    answering, checkpoint, identity = backend.call_identity(info, cfg)
    scoreless = profile["scoreless"]
    has_usage = "input_tokens" in info or "output_tokens" in info
    return {
        "choice": payload["choice"],
        "abstain": payload["abstain"],
        "abstention_reason": info["abstention_reason"],
        "scores": None if scoreless else payload["scores"],
        "score_kind": profile["score_kind"],
        "native_confidence": None if conf is None else float(conf),
        "native_confidence_kind": None if conf is None else backend.confidence_kind,
        "calibration_id": None,
        "call": {
            "adapter": adapter_ref,
            "requested_model": profile["model"],
            "answering_model": answering,
            "checkpoint": checkpoint,
            "identity_kind": identity,
            "rendered_digest": digest,
            "wire_bytes": wire_bytes,
            "usage": {"input_tokens": info.get("input_tokens"), "output_tokens": info.get("output_tokens"),
                      "basis": "provider_reported" if has_usage else "unknown"},
            "billing": backend.billing,
        },
    }


# ---------------------------------------------------------------------------
# choice-select/v1 (MR-11): finite runtime choice over host-admitted tools or
# agents, carried only by decision.evaluate v3. The host screens and renders;
# selection ids are c0..c{n-1}; the adapter forwards the rendered content
# verbatim and answers with the model-route/v2 result shape.
# ---------------------------------------------------------------------------

TASK_CHOICE = "choice-select/v1"
CHOICE_VERSION = 3
CHOICE_RENDERER = "semaprax.choice-render.v1"
CHOICE_KINDS = ("tool", "agent")
CHOICE_QUESTION_KEYS = {"schema", "destination_kind", "input_type", "output_type", "confidentiality"}
CHOICE_MAX_DESCRIPTION = 96


def _stable_id(s):
    if not isinstance(s, str) or not 1 <= len(s) <= 64 or not (s[0].islower() or s[0].isdigit()) or not s.isascii():
        return False
    return all(seg not in ("", ".", "..") and all(c.islower() or c.isdigit() or c in "._-" for c in seg) for seg in s.split("/"))


def validate_request_choice(payload):
    """Return {task, features, options, rendered, max_wire_bytes} after checking choice-select/v1."""
    if not isinstance(payload, dict) or payload.get("task") != TASK_CHOICE:
        raise CodecError("unsupported", "SPX-HPK004", "only task choice-select/v1 is supported here")
    allowed = {"task", "question", "candidates", "options", "disclosure", "rendered", "max_wire_bytes", "excerpt"}
    if set(payload) - allowed:
        raise _refuse("SPX-HPK006", "malformed choice request payload")
    for key in allowed - {"excerpt"}:
        if key not in payload:
            raise _refuse("SPX-HPK006", f"`{key}` is missing")
    q = payload["question"]
    if not isinstance(q, dict) or set(q) != CHOICE_QUESTION_KEYS:
        raise _refuse("SPX-HPK006", "`question` is malformed")
    if not all(_stable_id(q[k]) for k in ("schema", "input_type", "output_type")):
        raise _refuse("SPX-HPK006", "question schema and types must be stable ids")
    if q["destination_kind"] not in CHOICE_KINDS or q["confidentiality"] not in ENUMS["confidentiality"]:
        raise CodecError("unsupported", "SPX-HPK004", "question kind or confidentiality outside the supported set")
    cands = payload["candidates"]
    if not isinstance(cands, list) or not 2 <= len(cands) <= MAX_OPTIONS:
        raise _refuse("SPX-HPK005", f"choice takes 2..{MAX_OPTIONS} candidates")
    for i, c in enumerate(cands):
        if not isinstance(c, dict) or set(c) != {"id", "label"} or c["id"] != f"c{i}":
            raise _refuse("SPX-HPK010", "candidates must be exactly c0..c{n-1} in order")
        lab = c["label"]
        if not isinstance(lab, str) or not lab.isascii() or not 1 <= len(lab) <= CHOICE_MAX_DESCRIPTION or "://" in lab:
            raise _refuse("SPX-HPK006", "candidate label must be a bounded ASCII description")
    options = payload["options"]
    if options != [c["id"] for c in cands]:
        raise _refuse("SPX-HPK010", "`options` must equal the candidate ids in the same order")
    if payload["disclosure"] not in V2_DISCLOSURES:
        raise _refuse("SPX-HPK006", "`disclosure` outside the closed set")
    if (payload["disclosure"] == "excerpt") != ("excerpt" in payload):
        raise _refuse("SPX-HPK006", "`excerpt` is present exactly when disclosure is excerpt")
    if "excerpt" in payload and (not isinstance(payload["excerpt"], str) or len(payload["excerpt"].encode()) > V2_MAX_EXCERPT_BYTES):
        raise _refuse("SPX-HPK005", "excerpt exceeds its bound")
    if not _is_int(payload["max_wire_bytes"], 1, MAX_U64):
        raise _refuse("SPX-HPK006", "`max_wire_bytes` must be a positive integer")
    r = payload["rendered"]
    if not isinstance(r, dict) or set(r) != V2_RENDERED_KEYS or r.get("renderer") != CHOICE_RENDERER:
        raise _refuse("SPX-HPK006", "`rendered` is malformed or not the choice renderer")
    for key in ("instructions", "state", "digest"):
        if not isinstance(r[key], str) or not r[key]:
            raise _refuse("SPX-HPK006", f"rendered `{key}` must be a string")
    if len(r["state"].encode()) > V2_MAX_STATE_BYTES or len(r["instructions"].encode()) > V2_MAX_INSTRUCTIONS_BYTES:
        raise _refuse("SPX-HPK005", "rendered state or instructions exceed their bound")
    labels = r["option_labels"]
    if not isinstance(labels, dict) or set(labels) != set(options) or any(
            not isinstance(v, str) or not v or len(v.encode()) > V2_MAX_LABEL_BYTES for v in labels.values()):
        raise _refuse("SPX-HPK010", "`option_labels` must cover exactly the options")
    if r["digest"] != rendered_digest(r):
        raise _refuse("SPX-HPK006", "rendered digest does not match the rendered content")
    return {"task": TASK_CHOICE, "features": {"input_modalities": ["text"]}, "options": list(options),
            "rendered": r, "max_wire_bytes": payload["max_wire_bytes"]}


def build_body_choice(invocation_id, req):
    """SystemOne `choice` question over the host-rendered choice content, verbatim."""
    r = req["rendered"]
    h = hashlib.sha256(("\n".join([invocation_id, TASK_CHOICE] + req["options"] + [r["digest"]])).encode()).hexdigest()
    qid = "choice-" + h[:16]
    body = {
        "state": r["state"],
        "questions": {qid: {"type": "choice", "instructions": r["instructions"], "criteria": {o: r["option_labels"][o] for o in req["options"]}}},
    }
    return qid, body
