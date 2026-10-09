"""Independent Catalog v1 oracle; no compiler-derived schema or source imports."""
import json
import re


class Invalid(ValueError):
    pass


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise Invalid("duplicate field")
        value[key] = item
    return value


def reject_constant(value):
    raise Invalid("non-JSON constant")


def integer(value, lower, upper):
    return type(value) is int and lower <= value <= upper


def identifier(value):
    return type(value) is str and re.fullmatch(r"[A-Za-z0-9_-]{1,16}", value, re.ASCII) is not None


def expected(raw):
    try:
        request = json.loads(raw.decode("utf-8"), object_pairs_hook=unique_object,
                             parse_constant=reject_constant)
    except (UnicodeError, ValueError, RecursionError):
        return 2, b"", b"invalid catalog request\n"
    if type(request) is not dict or set(request) != {"tags", "items"}:
        return 2, b"", b"invalid catalog request\n"
    tags, rows = request["tags"], request["items"]
    if type(tags) is not list or len(tags) > 8 or not all(identifier(x) for x in tags) or len(set(tags)) != len(tags):
        return 2, b"", b"invalid catalog request\n"
    if type(rows) is not list or len(rows) > 256:
        return 2, b"", b"invalid catalog request\n"
    seen = set()
    selected = []
    for item in rows:
        if type(item) is not dict or set(item) != {"id", "department", "priority", "stock", "topup", "mark"}:
            return 2, b"", b"invalid catalog request\n"
        if not identifier(item["id"]) or item["id"] in seen:
            return 2, b"", b"invalid catalog request\n"
        seen.add(item["id"])
        if not (integer(item["department"], 0, 9) and integer(item["priority"], 0, 9)
                and integer(item["stock"], 0, 1000) and integer(item["topup"], 0, 1000)
                and integer(item["mark"], 0, 255)):
            return 2, b"", b"invalid catalog request\n"
        if not tags or any(item["id"].startswith(tag) for tag in tags):
            selected.append({"id": item["id"], "department": item["department"],
                             "priority": item["priority"], "stock": item["stock"] + item["topup"],
                             "mark": item["mark"]})
    selected.sort(key=lambda item: (item["department"], item["priority"], item["id"].encode("ascii")))
    report = {"items": selected, "metrics": {"selected": len(selected),
              "stock": sum(x["stock"] for x in selected), "checksum": sum(x["mark"] for x in selected)}}
    return 0, (json.dumps(report, separators=(",", ":"), ensure_ascii=True) + "\n").encode("ascii"), b""
