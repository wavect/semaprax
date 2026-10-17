"""Finite source-only successor view; no Rust/runtime or authority operation."""
from pathlib import Path
import hashlib
import json

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
LAYER = HERE / "integration-1bae9e890"


def digest(text):
    return hashlib.sha256(text.encode()).hexdigest()


def unique(pairs):
    result = {}
    for key, value in pairs:
        assert key not in result, ("duplicate JSON field", key)
        result[key] = value
    return result


raw = (LAYER / "cuts.json").read_text()
assert len(raw.encode()) == 46204
assert digest(raw) == "d8e91a3b1e723864a9193d39777b5b2a599ff75f76561377b90485ecefc4e8af"
DATA = json.loads(raw, object_pairs_hook=unique)
assert set(DATA) == {"schema", "root_base", "held_commit", "rows", "unions", "sealed_packet_files", "old_script_sha256"}
assert DATA["schema"] == "semaprax.codec-stdout-source-integration.v1"
assert len(DATA["rows"]) == 9 and len(DATA["unions"]) == 3
ROWS = {row["path"]: row for row in DATA["rows"]}
assert len(ROWS) == len(DATA["rows"])
for item in DATA["sealed_packet_files"]:
    assert digest((HERE / item["path"]).read_text()) == item["sha256"]
assert digest((LAYER / "verify-source.py.before").read_text()) == DATA["old_script_sha256"]


def project(name, text):
    row = ROWS.get(name)
    if row is None:
        return text
    assert set(row) == {"path", "current_sha256", "held_sha256", "held", "edits"}
    assert digest(text) == row["current_sha256"], (name, "whole current-source drift")
    lines = text.splitlines(keepends=True)
    last = len(lines)
    assert len(row["edits"]) <= 512
    for edit in reversed(row["edits"]):
        assert set(edit) == {"start", "end", "current", "previous"}
        start, end = edit["start"], edit["end"]
        assert 0 <= start <= end <= last <= len(lines)
        assert "".join(lines[start:end]) == edit["current"]
        lines[start:end] = edit["previous"].splitlines(keepends=True)
        last = start
    restored = "".join(lines)
    assert digest(restored) == row["held_sha256"]
    assert restored == (LAYER / row["held"]).read_text()
    return restored


def read_input(path):
    path = Path(path)
    name = path.resolve().relative_to(ROOT).as_posix()
    return project(name, path.read_text())


def physical_unions():
    # Authenticate actual raw carriers separately from their historical view.
    # These insertions are after the original leading three-subject block;
    # the original sentinel and every other carrier byte restore exactly.
    for row in DATA["unions"]:
        assert set(row) == {"path", "root", "root_sha256", "anchor", "added"}
        current = (ROOT / row["path"]).read_text()
        root = (LAYER / row["root"]).read_text()
        assert digest(root) == row["root_sha256"]
        assert root.count(row["anchor"]) == 1
        assert current.count(row["added"]) == 1
        assert row["anchor"] + row["added"] in current
        assert current.replace(row["added"], "", 1) == root


physical_unions()
