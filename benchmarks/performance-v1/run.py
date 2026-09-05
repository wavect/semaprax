#!/usr/bin/env python3
"""
Performance macrobenchmark runner for semaprax.

Usage:
  python3 benchmarks/performance-v1/run.py --output benchmarks/performance-v1/results/local.json
  python3 benchmarks/performance-v1/run.py --with-build --output /tmp/with-build.json
  python3 benchmarks/performance-v1/run.py --compare benchmarks/performance-v1/results/baseline.json --output /tmp/compare.json
  python3 benchmarks/performance-v1/run.py --quick --output /tmp/quick.json
  python3 benchmarks/performance-v1/run.py --dry-run --output /tmp/plan.json

Paths are resolved from the script location, never from the working directory:
scenario sources against the repository root, suite-owned files against this
suite directory. Only `--output`, `--compare` and `--markdown` follow the
caller's cwd.

What is timed: exactly one direct execution of one selected `semaprax` binary.
The compiler is built (or supplied with `--semaprax`) before any timing, so no
Cargo work is inside a sample. Every scenario's inputs are digested before and
after the run; drift fails the scenario closed. A scenario that does not reach
its expected outcome is a failure, never a fast "improvement".
"""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone

try:  # Python 3.11+
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - older interpreters
    tomllib = None

# `<repository>/benchmarks/performance-v1/run.py`: the suite directory owns
# `scenarios.json` and `results/`; scenario paths are repository-relative.
SUITE = pathlib.Path(__file__).resolve().parent
ROOT = SUITE.parent.parent
SCENARIOS = SUITE / "scenarios.json"
SCHEMA = "benchmark.performance.v2"
PLAN_SCHEMA = "benchmark.plan.v2"
TIMEOUT_SECONDS = 120

# Result statuses. `ok` is the only comparable one.
OK = "ok"
FAILED = "failed"
SKIPPED = "skipped"
DRIFTED = "drifted"


def sha256_bytes(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def sha256_file(path: pathlib.Path) -> str:
    return sha256_bytes(path.read_bytes())


def git_revision(root: pathlib.Path) -> dict:
    """Exact revision of the measured tree, including whether it is dirty."""
    revision = {"commit": "unknown", "dirty": None}
    try:
        revision["commit"] = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=root, text=True
        ).strip()
        status = subprocess.check_output(
            ["git", "status", "--porcelain"], cwd=root, text=True
        )
        revision["dirty"] = bool(status.strip())
    except Exception:
        pass
    return revision


def tool_version(command: list) -> str:
    try:
        return subprocess.check_output(command, text=True).strip().splitlines()[0]
    except Exception:
        return "unknown"


def host_facts() -> dict:
    """Actual host identity. Never a hardcoded platform string."""
    system = platform.system().lower()
    machine = platform.machine().lower()
    facts = {
        "platform": f"{system}-{machine}",
        "system": platform.system(),
        "release": platform.release(),
        "machine": platform.machine(),
        "cpu_count": os.cpu_count(),
        "python": platform.python_version(),
        "rustc": tool_version(["rustc", "--version"]),
        "cargo": tool_version(["cargo", "--version"]),
        "clang": tool_version(["clang", "--version"]),
    }
    try:
        facts["load_average"] = [round(value, 2) for value in os.getloadavg()]
    except (AttributeError, OSError):  # pragma: no cover - platform dependent
        facts["load_average"] = None
    return facts


def select_compiler(root: pathlib.Path, provided: str, release: bool) -> dict:
    """Select the measured binary *before* timing anything.

    Either the caller's `--semaprax` binary, or one Cargo build whose duration
    is recorded outside every sample.
    """
    if provided:
        binary = pathlib.Path(provided).resolve()
        if not binary.is_file():
            raise FileNotFoundError(f"selected semaprax binary not found: {binary}")
        return {
            "binary": str(binary),
            "digest": sha256_file(binary),
            "profile": "provided",
            "build_ms": None,
            "version": tool_version([str(binary), "--version"]),
        }
    command = ["cargo", "build", "--locked", "-p", "semaprax", "--bin", "semaprax"]
    if release:
        command.append("--release")
    command += ["--message-format", "json-render-diagnostics"]
    start = time.perf_counter()
    completed = subprocess.run(command, cwd=root, capture_output=True, text=True)
    build_ms = round((time.perf_counter() - start) * 1000, 2)
    if completed.returncode != 0:
        raise RuntimeError(f"cargo build failed:\n{completed.stderr}")
    binary = None
    for line in completed.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") == "compiler-artifact" and message.get("executable"):
            target = message.get("target", {})
            if target.get("name") == "semaprax" and "bin" in target.get("kind", []):
                binary = pathlib.Path(message["executable"])
    if binary is None:
        raise RuntimeError("cargo build reported no semaprax executable")
    return {
        "binary": str(binary),
        "digest": sha256_file(binary),
        "profile": "release" if release else "debug",
        "build_ms": build_ms,
        "version": tool_version([str(binary), "--version"]),
    }


