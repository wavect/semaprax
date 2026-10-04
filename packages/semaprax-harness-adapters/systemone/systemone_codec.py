"""Shared SystemOne codec for decision.evaluate/v1 (task model-route/v1).

Standard library only, no I/O. Renders the bounded feature text, builds the
documented `POST /v1/systemone` choice question, and validates the documented
response (docs/HARNESS-LAYA-JEV-V1.md). Every refusal is a CodecError carrying
the harness result status and a stable SPX-HPK code. Nothing here truncates.
"""

import hashlib
import json
import math

TASK = "model-route/v1"

# Closed feature set of model-route/v1 (crates/semaprax-harness/src/decision/route.rs).
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
    abstain = ans.get("abstention") == "abstained" or ans.get("low_confidence") is True
    if min_score is not None and probs[choice] < min_score:
        abstain = True
    info = {"model": doc.get("model") if isinstance(doc.get("model"), str) else None}
    usage = doc.get("usage")
    if isinstance(usage, dict) and isinstance(usage.get("input_tokens"), int):
        info["input_tokens"] = usage["input_tokens"]
    routing = doc.get("routing")
    if isinstance(routing, dict) and isinstance(routing.get("model"), str):
        info["checkpoint"] = routing["model"]
    payload = {
        "choice": None if abstain else choice,
        "scores": {o: float(probs[o]) for o in options},
        "abstain": abstain,
    }
    return payload, info
