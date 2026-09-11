#!/usr/bin/env python3
"""Independent reference oracle for the catalog-normalizer acceptance
application (SPX-AI-018 / GitHub issue #117).

This script is the ONE authority for "what is the correct output of the
catalog-normalizer for this input". It is written in Python, deliberately a
different language and toolchain than SEMAPRAX, so that it can never share a
parser, a compiler bug, or a library implementation with the Semaprax
candidate under test (issue #124). It has no dependency on any semaprax
crate, package, or binary.

The exact rules this script implements are frozen in
`docs/CATALOG-NORMALIZER-ORACLE-V1.md`. That document and this script must be
read together: the document is normative prose, this script is the
executable form of the same rules, and the frozen corpus under `cases/`
is a set of known answers produced by this exact script. See
`tests/oracle/catalog_normalizer/README.md` for the freeze/change procedure
and the protected-location notice.

This file, `README.md`, `fixtures/`, and everything under `cases/` are
FROZEN. Implementation agents (issue #124 and friends) may run this script
and read its published cases, but MUST NOT edit anything in this directory,
and MUST NOT read or copy `cases/hidden/**` into their implementation or
test sources. See the README for the exact policy.

Usage:
    python3 oracle.py [--enrich] [--fixture PATH] [--buggy MODE] < input
        Reads the raw request body (JSON Lines bytes) from stdin, writes the
        canonical response bytes (see the spec) to stdout. Always exits 0
        for a well-formed invocation: a documented application-level error
        is written to stdout as an error envelope, not signalled via the
        process exit code. `--fixture` selects the enrichment fixture table
        (defaults to fixtures/enrichment.json); `--enrich` turns on
        enrichment (default: disabled, matching "network disabled by
        default"). `--buggy MODE` swaps in one deliberately wrong behaviour;
        see BUGGY_MODES below. Exits 2 for a tool-usage error (bad flags,
        unreadable fixture file).

    python3 oracle.py --self-test
        Runs every published and hidden case against this oracle and
        checks the frozen expected output byte for byte, then runs every
        negative control and checks that the buggy variant's output differs
        from the correct oracle output on its designated exposing case.
        Prints a one-line summary and exits 0 iff every check passed, 1
        otherwise. This is the entry point the Rust harness
        (tests/useful_data/catalog_normalizer_oracle.rs) invokes.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Optional

HERE = Path(__file__).resolve().parent

# --------------------------------------------------------------------------
# Frozen limits (CNORM-010..CNORM-016 in the spec)
# --------------------------------------------------------------------------

I64_MAX = 9223372036854775807
MAX_TOTAL_INPUT_BYTES = 65536
MAX_RECORDS = 256
MAX_LINE_BYTES = 512
MIN_ID_BYTES = 1
MAX_ID_BYTES = 64
MAX_LABEL_BYTES = 256
MAX_NESTED_DEPTH = 8

ASCII_WS = frozenset({0x20, 0x09, 0x0A, 0x0D})
SIMPLE_ESCAPES = {
    0x22: 0x22,  # \"
    0x5C: 0x5C,  # \\
    0x2F: 0x2F,  # \/
    0x62: 0x08,  # \b
    0x66: 0x0C,  # \f
    0x6E: 0x0A,  # \n
    0x72: 0x0D,  # \r
    0x74: 0x09,  # \t
}
REQUIRED_KEYS = (b'"id"', b'"label"', b'"quantity"')


class OracleError(Exception):
    """One rejected batch: a category name plus its exact position."""

    def __init__(self, category: str, record_index: int, byte_offset: int):
        super().__init__(f"{category} at record {record_index}, byte {byte_offset}")
        self.category = category
        self.record_index = record_index
        self.byte_offset = byte_offset


# --------------------------------------------------------------------------
# Byte-level JSON value scanner, restricted to what one record line needs.
# Every offset below is a zero-based byte offset WITHIN THE LINE that raised
# it, per CNORM-030.
# --------------------------------------------------------------------------


def _is_digit(b: int) -> bool:
    return 0x30 <= b <= 0x39


def _hex_val(b: int) -> int:
    if 0x30 <= b <= 0x39:
        return b - 0x30
    if 0x61 <= b <= 0x66:
        return b - 0x61 + 10
    if 0x41 <= b <= 0x46:
        return b - 0x41 + 10
    raise ValueError("not hex")


def skip_ws(data: bytes, pos: int) -> int:
    n = len(data)
    while pos < n and data[pos] in ASCII_WS:
        pos += 1
    return pos


def parse_string(data: bytes, pos: int) -> tuple[bytes, int, int]:
    """data[pos] must be '"'. Returns (decoded_bytes, start, end)."""
    n = len(data)
    start = pos
    assert data[pos] == 0x22
    pos += 1
    out = bytearray()
    while True:
        if pos >= n:
            raise OracleError("malformed_json", -1, n)
        b = data[pos]
        if b == 0x22:
            return bytes(out), start, pos + 1
        if b == 0x5C:
            esc_at = pos
            if pos + 1 >= n:
                raise OracleError("malformed_json", -1, esc_at)
            e = data[pos + 1]
            if e in SIMPLE_ESCAPES:
                out.append(SIMPLE_ESCAPES[e])
                pos += 2
                continue
            if e == 0x75:  # 'u'
                unit, pos2 = _decode_hex4(data, pos + 2, esc_at)
                if 0xD800 <= unit <= 0xDBFF:
                    if (
                        pos2 + 1 < n
                        and data[pos2] == 0x5C
                        and data[pos2 + 1] == 0x75
                    ):
                        low, pos3 = _decode_hex4(data, pos2 + 2, esc_at)
                        if 0xDC00 <= low <= 0xDFFF:
                            scalar = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)
                            out.extend(chr(scalar).encode("utf-8"))
                            pos = pos3
                            continue
                    raise OracleError("malformed_json", -1, esc_at)
                if 0xDC00 <= unit <= 0xDFFF:
                    raise OracleError("malformed_json", -1, esc_at)
                out.extend(chr(unit).encode("utf-8"))
                pos = pos2
                continue
            raise OracleError("malformed_json", -1, esc_at)
        if b < 0x20:
            raise OracleError("malformed_json", -1, pos)
        out.append(b)
        pos += 1


def _decode_hex4(data: bytes, pos: int, esc_at: int) -> tuple[int, int]:
    n = len(data)
    if pos + 4 > n:
        raise OracleError("malformed_json", -1, esc_at)
    value = 0
    for i in range(4):
        b = data[pos + i]
        if not (
            (0x30 <= b <= 0x39) or (0x61 <= b <= 0x66) or (0x41 <= b <= 0x46)
        ):
            raise OracleError("malformed_json", -1, esc_at)
        value = value * 16 + _hex_val(b)
    return value, pos + 4


def parse_number(data: bytes, pos: int) -> tuple[bytes, bool, int, int]:
    n = len(data)
    start = pos
    if pos < n and data[pos] == 0x2D:  # '-'
        pos += 1
    if pos >= n or not _is_digit(data[pos]):
        raise OracleError("malformed_json", -1, start)
    if data[pos] == 0x30:  # '0'
        pos += 1
    else:
        while pos < n and _is_digit(data[pos]):
            pos += 1
    is_integer = True
    if pos < n and data[pos] == 0x2E:  # '.'
        is_integer = False
        pos += 1
        if pos >= n or not _is_digit(data[pos]):
            raise OracleError("malformed_json", -1, pos)
        while pos < n and _is_digit(data[pos]):
            pos += 1
    if pos < n and data[pos] in (0x65, 0x45):  # 'e' / 'E'
        is_integer = False
        pos += 1
        if pos < n and data[pos] in (0x2B, 0x2D):
            pos += 1
        if pos >= n or not _is_digit(data[pos]):
            raise OracleError("malformed_json", -1, pos)
        while pos < n and _is_digit(data[pos]):
            pos += 1
    return data[start:pos], is_integer, start, pos


def _expect_literal(data: bytes, pos: int, word: bytes):
    n = len(data)
    if pos + len(word) > n or data[pos : pos + len(word)] != word:
        raise OracleError("malformed_json", -1, pos)
    return pos + len(word)


def parse_value(data: bytes, pos: int, depth: int):
    """Returns (kind, payload, start, end). kind in
    string/number/true/false/null/object/array. payload is the decoded
    bytes for a string, (token_bytes, is_integer) for a number, else None.
    """
    n = len(data)
    if pos >= n:
        raise OracleError("malformed_json", -1, pos)
    b = data[pos]
    if b == 0x22:
        decoded, start, end = parse_string(data, pos)
        return "string", decoded, start, end
    if b == 0x7B:  # '{'
        return _parse_generic_object(data, pos, depth)
    if b == 0x5B:  # '['
        return _parse_generic_array(data, pos, depth)
    if b == 0x74:  # 't'
        end = _expect_literal(data, pos, b"true")
        return "true", True, pos, end
    if b == 0x66:  # 'f'
        end = _expect_literal(data, pos, b"false")
        return "false", False, pos, end
    if b == 0x6E:  # 'n'
        end = _expect_literal(data, pos, b"null")
        return "null", None, pos, end
    if b == 0x2D or _is_digit(b):
        token, is_integer, start, end = parse_number(data, pos)
        return "number", (token, is_integer), start, end
    raise OracleError("malformed_json", -1, pos)


def _parse_generic_object(data: bytes, pos: int, depth: int):
    n = len(data)
    start = pos
    if depth >= MAX_NESTED_DEPTH:
        raise OracleError("malformed_json", -1, pos)
    pos += 1
    pos = skip_ws(data, pos)
    if pos < n and data[pos] == 0x7D:
        return "object", None, start, pos + 1
    while True:
        pos = skip_ws(data, pos)
        if pos >= n or data[pos] != 0x22:
            raise OracleError("malformed_json", -1, pos)
        _decoded, _kstart, pos = parse_string(data, pos)
        pos = skip_ws(data, pos)
        if pos >= n or data[pos] != 0x3A:  # ':'
            raise OracleError("malformed_json", -1, pos)
        pos = skip_ws(data, pos + 1)
        _kind, _payload, _vstart, pos = parse_value(data, pos, depth + 1)
        pos = skip_ws(data, pos)
        if pos >= n:
            raise OracleError("malformed_json", -1, pos)
        if data[pos] == 0x2C:  # ','
            pos = skip_ws(data, pos + 1)
            continue
        if data[pos] == 0x7D:  # '}'
            return "object", None, start, pos + 1
        raise OracleError("malformed_json", -1, pos)


def _parse_generic_array(data: bytes, pos: int, depth: int):
    n = len(data)
    start = pos
    if depth >= MAX_NESTED_DEPTH:
        raise OracleError("malformed_json", -1, pos)
    pos += 1
    pos = skip_ws(data, pos)
    if pos < n and data[pos] == 0x5D:
        return "array", None, start, pos + 1
    while True:
        pos = skip_ws(data, pos)
        _kind, _payload, _vstart, pos = parse_value(data, pos, depth + 1)
        pos = skip_ws(data, pos)
        if pos >= n:
            raise OracleError("malformed_json", -1, pos)
        if data[pos] == 0x2C:
            pos = skip_ws(data, pos + 1)
            continue
        if data[pos] == 0x5D:
            return "array", None, start, pos + 1
        raise OracleError("malformed_json", -1, pos)


def parse_record_object(line: bytes):
    """The top-level grammar for one record line: exactly one JSON object.
    Returns a list of (raw_key_with_quotes, key_start, kind, payload,
    value_start).
    """
    n = len(line)
    pos = skip_ws(line, 0)
    if pos >= n or line[pos] != 0x7B:
        raise OracleError("malformed_json", -1, pos)
    obj_start = pos
    pos += 1
    pos = skip_ws(line, pos)
    members = []
    if pos < n and line[pos] == 0x7D:
        pos += 1
    else:
        while True:
            pos = skip_ws(line, pos)
            if pos >= n or line[pos] != 0x22:
                raise OracleError("malformed_json", -1, pos)
            key_start = pos
            _decoded, _kstart, key_end = parse_string(line, pos)
            raw_key = line[key_start:key_end]
            pos = skip_ws(line, key_end)
            if pos >= n or line[pos] != 0x3A:
                raise OracleError("malformed_json", -1, pos)
            pos = skip_ws(line, pos + 1)
            kind, payload, value_start, pos = parse_value(line, pos, 1)
            members.append((raw_key, key_start, kind, payload, value_start))
            pos = skip_ws(line, pos)
            if pos >= n:
                raise OracleError("malformed_json", -1, pos)
            if line[pos] == 0x2C:
                pos = skip_ws(line, pos + 1)
                continue
            if line[pos] == 0x7D:
                pos += 1
                break
            raise OracleError("malformed_json", -1, pos)
    pos = skip_ws(line, pos)
    if pos != n:
        raise OracleError("malformed_json", -1, pos)
    return members, obj_start


# --------------------------------------------------------------------------
# Canonical rendering (CNORM-040..CNORM-044). Never touches float; every
# number here is an exact Python int rendered as plain decimal, which is the
# same canonical form std.data.json.digits defines for i64.
# --------------------------------------------------------------------------


def render_int(value: int) -> bytes:
    return str(value).encode("ascii")


def render_string(raw: bytes) -> bytes:
    out = bytearray(b'"')
    for b in raw:
        if b == 0x22:
            out.extend(b'\\"')
        elif b == 0x5C:
            out.extend(b"\\\\")
        elif b == 0x08:
            out.extend(b"\\b")
        elif b == 0x09:
            out.extend(b"\\t")
        elif b == 0x0A:
            out.extend(b"\\n")
        elif b == 0x0C:
            out.extend(b"\\f")
        elif b == 0x0D:
            out.extend(b"\\r")
        elif b < 0x20:
            out.extend(f"\\u00{b:02x}".encode("ascii"))
        else:
            out.append(b)
    out.extend(b'"')
    return bytes(out)


def render_success(records: list[dict], total: int, enrich: bool) -> bytes:
    parts = [
        b'{"status":"ok","count":',
        render_int(len(records)),
        b',"total_quantity":',
        render_int(total),
        b',"records":[',
    ]
    for i, rec in enumerate(records):
        if i:
            parts.append(b",")
        parts.append(b'{"id":')
        parts.append(render_string(rec["id"]))
        parts.append(b',"label":')
        parts.append(render_string(rec["label"]))
        parts.append(b',"quantity":')
        parts.append(render_int(rec["quantity"]))
        if enrich:
            parts.append(b',"category":')
            parts.append(
                b"null" if rec["category"] is None else render_int(rec["category"])
            )
        parts.append(b"}")
    parts.append(b"]}\n")
    return b"".join(parts)


def render_error(category: str, record_index: int, byte_offset: int) -> bytes:
    return (
        b'{"status":"error","category":"'
        + category.encode("ascii")
        + b'","record_index":'
        + render_int(record_index)
        + b',"byte_offset":'
        + render_int(byte_offset)
        + b"}\n"
    )


# --------------------------------------------------------------------------
# Batch driver (CNORM-020..CNORM-029)
# --------------------------------------------------------------------------


def split_records(body: bytes) -> list[bytes]:
    if body.endswith(b"\n"):
        body = body[:-1]
    if body == b"":
        return []
    return body.split(b"\n")


def process_one_record(
    line: bytes, index: int, enrich: bool, fixture: dict, buggy: Optional[str]
):
    if len(line) == 0:
        raise OracleError("malformed_json", index, 0)
    line_ceiling_bug = buggy == "count-chars-as-bytes"
    if not line_ceiling_bug and len(line) > MAX_LINE_BYTES:
        raise OracleError("oversized_input", index, MAX_LINE_BYTES)
    try:
        line.decode("utf-8", errors="strict")
    except UnicodeDecodeError as exc:
        raise OracleError("invalid_utf8", index, exc.start) from None

    try:
        members, obj_start = parse_record_object(line)
    except OracleError as exc:
        raise OracleError(exc.category, index, exc.byte_offset) from None

    seen_required: dict[bytes, int] = {}
    for raw_key, key_start, _kind, _payload, _value_start in members:
        if raw_key not in REQUIRED_KEYS:
            raise OracleError("schema", index, key_start)
        allow_dup = buggy == "accept-duplicate-keys"
        if raw_key in seen_required and not allow_dup:
            raise OracleError("schema", index, key_start)
        seen_required[raw_key] = key_start

    by_key = {}
    for raw_key, _key_start, kind, payload, value_start in members:
        by_key[raw_key] = (kind, payload, value_start)  # last-wins if allow_dup

    for required in REQUIRED_KEYS:
        if required not in by_key:
            raise OracleError("schema", index, obj_start)

    id_kind, id_payload, id_value_start = by_key[b'"id"']
    if id_kind != "string":
        raise OracleError("schema", index, id_value_start)
    label_kind, label_payload, label_value_start = by_key[b'"label"']
    if label_kind != "string":
        raise OracleError("schema", index, label_value_start)
    qty_kind, qty_payload, qty_value_start = by_key[b'"quantity"']
    if qty_kind != "number":
        raise OracleError("schema", index, qty_value_start)
    qty_token, qty_is_integer = qty_payload
    if not qty_is_integer:
        raise OracleError("schema", index, qty_value_start)
    quantity = int(qty_token)
    if quantity < 0 or quantity > I64_MAX:
        raise OracleError("schema", index, qty_value_start)

    id_decoded: bytes = id_payload
    label_decoded: bytes = label_payload

    def byte_len(v: bytes) -> int:
        if line_ceiling_bug:
            return len(v.decode("utf-8"))
        return len(v)

    if len(id_decoded) < MIN_ID_BYTES:
        raise OracleError("schema", index, id_value_start)
    if byte_len(id_decoded) > MAX_ID_BYTES:
        raise OracleError("oversized_input", index, id_value_start)
    if byte_len(label_decoded) > MAX_LABEL_BYTES:
        raise OracleError("oversized_input", index, label_value_start)

    normalized_label = label_decoded.strip(bytes(sorted(ASCII_WS)))

    return {
        "id": id_decoded,
        "label": normalized_label,
        "quantity": quantity,
        "id_value_start": id_value_start,
    }


def normalize(
    body: bytes,
    enrich: bool = False,
    fixture: Optional[dict] = None,
    buggy: Optional[str] = None,
) -> bytes:
    fixture = fixture or {}
    try:
        if len(body) > MAX_TOTAL_INPUT_BYTES:
            raise OracleError("oversized_input", -1, MAX_TOTAL_INPUT_BYTES)
        lines = split_records(body)
        if len(lines) > MAX_RECORDS:
            raise OracleError("oversized_input", MAX_RECORDS, 0)

        seen_ids: list[bytes] = []
        records: list[dict] = []
        running_sum = 0
        for index, line in enumerate(lines):
            rec = process_one_record(line, index, enrich, fixture, buggy)
            if rec["id"] in seen_ids:
                raise OracleError("duplicate_id", index, rec["id_value_start"])

            if buggy == "unchecked-total":
                new_sum = running_sum + rec["quantity"]
                new_sum = ((new_sum + (1 << 63)) % (1 << 64)) - (1 << 63)
            else:
                new_sum = running_sum + rec["quantity"]
                if new_sum > I64_MAX:
                    raise OracleError("overflow", index, rec["id_value_start"])
            running_sum = new_sum

            category = None
            if enrich:
                key = rec["id"].decode("utf-8")
                outcome = fixture.get(key, {"kind": "missing"})
                kind = outcome["kind"]
                if kind == "found":
                    category = outcome["category_code"]
                elif kind == "missing":
                    category = None
                elif kind in ("denied", "timeout", "malformed"):
                    if buggy == "retry-nonretryable-provider-error" and kind == "denied":
                        category = None
                    else:
                        raise OracleError(
                            f"provider_{kind}", index, rec["id_value_start"]
                        )
                else:
                    raise ValueError(f"unknown fixture outcome kind {kind!r}")

            seen_ids.append(rec["id"])
            records.append(
                {
                    "id": rec["id"],
                    "label": rec["label"],
                    "quantity": rec["quantity"],
                    "category": category,
                }
            )

        return render_success(records, running_sum, enrich)
    except OracleError as exc:
        return render_error(exc.category, exc.record_index, exc.byte_offset)


def normalize_with_prefix_bug(
    body: bytes, enrich: bool, fixture: Optional[dict]
) -> bytes:
    """Separate driver for the publish-prefix-after-error control: on the
    first error, emit a success envelope covering every record accepted
    strictly before the failing one, instead of the correct all-or-nothing
    rejection."""
    fixture = fixture or {}
    lines = split_records(body)
    seen_ids: list[bytes] = []
    records: list[dict] = []
    running_sum = 0
    for index, line in enumerate(lines):
        try:
            rec = process_one_record(line, index, enrich, fixture, None)
            if rec["id"] in seen_ids:
                raise OracleError("duplicate_id", index, rec["id_value_start"])
            new_sum = running_sum + rec["quantity"]
            if new_sum > I64_MAX:
                raise OracleError("overflow", index, rec["id_value_start"])
            category = None
            if enrich:
                key = rec["id"].decode("utf-8")
                outcome = fixture.get(key, {"kind": "missing"})
                kind = outcome["kind"]
                if kind == "found":
                    category = outcome["category_code"]
                elif kind == "missing":
                    category = None
                else:
                    raise OracleError(f"provider_{kind}", index, rec["id_value_start"])
            running_sum = new_sum
            seen_ids.append(rec["id"])
            records.append(
                {
                    "id": rec["id"],
                    "label": rec["label"],
                    "quantity": rec["quantity"],
                    "category": category,
                }
            )
        except OracleError:
            return render_success(records, running_sum, enrich)
    return render_success(records, running_sum, enrich)


# --------------------------------------------------------------------------
# Entry points
# --------------------------------------------------------------------------

BUGGY_MODES = (
    "accept-duplicate-keys",
    "count-chars-as-bytes",
    "publish-prefix-after-error",
    "unchecked-total",
    "retry-nonretryable-provider-error",
    "hardcode-example",
)


def run_one(body: bytes, enrich: bool, fixture: dict, buggy: Optional[str]) -> bytes:
    if buggy == "hardcode-example":
        return HARDCODE_EXAMPLE_OUTPUT
    if buggy == "publish-prefix-after-error":
        return normalize_with_prefix_bug(body, enrich, fixture)
    return normalize(body, enrich, fixture, buggy)


def load_fixture(path: Path) -> dict:
    with open(path, "rb") as f:
        raw = json.load(f)
    return raw["ids"]


def load_manifest(path: Path) -> list[dict]:
    with open(path, "rb") as f:
        raw = json.load(f)
    return raw["cases"]


def case_input_bytes(case: dict) -> bytes:
    """Most cases store `input` as UTF-8 text, since almost every input this
    oracle accepts or rejects is itself valid UTF-8 text. The one family of
    case that cannot use that field is a case whose whole point is invalid
    UTF-8 bytes, which cannot be represented as a JSON string; those store
    `input_hex` instead."""
    if "input_hex" in case:
        return bytes.fromhex(case["input_hex"])
    return case["input"].encode("utf-8")


HARDCODE_EXAMPLE_OUTPUT: bytes = b""  # filled in by _load_hardcode_baseline()


def _load_hardcode_baseline():
    global HARDCODE_EXAMPLE_OUTPUT
    manifest = load_manifest(HERE / "cases" / "published" / "basics.json")
    for case in manifest:
        if case["name"] == "basic-single-record":
            HARDCODE_EXAMPLE_OUTPUT = case["expected_output"].encode("utf-8")
            return
    raise RuntimeError("basic-single-record case not found for hardcode-example bug")


def self_test() -> int:
    _load_hardcode_baseline()
    fixture = load_fixture(HERE / "fixtures" / "enrichment.json")
    total_cases = 0
    failures = []

    manifest_dir = HERE / "cases"
    for zone in ("published", "hidden"):
        for manifest_path in sorted((manifest_dir / zone).glob("*.json")):
            for case in load_manifest(manifest_path):
                total_cases += 1
                body = case_input_bytes(case)
                enrich = case.get("enrich", False)
                expected = case["expected_output"].encode("utf-8")
                actual = normalize(body, enrich, fixture, None)
                if actual != expected:
                    failures.append(
                        f"{zone}/{manifest_path.name}::{case['name']}: "
                        f"expected {expected!r}, got {actual!r}"
                    )

    controls_path = HERE / "cases" / "negative_controls.json"
    with open(controls_path, "rb") as f:
        controls = json.load(f)["controls"]
    total_controls = 0
    for control in controls:
        total_controls += 1
        mode = control["buggy"]
        case_name = control["case"]
        zone = control["zone"]
        manifest = load_manifest(manifest_dir / zone / control["manifest"])
        case = next(c for c in manifest if c["name"] == case_name)
        body = case_input_bytes(case)
        enrich = case.get("enrich", False)
        correct = normalize(body, enrich, fixture, None)
        buggy_output = run_one(body, enrich, fixture, mode)
        if buggy_output == correct:
            failures.append(
                f"negative-control {mode} on {zone}/{control['manifest']}::"
                f"{case_name}: buggy output matched the correct oracle output "
                f"({correct!r}); the control failed to expose the bug"
            )

    if failures:
        for line in failures:
            print(f"FAIL: {line}", file=sys.stderr)
        print(
            f"catalog-normalizer oracle self-test: "
            f"{total_cases - len([f for f in failures if 'negative-control' not in f])}"
            f"/{total_cases} known-answer cases passed, "
            f"{total_controls - len([f for f in failures if 'negative-control' in f])}"
            f"/{total_controls} negative controls passed",
            file=sys.stderr,
        )
        return 1

    print(
        f"catalog-normalizer oracle self-test: OK "
        f"({total_cases} known-answer cases, {total_controls} negative controls)"
    )
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--enrich", action="store_true")
    parser.add_argument(
        "--fixture", type=Path, default=HERE / "fixtures" / "enrichment.json"
    )
    parser.add_argument("--buggy", choices=BUGGY_MODES, default=None)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    try:
        fixture = load_fixture(args.fixture) if args.enrich else {}
    except OSError as exc:
        print(f"cannot read fixture {args.fixture}: {exc}", file=sys.stderr)
        return 2

    if args.buggy == "hardcode-example":
        _load_hardcode_baseline()

    body = sys.stdin.buffer.read()
    sys.stdout.buffer.write(run_one(body, args.enrich, fixture, args.buggy))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