def manifest_sources(manifest: pathlib.Path) -> list:
    """Source closure a project manifest declares, relative to the manifest."""
    text = manifest.read_text()
    if tomllib is not None:
        try:
            document = tomllib.loads(text)
        except Exception:
            document = {}
        sources = document.get("sources")
        if isinstance(sources, list):
            return [str(entry) for entry in sources]
    # Minimal fallback for interpreters without tomllib.
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("sources") and "=" in stripped:
            body = stripped.split("=", 1)[1].strip()
            if body.startswith("[") and body.endswith("]"):
                return [
                    item.strip().strip('"').strip("'")
                    for item in body[1:-1].split(",")
                    if item.strip()
                ]
    return []


def scenario_inputs(root: pathlib.Path, scenario: dict) -> list:
    """Every file whose bytes the measured operation reads.

    A project manifest does not bind its own source closure, so the declared
    sources are digested with it. The identity of a project therefore changes
    when any included source changes.
    """
    path = root / scenario["path"]
    inputs = [path]
    if scenario.get("kind") == "project":
        inputs += [path.parent / source for source in manifest_sources(path)]
    rows = []
    for member in inputs:
        rows.append(
            {
                "path": str(member.relative_to(root))
                if member.is_relative_to(root)
                else str(member),
                "digest": sha256_file(member),
            }
        )
    return rows


def subject_identity(inputs: list) -> str:
    """One digest over the whole authenticated input closure."""
    canonical = "\n".join(f"{row['path']}\0{row['digest']}" for row in inputs)
    return sha256_bytes(canonical.encode())


def percentiles(samples: list) -> dict:
    ordered = sorted(samples)
    p50 = statistics.median(ordered)
    index = min(int(0.95 * len(ordered)), len(ordered) - 1)
    return {"p50": round(p50, 2), "p95": round(ordered[index], 2), "samples": samples}


def classify(exit_code, expect: str, timed_out: bool) -> tuple:
    """Map one observed outcome onto the result contract.

    A timeout or an unexpected exit code is a failure. It is never `skipped`,
    and it never contributes a comparable timing sample.
    """
    if timed_out:
        return FAILED, "timeout"
    if expect == "failure":
        if exit_code == 0:
            return FAILED, "expected a failing exit status, observed success"
        return OK, ""
    if exit_code != 0:
        return FAILED, f"exit {exit_code}"
    return OK, ""


def build_destination(scenario_id: str) -> pathlib.Path:
    """A fresh owned output path for one build repetition.

    Publication requires a destination that does not exist yet inside a real
    parent directory, so each repetition gets its own freshly created parent and
    an `out` child inside it. The runner only ever removes that parent, which it
    created itself, and never a caller-selected or shared path.
    """
    parent = pathlib.Path(tempfile.mkdtemp(prefix=f"semaprax-bench-{scenario_id}-"))
    return parent / "out"


def command_line(binary: str, scenario: dict, path: pathlib.Path, destination) -> list:
    line = [binary, scenario["command"], str(path)] + list(scenario.get("args", []))
    if destination is not None:
        line += ["-o", str(destination)]
    return line


def missing_requirement(scenario: dict):
    """An honestly unsupported scenario: a declared external tool is absent."""
    for tool in scenario.get("requires", []):
        if shutil.which(tool) is None:
            return tool
    return None


