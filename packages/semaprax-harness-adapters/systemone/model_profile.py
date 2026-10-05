"""Model profile: the data that selects a model for an existing backend (MR-15).

A profile is JSON in SEMAPRAX_HARNESS_MODEL_PROFILE, validated against a closed
schema. It never carries credentials or endpoints (unknown members are
refused), so two profiles of one adapter can differ only in model facts.
Standard library only.
"""

import json

from systemone_codec import CodecError

ENV = "SEMAPRAX_HARNESS_MODEL_PROFILE"
MAX_PROFILE_BYTES = 4096
DEFAULT_RENDERER = "semaprax.route-render.v2"
IDENTITY_KINDS = ("immutable_checkpoint", "mutable_service", "local_declared", "unknown")
SCORE_KINDS = ("option_distribution", "candidate_relative", "none")
MODALITIES = ("text", "image")

# name -> (required, description). The only members a profile may carry.
SCHEMA = {
    "profile_id": "ASCII id, 1..64 bytes",
    "model": "upstream model name, 1..128 bytes",
    "checkpoint": "attestable immutable checkpoint, or null",
    "identity_kind": "|".join(IDENTITY_KINDS),
    "score_kind": "|".join(SCORE_KINDS),
    "scoreless": "boolean; true iff score_kind is none",
    "max_options": "integer 1..16",
    "max_state_bytes": "integer 1..4096",
    "modalities": "non-empty unique subset of text,image",
    "renderer": "optional renderer id (default " + DEFAULT_RENDERER + ")",
}
REQUIRED = tuple(k for k in SCHEMA if k != "renderer")


def _bad(message):
    return CodecError("refused", "SPX-HPK006", "invalid model profile: " + message)


def _ascii(v, key, lo, hi):
    if not isinstance(v, str) or not v.isascii() or not lo <= len(v) <= hi or v != v.strip() or not v.isprintable():
        raise _bad(f"`{key}` must be printable ASCII of {lo}..{hi} bytes")
    return v


def _int(v, key, lo, hi):
    if isinstance(v, bool) or not isinstance(v, int) or not lo <= v <= hi:
        raise _bad(f"`{key}` must be an integer in {lo}..{hi}")
    return v


def validate(doc):
    """Return a normalized profile dict or raise CodecError(refused, SPX-HPK006)."""
    if not isinstance(doc, dict):
        raise _bad("not a JSON object")
    unknown = set(doc) - set(SCHEMA)
    if unknown:
        raise _bad("unknown member (credentials and endpoints are not profile data)")
    for k in REQUIRED:
        if k not in doc:
            raise _bad(f"`{k}` is missing")
    p = {
        "profile_id": _ascii(doc["profile_id"], "profile_id", 1, 64),
        "model": _ascii(doc["model"], "model", 1, 128),
        "checkpoint": None if doc["checkpoint"] is None else _ascii(doc["checkpoint"], "checkpoint", 1, 128),
        "identity_kind": doc["identity_kind"],
        "score_kind": doc["score_kind"],
        "scoreless": doc["scoreless"],
        "max_options": _int(doc["max_options"], "max_options", 1, 16),
        "max_state_bytes": _int(doc["max_state_bytes"], "max_state_bytes", 1, 4096),
        "modalities": doc["modalities"],
        "renderer": _ascii(doc.get("renderer", DEFAULT_RENDERER), "renderer", 1, 64),
    }
    if p["identity_kind"] not in IDENTITY_KINDS:
        raise _bad("`identity_kind` outside the closed set")
    if p["score_kind"] not in SCORE_KINDS:
        raise _bad("`score_kind` outside the closed set")
    if not isinstance(p["scoreless"], bool) or p["scoreless"] != (p["score_kind"] == "none"):
        raise _bad("`scoreless` must be a boolean that is true exactly when score_kind is none")
    m = p["modalities"]
    if not isinstance(m, list) or not m or len(set(m)) != len(m) or any(x not in MODALITIES for x in m):
        raise _bad("`modalities` must be a non-empty unique subset of text,image")
    if p["identity_kind"] in ("immutable_checkpoint", "local_declared") and p["checkpoint"] is None:
        raise _bad(f"identity_kind {p['identity_kind']} requires a checkpoint")
    return p


def load(env, derive):
    """Profile from the environment, else `derive()` (the backend's defaults)."""
    raw = env.get(ENV)
    if not raw:
        return validate(derive())
    if len(raw.encode()) > MAX_PROFILE_BYTES:
        raise _bad(f"larger than {MAX_PROFILE_BYTES} bytes")
    try:
        doc = json.loads(raw)
    except ValueError:
        raise _bad("not valid JSON")
    return validate(doc)
