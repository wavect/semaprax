#!/usr/bin/env python3
"""Partition every workspace test target without changing feature unification."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
SHARDS = ("unit", "integration-0", "integration-1", "integration-2")
TEST = ["cargo", "test", "--locked", "--workspace", "--all-features"]


def plan(metadata):
    members = set(metadata["workspace_members"])
    packages = [p for p in metadata["packages"] if p["id"] in members]
    if not members or {p["id"] for p in packages} != members:
        raise ValueError("incomplete workspace package inventory")
    targets = []
    seen = set()
    for package in packages:
        for target in package["targets"]:
            kind = target["kind"]
            # Do not silently omit a newly introduced example, benchmark, or
            # other target kind: extend this partition and its tests first.
            if kind not in (["lib"], ["bin"], ["test"]):
                raise ValueError(f"unrouted target kind: {kind}")
            key = (package["id"], kind[0], target["name"])
            if key in seen:
                raise ValueError(f"duplicate workspace target: {key}")
            seen.add(key)
            targets.append(dict(package=key[0], kind=key[1], name=key[2]))
    targets.sort(key=lambda t: (t["package"], t["kind"], t["name"]))
    # Cargo's --test selector applies to every selected workspace package.
    # Keep shared names together so each matching package target runs once.
    names = sorted({t["name"] for t in targets if t["kind"] == "test"})
    shards = [{
        "name": "unit",
        "command": TEST + ["--lib", "--bins"],
        "targets": [t for t in targets if t["kind"] != "test"],
    }]
    for index, name in enumerate(SHARDS[1:]):
        selected = names[index::len(SHARDS) - 1]
        shards.append({
            "name": name,
            "command": TEST + [arg for target in selected for arg in ("--test", target)],
            "targets": [t for t in targets if t["kind"] == "test" and t["name"] in selected],
        })
    if any(not shard["targets"] for shard in shards):
        raise ValueError("empty shard would broaden Cargo's default target selection")
    return {"inventory": targets, "shards": shards}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--shard", choices=SHARDS)
    parser.add_argument("--plan-only", action="store_true")
    args = parser.parse_args(argv)
    if args.shard is None and not args.plan_only:
        parser.error("--shard is required unless --plan-only is selected")
    metadata = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--all-features", "--format-version", "1"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    )
    selected_plan = plan(json.loads(metadata.stdout))
    if args.plan_only:
        print(json.dumps(selected_plan, sort_keys=True))
        return 0
    shard = next(shard for shard in selected_plan["shards"] if shard["name"] == args.shard)
    print(f"MSRV {args.shard}: {len(shard['targets'])} workspace targets", flush=True)
    # One Cargo invocation; preserve its first failure and exact exit status.
    return subprocess.run(shard["command"], cwd=ROOT, check=False).returncode


if __name__ == "__main__":
    try:
        sys.exit(main())
    except subprocess.CalledProcessError as error:
        print(error.stderr or str(error), file=sys.stderr)
        sys.exit(error.returncode)
    except ValueError as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
