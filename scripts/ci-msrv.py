#!/usr/bin/env python3
"""Partition every workspace test target without changing feature unification."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
SHARDS = ("unit", "integration-0", "integration-1", "integration-2", "integration-3", "integration-4")
HEAVY_UNIT_SHARD = "unit-heavy"
HEAVY_UNIT_FILTERS = ("kernel_zero::differential::", "workspace_graph::tests::")
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


def macos_test_git(environment, discover=shutil.which):
    """Select the real Git binary for held-process tests, never Apple's shim."""
    selected = environment.get("SEMAPRAX_TEST_GIT")
    if selected is None:
        discovered = discover("git")
        selected = next(
            (
                candidate for candidate in (
                    discovered,
                    "/opt/homebrew/bin/git",
                    "/usr/local/bin/git",
                )
                if candidate and Path(candidate).is_file()
                and Path(candidate).resolve() != Path("/usr/bin/git")
            ),
            discovered,
        )
    if selected is None:
        raise ValueError("macOS Git tests require a selected executable")
    path = Path(selected)
    if not path.is_absolute() or not path.is_file():
        raise ValueError("SEMAPRAX_TEST_GIT must select an absolute Git file")
    path = path.resolve()
    if path == Path("/usr/bin/git"):
        raise ValueError("macOS Git tests require the real Git binary, not the xcrun shim")
    environment["SEMAPRAX_TEST_GIT"] = str(path)
    return environment


def without_dedicated_windows_agent_runtime(shard):
    """Route this one target to its own Windows job without running it twice."""
    if shard["name"] != "integration-3" or [
        target["name"] for target in shard["targets"]
    ].count("agent_runtime_v1") != 1:
        raise ValueError("Windows agent runtime split requires its exact integration-3 target")
    command = list(shard["command"])
    position = command.index("agent_runtime_v1")
    if command[position - 1] != "--test":
        raise ValueError("Windows agent runtime target is not a Cargo test selector")
    del command[position - 1:position + 1]
    if "--test" not in command:
        raise ValueError("Windows agent runtime split would empty its shard")
    return command


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
            if kind not in (
                ["lib"],
                ["bin"],
                ["test"],
                ["example"],
                ["bench"],
                ["custom-build"],
            ):
                raise ValueError(f"unrouted target kind: {kind}")
            key = (package["id"], kind[0], target["name"])
            if key in seen:
                raise ValueError(f"duplicate workspace target: {key}")
            seen.add(key)
            # Build scripts are routed with the unit shard because `cargo test
            # --lib --bins` builds them automatically. Bench is inventoried for
            # completeness but excluded from shards (it is run separately via
            # `cargo bench --benches`).
            if kind == ["bench"]:
                continue
            targets.append(dict(package=key[0], kind=key[1], name=key[2]))
    targets.sort(key=lambda t: (t["package"], t["kind"], t["name"]))
    # Cargo's --test selector applies to every selected workspace package.
    # Keep shared names together so each matching package target runs once.
    names = sorted({t["name"] for t in targets if t["kind"] == "test"})
    shards = [{
        "name": "unit",
        "command": test + ["--lib", "--bins", "--examples"],
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
    parser.add_argument("--shard", choices=(*SHARDS, HEAVY_UNIT_SHARD))
    parser.add_argument("--plan-only", action="store_true")
    parser.add_argument("--exclude-package", action="append", default=[])
    parser.add_argument("--split-windows-agent-runtime", action="store_true")
    parser.add_argument("--nocapture", action="store_true")
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
    shard = next((shard for shard in selected_plan["shards"] if shard["name"] == args.shard), None)
    if args.shard == HEAVY_UNIT_SHARD:
        if args.split_windows_agent_runtime:
            raise ValueError("agent runtime split does not apply to heavy unit tests")
        for test_filter in HEAVY_UNIT_FILTERS:
            command = ["cargo", "test", "--locked", "-p", "semaprax", "--all-features", "--lib", test_filter]
            if args.nocapture:
                command += ["--", "--nocapture"]
            print(f"{args.label} {args.shard}: {test_filter}", flush=True)
            result = subprocess.run(command, cwd=ROOT, env=cargo_env, check=False)
            if result.returncode:
                return result.returncode
        return 0
    assert shard is not None
    if args.split_windows_agent_runtime and not (
        os.name == "nt" and args.label == "Rust Windows" and args.shard == "integration-3"
    ):
        raise ValueError("agent runtime split is only valid for Rust Windows integration-3")
    command = (
        without_dedicated_windows_agent_runtime(shard)
        if args.split_windows_agent_runtime
        else list(shard["command"])
    )
    target_count = len(shard["targets"]) - int(args.split_windows_agent_runtime)
    print(f"{args.label} {args.shard}: {target_count} workspace targets", flush=True)

    test_arguments = []
    if args.nocapture:
        test_arguments.append("--nocapture")
    if args.shard == "unit":
        # These two expensive semaprax lib-test families run in unit-heavy.
        # Each test name is in exactly one side of this partition.
        for test_filter in HEAVY_UNIT_FILTERS:
            test_arguments.extend(("--skip", test_filter))
    if os.name == "nt" and args.label == "Rust Windows" and args.shard.startswith("integration-"):
        # C ABI fixtures allocate a 1 MiB aligned context on the stack, which
        # leaves no headroom under the Windows linker's 1 MiB default stack
        # reserve. LINK is inherited by link.exe even when tests invoke it via
        # clang. Apply to every integration shard so the fix does not break
        # when the `project` harness moves between shards via --exclude-package.
        cargo_env["LINK"] = "/STACK:8388608"
    if os.name == "nt" and args.label == "Rust Windows" and any(
        target["name"] == "project" for target in shard["targets"]
    ):
        # Project/npm fixtures share process/filesystem resources; serial
        # execution prevents cross-test contention from stalling the job.
        if "--test-threads=1" not in test_arguments:
            test_arguments.append("--test-threads=1")
    if os.name == "nt" and args.label == "Rust Windows" and args.shard == "integration-0":
        # Keep the historical integration-0 serialization for the current-Rust
        # Windows CI path so the generic/MSRV router contract remains stable.
        # The LINK assignment above already covers the stack reserve.
        if "--test-threads=1" not in test_arguments:
            test_arguments.append("--test-threads=1")
    if os.name == "nt" and args.label == "Rust Windows" and args.shard == "integration-3" and not args.split_windows_agent_runtime:
        # The typed execution-revision corpus runs in AGENT-06 on Windows.
        # Keeping it here as well exceeded the hosted six-hour job ceiling.
        test_arguments.extend(("--skip", "execution_revision::typed::"))
    if sys.platform == "darwin" and args.label == "Rust macOS" and args.shard == "unit":
        # The same repair module runs in its own macOS release blocker, so
        # its longer V2 deadline cannot push this near-six-hour shard over.
        test_arguments.extend(("--skip", "source_live_cli::repair::tests::"))
    if (
        sys.platform == "darwin"
        and args.label == "Rust macOS"
        and any(target["name"] in ("project", "project_candidate") for target in shard["targets"])
    ):
        # The bounded Git fixtures clear their environment, so explicitly
        # select the real binary before dispatch. Apple's /usr/bin/git shim
        # can intermittently fail even when the host Git process is healthy.
        macos_test_git(cargo_env)
        test_arguments.append("--test-threads=1")

    # One Cargo invocation; preserve its first failure and exact exit status.
    command += ["--", *test_arguments] if test_arguments else []
    return subprocess.run(
        command, cwd=ROOT, env=cargo_env, check=False
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
