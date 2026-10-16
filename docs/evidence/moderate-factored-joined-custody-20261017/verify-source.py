#!/usr/bin/env python3
"""Source-only full-domain composition; no Rust or provider execution."""
from pathlib import Path
import collections
import hashlib
import importlib.util
import json
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
CUTS = json.loads((HERE / "source-cuts.json").read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()

def rejects(operation):
    try: operation()
    except (AssertionError, ValueError): return
    raise AssertionError("negative accepted")

def predecessor(row, data):
    assert sha(data) == row["current_sha256"]
    for edit in reversed(row["edits"]):
        a, b = edit["start"], edit["end"]
        assert data[a:b] == edit["current"].encode()
        data = data[:a] + edit["before"].encode() + data[b:]
    assert sha(data) == row["before_sha256"]
    assert data == (ROOT / row["before"]).read_bytes()
    return data

def incoming(row, data):
    assert len(data) == row["current_length"] and sha(data) == row["current_sha256"]
    for edit in reversed(row["edits"]):
        a, b = edit["start"], edit["end"]
        assert data[a:b] == edit["current"].encode()
        data = data[:a] + edit["incoming"].encode() + data[b:]
    assert len(data) == row["incoming_length"] and sha(data) == row["incoming_sha256"]
    assert data == (ROOT / row["incoming_archive"]).read_bytes()
    return data

def main():
    negatives = 0
    for row in CUTS["changed"]:
        data = (ROOT / row["path"]).read_bytes()
        predecessor(row, data)
        for wrong in [b"!" + data[1:], data + b" "]:
            rejects(lambda: predecessor(row, wrong)); negatives += 1
    for row in CUTS["unchanged"] + CUTS["joined_originals"] + [CUTS["data"]]:
        data = (ROOT / row["path"]).read_bytes()
        assert len(data) == row["bytes"] and sha(data) == row["sha256"], row["path"]
    original_rows = json.loads((ROOT / "docs/evidence/moderate-factored-v1/sources.json").read_text())
    rows = json.loads((HERE / "joined-inputs.json").read_text())
    assert len(rows) == len(original_rows) == 3
    for row, old in zip(rows, original_rows):
        assert row["path"] == old["path"]
        raw = (ROOT / row["path"]).read_bytes()
        target = incoming(row, raw).decode()
        for wrong in [b"", raw[:len(raw)//2], b"!"+raw[1:], raw+b" "]:
            rejects(lambda: incoming(row, wrong)); negatives += 1
        restored = target
        for edit in reversed(old["edits"]):
            assert restored.count(edit["new"]) == 1
            restored = restored.replace(edit["new"], edit["old"], 1)
        archive = (ROOT / "docs/evidence/moderate-factored-v1" / old["archive"]).read_bytes()
        assert restored.encode() == archive and sha(archive) == old["sha256"]
    # Both full existing historical tables remain exact local parent originals.
    for path in ["docs/evidence/receiver-source-custody-v1/inverse.json", "docs/evidence/compact-staged-observations-v1/inverse.json"]:
        assert (ROOT / path).read_bytes() == subprocess.check_output(["git", "show", "20c451b91:"+path], cwd=ROOT)
    new_paths = ["src/cleanup_plan/replay/factored/moderate_tests.rs", "src/cleanup_plan/replay/factored/moderate_tests/before.rs"]
    allowed_path = "docs/evidence/main043-independent-custody-20261016/guard-extension-allowed.json"
    allowed = json.loads((ROOT / allowed_path).read_text())
    old_allowed = json.loads((HERE / "before" / allowed_path).read_text())
    for path, prefix in [
        ("tests/hir_module_boundaries.rs", "../"),
        ("tests/workspace/semantic_graph/source_inventory.rs", "../../"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "../../../../../"),
    ]:
        raw = (ROOT / path).read_text()
        tokens = "".join('include_str!("'+prefix+child+'"),' for child in new_paths)
        assert raw.count(tokens) == 1
        assert raw.replace(tokens, "", 1) == (HERE / "before" / path).read_text()
        cut = next(row for row in CUTS["changed"] if row["path"] == path)
        for wrong in [raw.replace(tokens, "", 1), raw.replace(tokens, tokens+tokens, 1), raw.replace(tokens, tokens.replace("moderate_tests.rs", "foreign.rs"), 1)]:
            rejects(lambda: predecessor(cut, wrong.encode())); negatives += 1
        for key in ["required", "maximum"]:
            for child in new_paths:
                assert allowed[path][key].pop(child) == 1
    assert allowed == old_allowed
    # Exercise the original whole-guard fragment grammar without endpoint writes.
    gp = ROOT / "docs/evidence/main043-independent-custody-20261016/guard-extensions.py"
    spec = importlib.util.spec_from_file_location("moderate_guard_grammar", gp)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    for row in CUTS["changed"]:
        if row["path"] not in old_allowed: continue
        path = row["path"]; current = (ROOT/path).read_text()
        original = subprocess.check_output(["git", "show", "a82e00cc8:"+path], cwd=ROOT).decode()
        fragments = module.fragments(path, current, original)
        restored = current
        for fragment in fragments: restored = restored.replace(fragment, "", 1)
        assert restored == original
    code = (ROOT/new_paths[0]).read_text(); data=(HERE/"joined-inputs.json").read_bytes()
    assert f"const JOINED_LENGTH: usize = {len(data)};" in code
    assert f'const JOINED_SHA256: &str = "{sha(data)}";' in code
    assert len(re.findall(r"#\[test\]", code)) == 4
    assert len((ROOT/"crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs").read_text().splitlines()) == 1500
    print(json.dumps({"status":"PASS_SOURCE_ONLY", "complete_changed_preimages":len(CUTS["changed"]),
      "whole_incoming_domains":3,"complete_unchanged_original_tables":2,"whole_original_moderate_inverses":3,
      "old_moderate_assertion_calls":CUTS["original_assertion_calls"],"whole_joined_original_archives":len(CUTS["joined_originals"]),
      "negative_controls":negatives,"complete_raw_guards_restored":3,"new_pending_owners":4,"rust_executed":False}, sort_keys=True))

if __name__ == "__main__": main()
