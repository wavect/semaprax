#!/usr/bin/env python3
"""Partition every workspace test target without changing feature unification."""

import argparse
import codecs
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
SHARDS = (
    "unit",
    "integration-0",
    "integration-1",
    "integration-2",
    "integration-3",
    "integration-4",
    "integration-5",
)
HEAVY_UNIT_SHARD = "unit-heavy"
HEAVY_UNIT_FILTERS = ("kernel_zero::differential::", "workspace_graph::tests::")
HEAVY_UNIT_TEST = ["cargo", "test", "--locked", "-p", "semaprax", "--all-features", "--lib"]
TEST = ["cargo", "test", "--locked", "--workspace", "--all-features"]
REPAIR_FILTER = "source_live_cli::repair::tests::"
REPAIR_TEST = [
    "cargo", "test", "--locked", "--offline", "-p", "semaprax-toolchain", "--all-features", "--lib",
    "--features",
    "semaprax/unstable-native-host-internal,semaprax/unstable-wit-component-harness,"
    "semaprax/unstable-workflow-profiling",
]


# Checked test selectors.
#
# A Cargo test command that selects nothing still exits 0, so a renamed or
# moved test behind an exact selector silently stops running. The checked
# route below lists the target with the same Cargo package, target, features
# and toolchain it executes, refuses a missing, malformed, out-of-scope or
# ignored-only selection before execution, then streams libtest's ordinary
# (stable, non-JSON) output and requires one completion record per selected
# case plus one summary whose counts are bound to the listed target.
#
# Migrated so far: the GEN-05B generic-instance exact-selector step in
# .github/workflows/ci.yml, both unit-heavy families, and the source-repair
# shards. Every other direct `cargo test ... -- --exact` selector in the
# workflows is still unchecked; this is a bounded slice, not whole-CI coverage.
LISTING_LIMIT = 32 * 1024 * 1024
LINE_LIMIT = 64 * 1024
IGNORE_POLICIES = {
    "default": (),
    "ignored": ("--ignored",),
    "include-ignored": ("--include-ignored",),
}
EXACT_SELECTOR = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z0-9_]+)*")
PREFIX_SELECTOR = re.compile(r"(?:[A-Za-z_][A-Za-z0-9_]*::)+")
LIST_SUMMARY = re.compile(r"(\d+) tests?, (\d+) benchmarks?")
RUNNING = re.compile(r"running (\d+) tests?")
RESULT = r"(?P<result>ok|FAILED|ignored(?:, .*)?)"
TEST_RECORD = re.compile(r"test (?P<name>\S+)(?: - should panic)? \.\.\. " + RESULT + r"$")
TEST_START = re.compile(r"test (?P<name>\S+)(?: - should panic)? \.\.\. ")
LATE_RESULT = re.compile(r"(?:^|\s)(?P<result>ok|FAILED)$")
SUMMARY = re.compile(
    r"test result: (?P<status>ok|FAILED)\. (?P<passed>\d+) passed; (?P<failed>\d+) failed; "
    r"(?P<ignored>\d+) ignored; (?P<measured>\d+) measured; (?P<filtered>\d+) filtered out;.*"
)
HARNESS_ARGUMENT = re.compile(r"--nocapture|--test-threads=[1-9][0-9]*")


def capture_listing(command, environment, limit=LISTING_LIMIT):
    """Run one `--list` command, keeping at most `limit` bytes of stdout."""
    with subprocess.Popen(command, cwd=ROOT, env=environment, stdout=subprocess.PIPE) as process:
        data = process.stdout.read(limit + 1)
        if len(data) > limit:
            process.kill()
            process.wait()
            raise ValueError(f"test inventory exceeded {limit} bytes")
        code = process.wait()
    if code:
        raise subprocess.CalledProcessError(code, command)
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError(f"malformed test inventory: {error}") from None


