#!/usr/bin/env python3
"""Small exhaustive semantic oracle for the ASCII-pattern capture fixtures.

This is a deliberately separate parser and count-vector enumerator. It does not
model the proposed engine's byte/control work meter or infer resource refusal
from a work limit.
"""
from __future__ import annotations

import argparse
import itertools
import json
import sys
from pathlib import Path
from typing import Any

SCHEMA = "semaprax.ascii-pattern-source-fixtures.v1"
STATUS = {"ready": 0, "matched": 1, "no_match": 2,
          "invalid_pattern": 3, "resource_refusal": 4}
RESOURCE_REASON = {
    "pattern_extent": 1, "input_extent": 2, "atom_count": 3,
    "class_count": 4, "capture_count": 5,
}
MAX_PATTERN = 1024
MAX_INPUT = 65536
MAX_ATOMS = 128
MAX_CLASSES = 16
MAX_CAPTURES = 16
ORACLE_MAX_VECTORS = 100_000
RESERVED = set(b".\\[]()?*+{}|^$")


class InvalidPattern(Exception):
    def __init__(self, offset: int):
        self.offset = offset


class ResourceRefusal(Exception):
    def __init__(self, reason: str):
        self.reason = reason


def _pattern_bytes(spec: str | dict[str, Any]) -> bytes:
    if isinstance(spec, str):
        text = spec
    elif isinstance(spec, dict) and set(spec) <= {"repeat", "count", "suffix"}:
        unit, count, suffix = spec.get("repeat"), spec.get("count"), spec.get("suffix", "")
        if not isinstance(unit, str) or type(count) is not int or count < 0 or not isinstance(suffix, str):
            raise ValueError("invalid pattern repeat descriptor")
        text = unit * count + suffix
    else:
        raise ValueError("pattern must be ASCII text or a repeat descriptor")
    try:
        return text.encode("ascii")
    except UnicodeEncodeError as error:
        raise InvalidPattern(len(text[:error.start].encode("utf-8"))) from error


def _input_bytes(spec: str | dict[str, Any]) -> bytes:
    if isinstance(spec, str):
        return spec.encode("utf-8")
    if not isinstance(spec, dict) or len(spec) != 1:
        raise ValueError("input must be text or one byte descriptor")
    if "ascii" in spec and isinstance(spec["ascii"], str):
        return spec["ascii"].encode("ascii")
    if "repeat_ascii" in spec:
        item = spec["repeat_ascii"]
        if not isinstance(item, dict) or set(item) != {"text", "count"}:
            raise ValueError("repeat_ascii needs text and count")
        text, count = item["text"], item["count"]
        if not isinstance(text, str) or type(count) is not int or count < 0:
            raise ValueError("invalid repeat_ascii descriptor")
        return (text * count).encode("ascii")
    if "repeat_ascii_suffix" in spec:
        item = spec["repeat_ascii_suffix"]
        if not isinstance(item, dict) or set(item) != {"text", "count", "suffix"}:
            raise ValueError("repeat_ascii_suffix needs text, count, and suffix")
        text, count, suffix = item["text"], item["count"], item["suffix"]
        if (not isinstance(text, str) or type(count) is not int or count < 0
                or not isinstance(suffix, str)):
            raise ValueError("invalid repeat_ascii_suffix descriptor")
        return (text * count + suffix).encode("ascii")
    if "hex" in spec and isinstance(spec["hex"], str):
        return bytes.fromhex(spec["hex"])
    if "repeat_hex" in spec:
        item = spec["repeat_hex"]
        if not isinstance(item, dict) or set(item) != {"byte", "count"}:
            raise ValueError("repeat_hex needs byte and count")
        byte, count = item["byte"], item["count"]
        if (not isinstance(byte, str) or len(byte) != 2 or type(count) is not int
                or count < 0):
            raise ValueError("invalid repeat_hex descriptor")
        return bytes.fromhex(byte) * count
    raise ValueError("unsupported input descriptor")


