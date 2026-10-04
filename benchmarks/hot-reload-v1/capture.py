#!/usr/bin/env python3
"""Capture one current-head HR-07 interpreter receipt with honest native/Wasm cells.

The runner accepts only an already-built Semaprax executable. It delegates the
interpreter measurement to run.py, binds the report bytes with SHA-256, and
copies the two native/Wasm unavailable cells from the committed cross-layer
manifest. It never builds a selector or promotes an unavailable cell.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import platform
import subprocess
import sys
import tempfile

SUITE = pathlib.Path(__file__).resolve().parent
ROOT = SUITE.parent.parent
BENCHMARK_MANIFEST = SUITE / "manifest.json"
CROSS_LAYER_MANIFEST = SUITE / "cross-layer-manifest.json"
RUNNER = SUITE / "run.py"
SCHEMA = "semaprax.hot-reload-evidence-capture.v1"
NATIVE_WASM_CELLS = ("native-process-identity", "native-or-wasm-state-swap")


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def read_cross_layer_manifest():
    manifest = json.loads(CROSS_LAYER_MANIFEST.read_text())
    if manifest.get("schema") != "semaprax.hot-reload-cross-layer-acceptance.v1":
        raise ValueError("cross-layer manifest has an unexpected schema")
    cells = {cell.get("id"): cell for cell in manifest.get("cells", [])}
    if set(NATIVE_WASM_CELLS) - set(cells):
        raise ValueError("cross-layer manifest is missing the native/Wasm cells")
    for identifier in NATIVE_WASM_CELLS:
        cell = cells[identifier]
        if cell.get("availability") != "unavailable" or cell.get("selector") is not None:
            raise ValueError("native/Wasm cells must remain explicitly unavailable")
    return manifest, cells


def current_commit():
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()


def plan(samples, warmups):
    manifest, cells = read_cross_layer_manifest()
    return {
        "schema": SCHEMA,
        "mode": "plan",
        "benchmark_acceptance_manifest_digest": digest(BENCHMARK_MANIFEST),
        "cross_layer_acceptance_manifest_digest": digest(CROSS_LAYER_MANIFEST),
        "repository_commit": current_commit(),
        "samples": samples,
        "warmups": warmups,
        "cells": {
            "interpreter-a-b": {"status": "requires-prebuilt-semaprax", "selector": "benchmarks/hot-reload-v1/run.py"},
            **{identifier: {"status": "unavailable", "selector": None, "requires": cells[identifier]["requires"]} for identifier in NATIVE_WASM_CELLS},
        },
        "nonclaims": manifest["nonclaims"],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--semaprax", type=pathlib.Path)
    parser.add_argument("--samples", type=int, default=11)
    parser.add_argument("--warmups", type=int, default=3)
    parser.add_argument("--expected-commit")
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if args.samples < 1 or args.warmups < 0:
        raise SystemExit("--samples must be positive and --warmups must be nonnegative")
    if args.dry_run:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(canonical(plan(args.samples, args.warmups)))
        return
    if args.semaprax is None or not args.semaprax.is_file():
        raise SystemExit("--semaprax must name an already-built executable")
    commit = current_commit()
    if args.expected_commit is not None and args.expected_commit != commit:
        raise SystemExit("--expected-commit must match the current checkout HEAD")
    expected = commit
    with tempfile.TemporaryDirectory(prefix="semaprax-hot-reload-capture-") as directory:
        benchmark_output = pathlib.Path(directory) / "benchmark.json"
        command = [
            sys.executable, str(RUNNER), "--semaprax", str(args.semaprax.resolve()),
            "--samples", str(args.samples), "--warmups", str(args.warmups),
            "--expected-commit", expected, "--output", str(benchmark_output),
        ]
        completed = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=False)
        if completed.returncode:
            raise RuntimeError("interpreter benchmark failed: " + completed.stderr.decode("utf-8", "replace"))
        report = json.loads(benchmark_output.read_text())
        if report.get("schema") != "semaprax.hot-reload-benchmark.v1":
            raise RuntimeError("interpreter benchmark emitted an unexpected schema")
        if report.get("compiler", {}).get("git_commit") != commit or report["compiler"].get("embedded_commit") != expected:
            raise RuntimeError("interpreter benchmark did not bind the expected current-head compiler")
        manifest, cells = read_cross_layer_manifest()
        value = {
            "schema": SCHEMA,
            "benchmark_acceptance_manifest_digest": digest(BENCHMARK_MANIFEST),
            "cross_layer_acceptance_manifest_digest": digest(CROSS_LAYER_MANIFEST),
            "repository_commit": commit,
            "host": {"system": platform.system(), "release": platform.release(), "machine": platform.machine()},
            "samples": args.samples,
            "warmups": args.warmups,
            "cells": {
                "interpreter-a-b": {
                    "status": "measured",
                    "selector": "benchmarks/hot-reload-v1/run.py",
                    "benchmark_report_sha256": digest(benchmark_output),
                    "compiler": report["compiler"],
                    "summary": report["loops"]["interpreter-save-to-ack"]["summary"],
                    "runner_stdout_sha256": "sha256:" + hashlib.sha256(completed.stdout).hexdigest(),
                    "runner_stderr_sha256": "sha256:" + hashlib.sha256(completed.stderr).hexdigest(),
                },
                **{identifier: {"status": "unavailable", "selector": None, "requires": cells[identifier]["requires"]} for identifier in NATIVE_WASM_CELLS},
            },
            "nonclaims": manifest["nonclaims"],
        }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(canonical(value))


if __name__ == "__main__":
    main()
