#!/usr/bin/env python3
"""Capture LAW-16 process-state samples without claiming OS-cache coldness."""
import argparse, hashlib, json, os, pathlib, statistics, subprocess, time

SCHEMA = "semaprax.bend2-law-benchmark.cold-warm-process-cell.v1"
MIN_SAMPLES = 30

def digest(path): return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
def invoke(argv, environment):
    started = time.monotonic_ns(); process = subprocess.run(argv, capture_output=True, env=environment)
    return {"argv": argv, "elapsed_ns": time.monotonic_ns() - started, "exit_code": process.returncode,
            "stdout_sha256": "sha256:" + hashlib.sha256(process.stdout).hexdigest(), "stderr_sha256": "sha256:" + hashlib.sha256(process.stderr).hexdigest()}
def summarize(samples):
    elapsed = [sample["elapsed_ns"] for sample in samples]
    if any(sample["exit_code"] != 0 for sample in samples): return {"status": "unavailable", "samples": samples}
    ordered = sorted(elapsed)
    return {"status": "completed", "samples": samples, "summary": {"count": len(samples), "p50_ns": statistics.median(elapsed), "p95_ns": ordered[(95 * len(ordered) + 99) // 100 - 1]}}
def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cold-command", required=True); parser.add_argument("--warm-command", required=True)
    parser.add_argument("--artifact", action="append", type=pathlib.Path, default=[]); parser.add_argument("--samples", type=int, default=MIN_SAMPLES)
    parser.add_argument("--output", required=True, type=pathlib.Path); args = parser.parse_args(argv)
    if args.samples < MIN_SAMPLES or args.output.exists(): parser.error("--samples must be at least 30 and --output must be new")
    if any(not artifact.is_file() for artifact in args.artifact): parser.error("every --artifact must be a regular file")
    environment = dict(os.environ, BEND_NO_TELEMETRY="1")
    document = {"schema": SCHEMA, "configuration": {"samples": args.samples, "BEND_NO_TELEMETRY": "1"},
        "artifacts": [{"path": str(path.resolve()), "sha256": digest(path)} for path in args.artifact],
        "cells": {"fresh_process": summarize([invoke(json.loads(args.cold_command), environment) for _ in range(args.samples)]), "repeat_process": summarize([invoke(json.loads(args.warm_command), environment) for _ in range(args.samples)])},
        "cold_state": {"status": "unavailable", "reason": "process separation cannot isolate macOS page, executable, solver, or tool caches"},
        "nonclaims": ["fresh_process is not an OS-cache-cold observation", "no cold-versus-warm ratio, winner, or cache-isolation claim"]}
    args.output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    return 0 if all(cell["status"] == "completed" for cell in document["cells"].values()) else 1
if __name__ == "__main__": raise SystemExit(main())
