#!/usr/bin/env python3
"""Finite complete-byte continuation proof. Does not execute Rust or compiler APIs."""
from pathlib import Path
import hashlib
import json
import re

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
DATA = json.loads((HERE / "source-cuts.json").read_text())

def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()

def restore(row, text):
    assert sha(text) == row["after_sha256"], (row["path"], "current whole-source drift")
    for edit in reversed(row["edits"]):
        start, end = edit["after_start"], edit["after_end"]
        assert text[start:end] == edit["after"], (row["path"], "inverse endpoint")
        text = text[:start] + edit["before"] + text[end:]
    assert sha(text) == row["before_sha256"], (row["path"], "predecessor digest")
    assert text == (ROOT / row["before"]).read_text(), (row["path"], "complete predecessor")
    return text

def refused(operation):
    try:
        operation()
    except AssertionError:
        return
    raise AssertionError("drift control unexpectedly accepted")

def main():
    negatives = 0
    old_assertions = 0
    for row in DATA["changed"]:
        current = (ROOT / row["path"]).read_text()
        before = restore(row, current)
        if row["path"].endswith(".rs"):
            old_assertions += len(re.findall(r"\bassert(?:_eq|_ne)?!", before))
        refused(lambda: restore(row, ("!" if current[0] != "!" else "?") + current[1:]))
        refused(lambda: restore(row, current + " "))
        negatives += 2
    for row in DATA["new"] + DATA["unchanged"]:
        text = (ROOT / row["path"]).read_text()
        assert sha(text) == row["sha256"], row["path"]
        refused(lambda: assert_hash(row, ("!" if text[0] != "!" else "?") + text[1:]))
        negatives += 1
    children = [
        "src/cli/json_codec/retained_receipt/stdout_seal.rs",
        "src/cli/json_codec/retained_receipt/stdout_seal/tests.rs",
        "tests/project/standard_library/application_json/retained_receipt/stdout_seal.rs",
    ]
    for filename, prefix in [
        ("tests/hir_module_boundaries.rs", "../"),
        ("tests/workspace/semantic_graph/source_inventory.rs", "../../"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "../../../../../"),
    ]:
        text = (ROOT / filename).read_text()
        row = next(row for row in DATA["changed"] if row["path"] == filename)
        added = "".join('include_str!("' + prefix + child + '"),' for child in children)
        assert text.count(added) == 1
        assert text.replace(added, "", 1) == (ROOT / row["before"]).read_text()
        for bad in [text.replace(added, "", 1), text.replace(added, added + added, 1),
                    text.replace(added, added.replace("stdout_seal.rs", "foreign_seal.rs", 1), 1)]:
            refused(lambda: restore(row, bad))
            negatives += 1
    for parent, registration in [
        ("src/cli/json_codec/retained_receipt.rs", '#[path = "retained_receipt/stdout_seal.rs"]\nmod stdout_seal;'),
        (children[0], '#[path = "stdout_seal/tests.rs"]\nmod tests;'),
        ("tests/project/standard_library/application_json/retained_receipt.rs", '#[path = "retained_receipt/stdout_seal.rs"]\nmod stdout_seal;'),
    ]:
        assert (ROOT / parent).read_text().count(registration) == 1, parent
    native = ROOT / "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
    assert len(native.read_text().splitlines()) <= 1500
    owners = json.loads((HERE / "pending-owners.json").read_text())["new"]
    assert len(owners) == 8
    for owner in owners:
        text = (ROOT / owner["source"]).read_text()
        assert text.count("fn " + owner["selector"].split("::")[-1] + "(") == 1
        assert owner["status"] == "PENDING"
    print(json.dumps({"status": "PASS_SOURCE_ONLY", "complete_preimages": len(DATA["changed"]),
        "new_files": len(DATA["new"]), "unchanged_whole_files": len(DATA["unchanged"]),
        "old_assertion_calls_in_changed_rust": old_assertions, "drift_controls": negatives,
        "raw_joins": 3, "new_pending_rust_owners": len(owners),
        "rust_or_provider_execution": False}, sort_keys=True))

def assert_hash(row, text):
    assert sha(text) == row["sha256"], row["path"]

if __name__ == "__main__":
    main()
