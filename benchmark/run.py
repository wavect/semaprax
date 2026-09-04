#!/usr/bin/env python3
"""
Performance macrobenchmark runner for semaprax.

Usage:
  python3 benchmark/run.py --output benchmark/results/local.json
  python3 benchmark/run.py --with-build --output /tmp/with-build.json
  python3 benchmark/run.py --compare benchmark/results/baseline.json --output /tmp/compare.json
  python3 benchmark/run.py --quick --output /tmp/quick.json
"""
import argparse
import hashlib
import json
import pathlib
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCENARIOS = ROOT / "benchmark" / "scenarios.json"
SEMAPRAX = ["cargo", "run", "--locked", "-p", "semaprax", "--"]

def sha256_file(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    h.update(path.read_bytes())
    return "sha256:" + h.hexdigest()

def git_commit() -> str:
    try:
        out = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True)
        return out.strip()[:8]
    except Exception:
        return "unknown"

def rustc_version() -> str:
    try:
        out = subprocess.check_output(["rustc", "--version"], text=True)
        # rustc 1.88.0 (6b00bc388 2025-06-23)
        return out.split()[1]
    except Exception:
        return "unknown"

def run_one(scenario: dict, quick: bool, with_build: bool) -> dict:
    path = ROOT / scenario["path"]
    command = scenario["command"]
    reps = 2 if quick else scenario.get("repetitions", 5)
    args = scenario.get("args", [])

    # Digest
    try:
        digest = sha256_file(path)
    except FileNotFoundError:
        return {
            "id": scenario["id"],
            "command": command,
            "path": scenario["path"],
            "repetitions": reps,
            "wall_ms": {"p50": 0, "p95": 0, "samples": []},
            "digest": "missing",
            "status": "skipped",
            "reason": "path not found",
        }

    # Build command line
    # For context, need stable-id; for build, need target etc.
    # For our scenarios, command is check/graph/run/test/context (and build with --with-build)
    # We handle build separately via with_build flag; otherwise use command as is.
    if command == "build" and not with_build:
        return {
            "id": scenario["id"],
            "command": command,
            "path": scenario["path"],
            "repetitions": reps,
            "wall_ms": {"p50": 0, "p95": 0, "samples": []},
            "digest": digest,
            "status": "skipped",
            "reason": "requires --with-build",
        }

    # Context needs extra args; for our scenarios, context already has args
    # For build, scenario would have args like ["--target","web","-o","/tmp/..."]
    # We synthesize build args if with_build and command is check but we want build variant
    # Instead, our scenarios.json separates build scenarios as command=build with args
    # So just use command+path+args
    cli = SEMAPRAX + [command, str(path)] + args
    # For build, need -o; if scenario is build and args doesn't contain -o, synthesize
    if command == "build" and "-o" not in args:
        # Use temp dir per scenario
        tmp = pathlib.Path("/tmp") / f"semaprax-bench-{scenario['id']}"
        cli += ["-o", str(tmp)]

    samples = []
    status = "ok"
    reason = ""
    for _ in range(reps):
        start = time.perf_counter()
        try:
            result = subprocess.run(
                cli,
                cwd=ROOT,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=30,
            )
            elapsed_ms = (time.perf_counter() - start) * 1000
            samples.append(round(elapsed_ms, 2))
            if result.returncode != 0:
                status = "skipped"
                reason = f"exit {result.returncode}"
                break
        except subprocess.TimeoutExpired:
            samples.append(30000)
            status = "skipped"
            reason = "timeout"
            break
        except Exception as e:
            samples.append(0)
            status = "skipped"
            reason = str(e)
            break

    # p50/p95
    if samples:
        sorted_s = sorted(samples)
        p50 = statistics.median(sorted_s)
        # p95: 95th percentile
        idx = int(0.95 * len(sorted_s))
        if idx >= len(sorted_s):
            idx = len(sorted_s) - 1
        p95 = sorted_s[idx]
    else:
        p50 = p95 = 0

    out = {
        "id": scenario["id"],
        "command": command,
        "path": scenario["path"],
        "repetitions": reps,
        "wall_ms": {"p50": round(p50, 2), "p95": round(p95, 2), "samples": samples},
        "digest": digest,
        "status": status,
    }
    if reason:
        out["reason"] = reason
    if args:
        out["args"] = args
    return out

def main():
    parser = argparse.ArgumentParser(description="semaprax macro benchmark runner")
    parser.add_argument("--output", required=True, help="output JSON path")
    parser.add_argument("--compare", help="baseline JSON to compare against")
    parser.add_argument("--with-build", action="store_true", help="include build scenarios (requires Clang/wasm)")
    parser.add_argument("--quick", action="store_true", help="2 repetitions instead of 5")
    args = parser.parse_args()

    scenarios_data = json.loads(SCENARIOS.read_text())
    scenarios = scenarios_data["scenarios"]
    # Sort by id for determinism
    scenarios = sorted(scenarios, key=lambda s: s["id"])

    results = []
    for sc in scenarios:
        # Skip build scenarios without --with-build
        if sc["command"] == "build" and not args.with_build:
            continue
        print(f"[{sc['id']}] {sc['command']} {sc['path']} ...", flush=True)
        res = run_one(sc, quick=args.quick, with_build=args.with_build)
        results.append(res)
        print(f"  -> p50={res['wall_ms']['p50']}ms p95={res['wall_ms']['p95']}ms status={res['status']}", flush=True)

    output = {
        "schema": "benchmark.performance.v1",
        "host": "darwin-arm64",
        "rustc": rustc_version(),
        "commit": git_commit(),
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "scenarios": results,
    }

    out_path = pathlib.Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    # canonical JSON: sorted keys, 2-space indent, terminal LF
    out_path.write_text(json.dumps(output, sort_keys=True, indent=2) + "\n")
    print(f"Wrote {out_path} ({len(results)} scenarios)")

    if args.compare:
        baseline = json.loads(pathlib.Path(args.compare).read_text())
        baseline_map = {s["id"]: s for s in baseline.get("scenarios", [])}
        print("\nComparison (local vs baseline):")
        for res in results:
            base = baseline_map.get(res["id"])
            if not base:
                print(f"  {res['id']}: no baseline")
                continue
            base_p50 = base["wall_ms"]["p50"]
            local_p50 = res["wall_ms"]["p50"]
            if base_p50 == 0:
                delta = 0
            else:
                delta = (local_p50 - base_p50) / base_p50
            flag = ""
            if delta > 0.15:
                flag = " REGRESSION"
            elif delta < -0.15:
                flag = " improvement"
            print(f"  {res['id']}: baseline {base_p50}ms -> local {local_p50}ms delta {delta:+.1%}{flag}")

if __name__ == "__main__":
    main()
