#!/usr/bin/env python3
"""Verify all prior hooks, complete receivers, and every archived patch path."""
from pathlib import Path
import argparse
import json
import os
import re
import subprocess
import tempfile
from source_tools import Git, CACHE_BYTES, digest, sha

parser = argparse.ArgumentParser()
parser.add_argument("--basis", required=True)
args = parser.parse_args()
ROOT = Path.cwd()
HERE = Path(__file__).resolve().parent
BASE = args.basis
PRIOR_BASIS = "ea0be15f4dea6f3db36a031e17988e9fa105dbf5"
PRIOR = "docs/evidence/incremental-source-composition-d39fe67-20261015"
REVERSE = {"loan-candidate-inventory-reuse", "exit-status-directory-reuse",
           "referenced-edge-ownership-proof", "prefix-certified-expression-lookup",
           "receiving-transitive-results-816", "exit-prefix-reference-proof",
           "status-fact-borrow-reuse", "constructor-inline-failure-825",
           "default-static-error-custody-825", "prelude-forecast-286", "pin-capture",
           "region-entry-receiving-816"}
assert re.fullmatch(r"[0-9a-f]{40}", BASE)
git = Git(ROOT)


def read(path):
    return json.loads((ROOT / path).read_bytes())


def rows(packet):
    return packet if isinstance(packet, list) else packet.get("whole_sources", packet.get("owners", [packet]))


def blob(revision, path):
    value = git.get(revision, path)
    assert value is not None, (revision, path)
    return value


def apply_whole(current, edit):
    new, old = edit["new"].encode(), edit["old"].encode()
    assert current == new, "join must authenticate the complete current source"
    return old