def _bitmap(values: set[int]) -> bytes:
    result = bytearray(32)
    for value in values:
        result[value // 8] |= 1 << (value % 8)
    return bytes(result)


def _hex_value(pattern: bytes, offset: int) -> tuple[int, int]:
    # The first non-hex byte is the specified offending byte; missing digits
    # report EOF at the pattern length.
    if offset >= len(pattern):
        raise InvalidPattern(len(pattern))
    first = pattern[offset]
    if first not in b"0123456789abcdefABCDEF":
        raise InvalidPattern(offset)
    if offset + 1 >= len(pattern):
        raise InvalidPattern(len(pattern))
    second = pattern[offset + 1]
    if second not in b"0123456789abcdefABCDEF":
        raise InvalidPattern(offset + 1)
    return int(pattern[offset:offset + 2], 16), offset + 2


def _escaped(pattern: bytes, cursor: int) -> tuple[int, int]:
    """Decode one explicit byte escape; only printable punctuation may quote."""
    if cursor + 1 >= len(pattern):
        raise InvalidPattern(len(pattern))
    following = pattern[cursor + 1]
    if following == ord("x"):
        return _hex_value(pattern, cursor + 2)
    if (33 <= following <= 126
            and following not in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"):
        return following, cursor + 2
    raise InvalidPattern(cursor)


def _class(pattern: bytes, opening: int) -> tuple[set[int], int]:
    cursor = opening + 1
    complement = cursor < len(pattern) and pattern[cursor] == ord("^")
    if complement:
        cursor += 1
    items: list[tuple[int, bool, int]] = []
    while cursor < len(pattern) and pattern[cursor] != ord("]"):
        item_offset = cursor
        byte = pattern[cursor]
        if byte == ord("\\"):
            value, cursor = _escaped(pattern, cursor)
            items.append((value, True, item_offset))
        elif byte == ord("-"):
            # Raw '-' is literal only at the end of a class, immediately
            # before its closing bracket. Else it is a range operator.
            if cursor + 1 < len(pattern) and pattern[cursor + 1] == ord("]"):
                items.append((byte, True, item_offset))
                cursor += 1
            else:
                items.append((byte, False, item_offset))
                cursor += 1
        elif byte == ord("^"):
            raise InvalidPattern(cursor)
        elif 32 <= byte <= 126 and byte not in RESERVED:
            items.append((byte, True, item_offset))
            cursor += 1
        else:
            raise InvalidPattern(cursor)
    if cursor >= len(pattern):
        raise InvalidPattern(len(pattern))
    if not items:
        raise InvalidPattern(cursor)  # `]` is the explicit empty-class token.
    values: set[int] = set()
    index = 0
    while index < len(items):
        start, start_is_value, start_offset = items[index]
        if not start_is_value:
            raise InvalidPattern(start_offset)
        if index + 1 < len(items) and not items[index + 1][1]:
            if index + 2 >= len(items) or not items[index + 2][1]:
                raise InvalidPattern(items[index + 2][2] if index + 2 < len(items)
                                     else items[index + 1][2])
            end = items[index + 2][0]
            if end < start:
                raise InvalidPattern(items[index + 2][2])
            values.update(range(start, end + 1))
            index += 3
        else:
            values.add(start)
            index += 1
    if complement:
        values = set(range(256)) - values
    return values, cursor + 1


def compile_pattern(spec: str | dict[str, Any]) -> dict[str, Any]:
    try:
        pattern = _pattern_bytes(spec)
    except InvalidPattern as error:
        return _invalid(error.offset)
    if len(pattern) > MAX_PATTERN:
        return _resource("pattern_extent")

    atoms: list[dict[str, Any]] = []
    classes: list[bytes] = []
    captures: list[list[int]] = []
    open_capture: int | None = None
    cursor = 0
    quantified = False
    # Raw printable ASCII is a literal except for the syntax punctuation
    # reserved by the draft. Non-ASCII and controls are rejected bytewise.
    while cursor < len(pattern):
        byte = pattern[cursor]
        if byte == ord("("):
            if open_capture is not None:
                return _invalid(cursor)
            if len(captures) >= MAX_CAPTURES:
                return _resource("capture_count")
            open_capture = len(atoms)
            cursor += 1
            quantified = False
            continue
        if byte == ord(")"):
            if open_capture is None:
                return _invalid(cursor)
            captures.append([open_capture, len(atoms)])
            open_capture = None
            cursor += 1
            quantified = False
            continue
        if byte == ord("["):
            try:
                values, cursor = _class(pattern, cursor)
            except InvalidPattern as error:
                return _invalid(error.offset)
            bitmap = _bitmap(values)
            try:
                value = classes.index(bitmap)
            except ValueError:
                value = len(classes)
                classes.append(bitmap)
                if len(classes) > MAX_CLASSES:
                    return _resource("class_count")
            atom = {"kind": 2, "value": value, "min": 1, "max": 1}
        elif byte == ord("."):
            cursor += 1
            atom = {"kind": 1, "value": 0, "min": 1, "max": 1}
        elif byte == ord("\\"):
            try:
                value, cursor = _escaped(pattern, cursor)
            except InvalidPattern as error:
                return _invalid(error.offset)
            atom = {"kind": 0, "value": value, "min": 1, "max": 1}
        elif 32 <= byte <= 126 and byte not in RESERVED:
            cursor += 1
            atom = {"kind": 0, "value": byte, "min": 1, "max": 1}
        elif byte in b"?*+{":
            return _invalid(cursor)
        else:
            # This includes unsupported alternation and any unspecified
            # reserved punctuation; no literal-fallback behavior is assumed.
            return _invalid(cursor)

        if cursor < len(pattern) and pattern[cursor] in b"?*+{":
            suffix = pattern[cursor]
            if suffix == ord("?"):
                atom["min"], atom["max"] = 0, 1
                cursor += 1
            elif suffix == ord("*"):
                atom["min"], atom["max"] = 0, None
                cursor += 1
            elif suffix == ord("+"):
                atom["min"], atom["max"] = 1, None
                cursor += 1
            else:
                end = pattern.find(b"}", cursor + 1)
                if end < 0:
                    return _invalid(len(pattern))
                body = pattern[cursor + 1:end]
                pieces = body.split(b",")
                if len(pieces) == 1:
                    lower_text = upper_text = pieces[0]
                elif len(pieces) == 2:
                    lower_text, upper_text = pieces
                else:
                    return _invalid(cursor)
                lower_start = cursor + 1
                upper_start = lower_start + len(lower_text) + (1 if len(pieces) == 2 else 0)
                if not lower_text.isdigit() or not upper_text.isdigit():
                    invalid = next((cursor + 1 + index for index, digit in enumerate(body)
                                    if digit not in b"0123456789,"), cursor + 1)
                    return _invalid(invalid)
                if ((len(lower_text) > 1 and lower_text.startswith(b"0"))
                        or (len(upper_text) > 1 and upper_text.startswith(b"0"))):
                    bad = lower_start + 1 if len(lower_text) > 1 and lower_text.startswith(b"0") else (
                        upper_start + 1)
                    return _invalid(bad)
                minimum, maximum = int(lower_text), int(upper_text)
                def first_overflow(text: bytes, start: int) -> int:
                    prefix = 0
                    for index, digit in enumerate(text):
                        prefix = prefix * 10 + digit - ord("0")
                        if prefix > 255:
                            return start + index
                    return start + len(text) - 1

                if minimum > 255:
                    return _invalid(first_overflow(lower_text, lower_start))
                if maximum > 255:
                    return _invalid(first_overflow(upper_text, upper_start))
                if minimum > maximum:
                    return _invalid(end)
                atom["min"], atom["max"] = minimum, maximum
                cursor = end + 1
            if cursor < len(pattern) and pattern[cursor] in b"?*+{":
                return _invalid(cursor)
            quantified = True
        else:
            quantified = False
        atoms.append(atom)
        if len(atoms) > MAX_ATOMS:
            return _resource("atom_count")

    if open_capture is not None:
        return _invalid(len(pattern))
    return {"status": "ready", "status_code": STATUS["ready"], "reason_code": 0,
            "detail_offset": 0, "atoms": atoms, "classes": [list(item) for item in classes],
            "captures": captures}


def _invalid(offset: int) -> dict[str, Any]:
    return {"status": "invalid_pattern", "status_code": STATUS["invalid_pattern"],
            "reason_code": 1, "detail_offset": offset, "atoms": [], "classes": [], "captures": []}


def _resource(reason: str) -> dict[str, Any]:
    return {"status": "resource_refusal", "status_code": STATUS["resource_refusal"],
            "reason_code": RESOURCE_REASON[reason], "detail_offset": 0,
            "resource": reason, "atoms": [], "classes": [], "captures": []}


def _atom_accepts(atom: dict[str, Any], classes: list[list[int]], byte: int) -> bool:
    if atom["kind"] == 1:
        return True
    if atom["kind"] == 0:
        return byte == atom["value"]
    bitmap = classes[atom["value"]]
    return bool(bitmap[byte // 8] & (1 << (byte % 8)))


def match(compiled: dict[str, Any], data: bytes) -> dict[str, Any]:
    if len(data) > MAX_INPUT:
        return _resource("input_extent")
    if compiled["status"] != "ready":
        return {"status": "not_attempted", "status_code": 0, "spans": []}

    atoms = compiled["atoms"]
    variable: list[int] = []
    ranges: list[range] = []
    for index, atom in enumerate(atoms):
        low, high = atom["min"], atom["max"]
        if high is None or high != low:
            variable.append(index)
            upper = len(data) if high is None else min(high, len(data))
            if upper < low:
                return {"status": "no_match", "status_code": STATUS["no_match"], "spans": []}
            ranges.append(range(upper, low - 1, -1))
    vector_count = 1
    for choices in ranges:
        vector_count *= len(choices)
        if vector_count > ORACLE_MAX_VECTORS:
            return {"status": "oracle_inconclusive", "status_code": None,
                    "spans": [], "reason": "count-vector enumeration cap"}

    vectors = itertools.product(*ranges) if ranges else [()]
    for vector in vectors:
        counts = [atom["min"] for atom in atoms]
        for index, count in zip(variable, vector):
            counts[index] = count
        if sum(counts) != len(data):
            continue
        cursor = 0
        accepted = True
        for atom, count in zip(atoms, counts):
            for byte in data[cursor:cursor + count]:
                if not _atom_accepts(atom, compiled["classes"], byte):
                    accepted = False
                    break
            if not accepted:
                break
            cursor += count
        if not accepted or cursor != len(data):
            continue
        boundaries = [0]
        for count in counts:
            boundaries.append(boundaries[-1] + count)
        spans = [[boundaries[start], boundaries[end]] for start, end in compiled["captures"]]
        return {"status": "matched", "status_code": STATUS["matched"], "spans": spans,
                "count_vector": [counts[index] for index in variable]}
    return {"status": "no_match", "status_code": STATUS["no_match"], "spans": []}


def _project_compile(value: dict[str, Any]) -> dict[str, Any]:
    projected = {key: value.get(key) for key in
                 ("status", "status_code", "reason_code", "detail_offset", "resource")}
    if value["status"] == "ready":
        projected.update({"atom_count": len(value["atoms"]), "class_count": len(value["classes"]),
                          "capture_boundaries": value["captures"]})
    return projected


def evaluate(case: dict[str, Any]) -> dict[str, Any]:
    if case.get("oracle_status") == "pending_spec":
        return {"id": case["id"], "oracle_status": "pending_spec",
                "pending": case.get("pending_reason")}
    compiled = compile_pattern(case["pattern"])
    if compiled["status"] == "syntax_offset_pending":
        return {"id": case["id"], "oracle_status": "pending_spec",
                "pending": compiled["pending"]}
    observed_match = match(compiled, _input_bytes(case["input"]))
    return {"id": case["id"], "compile": _project_compile(compiled),
            "match": {key: observed_match.get(key) for key in
                      ("status", "status_code", "reason_code", "detail_offset", "resource", "spans",
                       "count_vector")}}


def _subset_matches(observed: Any, expected: Any) -> bool:
    if isinstance(expected, dict):
        return (isinstance(observed, dict) and all(
            key in observed and _subset_matches(observed[key], value) for key, value in expected.items()))
    return observed == expected


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cases", nargs="?", type=Path, default=Path(__file__).with_name("cases.json"))
    args = parser.parse_args()
    document = json.loads(args.cases.read_text(encoding="utf-8"))
    if document.get("schema") != SCHEMA:
        raise SystemExit(f"unsupported cases schema: {document.get('schema')!r}")
    outputs, failures = [], []
    for case in document["cases"]:
        observed = evaluate(case)
        outputs.append(observed)
        expected = case.get("expected")
        if expected is not None and observed.get("oracle_status") != "pending_spec":
            for key, value in expected.items():
                if not _subset_matches(observed.get(key), value):
                    failures.append({"case": case["id"], "field": key,
                                     "expected": value, "observed": observed.get(key)})
    print(json.dumps({"schema": "semaprax.ascii-pattern-oracle-results.v1",
                      "status": "passed" if not failures else "failed",
                      "cases": outputs, "failures": failures,
                      "work_meter": "not modeled; compare only semantic outcome and do not infer work refusal"},
                     indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