def parse_listing(text):
    """Parse libtest's pretty `--list` output for exactly one test target."""
    names, summaries = [], []
    for line in text.splitlines():
        line = line.rstrip("\r")
        if not line.strip():
            continue
        summary = LIST_SUMMARY.fullmatch(line)
        if summary:
            summaries.append((int(summary[1]), int(summary[2])))
            continue
        name, separator, kind = line.rpartition(": ")
        if not separator or kind != "test" or not name or any(c.isspace() for c in name):
            raise ValueError(f"malformed test inventory line: {line[:200]!r}")
        names.append(name)
    if len(summaries) != 1:
        raise ValueError(
            f"test inventory must come from exactly one libtest target, found {len(summaries)}"
        )
    if summaries[0] != (len(names), 0):
        raise ValueError("test inventory summary does not match its listed tests")
    if len(set(names)) != len(names):
        raise ValueError("test inventory lists a duplicate test")
    return names


def discover(base, environment):
    """List every case and every ignored case of the one target `base` selects."""
    names = parse_listing(capture_listing([*base, "--", "--list"], environment))
    ignored = parse_listing(capture_listing([*base, "--", "--list", "--ignored"], environment))
    if not set(ignored) <= set(names):
        raise ValueError("ignored test inventory escaped the full inventory")
    return names, frozenset(ignored)


def _runnable(name, ignored, policy):
    if policy == "default":
        return name not in ignored
    if policy == "ignored":
        return name in ignored
    return True


def select_cases(names, ignored, exact=(), prefix=None, policy="default"):
    """Return libtest filters and the exact outcome expected for each selected case."""
    if policy not in IGNORE_POLICIES:
        raise ValueError(f"unknown ignore policy {policy!r}")
    if bool(exact) == (prefix is not None):
        raise ValueError("select exact tests or one prefix family, not both or neither")
    inventory = set(names)
    if exact:
        if len(set(exact)) != len(exact):
            raise ValueError("an exact test is selected twice")
        for name in exact:
            if not EXACT_SELECTOR.fullmatch(name):
                raise ValueError(f"malformed exact test selector {name!r}")
            if name not in inventory:
                raise ValueError(f"exact test {name} is not in the listed target")
            if not _runnable(name, ignored, policy):
                if policy == "default":
                    raise ValueError(
                        f"required test {name} is ignored; select an ignore policy deliberately"
                    )
                raise ValueError(f"required test {name} is not ignored, so --ignored skips it")
        return ["--exact", *exact], {name: "ok" for name in exact}
    if not PREFIX_SELECTOR.fullmatch(prefix):
        raise ValueError(f"malformed test family selector {prefix!r}; use a `module::` path")
    # libtest's non-exact filter is a substring match; inventory it the same way.
    family = sorted(name for name in names if prefix in name)
    escaped = [name for name in family if not name.startswith(prefix)]
    if escaped:
        raise ValueError(f"test family {prefix} escapes its declared scope: {escaped[0]}")
    expected = {}
    for name in family:
        if _runnable(name, ignored, policy):
            expected[name] = "ok"
        elif policy == "default":
            expected[name] = "ignored"
    if "ok" not in expected.values():
        raise ValueError(f"test family {prefix} selects no runnable case")
    return [prefix], expected


