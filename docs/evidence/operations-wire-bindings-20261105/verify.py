#!/usr/bin/env python3
"""Authenticate all complete Operations wires and every receipt reference."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
MANIFEST = json.loads((ROOT / "manifest.json").read_bytes())
KINDS = {"derivation", "change", "child-evidence", "evidence", "verification", "application"}
def sha(raw):
    return "sha256:" + hashlib.sha256(raw).hexdigest()
def artifact(raw):
    schema = json.loads(raw)["schema"]
    if schema.endswith(("verification.v1", "application.v1")):
        return None
    suffix = ".proposal-digest.v1\0" if schema == "semaprax.workspace-semantic-change.v1" else ".artifact-digest.v1\0"
    return sha((schema[:-3] + suffix).encode() + raw)
rows = {}
for row in MANIFEST["rows"]:
    key = row["host"], row["kind"]
    assert key not in rows and key[0] in {"macos", "portable"} and key[1] in KINDS
    assert row["file"] == f"{key[0]}/{key[1]}.json"
    path = ROOT / row["file"]
    assert not path.is_symlink()
    raw = path.read_bytes()
    assert len(raw) == row["bytes"] and sha(raw) == row["raw_sha256"]
    assert artifact(raw) == row["artifact_digest"]
    assert raw.endswith(b"\n") and not raw.endswith(b"\n\n")
    rows[key] = raw, json.loads(raw)
assert len(rows) == 12
for host in ["macos", "portable"]:
    child, child_value = rows[host, "child-evidence"]
    evidence, evidence_value = rows[host, "evidence"]
    assert evidence_value["workspace_change_evidence"]["document"] == child.decode()
    for value in [evidence_value, rows[host, "verification"][1], rows[host, "application"][1]]:
        for kind, field in [("derivation", "operations_derivation"), ("change", "derived_workspace_change_proposal"),
                            ("child-evidence", "workspace_change_evidence")]:
            raw = rows[host, kind][0]
            assert value[field]["digest"] == artifact(raw)
            assert value[field]["bytes"] == len(raw)
        for key in ["base_workspace_revision", "candidate_workspace_revision", "entry_module"]:
            assert value[key] == child_value[key]
    for kind, result in [("verification", "exact_replay"), ("application", "applied")]:
        value = rows[host, kind][1]
        assert value["result"] == result
        assert value["workspace_operations_evidence"]["digest"] == artifact(evidence)
        assert value["workspace_operations_evidence"]["bytes"] == len(evidence)

def compare(left, right, path=""):
    assert type(left) is type(right), path
    if isinstance(left, dict):
        assert left.keys() == right.keys(), path
        for key in left:
            compare(left[key], right[key], path + "/" + key)
    elif isinstance(left, list):
        assert len(left) == len(right), path
        for index, (a, b) in enumerate(zip(left, right)):
            compare(a, b, path + "/" + str(index))
    elif left != right:
        if path.endswith("/document"):
            compare(json.loads(left), json.loads(right), path)
        elif path.endswith("/digest"):
            assert left.startswith("sha256:") and right.startswith("sha256:"), path
        else:
            delta = {"used_candidate_graph_builder_bytes": 144, "used_operations_builder_bytes": 576,
                     "used_analysis_builder_bytes": 288}
            assert "/budget/" in path and path.rsplit("/", 1)[1] in delta, path
            assert left - right == delta[path.rsplit("/", 1)[1]], path
for kind in KINDS:
    compare(rows["macos", kind][1], rows["portable", kind][1])
for kind in ["verification", "application"]:
    original = rows["macos", kind][0]
    expected = original
    for key in ["derivation", "child-evidence", "evidence"]:
        before = artifact(rows["macos", key][0]).encode()
        after = artifact(rows["portable", key][0]).encode()
        assert expected.count(before) == 1
        expected = expected.replace(before, after)
    assert expected == rows["portable", kind][0]
print("PASS: 12 complete wires, every receipt binding, exact three-reference reconstruction, complete semantic equality")
