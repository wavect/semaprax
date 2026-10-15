"""Authenticate new source joins while retaining all 38 original starter inverses."""
from pathlib import Path
import argparse
import copy
import hashlib
import json
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("--basis", required=True)
args = parser.parse_args()
root = Path.cwd()
output = Path(__file__).resolve().parent
prior_path = "docs/evidence/incremental-source-composition-d39fe67-20261015/starter-current-bindings.json"
sha = lambda value: hashlib.sha256(value).hexdigest()


def blob(revision, path):
    return subprocess.check_output(["git", "show", revision + ":" + path], cwd=root)


head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
assert head == args.basis
prior_raw = blob(args.basis, prior_path)
assert (root / prior_path).read_bytes() == prior_raw
rows = json.loads(prior_raw)
assert len(rows) == 38
changes = []
for row in rows:
    old = copy.deepcopy(row)
    current = (root / row["source"]).read_bytes()
    if sha(current) != row["current_sha256"]:
        checkpoint = "d39fe676a7c1df3323888b25c9481f7bbb65feaa"
        previous = blob(checkpoint, row["source"])
        assert sha(previous) == row["current_sha256"], row["source"]
        row["current_to_authored"] = [
            {"new": current.decode(), "old": previous.decode()}
        ] + row["current_to_authored"]
        row["current_sha256"] = sha(current)
        assert row["current_to_authored"][1:] == old["current_to_authored"]
        changes.append({"owner": row["owner"], "source": row["source"],
                        "checkpoint": checkpoint, "previous_sha256": sha(previous),
                        "current_sha256": sha(current)})
    for key, value in old.items():
        if key not in ("current_sha256", "current_to_authored"):
            assert row[key] == value
    projected = current
    for edit in row["current_to_authored"]:
        new, before = edit["new"].encode(), edit["old"].encode()
        assert projected.count(new) == 1
        projected = projected.replace(new, before, 1)
    assert projected == blob(row["authored_revision"], row["source"])
    assert sha(projected) == row["authored_sha256"]
    assert sha((root / row["source"]).read_bytes()) == row["current_sha256"]
assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip() == head
assert (root / prior_path).read_bytes() == prior_raw
for filename, data in [
    ("starter-current-bindings.json", rows),
    ("starter-binding-transition-proof.json", {
        "status": "SOURCE_ONLY_NO_RUST_EXECUTION", "source_basis": head,
        "prior_binding_file": prior_path, "prior_binding_sha256": sha(prior_raw),
        "bindings_verified": len(rows), "all_old_inverse_hooks_retained": True,
        "authenticated_added_joins": changes,
    }),
]:
    target = output / filename
    assert not target.exists(), "Preserve prior proof outputs"
    target.write_text(json.dumps(data, indent=2) + "\n")
print(json.dumps({"bindings_verified": len(rows), "added_joins": len(changes),
                  "runtime_execution": False}))