class ResultVerifier:
    """Bind streamed libtest records and summary counts to the selected cases."""

    def __init__(self, expected):
        self.expected = expected
        self.records = {}
        self.errors = []
        self.running = []
        self.summaries = []
        self.pending = None
        self.partial = b""
        self.overlong = False

    def feed(self, raw):
        if not raw.endswith(b"\n"):
            if len(self.partial) + len(raw) >= LINE_LIMIT:
                self.overlong, self.partial = True, b""
            else:
                self.partial += raw
            return
        raw, self.partial = self.partial + raw, b""
        if self.overlong:
            # The tail of an over-long line is test output, never a record.
            self.overlong = False
            return
        self.line(raw.decode("utf-8", "replace").rstrip("\r\n"))

    def record(self, name, result):
        if name in self.records:
            self.errors.append(f"duplicate completion record for {name}")
        elif name not in self.expected:
            self.errors.append(f"unselected test {name} reported a result")
        self.records[name] = "ignored" if result.startswith("ignored") else result

    def line(self, line):
        running = RUNNING.fullmatch(line)
        if running:
            self.running.append(int(running[1]))
            return
        summary = SUMMARY.fullmatch(line)
        if summary:
            self.summaries.append(summary)
            return
        record = TEST_RECORD.match(line)
        if record:
            self.pending = None
            self.record(record["name"], record["result"])
            return
        start = TEST_START.match(line)
        if start:
            # Single-threaded runs print the name before uncaptured output.
            self.pending = start["name"]
            return
        late = LATE_RESULT.search(line) if self.pending else None
        if late:
            self.record(self.pending, late["result"])
            self.pending = None

    def finish(self, discovered):
        if self.partial and not self.overlong:
            self.line(self.partial.decode("utf-8", "replace").rstrip("\r\n"))
        errors = list(self.errors)
        selected = len(self.expected)
        if self.running != [selected]:
            errors.append(f"expected one libtest run of {selected} tests, saw {self.running}")
        for name, outcome in self.expected.items():
            actual = self.records.get(name)
            if actual is None:
                errors.append(f"missing completion record for {name}")
            elif actual != outcome:
                errors.append(f"{name} reported {actual}, expected {outcome}")
        passed = sum(1 for outcome in self.expected.values() if outcome == "ok")
        ignored = selected - passed
        if len(self.summaries) != 1:
            errors.append(f"expected one libtest summary, saw {len(self.summaries)}")
        else:
            summary = self.summaries[0]
            observed = tuple(int(summary[key]) for key in ("passed", "failed", "ignored", "measured", "filtered"))
            wanted = (passed, 0, ignored, 0, discovered - selected)
            if summary["status"] != "ok" or observed != wanted:
                errors.append(
                    "libtest summary (passed, failed, ignored, measured, filtered out) "
                    f"was {observed}, expected {wanted} for the listed target"
                )
        if errors:
            raise ValueError("checked test selection failed: " + "; ".join(errors[:20]))
        return {
            "discovered": discovered,
            "selected": selected,
            "executed": passed,
            "passed": passed,
            "ignored": ignored,
        }


def run_checked(base, filters, expected, discovered, environment, *,
                policy="default", harness=(), label="checked", out=None):
    """Execute one selection and verify every expected case ran with its outcome.

    Returns libtest's exit status when it is nonzero; raises ValueError when a
    zero exit is not backed by a complete, target-bound result record.
    """
    out = sys.stdout if out is None else out
    for argument in harness:
        if not HARNESS_ARGUMENT.fullmatch(argument):
            raise ValueError(f"unsupported libtest argument {argument!r}")
    command = [*base, "--", *filters, *IGNORE_POLICIES[policy], *harness]
    verifier = ResultVerifier(expected)
    decoder = codecs.getincrementaldecoder("utf-8")("replace")
    with subprocess.Popen(command, cwd=ROOT, env=environment, stdout=subprocess.PIPE) as process:
        for raw in iter(lambda: process.stdout.readline(LINE_LIMIT), b""):
            out.write(decoder.decode(raw))
            out.flush()
            verifier.feed(raw)
        code = process.wait()
    out.write(decoder.decode(b"", final=True))
    if code:
        print(f"{label}: libtest exited with status {code}", file=out, flush=True)
        return code
    counts = verifier.finish(discovered)
    print(
        f"{label}: discovered {counts['discovered']}, selected {counts['selected']}, "
        f"executed {counts['executed']}, passed {counts['passed']}, ignored {counts['ignored']}",
        file=out, flush=True,
    )
    return 0


def run_checked_selection(base, environment, *, exact=(), prefix=None, policy="default",
                          harness=(), label="checked", out=None):
    names, ignored = discover(base, environment)
    filters, expected = select_cases(names, ignored, exact, prefix, policy)
    return run_checked(
        base, filters, expected, len(names), environment,
        policy=policy, harness=harness, label=label, out=out,
    )


def repair_shard_names(names, index, count):
    """Select every source-repair case in exactly one of `count` shards.

    `names` is the complete listed inventory of the repair target. Cases are
    those libtest's REPAIR_FILTER substring would select; sorted names are
    dealt round-robin, so the shards partition that inventory. A shard that
    would run nothing is refused rather than broadened to libtest's
    unfiltered default.
    """
    if count < 1 or not 0 <= index < count:
        raise ValueError(f"source-repair shard {index}/{count} is out of range")
    names = sorted({name for name in names if REPAIR_FILTER in name})
    if any(not name.startswith(REPAIR_FILTER) for name in names):
        raise ValueError("source-repair listing escaped its module filter")
    selected = names[index::count]
    if not selected:
        raise ValueError(f"source-repair shard {index}/{count} would select no case")
    return selected


