#!/usr/bin/env python3
"""Build and execute the bounded HR-07 cross-layer evidence suite on macOS.

The runner owns its Cargo invocations: every measured test executable is built
from this checkout into one private target directory, byte-bound before and
after its one exact test invocation, and never runs concurrently with another
selector.  It does not claim native/Wasm support, process identity, provider
latency, or external resource telemetry.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import platform
import re
import subprocess
import sys
import tempfile
import time

SUITE = pathlib.Path(__file__).resolve().parent
ROOT = SUITE.parent.parent
MANIFEST = SUITE / "cross-layer-manifest.json"
CAPTURE = SUITE / "capture.py"
SCHEMA = "semaprax.hot-reload-macos-cross-layer-evidence.v1"
SUMMARY = re.compile(
    r"test result: ok\. (?P<passed>\d+) passed; (?P<failed>\d+) failed; "
    r"(?P<ignored>\d+) ignored; (?P<measured>\d+) measured; (?P<filtered>\d+) filtered out"
)

# Every command is an owned Cargo target, never caller-provided argv or paths.
TARGETS = {
    "root-lib": {
        "cargo": ["test", "--locked", "--lib", "--no-run", "--message-format=json", "--jobs", "1"],
        "target_name": "semaprax",
        "target_kind": "lib",
    },
    "toolchain-lib": {
        "cargo": ["test", "--locked", "-p", "semaprax-toolchain", "--lib", "--no-run", "--message-format=json", "--jobs", "1"],
        "target_name": "semaprax_toolchain",
        "target_kind": "lib",
    },
    "source-agent-integration": {
        "cargo": ["test", "--locked", "-p", "semaprax-toolchain", "--test", "cli_help_surface_v1", "--no-run", "--message-format=json", "--jobs", "1"],
        "target_name": "cli_help_surface_v1",
        "target_kind": "test",
    },
}
SELECTORS = {
    "watcher-a-b-invalid-c": ("root-lib", "project::hot_reload_watcher::tests::invalid_c_rejects_after_b_without_replacing_active_a_or_admitting_stale_b"),
    "watcher-invalid-c-repair": ("root-lib", "project::hot_reload_watcher::tests::valid_repair_after_invalid_c_admits_once"),
    "source-agent-a-b": ("source-agent-integration", "cli_help_surface_v1::source_agent_hot_reload::full_dev_source_agent_migrates_real_journal_a_to_b_with_local_opencode_stub"),
    "source-agent-a-b-c": ("toolchain-lib", "source_live_cli::hr04_state_handoff_tests::retained_a_to_b_to_c_handoff_carries_state_without_initialize_or_redispatch"),
    "prepared-worker-a-b-c-identity": ("root-lib", "project::hot_reload::tests::real_a_to_b_to_c_keeps_one_worker_and_binds_each_trace_to_its_revision"),
    "watcher-stop-resource-release": ("root-lib", "project::hot_reload_watcher::tests::external_stop_during_admission_clears_pending_work_and_releases_the_fixture"),
}
UNAVAILABLE = ("native-process-identity", "native-or-wasm-state-swap")


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def current_commit():
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()


def read_manifest():
    value = json.loads(MANIFEST.read_text())
    if value.get("schema") != "semaprax.hot-reload-cross-layer-acceptance.v1":
        raise ValueError("cross-layer manifest has an unexpected schema")
    cells = {cell.get("id"): cell for cell in value.get("cells", [])}
    expected = {"interpreter-a-b", *SELECTORS, *UNAVAILABLE}
    if set(cells) != expected:
        raise ValueError("cross-layer manifest does not name the exact supported selector inventory")
    for identifier, (_, selector) in SELECTORS.items():
        cell = cells[identifier]
        if cell.get("availability") != "selector-required" or cell.get("selector") != selector:
            raise ValueError("cross-layer selector is not the owned exact test")
    if cells["interpreter-a-b"].get("availability") != "runnable-with-prebuilt-semaprax":
        raise ValueError("interpreter cell must retain its current-head compiler contract")
    for identifier in UNAVAILABLE:
        if cells[identifier].get("availability") != "unavailable" or cells[identifier].get("selector") is not None:
            raise ValueError("native/Wasm limitation must remain explicit")
    return value, cells


def private_target(path):
    resolved = path.resolve()
    try:
        resolved.relative_to(ROOT / "target")
    except ValueError as error:
        raise ValueError("--target-dir must be an absolute private directory below this checkout's target/") from error
    return resolved


def cargo_environment(target, commit):
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target)
    env["CARGO_BUILD_JOBS"] = "1"
    env["SEMAPRAX_BUILD_COMMIT"] = commit
    return env


def compile_target(cargo, target, commit, name):
    spec = TARGETS[name]
    completed = subprocess.run(
        [cargo, *spec["cargo"]], cwd=ROOT, env=cargo_environment(target, commit),
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
    )
    if completed.returncode:
        diagnostics = []
        for line in completed.stdout.splitlines():
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get("reason") == "compiler-message":
                message = event.get("message", {})
                if message.get("level") == "error":
                    diagnostics.append(message.get("rendered") or message.get("message", "compiler error"))
        raise RuntimeError("Cargo failed while building %s:\n%s\n%s" % (
            name, "\n".join(diagnostics), completed.stderr,
        ))
    binaries = []
    for line in completed.stdout.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        target_info = event.get("target", {})
        if event.get("reason") == "compiler-artifact" and event.get("executable") and target_info.get("name") == spec["target_name"] and spec["target_kind"] in target_info.get("kind", []):
            binaries.append(pathlib.Path(event["executable"]).resolve())
    binaries = list(dict.fromkeys(binaries))
    if len(binaries) != 1 or not binaries[0].is_file():
        raise RuntimeError("Cargo did not emit exactly one executable for %s" % name)
    return binaries[0], {
        "cargo_command": [cargo, *spec["cargo"]],
        "stdout_sha256": "sha256:" + hashlib.sha256(completed.stdout.encode()).hexdigest(),
        "stderr_sha256": "sha256:" + hashlib.sha256(completed.stderr.encode()).hexdigest(),
    }


def exact_test(binary, selector):
    before = digest(binary)
    started = time.perf_counter_ns()
    completed = subprocess.run([str(binary), selector, "--exact"], cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=False)
    elapsed = round((time.perf_counter_ns() - started) / 1_000_000, 3)
    after = digest(binary)
    if before != after:
        raise RuntimeError("test executable changed while it was measured")
    stdout = completed.stdout.decode("utf-8", "replace")
    matches = list(SUMMARY.finditer(stdout))
    if completed.returncode or len(matches) != 1:
        raise RuntimeError("exact selector failed: %s" % selector)
    counts = {key: int(value) for key, value in matches[0].groupdict().items()}
    if counts["passed"] != 1 or counts["failed"] != 0 or counts["ignored"] != 0 or counts["measured"] != 0:
        raise RuntimeError("exact selector did not report one passed test: %s" % selector)
    return {
        "selector": selector,
        "argv": [str(binary), selector, "--exact"],
        "serial_ordinal": None,
        "elapsed_ms": elapsed,
        "test_counts": counts,
        "executable_sha256": before,
        "stdout_sha256": "sha256:" + hashlib.sha256(completed.stdout).hexdigest(),
        "stderr_sha256": "sha256:" + hashlib.sha256(completed.stderr).hexdigest(),
    }


def capture_interpreter(semaprax, commit, samples, warmups):
    with tempfile.TemporaryDirectory(prefix="semaprax-hot-reload-cross-layer-") as directory:
        output = pathlib.Path(directory) / "capture.json"
        completed = subprocess.run(
            [sys.executable, str(CAPTURE), "--semaprax", str(semaprax), "--samples", str(samples), "--warmups", str(warmups), "--expected-commit", commit, "--output", str(output)],
            cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=False,
        )
        if completed.returncode:
            raise RuntimeError("interpreter capture failed: " + completed.stderr.decode("utf-8", "replace"))
        report = json.loads(output.read_text())
    cell = report.get("cells", {}).get("interpreter-a-b", {})
    summary = cell.get("summary", {})
    if cell.get("status") != "measured" or summary.get("samples") != samples:
        raise RuntimeError("interpreter capture did not produce the requested nonzero samples")
    return {
        **cell,
        "serial_ordinal": 1,
        "clean_stop_evidence": {
            "source": "benchmarks/hot-reload-v1/run.py",
            "required_per_record": {"event": "stopped", "process_exit": 0},
            "status": "protocol stop and child exit asserted by the captured interpreter runner",
        },
        "capture_stdout_sha256": "sha256:" + hashlib.sha256(completed.stdout).hexdigest(),
        "capture_stderr_sha256": "sha256:" + hashlib.sha256(completed.stderr).hexdigest(),
    }


def plan(samples, warmups, target):
    manifest, cells = read_manifest()
    return {
        "schema": SCHEMA,
        "mode": "plan",
        "acceptance_manifest_digest": digest(MANIFEST),
        "repository_commit": current_commit(),
        "target_dir": str(target),
        "interpreter_samples": samples,
        "interpreter_warmups": warmups,
        "serial_order": ["interpreter-a-b", *SELECTORS],
        "cells": {identifier: {"selector": cells[identifier]["selector"], "status": "will-run"} for identifier in ["interpreter-a-b", *SELECTORS]},
        "unavailable": {identifier: {"selector": None, "requires": cells[identifier]["requires"]} for identifier in UNAVAILABLE},
        "nonclaims": manifest["nonclaims"],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target-dir", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--samples", type=int, default=11)
    parser.add_argument("--warmups", type=int, default=3)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    target = private_target(args.target_dir)
    if args.samples < 1 or args.warmups < 0:
        raise SystemExit("--samples must be positive and --warmups must be nonnegative")
    if args.dry_run:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(canonical(plan(args.samples, args.warmups, target)))
        return
    if platform.system() != "Darwin":
        raise SystemExit("macOS evidence requires Darwin")
    manifest, cells = read_manifest()
    commit = current_commit()
    target.mkdir(parents=True, exist_ok=True)
    root_test, compiler_build = compile_target(args.cargo, target, commit, "root-lib")
    # The root library build does not create the CLI; build it as the same serial source-attributed step.
    cli = subprocess.run([args.cargo, "build", "--locked", "--bin", "semaprax", "--jobs", "1"], cwd=ROOT, env=cargo_environment(target, commit), stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if cli.returncode:
        raise RuntimeError("Cargo failed while building semaprax CLI:\n" + cli.stderr)
    semaprax = target / "debug" / "semaprax"
    if not semaprax.is_file():
        raise RuntimeError("Cargo did not create the source-built semaprax CLI")
    results = {"interpreter-a-b": capture_interpreter(semaprax, commit, args.samples, args.warmups)}
    compiled = {"root-lib": (root_test, compiler_build)}
    for ordinal, (identifier, (target_name, selector)) in enumerate(SELECTORS.items(), start=2):
        if target_name not in compiled:
            compiled[target_name] = compile_target(args.cargo, target, commit, target_name)
        binary, build = compiled[target_name]
        row = exact_test(binary, selector)
        row["serial_ordinal"] = ordinal
        row["source_build"] = build
        if identifier == "watcher-stop-resource-release":
            row["clean_stop_resource_evidence"] = {"status": "selector asserts Stop clears pending work and fixture removal succeeds", "requirements": cells[identifier]["requires"]}
        results[identifier] = row
    report = {
        "schema": SCHEMA,
        "acceptance_manifest_digest": digest(MANIFEST),
        "repository_commit": commit,
        "source_build": {"target_dir": str(target), "cargo_build_jobs": 1, "semaprax_build_commit": commit, "cli_sha256": digest(semaprax), "cli_build_stdout_sha256": "sha256:" + hashlib.sha256(cli.stdout.encode()).hexdigest(), "cli_build_stderr_sha256": "sha256:" + hashlib.sha256(cli.stderr.encode()).hexdigest()},
        "host": {"system": platform.system(), "release": platform.release(), "machine": platform.machine()},
        "serial_order": ["interpreter-a-b", *SELECTORS],
        "cells": results,
        "unavailable": {identifier: {"selector": None, "requires": cells[identifier]["requires"]} for identifier in UNAVAILABLE},
        "nonclaims": manifest["nonclaims"],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(canonical(report))


if __name__ == "__main__":
    main()
