#!/usr/bin/env python3
"""Time explicitly supplied HR-07 selectors without inventing unavailable evidence.

This runner deliberately knows the required cross-layer cells but not how to
build them.  A caller supplies an already-built test executable or another
exact selector command for each cell it intends to measure.  Missing commands
remain unavailable in the report; they are never reported as passing.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import platform
import statistics
import subprocess
import time

SUITE = pathlib.Path(__file__).resolve().parent
ROOT = SUITE.parent.parent
MANIFEST = SUITE / "cross-layer-manifest.json"
SCHEMA = "semaprax.hot-reload-cross-layer-report.v1"


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def validate(manifest):
    if set(manifest) != {"schema", "scope", "subject", "cells", "nonclaims"}:
        raise ValueError("cross-layer manifest has an unknown or missing top-level field")
    if manifest["schema"] != "semaprax.hot-reload-cross-layer-acceptance.v1":
        raise ValueError("cross-layer manifest has the wrong schema")
    if manifest["scope"] != "local-selector-timing":
        raise ValueError("cross-layer manifest has the wrong scope")
    if manifest["subject"] != {"revision": "git-head-at-run", "binary_digest": "captured-per-selector-in-report"}:
        raise ValueError("cross-layer manifest does not bind its run-time subject")
    if not isinstance(manifest["cells"], list) or not manifest["cells"]:
        raise ValueError("cross-layer manifest has no cells")
    ids = set()
    for cell in manifest["cells"]:
        if set(cell) != {"id", "availability", "selector", "requires"}:
            raise ValueError("cross-layer cell has an unknown or missing field")
        if not isinstance(cell["id"], str) or not cell["id"] or cell["id"] in ids:
            raise ValueError("cross-layer cell IDs must be nonempty and unique")
        ids.add(cell["id"])
        if cell["availability"] not in {"runnable-with-prebuilt-semaprax", "selector-required", "unavailable"}:
            raise ValueError("cross-layer cell has an unknown availability")
        if cell["availability"] == "unavailable":
            if cell["selector"] is not None:
                raise ValueError("unavailable cells cannot name a selector")
        elif not isinstance(cell["selector"], str) or not cell["selector"]:
            raise ValueError("runnable cells must name their selector")
        if not isinstance(cell["requires"], list) or not cell["requires"] or not all(isinstance(item, str) and item for item in cell["requires"]):
            raise ValueError("cross-layer cell has invalid requirements")
    unavailable = {cell["id"] for cell in manifest["cells"] if cell["availability"] == "unavailable"}
    if unavailable != {"native-process-identity", "native-or-wasm-state-swap"}:
        raise ValueError("cross-layer manifest must retain its explicit unavailable cells")
    if "committed timing samples" not in manifest["nonclaims"]:
        raise ValueError("cross-layer manifest must not claim samples before a run")


def parse_command(raw):
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as error:
        raise ValueError("--selector-command must be JSON") from error
    if set(value) != {"id", "argv"} or not isinstance(value["id"], str):
        raise ValueError("selector command must have exactly id and argv")
    if not isinstance(value["argv"], list) or not value["argv"] or not all(isinstance(arg, str) and arg for arg in value["argv"]):
        raise ValueError("selector command argv must be a nonempty string array")
    return value


def summarize(values):
    ordered = sorted(values)
    count = len(ordered)
    return {
        "samples": count,
        "median_ms": round(statistics.median(ordered), 3),
        "p95_ms": round(ordered[min(count - 1, max(0, (95 * count + 99) // 100 - 1))], 3),
        "values_ms": [round(value, 3) for value in values],
    }


def invoke(argv):
    executable = pathlib.Path(argv[0])
    if not executable.is_absolute() or not executable.is_file() or not os.access(executable, os.X_OK):
        raise RuntimeError("selector argv[0] must name an absolute executable file")
    executable = executable.resolve()
    executable_digest = digest(executable)
    argv = [str(executable), *argv[1:]]
    started = time.perf_counter_ns()
    completed = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=False)
    elapsed = (time.perf_counter_ns() - started) / 1_000_000
    if digest(executable) != executable_digest:
        raise RuntimeError("selector executable changed while it was measured")
    if completed.returncode:
        raise RuntimeError("selector failed (%s): %s" % (completed.returncode, " ".join(argv)))
    return elapsed, {
        "executable": str(executable),
        "executable_sha256": executable_digest,
        "stdout_sha256": "sha256:" + hashlib.sha256(completed.stdout).hexdigest(),
        "stderr_sha256": "sha256:" + hashlib.sha256(completed.stderr).hexdigest(),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--selector-command", action="append", default=[], metavar="JSON")
    parser.add_argument("--output", type=pathlib.Path, default=pathlib.Path("/tmp/hot-reload-cross-layer.json"))
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_text())
    validate(manifest)
    if args.samples < 1:
        raise SystemExit("--samples must be positive")
    commands = [parse_command(raw) for raw in args.selector_command]
    known = {cell["id"] for cell in manifest["cells"] if cell["availability"] != "unavailable"}
    supplied = {command["id"] for command in commands}
    if len(supplied) != len(commands) or not supplied <= known:
        raise SystemExit("selector commands must name distinct runnable manifest cells")
    if args.dry_run:
        args.output.write_text(canonical({"schema": SCHEMA, "mode": "plan", "acceptance_manifest_digest": digest(MANIFEST), "cells": manifest["cells"]}))
        return
    by_id = {command["id"]: command["argv"] for command in commands}
    cells = {}
    for cell in manifest["cells"]:
        command = by_id.get(cell["id"])
        if command is None:
            cells[cell["id"]] = {"status": "unavailable", "reason": "no selector command was supplied", "selector": cell["selector"]}
            continue
        samples, outputs = [], []
        for _ in range(args.samples):
            elapsed, output = invoke(command)
            samples.append(elapsed)
            outputs.append(output)
        cells[cell["id"]] = {"status": "measured", "selector": cell["selector"], "argv": command, "timing": summarize(samples), "output_digests": outputs}
    report = {
        "schema": SCHEMA,
        "acceptance_manifest_digest": digest(MANIFEST),
        "repository_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "host": {"system": platform.system(), "release": platform.release(), "machine": platform.machine(), "cpu_count": os.cpu_count()},
        "samples_requested": args.samples,
        "cells": cells,
        "nonclaims": manifest["nonclaims"],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(canonical(report))


if __name__ == "__main__":
    main()
