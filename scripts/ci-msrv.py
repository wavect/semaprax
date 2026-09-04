#!/usr/bin/env python3
"""Partition every workspace test target without changing feature unification."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
SHARDS = ("unit", "integration-0", "integration-1", "integration-2")
TEST = ["cargo", "test", "--locked", "--workspace", "--all-features"]


def cargo_environment(environment=None, executable=None):
    environment = dict(os.environ if environment is None else environment)
    selected = environment.get("SEMAPRAX_TEST_PYTHON")
    if selected is None:
        selected = sys.executable if executable is None else executable
    path = Path(selected)
    if not path.is_absolute() or not path.is_file():
        raise ValueError("SEMAPRAX_TEST_PYTHON must select an absolute Python file")
    environment["SEMAPRAX_TEST_PYTHON"] = str(path)
    return environment


def plan(metadata, excluded_packages=()):
    members = set(metadata["workspace_members"])
    packages = [p for p in metadata["packages"] if p["id"] in members]
    if not members or {p["id"] for p in packages} != members:
        raise ValueError("incomplete workspace package inventory")
    package_names = {p["name"] for p in packages}
    excluded_packages = set(excluded_packages)
    missing_exclusions = excluded_packages - package_names
    if missing_exclusions:
        raise ValueError(
            f"unknown excluded workspace package: {sorted(missing_exclusions)}"
        )
    packages = [p for p in packages if p["name"] not in excluded_packages]
    test = TEST + [
        argument
        for package in sorted(excluded_packages)
        for argument in ("--exclude", package)
    ]
    targets = []
    seen = set()
    for package in packages:
        for target in package["targets"]:
            kind = target["kind"]
            # Do not silently omit a newly introduced example, benchmark, or
            # other target kind: extend this partition and its tests first.
            # Bench targets are for `cargo bench` (criterion) and are not
            # part of `cargo test` sharding; they are inventoried but not
            # routed to a test shard.
            if kind not in (["lib"], ["bin"], ["test"], ["bench"]):
                raise ValueError(f"unrouted target kind: {kind}")
            key = (package["id"], kind[0], target["name"])
            if key in seen:
                raise ValueError(f"duplicate workspace target: {key}")
            seen.add(key)
            # Only test-related kinds are part of `cargo test` inventory;
            # bench is inventoried for completeness but excluded from shards
            # (it is run via `cargo bench --benches` separately).
            if kind == ["bench"]:
                continue
            targets.append(dict(package=key[0], kind=key[1], name=key[2]))
    targets.sort(key=lambda t: (t["package"], t["kind"], t["name"]))
    # Cargo's --test selector applies to every selected workspace package.
    # Keep shared names together so each matching package target runs once.
    names = sorted({t["name"] for t in targets if t["kind"] == "test"})
    shards = [{
        "name": "unit",
        "command": test + ["--lib", "--bins"],
        "targets": [t for t in targets if t["kind"] != "test"],
    }]
    for index, name in enumerate(SHARDS[1:]):
        selected = names[index::len(SHARDS) - 1]
        shards.append({
            "name": name,
            "command": test + [arg for target in selected for arg in ("--test", target)],
            "targets": [t for t in targets if t["kind"] == "test" and t["name"] in selected],
        })
    if any(not shard["targets"] for shard in shards):
        raise ValueError("empty shard would broaden Cargo's default target selection")
    return {"inventory": targets, "shards": shards}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--shard", choices=SHARDS)
    parser.add_argument("--plan-only", action="store_true")
    parser.add_argument("--exclude-package", action="append", default=[])
    parser.add_argument("--label", default="MSRV")
    args = parser.parse_args(argv)
    if args.shard is None and not args.plan_only:
        parser.error("--shard is required unless --plan-only is selected")
    cargo_env = cargo_environment()
    metadata = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--all-features", "--format-version", "1"],
        cwd=ROOT, env=cargo_env, capture_output=True, text=True, check=True,
    )
    selected_plan = plan(json.loads(metadata.stdout), args.exclude_package)
    if args.plan_only:
        print(json.dumps(selected_plan, sort_keys=True))
        return 0
    shard = next(shard for shard in selected_plan["shards"] if shard["name"] == args.shard)
    print(f"{args.label} {args.shard}: {len(shard['targets'])} workspace targets", flush=True)
    # One Cargo invocation; preserve its first failure and exact exit status.
    return subprocess.run(
        shard["command"], cwd=ROOT, env=cargo_env, check=False
    ).returncode


if __name__ == "__main__":
    try:
        sys.exit(main())
    except subprocess.CalledProcessError as error:
        print(error.stderr or str(error), file=sys.stderr)
        sys.exit(error.returncode)
    except ValueError as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
