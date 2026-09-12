#!/usr/bin/env python3
"""Offline, disposable simulation of `gh release create`/`gh release view` (#167).

This is NOT a GitHub client. It makes no network call, ever, and creates no
real GitHub Release. It exists only so `scripts/release-dry-run.py` and its
owning tests can exercise the create -> (possible crash before create) ->
retry -> published state machine that the real `publish-release` CI job
implements with the actual `gh release create` (see `.github/workflows/ci.yml`,
which this script does not touch, read, or stand in for), without hosted CI,
GitHub credentials, or a live repository.

A record this script writes is a fixture, never release-promotion evidence.
`docs/RELEASE-PROCESS.md`'s hosted-evidence sections and the real `gh` CLI
remain the only source of truth for whether anything is actually published;
nothing in this file may be cited as if it were.

The "store" is a single JSON file the caller owns (`--store PATH`), shaped as
`{"schema": "semaprax.release-publish-simulation.v1", "releases": {tag: {...}}}`.
`create` refuses to add a second entry for a tag that already has one -- the
same one-shot behavior a real `gh release create` has for an existing tag --
so a retried `create` after a simulated mid-publish crash can never produce a
duplicate release record or duplicate assets.
"""

import argparse
import hashlib
import json
import sys
from pathlib import Path

SCHEMA = "semaprax.release-publish-simulation.v1"


def reject(message):
    raise ValueError(message)


def sha256_digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def load_store(path):
    if not path.is_file():
        return {"schema": SCHEMA, "releases": {}}
    store = json.loads(path.read_text(encoding="utf-8"))
    if store.get("schema") != SCHEMA:
        reject(f"{path} is not a {SCHEMA} store")
    store.setdefault("releases", {})
    return store


def save_store(path, store):
    path.write_text(json.dumps(store, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def create_release(store, tag, commit, manifest, notes, asset_paths):
    """Simulate `gh release create`: validate, then add exactly one entry.

    Refuses (raises) rather than overwriting when `tag` already has an entry,
    when the manifest disagrees with the requested tag/commit, when an asset
    is not in the manifest's artifact inventory, or when an asset's actual
    bytes disagree with the manifest's recorded size/digest for it. Every
    manifest artifact must be matched by exactly one supplied asset; a
    partially-uploaded release is refused rather than recorded as complete.
    """
    if tag in store["releases"]:
        reject(
            f"a release already exists for {tag} in this simulated store "
            "(refusing a duplicate, matching `gh release create`'s behavior "
            "against an existing tag)"
        )
    if manifest.get("tag") != tag:
        reject(f"manifest tag {manifest.get('tag')!r} does not match requested tag {tag!r}")
    if manifest.get("commit") != commit:
        reject(
            f"manifest commit {manifest.get('commit')!r} does not match "
            f"requested commit {commit!r}"
        )
    manifest_artifacts = {entry["name"]: entry for entry in manifest.get("artifacts", [])}
    assets = []
    seen = set()
    for asset_path in asset_paths:
        name = asset_path.name
        if name in seen:
            reject(f"asset {name} was supplied more than once")
        seen.add(name)
        entry = manifest_artifacts.get(name)
        if entry is None:
            reject(f"asset {name} is not in the release manifest's artifact inventory")
        data = asset_path.read_bytes()
        digest = sha256_digest(data)
        if digest != entry.get("digest"):
            reject(
                f"asset {name} digest {digest} disagrees with manifest digest "
                f"{entry.get('digest')}"
            )
        if len(data) != entry.get("size"):
            reject(
                f"asset {name} size {len(data)} disagrees with manifest size "
                f"{entry.get('size')}"
            )
        assets.append({"name": name, "size": len(data), "digest": digest})
    missing = sorted(set(manifest_artifacts) - seen)
    if missing:
        reject(f"release is missing manifest artifact(s): {missing}")
    store["releases"][tag] = {
        "tag": tag,
        "commit": commit,
        "prerelease": bool(manifest.get("prerelease", True)),
        "notes": notes,
        "assets": sorted(assets, key=lambda asset: asset["name"]),
    }
    return store


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    create = subparsers.add_parser("create", help="simulate `gh release create`")
    create.add_argument("--store", required=True, type=Path)
    create.add_argument("--tag", required=True)
    create.add_argument("--commit", required=True)
    create.add_argument("--manifest", required=True, type=Path)
    create.add_argument("--notes-file", required=True, type=Path)
    create.add_argument("assets", nargs="+", type=Path)

    view = subparsers.add_parser("view", help="simulate `gh release view --json`")
    view.add_argument("--store", required=True, type=Path)
    view.add_argument("--tag", required=True)

    args = parser.parse_args(argv)

    if args.command == "create":
        store = load_store(args.store)
        manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
        notes = args.notes_file.read_text(encoding="utf-8")
        if not notes.strip():
            reject("release notes file is empty")
        store = create_release(store, args.tag, args.commit, manifest, notes, args.assets)
        save_store(args.store, store)
        asset_count = len(store["releases"][args.tag]["assets"])
        print(
            f"release publish simulation: created {args.tag} with {asset_count} asset(s)"
        )
        return 0

    if args.command == "view":
        store = load_store(args.store)
        release = store["releases"].get(args.tag)
        if release is None:
            print(
                f"release publish simulation: no simulated release for {args.tag}",
                file=sys.stderr,
            )
            return 1
        print(json.dumps(release, indent=2, sort_keys=True))
        return 0

    reject(f"unknown command {args.command!r}")
    return 2


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print(f"release publish simulation rejected: {error}", file=sys.stderr)
        sys.exit(2)
