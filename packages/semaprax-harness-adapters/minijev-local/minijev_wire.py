"""Shared constants and pure helpers for the Mini Jev adapter and its worker (stdlib only)."""

import hashlib
import json
import math
import re

UPSTREAM_REPO = "https://github.com/r-ms/mini-jev"
PINNED_CODE_COMMIT = "ca612198bfb69f538f029a4615f6d0a18b4f814c"
UPSTREAM_LICENSE = "MIT"
MAX_OPTIONS = 16               # host bound; upstream demo admits 2..26
MIN_OPTIONS = 2
TEMPLATE_ID = "minijev-letters.v1"
CONFIDENCE_KIND = "minijev.choice_confidence"
WIRE_VERSION = 1
MAX_REQUEST_LINE = 16384       # worker refuses a longer request line
MAX_RESPONSE_LINE = 8192       # adapter refuses a longer response line
MAX_USER_BYTES = 8192
IDENTITY_KEYS = ("engine", "model", "revision", "tokenizer_sha", "code_commit", "system_sha", "letters_sha", "dtype", "device")
_HEX40 = re.compile(r"^[0-9a-f]{40}$")
_HEX64 = re.compile(r"^[0-9a-f]{64}$")


def letter(i):
    if not 0 <= i < 26:
        raise ValueError("letter index outside A..Z")
    return chr(65 + i)


def render_user(rendered, options=None):
    """Fixed template around the host-rendered content; instructions/state/labels are verbatim.

    Letters follow `options` order (the request array), never the key order of
    `option_labels`, which the host serializes sorted.
    """
    options = list(rendered["option_labels"]) if options is None else options
    lines = [f"{letter(i)} = {rendered['option_labels'][o]}" for i, o in enumerate(options)]
    return f"TEXT:\n{rendered['state']}\n\nQUESTION: {rendered['instructions']}\n" + "\n".join(lines) + "\n\nANSWER:"


def dumps(obj):
    return json.dumps(obj, separators=(",", ":"), sort_keys=True).encode()


def valid_identity(i):
    """Closed shape check of a worker identity block."""
    if not isinstance(i, dict) or set(i) != set(IDENTITY_KEYS):
        return False
    if not all(isinstance(i[k], str) and i[k] and i[k].isascii() and len(i[k]) <= 128 for k in IDENTITY_KEYS):
        return False
    return bool(_HEX40.match(i["revision"]) and _HEX40.match(i["code_commit"])
                and _HEX64.match(i["tokenizer_sha"]) and _HEX64.match(i["system_sha"]) and _HEX64.match(i["letters_sha"]))


def renderer_pin(identity):
    """Short pin of the option renderer: template id plus the upstream system prompt actually applied."""
    return hashlib.sha256((TEMPLATE_ID + "\n" + identity["system_sha"]).encode()).hexdigest()[:8]


def composite_checkpoint(identity):
    """Model revision + tokenizer + code commit + renderer in one attestable string (<=128 ASCII)."""
    return (f"rev:{identity['revision']}+tok:{identity['tokenizer_sha'][:12]}"
            f"+code:{identity['code_commit'][:12]}+rnd:{renderer_pin(identity)}")


def softmax(xs):
    m = max(xs)
    es = [math.exp(x - m) for x in xs]
    s = sum(es)
    return [e / s for e in es]


def choice_confidence(probs):
    """Upstream minijev.letters.choice_confidence: peak mass rescaled from uniform to certainty."""
    if len(probs) == 1:
        return 1.0
    u = 1.0 / len(probs)
    return (max(probs) - u) / (1.0 - u)