def run_one(root: pathlib.Path, binary: str, scenario: dict, quick: bool) -> dict:
    identifier = scenario["id"]
    reps = 2 if quick else scenario.get("repetitions", 5)
    expect = scenario.get("expect", "success")
    result = {
        "id": identifier,
        "command": scenario["command"],
        "path": scenario["path"],
        "kind": scenario.get("kind", "single"),
        "args": list(scenario.get("args", [])),
        "expect": expect,
        "repetitions": reps,
        "completed_samples": 0,
    }

    path = root / scenario["path"]
    if not path.exists():
        result.update(status=SKIPPED, reason=f"path not found: {scenario['path']}")
        return result
    tool = missing_requirement(scenario)
    if tool is not None:
        result.update(status=SKIPPED, reason=f"required tool not installed: {tool}")
        return result

    inputs = scenario_inputs(root, scenario)
    identity = subject_identity(inputs)
    result["subject"] = {"digest": identity, "inputs": inputs}
    expected = scenario.get("expected_digest")
    if expected is not None and expected != identity:
        result.update(
            status=DRIFTED,
            reason=f"subject digest {identity} does not match the expected {expected}",
        )
        return result

    # Expected outcome first, outside timing: one untimed verification run.
    destination = build_destination(identifier) if scenario["command"] == "build" else None
    try:
        verification = subprocess.run(
            command_line(binary, scenario, path, destination),
            cwd=root,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            timeout=TIMEOUT_SECONDS,
            text=True,
        )
        verified_status, verified_reason = classify(
            verification.returncode, expect, False
        )
        verification_stderr = verification.stderr
    except subprocess.TimeoutExpired:
        verified_status, verified_reason, verification_stderr = classify(
            None, expect, True
        ) + ("",)
    finally:
        if destination is not None:
            shutil.rmtree(destination.parent, ignore_errors=True)
    result["verification"] = {"status": verified_status, "reason": verified_reason}
    if verified_status != OK:
        result.update(status=FAILED, reason=verified_reason)
        if verification_stderr:
            result["stderr_tail"] = verification_stderr.strip().splitlines()[-3:]
        return result

    samples = []
    status, reason = OK, ""
    for _ in range(reps):
        destination = (
            build_destination(identifier) if scenario["command"] == "build" else None
        )
        line = command_line(binary, scenario, path, destination)
        start = time.perf_counter()
        try:
            completed = subprocess.run(
                line,
                cwd=root,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=TIMEOUT_SECONDS,
            )
            elapsed_ms = (time.perf_counter() - start) * 1000
            status, reason = classify(completed.returncode, expect, False)
            if status != OK:
                break
            samples.append(round(elapsed_ms, 2))
        except subprocess.TimeoutExpired:
            status, reason = classify(None, expect, True)
            break
        finally:
            if destination is not None:
                shutil.rmtree(destination.parent, ignore_errors=True)

    # Post-run drift: the measured bytes must still be the measured bytes.
    after = subject_identity(scenario_inputs(root, scenario))
    if after != identity:
        result.update(
            status=DRIFTED,
            reason="scenario inputs changed during measurement",
            observed_ms=samples,
        )
        return result

    result["completed_samples"] = len(samples)
    if status != OK:
        result.update(status=FAILED, reason=reason)
        if samples:
            # Retained for diagnosis only; a failure has no comparable timing.
            result["observed_ms"] = samples
        return result
    result["status"] = OK
    result["wall_ms"] = percentiles(samples)
    return result


def comparable(local: dict, base: dict) -> tuple:
    """Whether two records measure the same successful work."""
    if local.get("status") != OK or base.get("status") != OK:
        return False, "not both successful"
    if local.get("command") != base.get("command") or local.get("args") != base.get(
        "args"
    ):
        return False, "different operation"
    local_subject = local.get("subject", {}).get("digest")
    base_subject = base.get("subject", {}).get("digest")
    if local_subject != base_subject:
        return False, "different subject bytes"
    if not local.get("wall_ms") or not base.get("wall_ms"):
        return False, "missing timing"
    return True, ""


def compare_rows(local: dict, baseline: dict) -> list:
    """Score only compatible successful pairs; report everything else."""
    base_map = {row["id"]: row for row in baseline.get("scenarios", [])}
    rows = []
    for record in local.get("scenarios", []):
        base = base_map.get(record["id"])
        if base is None:
            rows.append({"id": record["id"], "verdict": "no baseline"})
            continue
        allowed, why = comparable(record, base)
        if not allowed:
            rows.append({"id": record["id"], "verdict": "incomparable", "reason": why})
            continue
        base_p50 = base["wall_ms"]["p50"]
        local_p50 = record["wall_ms"]["p50"]
        delta = 0.0 if base_p50 == 0 else (local_p50 - base_p50) / base_p50
        verdict = "unchanged"
        if delta > 0.15:
            verdict = "regression"
        elif delta < -0.15:
            verdict = "improvement"
        rows.append(
            {
                "id": record["id"],
                "verdict": verdict,
                "baseline_p50": base_p50,
                "local_p50": local_p50,
                "delta": round(delta, 4),
            }
        )
    return rows


def summarize(results: list) -> dict:
    summary = {OK: 0, FAILED: 0, SKIPPED: 0, DRIFTED: 0}
    for record in results:
        summary[record["status"]] = summary.get(record["status"], 0) + 1
    return summary


