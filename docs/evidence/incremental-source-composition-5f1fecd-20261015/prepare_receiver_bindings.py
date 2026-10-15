#!/usr/bin/env python3
"""Prepare whole authenticated current-to-original joins for all 53 old receivers."""
from pathlib import Path
import argparse
import json
import re
import subprocess
from source_tools import Git, sha

parser = argparse.ArgumentParser()
parser.add_argument("--basis", required=True)
args = parser.parse_args()
assert re.fullmatch(r"[0-9a-f]{40}", args.basis)
root = Path.cwd()
here = Path(__file__).resolve().parent
assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip() == args.basis
prior = "ea0be15f4dea6f3db36a031e17988e9fa105dbf5"
manifests = ["docs/evidence/opt836-field-update/manifest.json",
             "docs/evidence/record-copy-field-update-836-backends/unchanged-originals.json"]
git = Git(root)
rows = []
try:
    for manifest in manifests:
        raw = (root / manifest).read_bytes()
        assert raw == git.get(prior, manifest), (manifest, "immutable receiver manifest")
        records = json.loads(raw)
        if isinstance(records, dict):
            records = records["complete_prior_independent_receivers_unchanged"]
        for record in records:
            source = record.get("path", record.get("source"))
            current = (root / source).read_bytes()
            expected = record["sha256"]
            checkpoint = None
            original = None
            for revision in [prior, "6e549d1ae", "5f1fecd42694c52fbdcec67f9ef3d3fd0b7d7f3b",
                             "d39fe676a7c1df3323888b25c9481f7bbb65feaa"]:
                candidate = git.get(revision, source)
                if candidate is not None and sha(candidate) == expected:
                    checkpoint, original = revision, candidate
                    break
            if original is None:
                revisions = subprocess.check_output(["git", "log", "--all", "--format=%H", "--", source], cwd=root, text=True).splitlines()
                for revision in revisions:
                    candidate = git.get(revision, source)
                    if candidate is not None and sha(candidate) == expected:
                        checkpoint, original = revision, candidate
                        break
            assert original is not None, (source, "complete immutable old receiver missing", expected)
            if "archive" in record:
                assert (root / record["archive"]).read_bytes() == original
            edits = [] if current == original else [dict(new=current.decode(), old=original.decode())]
            rows.append(dict(source=source, manifest=manifest, current_sha256=sha(current),
                             checkpoint_revision=checkpoint, original_sha256=expected, edits=edits,
                             disposition="unchanged" if not edits else "complete_authenticated_current_to_old_join"))
    assert len(rows) == 53, "Every original independent receiver is mandatory"
    target = here / "independent-receiver-bindings.json"
    assert not target.exists(), "Preserve earlier source-basis binding outputs"
    target.write_text(json.dumps(dict(source_basis=args.basis, immutable_manifest_basis=prior,
                                     receiver_count=53, records=rows), indent=2) + "\n")
    print(json.dumps(dict(receivers=53, unchanged=sum(not r["edits"] for r in rows),
                          complete_current_to_old_joins=sum(bool(r["edits"]) for r in rows),
                          cache_peak_bytes=git.peak, runtime_execution=False)))
finally:
    git.close()
