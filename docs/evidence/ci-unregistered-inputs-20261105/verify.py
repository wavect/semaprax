#!/usr/bin/env python3
"""Check all eight exact source successors without changing any source."""
from pathlib import Path
import hashlib
import json

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
DATA = HERE / "inverses.json"
raw = DATA.read_bytes()
assert len(raw) == 18810
assert hashlib.sha256(raw).hexdigest() == "12e4d04ace3cb951ffafb24662b855bbaf204e03b08ee011ec5d2d7638801416"
rows = json.loads(raw)
expected = {"std/io-lines/src/lines.spx", "src/byte_ops.rs", "src/hir/declaration_index.rs",
            "tests/useful_data/byte_slice_for_runtime/host.mjs",
            "tests/workspace/semantic_change.rs", "tests/workspace/semantic_operations.rs",
            "tests/workspace/semantic_structural_change.rs", "src/cleanup_plan/replay/factored/canonical_shared/before_ordered.rs"}
assert len(rows) == len({row["source"] for row in rows}) == 8
assert {row["source"] for row in rows} == expected

def read(path, length, digest):
    assert length <= 2097152 and not path.is_symlink()
    with path.open("rb") as stream:
        body = stream.read(2097153)
    assert len(body) == length
    assert hashlib.sha256(body).hexdigest() == digest
    return body

for row in rows:
    current = read(ROOT / row["source"], row["current_bytes"], row["current_sha256"])
    assert read(ROOT / "e" / row["current_sha256"], row["current_bytes"], row["current_sha256"]) == current
    before = read(ROOT / "e" / row["before_sha256"], row["before_bytes"], row["before_sha256"])
    restored = current
    for at, new, old in reversed(row["inverse"]):
        new, old = new.encode(), old.encode()
        assert restored[at:at+len(new)] == new
        restored = restored[:at] + old + restored[at+len(new):]
    assert restored == before
print("PASS: eight complete current/compiled/archive source successors reconstruct all original predecessor bytes")
