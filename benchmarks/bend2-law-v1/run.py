#!/usr/bin/env python3
"""Run the pinned Bend 2 / SEMAPRAX law benchmark without inventing a result.

The command file is deliberately local and untracked: it names the provisioned
executables and checked-out subject trees.  Every timed cell runs both Bend
ordinary checking and Bend's verdict kernel as distinct paths.  A missing
tool, a wrong Git identity, a timeout, or a rejected law-gaming control is
reported as unavailable or failed; neither can be counted as a win.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone

SCHEMA = "semaprax.bend2-law-benchmark.result.v1"
MANIFEST_SCHEMA = "semaprax.bend2-law-benchmark.manifest.v1"
COMMANDS_SCHEMA = "semaprax.bend2-law-benchmark.commands.v1"
PATHS = ("bend_normal", "bend_verdict", "semaprax_smt", "semaprax_lean", "semaprax_runtime")
ENVIRONMENT_FIELDS = ("hardware", "os", "bend_toolchain", "semaprax_toolchain", "backend", "optimization_flags", "inputs")


def canonical(value):
    return json.dumps(value, sort_keys=True, indent=2) + "\n"


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def git_head(root):
    return subprocess.check_output(
        ["git", "-C", str(root), "rev-parse", "HEAD"], text=True, stderr=subprocess.DEVNULL
    ).strip()


def load(path, schema):
    value = json.loads(path.read_text())
    if value.get("schema") != schema:
        raise ValueError(f"{path} has unsupported schema")
    return value


def require_manifest(manifest, root):
    if set(manifest) != {"schema", "bend", "required_paths", "cells"}:
        raise ValueError("manifest keys are not exact")
    if manifest["required_paths"] != list(PATHS):
        raise ValueError("required execution paths changed")
    seen = set()
    for cell in manifest["cells"]:
        if set(cell) != {"id", "fixture", "numeric_domain", "laws", "attacks"} or not cell["laws"] or not cell["attacks"]:
            raise ValueError("cell lacks equal-semantics laws or attacks")
        fixture = root / cell["fixture"]
        try:
            fixture_value = json.loads(fixture.read_text())
        except (OSError, json.JSONDecodeError):
            raise ValueError("cell fixture is unavailable")
        if (set(fixture_value) != {"schema", "id", "numeric_domain", "success", "attacks"}
                or fixture_value["schema"] != "semaprax.bend2-law-benchmark.fixture.v1"
                or fixture_value["id"] != cell["id"]
                or fixture_value["numeric_domain"] != cell["numeric_domain"]
                or set(fixture_value["attacks"]) != set(cell["attacks"])):
            raise ValueError("cell fixture does not bind the declared semantics")
        if cell["id"] in seen:
            raise ValueError("duplicate cell")
        seen.add(cell["id"])


def require_commands(commands, bend_commit):
    if set(commands) != {"schema", "semaprax", "bend", "environment", "commands"}:
        raise ValueError("commands keys are not exact")
    if commands["bend"].get("commit") != bend_commit:
        raise ValueError("commands do not pin the reviewed Bend commit")
    if not isinstance(commands["semaprax"].get("commit"), str) or not commands["semaprax"]["commit"]:
        raise ValueError("commands must pin a SEMAPRAX commit")
    if set(commands["commands"]) != set(PATHS):
        raise ValueError("commands must separately declare every execution path")
    if set(commands["environment"]) != set(ENVIRONMENT_FIELDS) or not all(
        isinstance(value, str) and value for value in commands["environment"].values()
    ):
        raise ValueError("commands must pin hardware, OS, toolchains, dependencies, backend, flags, and inputs")
    for line in commands["commands"].values():
        if not isinstance(line, list) or not line or not all(isinstance(part, str) and part for part in line):
            raise ValueError("command is invalid")
        if "{fixture}" not in line or "{case}" not in line:
            raise ValueError("command must bind the exact fixture and case")


def identities(commands):
    rows = {}
    for name in ("bend", "semaprax"):
        root = pathlib.Path(commands[name]["root"])
        try:
            observed = git_head(root)
        except (OSError, subprocess.CalledProcessError):
            rows[name] = {"status": "unavailable", "reason": "subject root is not an accessible Git checkout"}
            continue
        expected = commands[name]["commit"]
        rows[name] = {"status": "ok" if observed == expected else "drifted", "expected": expected, "observed": observed}
    return rows


def invoke(template, cell, case, fixture, env, timeout):
    command = [part.format(cell=cell, case=case, fixture=str(fixture.resolve())) for part in template]
    started = time.perf_counter()
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=timeout, env=env)
        return {"status": "accepted" if result.returncode == 0 else "rejected", "fixture_sha256": digest(fixture), "wall_ms": round((time.perf_counter() - started) * 1000, 3), "exit_code": result.returncode, "stdout_sha256": "sha256:" + hashlib.sha256(result.stdout.encode()).hexdigest(), "stderr_sha256": "sha256:" + hashlib.sha256(result.stderr.encode()).hexdigest()}
    except FileNotFoundError:
        return {"status": "unavailable", "fixture_sha256": digest(fixture), "reason": "tool not found"}
    except subprocess.TimeoutExpired:
        return {"status": "timed_out", "fixture_sha256": digest(fixture), "reason": f"timeout after {timeout} seconds"}


def percentiles(samples):
    ordered = sorted(samples)
    return {"p50_ms": round(statistics.median(ordered), 3), "p95_ms": round(ordered[min(len(ordered) - 1, int(len(ordered) * .95))], 3), "samples_ms": ordered}


def execute(manifest, commands, samples, timeout):
    environment = dict(os.environ)
    environment.update(manifest["bend"]["telemetry_environment"])
    rows = []
    for cell in manifest["cells"]:
        fixture = pathlib.Path(cell["fixture"])
        if not fixture.is_absolute():
            fixture = pathlib.Path(manifest.get("_root", ".")) / fixture
        paths = {}
        for path in PATHS:
            cold = invoke(commands["commands"][path], cell["id"], "success", fixture, environment, timeout)
            attacks = {attack: invoke(commands["commands"][path], cell["id"], attack, fixture, environment, timeout) for attack in cell["attacks"]}
            if cold["status"] != "accepted":
                paths[path] = {"status": "unavailable", "cold": cold, "attacks": attacks}
                continue
            if any(result["status"] != "rejected" for result in attacks.values()):
                paths[path] = {"status": "failed", "cold": cold, "attacks": attacks, "reason": "law-gaming control was accepted"}
                continue
            warm = [invoke(commands["commands"][path], cell["id"], "success", fixture, environment, timeout) for _ in range(samples)]
            if any(result["status"] != "accepted" for result in warm):
                paths[path] = {"status": "unavailable", "cold": cold, "attacks": attacks, "warm": warm}
            else:
                paths[path] = {"status": "ok", "cold": cold, "attacks": attacks, "warm": percentiles([result["wall_ms"] for result in warm])}
        rows.append({"id": cell["id"], "numeric_domain": cell["numeric_domain"], "laws": cell["laws"], "paths": paths})
    return rows


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--commands", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--manifest", type=pathlib.Path, default=pathlib.Path(__file__).with_name("manifest.json"))
    parser.add_argument("--samples", type=int, default=30)
    parser.add_argument("--pilot", action="store_true")
    parser.add_argument("--timeout-seconds", type=float, default=120)
    args = parser.parse_args(argv)
    if args.samples < 1 or (args.samples < 30 and not args.pilot):
        parser.error("--samples must be at least 30 unless --pilot labels the smaller run")
    try:
        manifest = load(args.manifest, MANIFEST_SCHEMA); require_manifest(manifest, args.manifest.parent)
        manifest["_root"] = str(args.manifest.parent.resolve())
        commands = load(args.commands, COMMANDS_SCHEMA); require_commands(commands, manifest["bend"]["commit"])
    except (OSError, ValueError, json.JSONDecodeError) as error:
        parser.error(str(error))
    identity = identities(commands)
    fixture_digests = {cell["id"]: digest(args.manifest.parent / cell["fixture"]) for cell in manifest["cells"]}
    document = {"schema": SCHEMA, "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"), "manifest": {"path": str(args.manifest.resolve()), "sha256": digest(args.manifest)}, "fixture_digests": fixture_digests, "commands_sha256": digest(args.commands), "identities": identity, "environment": commands["environment"], "configuration": {"samples": args.samples, "trial_class": "pilot" if args.pilot else "admitted", "timeout_seconds": args.timeout_seconds, "bend_no_telemetry": True}, "nonclaims": ["no cross-host or superiority claim", "ordinary Bend and verdict kernel are separate paths", "SMT, Lean, and runtime paths are separate"], "cells": []}
    if any(row["status"] != "ok" for row in identity.values()):
        document["status"] = "unavailable"
        document["reason"] = "pinned subject identity is unavailable or drifted"
    else:
        document["cells"] = execute(manifest, commands, args.samples, args.timeout_seconds)
        states = [path["status"] for cell in document["cells"] for path in cell["paths"].values()]
        document["status"] = "failed" if "failed" in states else ("unavailable" if "unavailable" in states else "completed")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(canonical(document))
    return 0 if document["status"] == "completed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
