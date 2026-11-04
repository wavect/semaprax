"""Authenticate complete report bodies and their independently captured references."""
from pathlib import Path
import hashlib
import json

HERE = Path(__file__).resolve().parent
MANIFEST = json.loads((HERE / "manifest.json").read_text())


def sha(raw):
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def read(fixture, host, suffix):
    prefix = "semaprax.workspace-semantic-" + ("structural-" if fixture == "structural" else "") + "change-"
    schema = prefix + suffix + ".v1"
    row = next(r for r in MANIFEST["rows"] if (r["fixture"], r["host"], r["schema"]) == (fixture, host, schema))
    raw = (HERE / row["path"]).read_bytes()
    assert sha(raw) == row["sha256"]
    value = json.loads(raw)
    assert value["schema"] == schema
    assert json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode() + b"\n" == raw
    return raw, value


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
        # The complete real reports differ only in references to the native
        # graph digests and their dependent report digests. All semantic and
        # resource-accounting fields must remain byte-for-byte equal.
        assert path.endswith("/digest"), path
        assert isinstance(left, str) and left.startswith("sha256:")
        assert isinstance(right, str) and right.startswith("sha256:")


assert len(MANIFEST["rows"]) == 28
for fixture in ["change", "structural"]:
    preview_key = "structural_change_preview" if fixture == "structural" else "change_preview"
    evidence_key = "workspace_structural_change_evidence" if fixture == "structural" else "workspace_change_evidence"
    for host in ["macos", "portable"]:
        _, receipt = read(fixture, host, "evidence-verification")
        _, applied = read(fixture, host, "evidence-application")
        for suffix, key in [("preview", preview_key), ("context", "context"), ("impact", "impact"), ("review", "review"), ("evidence", evidence_key)]:
            raw, value = read(fixture, host, suffix)
            domain = (value["schema"][:-3] + ".artifact-digest.v1\0").encode()
            digest = sha(domain + raw)
            assert receipt[key]["digest"] == digest, (fixture, host, suffix)
            assert applied[key]["digest"] == digest
            assert receipt[key]["bytes"] == len(raw)
            assert applied[key]["bytes"] == len(raw)
            for name in ["base_workspace_graph", "candidate_workspace_graph"]:
                if name in value:
                    assert value[name] == receipt[name]
        for suffix in ["preview", "context", "impact", "review", "evidence", "evidence-verification", "evidence-application"]:
            compare(read(fixture, "macos", suffix)[1], read(fixture, "portable", suffix)[1])
print("PASS: all 28 full wires, 20 report digest anchors, all receipt lengths, complete cross-host semantic equality")
