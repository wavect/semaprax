#!/usr/bin/env python3
"""Generate separate diagnostic fixtures; preserve every original test body."""
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / ".wire-diagnostic"
TARGETS = [
    ("src/semantic_workspace_change/artifact/tests.rs", "literal_kats_wire_order_domains_and_reference_parity"),
    ("src/semantic_workspace_structural_change/artifact.rs", "whole_document_kats_keys_domains_refs_paths_and_fixed_point_are_exact"),
    ("src/semantic_workspace_structural_change/tests.rs", "managed_generate_and_verify_are_exact_read_only_kats_under_one_shared_lock"),
    ("src/semantic_workspace_structural_change/tests.rs", "structural_apply_publishes_exact_candidate_once_without_raw_writes"),
    ("src/semantic_workspace_operations/tests.rs", "authenticated_derivation_kat_binds_refs_budget_nonclaims_and_build_counts"),
    ("src/semantic_workspace_operations/tests.rs", "operations_evidence_and_verification_are_exact_one_build_kats"),
    ("src/semantic_workspace_operations/tests.rs", "operations_apply_is_exact_stale_and_zero_write_before_replay"),
    ("src/workspace_graph/type_proof_tests.rs", "rendered_document_has_literal_sha_exact_wire_order_and_digest_binding"),
    ("src/workspace_analysis/tests.rs", "context_impact_and_review_documents_have_frozen_kats_and_exact_digest_replay"),
]
def digest(raw):
    return hashlib.sha256(raw).hexdigest()
def skip(text, at):
    if text.startswith("//", at):
        end = text.find("\n", at)
        return len(text) if end < 0 else end
    if text.startswith("/*", at):
        depth, end = 1, at + 2
        while depth:
            if text.startswith("/*", end):
                depth += 1
                end += 2
            elif text.startswith("*/", end):
                depth -= 1
                end += 2
            else:
                end += 1
            assert end <= len(text)
        return end
    raw = re.match(r'(?:b?r)(#*)"', text[at:])
    if raw:
        suffix = '"' + raw[1]
        return text.index(suffix, at + len(raw[0])) + len(suffix)
    if text[at] == '"':
        end = at + 1
        while True:
            if text[end] == "\\":
                end += 2
            elif text[end] == '"':
                return end + 1
            else:
                end += 1
    if text[at] == "'":
        char = re.match(r"'(?:\\(?:u\{[0-9a-fA-F]+\}|x[0-9a-fA-F]{2}|.)|[^'\n])'", text[at:])
        if char:
            return at + len(char[0])
    return at
def close(text, at):
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack = [pairs[text[at]]]
    at += 1
    while stack:
        end = skip(text, at)
        if end != at:
            at = end
            continue
        if text[at] in pairs:
            stack.append(pairs[text[at]])
        elif text[at] in ")]}":
            assert text[at] == stack.pop()
        at += 1
    return at
def first_argument(text, opening):
    at = opening + 1
    while True:
        end = skip(text, at)
        if end != at:
            at = end
            continue
        if text[at] in "([{":
            at = close(text, at)
            continue
        if text[at] in ",)":
            return text[opening + 1:at]
        at += 1
def function(text, name):
    matches = list(re.finditer(r"\bfn\s+" + re.escape(name) + r"\s*\(\s*\)\s*\{", text))
    assert len(matches) == 1, name
    start = matches[0].start()
    opening = matches[0].end() - 1
    return start, close(text, opening)

assert not OUT.exists(), "refuse to overwrite an existing diagnostic directory"
if os.environ.get("GITHUB_SHA"):
    assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() == os.environ["GITHUB_SHA"]
    subprocess.run(["git", "diff", "--quiet", "--", "src", "std", "Cargo.toml", "Cargo.lock"], cwd=ROOT, check=True)
OUT.mkdir()
originals = {path: (ROOT / path).read_bytes() for path, _ in TARGETS}
insertions = {}
records = []
for index, (path, name) in enumerate(TARGETS):
    original = originals[path].decode()
    start, end = function(original, name)
    body = original[start:end]
    edits = []
    at = 0
    while at < len(body):
        following = skip(body, at)
        if following != at:
            at = following
            continue
        if body.startswith("assert_eq!(", at):
            opening = at + len("assert_eq!")
            finish = close(body, opening)
            actual = first_argument(body, opening)
            if actual.strip() == "document_sha" or re.search(r"\b(?:raw_sha(?:256)?|document_sha)\s*\(|\.derivation_digest\(", actual):
                observation = "let _ = " + actual.strip()
                if ".derivation_digest()" in actual:
                    source = actual.strip().replace(".derivation_digest()", ".derivation().as_bytes()")
                    observation = '{ crate::source_reflows::record_wire_fixture("operations-derivation", ' + source + '); ' + observation + '; }'
                edits.append((at, finish, observation))
            at = finish
        else:
            at += 1
    assert edits, name
    replica = body
    for a, b, replacement in reversed(edits):
        replica = replica[:a] + replacement + replica[b:]
    replica = replica.replace("fn " + name, "fn capture_complete_" + name, 1)
    replica = '#[test]\n#[ignore = "diagnostic fixture capture only; original literal gates remain unchanged"]\n' + replica + "\n"
    generated = OUT / f"fixture-{index}.rs"
    generated.write_bytes(replica.encode())
    # Replace an existing blank line, preserving the source-file line budget.
    blank = end
    while blank < len(original) and original[blank] in " \t":
        blank += 1
    assert original[blank:blank+2] == "\n\n", (path, name)
    include = '\ninclude!(concat!(env!("CARGO_MANIFEST_DIR"), "/.wire-diagnostic/' + generated.name + '"));\n'
    insertions.setdefault(path, []).append((blank, blank + 2, include))
    records.append(dict(source=path, original_test=name, capture_test="capture_complete_" + name,
                        original_body_sha256=digest(body.encode()), observations=len(edits),
                        generated=generated.name, generated_sha256=digest(replica.encode())))
for path, edits in insertions.items():
    text = originals[path].decode()
    for a, b, new in sorted(edits, reverse=True):
        text = text[:a] + new + text[b:]
    assert len(text.splitlines()) == len(originals[path].splitlines()), path
    for item in [x for x in records if x["source"] == path]:
        a, b = function(text, item["original_test"])
        assert digest(text[a:b].encode()) == item["original_body_sha256"]
    (ROOT / path).write_bytes(text.encode())
files = []
for index, (path, original) in enumerate(originals.items()):
    backup = f"backup-{index}.rs"
    (OUT / backup).write_bytes(original)
    files.append(dict(source=path, original_sha256=digest(original),
                      diagnostic_sha256=digest((ROOT / path).read_bytes()), backup=backup))
(OUT / "provenance.json").write_text(json.dumps({
    "schema": "semaprax.fixture-capture-source.v1",
    "qualification": "diagnostic fixtures only; every original test body and file line budget preserved",
    "files": files, "fixtures": records,
}, indent=2) + "\n")
print("Prepared", len(records), "separate data-capture fixtures; original assertions remain intact.")
