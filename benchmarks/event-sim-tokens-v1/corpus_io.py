"""Deterministic rendering for compact ShiftSim acceptance cases."""

from __future__ import annotations

import json
from typing import Any

ESCAPED_KEYS_AND_IDENTIFIERS = "unicode-escaped-keys-and-identifiers-v1"


def _escaped_ascii_string(value: str) -> str:
    if not value.isascii():
        raise ValueError("escaped-key/identifier fixtures must contain ASCII strings")
    return '"' + "".join(f"\\u{ord(char):04x}" for char in value) + '"'


def _render_escaped_json(value: Any, parent_key: str | None = None) -> str:
    if isinstance(value, dict):
        entries = []
        for key, item in value.items():
            if not isinstance(key, str):
                raise ValueError("JSON object keys must be strings")
            entries.append(
                _escaped_ascii_string(key) + ":" + _render_escaped_json(item, key)
            )
        return "{" + ",".join(entries) + "}"
    if isinstance(value, list):
        return "[" + ",".join(_render_escaped_json(item, parent_key) for item in value) + "]"
    if isinstance(value, str):
        if parent_key in ("id", "servers"):
            return _escaped_ascii_string(value)
        return json.dumps(value, ensure_ascii=False, separators=(",", ":"))
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def render_request(case: dict[str, Any], kind: str) -> bytes:
    """Render the exact deterministic request bytes described by a corpus row."""
    request_encoding = case.get("request_encoding", "compact")
    if request_encoding == "compact":
        request = json.dumps(case["input"], ensure_ascii=False, separators=(",", ":"))
    elif request_encoding == ESCAPED_KEYS_AND_IDENTIFIERS and kind == "valid":
        request = _render_escaped_json(case["input"])
    else:
        raise ValueError(f"unsupported ShiftSim request encoding: {request_encoding}")

    prefix_bytes = case.get("leading_whitespace_bytes", 0) if kind == "valid" else 0
    if type(prefix_bytes) is not int or prefix_bytes < 0:
        raise ValueError("leading_whitespace_bytes must be a nonnegative integer")
    return (" " * prefix_bytes + request + "\n").encode("utf-8")
