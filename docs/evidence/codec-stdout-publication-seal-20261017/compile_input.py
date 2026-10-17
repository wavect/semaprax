"""One exact source-only compile repair acquisition; no runtime authority."""
from pathlib import Path
import hashlib
import json
HERE = Path(__file__).resolve().parent / "compile-cc7bb"
SOURCE = "src/cli/json_codec/retained_receipt/stdout_seal/tests.rs"
def digest(text):
    return hashlib.sha256(text.encode()).hexdigest()
def unique(pairs):
    result = {}
    for key, value in pairs:
        assert key not in result, ("duplicate source metadata", key)
        result[key] = value
    return result
def record():
    raw = (HERE / "row.json").read_text()
    assert len(raw.encode()) <= 65536
    assert digest(raw) == "63985776a2ca44308e1e95138e36e0b804f9584cda8b4b3f7c45c972d757f92a"
    row = json.loads(raw, object_pairs_hook=unique)
    assert set(row) == {"source", "current_sha256", "previous_sha256", "current", "previous", "inverse"}
    assert row["source"] == SOURCE
    return row
def restore(row, text):
    assert digest(text) == row["current_sha256"]
    assert text == row["current"] == (HERE / "tests.rs.current.txt").read_text()
    edits = row["inverse"]
    assert 1 <= len(edits) <= 16
    lines = text.splitlines(keepends=True)
    last = len(lines)
    for edit in reversed(edits):
        assert set(edit) == {"start", "end", "current", "previous"}
        start, end = edit["start"], edit["end"]
        assert type(start) is int and type(end) is int and 0 <= start <= end <= last <= len(lines)
        assert "".join(lines[start:end]) == edit["current"]
        lines[start:end] = edit["previous"].splitlines(keepends=True)
        last = start
    previous = "".join(lines)
    assert digest(previous) == row["previous_sha256"]
    assert previous == row["previous"] == (HERE / "tests.rs.before.txt").read_text()
    return previous
def project(name, text):
    return restore(record(), text) if name == SOURCE else text