try:
    proof = read(str(HERE.relative_to(ROOT)) + "/combined-source-proof.json")
    assert proof["immutable_basis"] == BASE
    oldproof = json.loads(blob(PRIOR_BASIS, PRIOR + "/combined-source-proof.json"))
    prior_names = [entry["packet"] for entry in oldproof["packets"]]
    assert len(prior_names) == 90 and sum(p["rows"] for p in oldproof["packets"]) == 795
    assert [p["packet"] for p in proof["packets"][:90]] == prior_names
    histories = read(str(HERE.relative_to(ROOT)) + "/next-packet-histories.json")
    seeds = {entry["packet"]: entry for entry in histories}
    inventory_path = "docs/evidence/nominal-payload-827/source-inventory-activation-v14.json"
    old_inventory = json.loads(blob(PRIOR_BASIS, inventory_path))
    nominal = {row["source"]: row for row in old_inventory["rows"]}
    current_inventory = read(inventory_path)
    assert len(current_inventory["rows"]) == len(old_inventory["rows"])
    for old, current in zip(old_inventory["rows"], current_inventory["rows"]):
        assert current["source"] == old["source"]
        assert current["sha256"] == digest(ROOT / current["source"])
        for field, value in old.items():
            if field != "sha256":
                assert current[field] == value, (inventory_path, old["source"], field)
    oldhooks = checkpoints = 0
    independent_joins = {(r["packet"], r["source"]): r for r in proof["new_source_prior_bindings"]}
    for packet in proof["packets"]:
        name = packet["packet"]
        now = read(packet.get("active_packet", name))
        if name in seeds:
            info = seeds[name]
            assert digest(ROOT / info["seed"]) == info["seed_sha256"]
            old = read(info["seed"])
        else:
            old = json.loads(blob(PRIOR_BASIS if name in prior_names else BASE, name))
        before, after = rows(old), rows(now)
        assert len(before) == len(after) == packet["rows"], name
        repair = {entry["source"]: entry for entry in packet["repairs"]}
        reverse = Path(name).parent.name in REVERSE
        for original, current in zip(before, after):
            source = original.get("source", original.get("file", original.get("path")))
            key = "inverse" if "inverse" in original else "hooks" if "hooks" in original else "edits"
            prior, active = original.get(key, []), current.get(key, [])
            assert len(active) in (len(prior), len(prior) + 1), (name, source)
            if source in repair:
                entry = repair[source]
                previous = blob(entry["checkpoint"], source)
                assert sha(previous) == entry["checkpoint_sha256"]
                extra = active[-1] if reverse else active[0]
                nk = "current" if "current" in extra else "after" if "after" in extra else "new"
                bk = "previous" if "previous" in extra else "old" if "old" in extra else "before"
                assert extra[nk].encode() == (ROOT / source).read_bytes()
                assert extra[bk].encode() == previous
                assert (active[:-1] if reverse else active[1:]) == prior
                checkpoints += 1
            else:
                assert active == prior
            mutable = {key, "current_sha256", "current_bytes"}
            bound = independent_joins.get((name, source))
            if bound is not None:
                assert not prior and (original.get("new_file") or nominal.get(source, {}).get("new_file"))
                assert sha(blob(bound["prior_revision"], source)) == bound["prior_sha256"]
                assert sha((ROOT / source).read_bytes()) == bound["current_sha256"]
                mutable.add("sha256")
            for field, value in original.items():
                if field not in mutable:
                    assert current[field] == value, (name, source, field)
            oldhooks += len(prior)
        if isinstance(old, dict):
            for field, value in old.items():
                if field not in {"whole_sources", "owners", "children"} and old not in before:
                    assert now[field] == value, (name, field)
            assert len(old.get("children", [])) == len(now.get("children", []))
            for oldchild, child in zip(old.get("children", []), now.get("children", [])):
                assert oldchild["source"] == child["source"]
                assert sha((ROOT / child["source"]).read_bytes()) == child["sha256"]
                if child["sha256"] != oldchild["sha256"]:
                    appended = child["prior_bindings"][-1]
                    assert appended["sha256"] == oldchild["sha256"]
                    assert sha(blob(appended["revision"], child["source"])) == oldchild["sha256"]
                    assert child["prior_bindings"][:-1] == oldchild.get("prior_bindings", [])
                for field, value in oldchild.items():
                    if field not in {"sha256", "prior_bindings"}:
                        assert child[field] == value
    for source, expected in proof["preserved_prior_proofs_and_scripts"].items():
        assert digest(ROOT / source) == expected, source
    for source, expected in oldproof["preserved_prior_proofs_and_scripts"].items():
        assert digest(ROOT / source) == expected, (source, "immutable prior proof/archive/script")
    for source, expected in proof["protected_runtime_stage_packets"].items():
        assert (ROOT / source).read_bytes() == blob(BASE, source)
        assert digest(ROOT / source) == expected
    for source, expected in proof["source_inputs"].items():
        assert digest(ROOT / source) == expected
    frozen = ["*.rs", "*.spx", "Cargo.toml", "Cargo.lock", "**/Cargo.toml", "**/Cargo.lock", ".cargo/*"]
    assert not subprocess.check_output(["git", "diff", "--name-only", BASE, "--", *frozen], cwd=ROOT).strip()
    allowed = {row["packet"] for row in proof["packets"]} | {"docs/evidence/nominal-payload-827/source-inventory-activation-v14.json"}
    tracked = subprocess.check_output(["git", "diff", "--name-only", BASE], cwd=ROOT, text=True).splitlines()
    assert not {path for path in tracked if path not in allowed and not path.startswith(str(HERE.relative_to(ROOT)) + "/")}

    # Every old receiver remains a full independent source. MAIN formatting may
    # require an explicit whole-source join; it never changes the old manifests.
    custody = read(str(HERE.relative_to(ROOT)) + "/independent-receiver-bindings.json")
    assert custody["source_basis"] == BASE and custody["receiver_count"] == 53
    manifests = ["docs/evidence/opt836-field-update/manifest.json",
                 "docs/evidence/record-copy-field-update-836-backends/unchanged-originals.json"]
    expected_receivers = []
    for manifest in manifests:
        raw = (ROOT / manifest).read_bytes()
        assert raw == blob(PRIOR_BASIS, manifest)
        records = json.loads(raw)
        if isinstance(records, dict):
            records = records["complete_prior_independent_receivers_unchanged"]
        expected_receivers += [(manifest, row) for row in records]
    assert len(expected_receivers) == len(custody["records"]) == 53
    unchanged = joined = 0
    for (manifest, expected), row in zip(expected_receivers, custody["records"]):
        source = expected.get("path", expected.get("source"))
        assert row["source"] == source and row["manifest"] == manifest
        current = (ROOT / source).read_bytes()
        assert sha(current) == row["current_sha256"]
        previous = blob(row["checkpoint_revision"], source)
        assert sha(previous) == row["original_sha256"] == expected["sha256"]
        assert len(row["edits"]) <= 1
        for edit in row["edits"]:
            current = apply_whole(current, edit)
        assert current == previous
        if "archive" in expected:
            assert previous == (ROOT / expected["archive"]).read_bytes()
        if row["edits"]:
            joined += 1
            assert row["disposition"] == "complete_authenticated_current_to_old_join"
        else:
            unchanged += 1
            assert row["disposition"] == "unchanged"

    # Preserve every old starter inverse, not just its final target hash.
    prior_starters = json.loads(blob(PRIOR_BASIS, PRIOR + "/starter-current-bindings.json"))
    starters = read(str(HERE.relative_to(ROOT)) + "/starter-current-bindings.json")
    assert len(prior_starters) == len(starters) == 38
    for old, row in zip(prior_starters, starters):
        current = (ROOT / row["source"]).read_bytes()
        assert sha(current) == row["current_sha256"]
        edits = row["current_to_authored"]
        before = old["current_to_authored"]
        assert edits == before or edits[1:] == before
        for field, value in old.items():
            if field not in {"current_sha256", "current_to_authored"}:
                assert row[field] == value
        for edit in edits:
            assert current.count(edit["new"].encode()) == 1
            current = current.replace(edit["new"].encode(), edit["old"].encode(), 1)
        assert current == blob(row["authored_revision"], row["source"])
        assert sha(current) == row["authored_sha256"]
    layout = (ROOT / "src/workspace_graph/expected_projection/private_layout.rs").read_text()
    assert layout.count("        let capacity = capacity.min(count);") == 2
    for module in ["capacity_before", "capacity_tests", "before_registry"]:
        assert "mod " + module + ";" in layout
        assert (ROOT / ("src/workspace_graph/expected_projection/private_layout/" + module + ".rs")).is_file()

    def verify_patch(label, revision, base, path, archived, complete_commit=False):
        patch = (ROOT / path).read_bytes()
        if complete_commit:
            assert patch == subprocess.check_output(["git", "diff", "--binary", base, revision], cwd=ROOT)
        else:
            assert patch == blob(PRIOR_BASIS, path), "immutable full archived starter patch"
        paths = re.findall(rb"^diff --git a/(.+) b/(.+)$", patch, re.M)
        assert all(left == right for left, right in paths)
        with tempfile.TemporaryDirectory(prefix=".patch-index-", dir=HERE) as temp:
            env = dict(os.environ, GIT_INDEX_FILE=str(Path(temp) / "index"))
            subprocess.run(["git", "read-tree", revision], cwd=ROOT, env=env, check=True)
            subprocess.run(["git", "apply", "--cached", "--reverse", "--unidiff-zero", str(ROOT / path)], cwd=ROOT, env=env, check=True)
            for _, encoded in paths:
                source = encoded.decode()
                projected = subprocess.run(["git", "show", ":" + source], cwd=ROOT, env=env,
                                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                previous = git.get(base, source)
                if previous is None:
                    assert projected.returncode != 0 and not projected.stdout
                else:
                    assert projected.returncode == 0 and projected.stdout == previous, (label, source)
                if source in archived:
                    assert archived[source].read_bytes() == (b"" if previous is None else previous)
        return dict(owner=label, authored_revision=revision, original_base=base,
                    all_patch_sources_equal=True, patch_paths=len(paths), archived_files=len(archived))

    patches = []
    folder = ROOT / "docs/evidence/application-authoring-batch-836-837-20261015"
    for child, revision in [("starter-original", "604e7fb05203a6701be6bdbcda8a181c5b969df2"),
                            ("starter-held-input-fix", "2ae6fee203fb726980524885ec36da268d5361bd")]:
        directory = folder / child
        manifest = json.loads((directory / "immediate-base-full-source.json").read_bytes())
        originals = {}
        for row in manifest["files"]:
            value = (directory / "old-source" / row["path"]).read_bytes()
            assert len(value) == row["bytes"] and sha(value) == row["sha256"]
            assert value == blob(row["base"], row["path"])
            originals[row["path"]] = directory / "old-source" / row["path"]
        patches.append(verify_patch(child, revision, manifest["base"],
                                   str((directory / "immediate-source-inverse.patch").relative_to(ROOT)), originals))
    assert [p["patch_paths"] for p in patches] == [32, 6]
    next_patches = []
    for history in histories:
        assert digest(ROOT / history["seed"]) == history["seed_sha256"]
        assert digest(ROOT / history["complete_patch"]) == history["patch_sha256"]
        seed = read(history["seed"])
        originals = {row["source"]: ROOT / row["archive"] for row in seed}
        assert list(originals) == history["complete_patch_paths"]
        result = verify_patch(history["label"], history["authored_revision"], history["original_base"], history["complete_patch"], originals, complete_commit=True)
        assert result["patch_paths"] == history["rows"]
        next_patches.append(result)
    assert len(next_patches) == 4 and sum(p["patch_paths"] for p in next_patches) == 66

    # Preserve the local Source36 JSON addition inverse separately; it is not
    # recursively inserted as an active source row containing its own manifest.
    metadata = read("docs/evidence/source36-after-next-838-839-20261015/metadata-history/local-metadata-inverse.json")
    current = (ROOT / metadata["source"]).read_bytes()
    assert sha(current) == metadata["current_sha256"]
    for edit in metadata["edits"]:
        current = apply_whole(current, edit)
    assert current == (ROOT / metadata["archive"]).read_bytes() == b""
    assert sha(current) == metadata["original_sha256"]
    assert git.get(metadata["checkpoint_revision"], metadata["source"]) is None
    result = dict(status="SOURCE_ONLY_NO_RUST_EXECUTION", immutable_basis=BASE,
                  prior_active_packets=90, prior_active_rows=795, old_hooks_preserved=oldhooks,
                  authenticated_added_checkpoint_rows=checkpoints,
                  all_source_frozen_at=BASE, independent_receivers_verified=53,
                  independent_receivers_byte_unchanged=unchanged,
                  independent_receiver_complete_current_to_old_joins=joined,
                  starter_complete_patch_source_inverses=patches, starter_current_bindings=38,
                  next_complete_patch_source_inverses=next_patches, next_complete_patch_paths=66,
                  source36_local_metadata_addition_inverse=True,
                  capacity834_original_and833_receiving_bound_retained=True,
                  candidate_cache_limit_bytes=CACHE_BYTES, candidate_cache_peak_bytes=git.peak)
    (HERE / "joined-source-proof.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
finally:
    git.close()