def render_markdown(document: dict) -> str:
    host = document["host"]
    subject = document["subject"]
    lines = [
        "# Performance macrobenchmark results",
        "",
        f"- Schema: `{document['schema']}`",
        f"- Recorded: {document['timestamp']}",
        f"- Host: `{host['platform']}` ({host['system']} {host['release']}, "
        f"{host['cpu_count']} logical CPUs, load average at start "
        f"{host.get('load_average')})",
        f"- Toolchain: {host['rustc']}, {host['cargo']}",
        f"- Subject: `{subject['version']}` profile `{subject['profile']}`, "
        f"binary digest `{subject['digest']}`",
        f"- Revision: `{subject['commit']}` (dirty working tree: {subject['dirty']})",
        "",
        "Wall times are advisory local evidence for this one host and build. "
        "They are not hosted, release, or cross-platform claims.",
        "",
        "| Scenario | Command | Status | p50 ms | p95 ms | Samples | Subject digest |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for record in document["scenarios"]:
        wall = record.get("wall_ms") or {}
        digest = record.get("subject", {}).get("digest", "-")
        lines.append(
            "| {id} | {command} | {status} | {p50} | {p95} | {samples} | `{digest}` |".format(
                id=record["id"],
                command=record["command"],
                status=record["status"],
                p50=wall.get("p50", "-"),
                p95=wall.get("p95", "-"),
                samples=record.get("completed_samples", 0),
                digest=digest,
            )
        )
    summary = document["summary"]
    lines += [
        "",
        f"Summary: {summary[OK]} ok, {summary[FAILED]} failed, "
        f"{summary[SKIPPED]} skipped, {summary[DRIFTED]} drifted.",
        "",
    ]
    return "\n".join(lines)


def fail(message: str) -> int:
    """Report one actionable startup failure without a traceback."""
    print(f"error: {message}", file=sys.stderr)
    return 2


def write_json(path: pathlib.Path, document: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    # canonical JSON: sorted keys, 2-space indent, terminal LF
    path.write_text(json.dumps(document, sort_keys=True, indent=2) + "\n")


def selected(scenarios: list, args) -> list:
    chosen = []
    for scenario in scenarios:
        if scenario["command"] == "build" and not args.with_build:
            continue
        if args.only and scenario["id"] not in args.only:
            continue
        chosen.append(scenario)
    return chosen


def dry_run(root: pathlib.Path, scenarios: list, args) -> int:
    """Resolve every scenario and time nothing.

    This is the startup and identity seam: it proves the inventory, the
    repository root and the caller-selected output path resolve from any
    working directory, and it reports each scenario's subject identity.
    """
    planned = []
    missing = []
    for scenario in scenarios:
        resolved = root / scenario["path"]
        exists = resolved.exists()
        if not exists:
            missing.append(scenario["path"])
        row = {
            "id": scenario["id"],
            "command": scenario["command"],
            "kind": scenario.get("kind", "single"),
            "path": scenario["path"],
            "resolved_path": str(resolved),
            "exists": exists,
            "repetitions": 2 if args.quick else scenario.get("repetitions", 5),
            "args": list(scenario.get("args", [])),
            "expect": scenario.get("expect", "success"),
        }
        if exists:
            inputs = scenario_inputs(root, scenario)
            row["subject"] = {"digest": subject_identity(inputs), "inputs": inputs}
        planned.append(row)
    document = {
        "schema": PLAN_SCHEMA,
        "root": str(root),
        "suite": str(SUITE),
        # Caller-selected paths, recorded exactly as received: a wrapper that
        # split an argument on whitespace is visible here.
        "output": args.output,
        "compare": args.compare,
        "scenarios": planned,
    }
    write_json(pathlib.Path(args.output), document)
    print(f"Wrote {args.output} ({len(planned)} scenarios planned, timed nothing)")
    for path in missing:
        print(f"error: scenario path not found: {root / path}", file=sys.stderr)
    return 1 if missing else 0


def main():
    parser = argparse.ArgumentParser(description="semaprax macro benchmark runner")
    parser.add_argument("--output", required=True, help="output JSON path")
    parser.add_argument("--compare", help="baseline JSON to compare against")
    parser.add_argument("--markdown", help="also render the result as markdown")
    parser.add_argument(
        "--with-build",
        action="store_true",
        help="include build scenarios (they may require Clang or a wasm toolchain)",
    )
    parser.add_argument("--quick", action="store_true", help="2 repetitions instead of 5")
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="resolve the inventory and report every scenario's plan without timing anything",
    )
    parser.add_argument(
        "--only",
        action="append",
        metavar="ID",
        help="measure only this scenario id (repeatable)",
    )
    parser.add_argument(
        "--scenarios",
        help=f"scenario inventory to read (default: {SCENARIOS})",
    )
    parser.add_argument(
        "--root",
        help=f"repository root scenario paths resolve against (default: {ROOT})",
    )
    parser.add_argument(
        "--semaprax",
        help="measure this already built binary instead of building one with cargo",
    )
    parser.add_argument(
        "--release",
        action="store_true",
        help="build and measure the release profile instead of debug",
    )
    args = parser.parse_args()

    root = pathlib.Path(args.root).resolve() if args.root else ROOT
    inventory = pathlib.Path(args.scenarios).resolve() if args.scenarios else SCENARIOS
    try:
        scenarios_data = json.loads(inventory.read_text())
    except FileNotFoundError:
        return fail(f"scenario inventory not found: {inventory}")
    except json.JSONDecodeError as error:
        return fail(f"scenario inventory is not valid JSON: {inventory}: {error}")
    scenarios = sorted(scenarios_data["scenarios"], key=lambda item: item["id"])
    unknown = set(args.only or []) - {scenario["id"] for scenario in scenarios}
    if unknown:
        return fail(f"unknown scenario id(s): {', '.join(sorted(unknown))}")
    scenarios = selected(scenarios, args)
    if not scenarios:
        return fail("no scenario selected")

    if args.dry_run:
        return dry_run(root, scenarios, args)

    host = host_facts()
    try:
        subject = select_compiler(root, args.semaprax, args.release)
    except (FileNotFoundError, RuntimeError) as error:
        return fail(str(error))
    subject.update(git_revision(root))
    print(
        f"Measuring {subject['binary']} ({subject['profile']}, {subject['version']})",
        flush=True,
    )

    results = []
    for scenario in scenarios:
        print(f"[{scenario['id']}] {scenario['command']} {scenario['path']} ...", flush=True)
        record = run_one(root, subject["binary"], scenario, args.quick)
        results.append(record)
        wall = record.get("wall_ms")
        detail = (
            f"p50={wall['p50']}ms p95={wall['p95']}ms"
            if wall
            else record.get("reason", "")
        )
        print(f"  -> status={record['status']} {detail}", flush=True)

    summary = summarize(results)
    document = {
        "schema": SCHEMA,
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "host": host,
        "subject": subject,
        "timing": {
            "clock": "time.perf_counter",
            "measures": "one direct execution of the selected semaprax binary",
            "excludes": "cargo startup, compiler build, input digesting, and the untimed verification run",
            "timeout_seconds": TIMEOUT_SECONDS,
        },
        "summary": summary,
        "scenarios": results,
    }

    out_path = pathlib.Path(args.output)
    write_json(out_path, document)
    print(f"Wrote {out_path} ({len(results)} scenarios)")
    if args.markdown:
        markdown_path = pathlib.Path(args.markdown)
        markdown_path.parent.mkdir(parents=True, exist_ok=True)
        markdown_path.write_text(render_markdown(document))
        print(f"Wrote {markdown_path}")

    status = 0
    if summary[FAILED] or summary[DRIFTED]:
        print(
            f"error: {summary[FAILED]} failed and {summary[DRIFTED]} drifted scenario(s)",
            file=sys.stderr,
        )
        status = 1

    if args.compare:
        compare_path = pathlib.Path(args.compare)
        try:
            baseline = json.loads(compare_path.read_text())
        except FileNotFoundError:
            return fail(f"baseline not found: {compare_path}")
        except json.JSONDecodeError as error:
            return fail(f"baseline is not valid JSON: {compare_path}: {error}")
        if not baseline.get("scenarios"):
            return fail(
                f"baseline holds no recorded measurement: {compare_path}"
                f" ({baseline.get('reason', 'no scenarios')})"
            )
        print("\nComparison (local vs baseline):")
        for row in compare_rows(document, baseline):
            if row["verdict"] in ("no baseline", "incomparable"):
                reason = row.get("reason", "")
                print(f"  {row['id']}: {row['verdict']} {reason}".rstrip())
                continue
            print(
                f"  {row['id']}: baseline {row['baseline_p50']}ms -> local "
                f"{row['local_p50']}ms delta {row['delta']:+.1%} {row['verdict']}"
            )

    return status


if __name__ == "__main__":
    sys.exit(main())
