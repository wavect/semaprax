"""Verify every preserved byte and independently authenticate all digest bindings."""
from pathlib import Path
import copy
import hashlib
import json

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def wire(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode()


def sha(raw):
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def verify_digests(value, graph):
    if isinstance(value, list):
        for child in value:
            verify_digests(child, graph)
    elif isinstance(value, dict):
        for child in value.values():
            verify_digests(child, graph)
        if "workspace_graph_digest" in value:
            assert value["workspace_graph_digest"] == graph
        schema = value.get("schema", "")
        if schema in {
            "semaprax.workspace-semantic-graph.v1",
            "semaprax.workspace-semantic-context.v1",
            "semaprax.workspace-semantic-impact.v1",
            "semaprax.workspace-semantic-review.v1",
        }:
            key = "graph_digest" if schema.endswith("graph.v1") else "artifact_digest"
            payload = wire({k: v for k, v in value.items() if k != key})
            domain = (schema[:-3] + ".artifact-digest.v1\0").encode()
            assert value[key] == sha(domain + len(payload).to_bytes(8, "little") + payload)


def allowed(path, before, after, host):
    if path.endswith(("/graph_digest", "/artifact_digest", "/workspace_graph_digest")):
        return isinstance(before, str) and isinstance(after, str)
    if path.endswith("/limits/max_builder_bytes") or path.endswith("/limits/workspace/max_builder_bytes"):
        return before == 67108864 and after == 134217728
    if path.endswith("/budget/used_builder_bytes") or path.endswith("/budget/workspace/used_builder_bytes"):
        return (before, after) == ((926453, 732686) if host == "macos" else (926309, 732542))
    # A decimal digit added to the already-existing builder limit changes the
    # serialized graph and nested report lengths. No payload bytes are erased.
    return path.endswith((
        "/budget/used_output_bytes", "/budget/workspace/used_output_bytes",
        "/budget/analysis/used_output_bytes", "/budget/analysis/used_context_bytes",
        "/budget/analysis/used_impact_bytes",
    )) and isinstance(before, int) and after > before


proof = json.loads((HERE / "proof.json").read_text())
assert len(proof["rows"]) == 18
assert len({(r["host"], r["name"]) for r in proof["rows"]}) == 18
for row in proof["rows"]:
    oldraw = (ROOT / row["old"]).read_bytes()
    raw = (HERE / row["current"]).read_bytes()
    assert sha(oldraw) == row["old_sha256"]
    assert sha(raw) == row["current_sha256"]
    old, current = json.loads(oldraw), json.loads(raw)
    assert wire(old) == oldraw and wire(current) == raw
    for kind, document in [("old", old), ("current", current)]:
        graph_row = next(r for r in proof["rows"] if r["host"] == row["host"] and r["name"] == "public-graph")
        graph_path = ROOT / graph_row["old"] if kind == "old" else HERE / graph_row["current"]
        graph = json.loads(graph_path.read_bytes())["graph_digest"]
        verify_digests(document, graph)
    transformed = copy.deepcopy(old)
    for path, before, after in row["changes"]:
        assert allowed(path, before, after, row["host"]), path
        parts = path.strip("/").split("/")
        at = transformed
        for part in parts[:-1]:
            at = at[part]
        assert at[parts[-1]] == before
        at[parts[-1]] = after
    assert wire(transformed) == raw, (row["name"], row["host"])
print("PASS: 18 complete wire transitions; all semantic fields unchanged; all nested digests verified")