def run_repair_shard(label, selector, cargo_env, base=REPAIR_TEST, out=None):
    index, separator, count = selector.partition("/")
    if not separator or not index.isdigit() or not count.isdigit():
        raise ValueError(f"source-repair shard {selector!r} is not <index>/<count>")
    inventory, ignored = discover(base, cargo_env)
    names = repair_shard_names(inventory, int(index), int(count))
    expected = {name: "ignored" if name in ignored else "ok" for name in names}
    if "ok" not in expected.values():
        raise ValueError(f"source-repair shard {selector} would run only ignored cases")
    print(f"{label} {selector}: {len(names)} source-repair cases", flush=True, file=out or sys.stdout)
    # One case at a time, as the unsharded job ran them.
    return run_checked(
        base, ["--exact", *names], expected, len(inventory), cargo_env,
        harness=["--test-threads=1"], label=f"{label} {selector}", out=out,
    )


def checked_main(argv):
    """`ci-msrv.py checked`: run exact tests or one module family, checked."""
    parser = argparse.ArgumentParser(prog="ci-msrv.py checked", description=checked_main.__doc__)
    parser.add_argument("--label", default="checked")
    parser.add_argument("-p", "--package", required=True)
    target = parser.add_mutually_exclusive_group(required=True)
    target.add_argument("--lib", action="store_true")
    target.add_argument("--test", metavar="TARGET")
    parser.add_argument("--manifest-path")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--all-features", action="store_true")
    parser.add_argument("--features")
    selection = parser.add_mutually_exclusive_group(required=True)
    selection.add_argument("--exact", action="append", metavar="TEST")
    selection.add_argument("--prefix", metavar="MODULE::")
    parser.add_argument("--ignore-policy", choices=tuple(IGNORE_POLICIES), default="default")
    parser.add_argument("--nocapture", action="store_true")
    parser.add_argument("--test-threads", type=int)
    args = parser.parse_args(argv)
    base = ["cargo", "test", "--locked"]
    if args.offline:
        base.append("--offline")
    if args.manifest_path:
        base += ["--manifest-path", args.manifest_path]
    base += ["-p", args.package]
    if args.all_features:
        base.append("--all-features")
    if args.features:
        base += ["--features", args.features]
    base += ["--lib"] if args.lib else ["--test", args.test]
    harness = []
    if args.nocapture:
        harness.append("--nocapture")
    if args.test_threads is not None:
        harness.append(f"--test-threads={args.test_threads}")
    return run_checked_selection(
        base, dict(os.environ), exact=tuple(args.exact or ()), prefix=args.prefix,
        policy=args.ignore_policy, harness=harness, label=args.label,
    )


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
    argv = sys.argv[1:] if argv is None else list(argv)
    if argv[:1] == ["checked"]:
        return checked_main(argv[1:])
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--shard", choices=(*SHARDS, HEAVY_UNIT_SHARD))
    parser.add_argument("--repair-shard", metavar="INDEX/COUNT")
    parser.add_argument("--plan-only", action="store_true")
    parser.add_argument("--exclude-package", action="append", default=[])
    parser.add_argument("--split-windows-agent-runtime", action="store_true")
    parser.add_argument("--nocapture", action="store_true")
    parser.add_argument("--label", default="MSRV")
    args = parser.parse_args(argv)
    if args.repair_shard is not None:
        if args.shard is not None or args.plan_only:
            parser.error("--repair-shard selects its own cases; omit --shard and --plan-only")
        return run_repair_shard(args.label, args.repair_shard, cargo_environment())
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
            print(f"{args.label} {args.shard}: {test_filter}", flush=True)
            # Each family is inventoried, must stay inside its module prefix,
            # and must report every listed case.
            code = run_checked_selection(
                HEAVY_UNIT_TEST, cargo_env, prefix=test_filter,
                harness=["--nocapture"] if args.nocapture else [],
                label=f"{args.label} {args.shard} {test_filter}",
            )
            if code:
                return code
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
    if args.shard == "unit" and (
        (sys.platform == "darwin" and args.label == "Rust macOS")
        or (sys.platform.startswith("linux") and args.label == "Rust Linux")
        or (sys.platform == "win32" and args.label == "Rust Windows")
    ):
        # Every repair case runs once in the dedicated source-repair jobs.
        # Keep its multi-case replay cost out of the main unit lane.
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
