#!/usr/bin/env python3
"""Measure pinned Boolean routes with raw cold/warm samples and honest memory scope."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import platform
import re
import statistics
import subprocess
import time
from datetime import datetime, timezone


SCHEMA = "semaprax.bend2-law-benchmark.boolean-measurement.v1"
PINNED_BEND_COMMIT = "947db722640c86247849343657bf2f7ef01cb7f1"
MIN_WARM_SAMPLES = 30


def digest(path: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def git_head(root: pathlib.Path) -> str:
    return subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True, stderr=subprocess.DEVNULL).strip()


def artifact(directory: pathlib.Path, name: str, value: str) -> dict:
    path = directory / name
    path.write_text(value)
    return {"path": name, "sha256": digest(path)}


def memory_wrapper(command: list[str], path: pathlib.Path) -> tuple[list[str], str]:
    if platform.system() == "Darwin" and pathlib.Path("/usr/bin/time").is_file():
        return ["/usr/bin/time", "-l", "-o", str(path), *command], "darwin_time_l_bytes"
    if platform.system() == "Linux" and pathlib.Path("/usr/bin/time").is_file():
        return ["/usr/bin/time", "-f", "%M", "-o", str(path), *command], "gnu_time_maxrss_kib"
    return command, "unavailable"


def parse_memory(kind: str, path: pathlib.Path) -> dict:
    if kind == "unavailable":
        return {"status": "unavailable", "reason": "no supported per-command peak-RSS wrapper"}
    try:
        text = path.read_text()
    except OSError:
        return {"status": "unavailable", "reason": "peak-RSS wrapper produced no observation"}
    if kind == "darwin_time_l_bytes":
        match = re.search(r"(\d+)\s+maximum resident set size", text)
        unit = "bytes"
    else:
        match = re.search(r"^\s*(\d+)\s*$", text)
        unit = "KiB"
    if not match:
        return {"status": "unavailable", "reason": "peak-RSS wrapper output was unparseable", "raw_sha256": "sha256:" + hashlib.sha256(text.encode()).hexdigest()}
    value = int(match.group(1))
    return {"status": "observed", "peak_rss": value if unit == "bytes" else value * 1024, "unit": "bytes", "raw_sha256": "sha256:" + hashlib.sha256(text.encode()).hexdigest()}


def invoke(directory: pathlib.Path, route: str, phase: str, ordinal: int, command: list[str], environment: dict[str, str]) -> dict:
    memory_path = directory / f"{route}-{phase}-{ordinal}.memory"
    wrapped, memory_kind = memory_wrapper(command, memory_path)
    started = time.perf_counter_ns()
    try:
        result = subprocess.run(wrapped, capture_output=True, text=True, env=environment)
    except FileNotFoundError:
        return {"status": "unavailable", "reason": "tool not found"}
    elapsed_ns = time.perf_counter_ns() - started
    return {
        "status": "accepted" if result.returncode == 0 else "rejected",
        "exit_code": result.returncode,
        "elapsed_ns": elapsed_ns,
        "memory": parse_memory(memory_kind, memory_path),
        "stdout": artifact(directory, f"{route}-{phase}-{ordinal}.stdout", result.stdout),
        "stderr": artifact(directory, f"{route}-{phase}-{ordinal}.stderr", result.stderr),
    }


def expected(row: dict, stdout_path: pathlib.Path, exact_stdout: str) -> dict:
    if row["status"] == "unavailable":
        return row
    if row["status"] != "accepted" or stdout_path.read_text() != exact_stdout:
        row["status"] = "failed"
        row["reason"] = "route did not accept the exact Boolean success witness"
    return row


def percentile(samples: list[int], fraction: float) -> float:
    ordered = sorted(samples)
    position = (len(ordered) - 1) * fraction
    lower, upper = int(position), min(int(position) + 1, len(ordered) - 1)
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def summarize(cold: dict, warm: list[dict]) -> dict:
    if cold["status"] != "accepted" or any(row["status"] != "accepted" for row in warm):
        return {"status": "unavailable", "cold": cold, "warm": warm, "reason": "route did not complete every required sample"}
    samples = [row["elapsed_ns"] for row in warm]
    memory = [row["memory"] for row in warm]
    observed = [row["peak_rss"] for row in memory if row["status"] == "observed"]
    return {
        "status": "completed",
        "cold": cold,
        "warm_samples": warm,
        "summary": {
            "warm_sample_count": len(samples),
            "p50_ns": round(percentile(samples, 0.50), 1),
            "p95_ns": round(percentile(samples, 0.95), 1),
            "mean_ns": round(statistics.mean(samples), 1),
            "peak_rss_bytes": {"status": "observed", "max": max(observed), "samples": observed} if len(observed) == len(memory) else {"status": "unavailable", "reason": "one or more samples lack attributable peak RSS", "samples": memory},
        },
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bend-root", required=True, type=pathlib.Path)
    parser.add_argument("--bun", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax-root", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax-commit", required=True)
    parser.add_argument("--artifact-dir", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--warm-samples", type=int, default=MIN_WARM_SAMPLES)
    args = parser.parse_args(argv)
    if args.warm_samples < MIN_WARM_SAMPLES:
        parser.error(f"--warm-samples must be at least {MIN_WARM_SAMPLES}")
    root = pathlib.Path(__file__).parent
    document = {
        "schema": SCHEMA,
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "configuration": {"warm_samples": args.warm_samples, "BEND_NO_TELEMETRY": "1"},
        "identities": {"bend": {"expected": PINNED_BEND_COMMIT}, "semaprax": {"expected": args.semaprax_commit}},
        "paths": {},
        "unavailable_routes": {
            "semaprax_smt": "no compiler-owned Boolean SMT command is supplied by this bounded runner",
            "semaprax_lean": "no external Lean Boolean command is supplied by this bounded runner",
        },
        "nonclaims": [
            "ordinary Bend and Bend verdict timings are separate observations",
            "no route-to-route ratio, winner, GPU, or runtime-throughput claim",
            "this Boolean microcell does not establish the checked-u32 benchmark cells",
        ],
    }
    try:
        document["identities"]["bend"]["observed"] = git_head(args.bend_root)
        document["identities"]["semaprax"]["observed"] = git_head(args.semaprax_root)
    except (OSError, subprocess.CalledProcessError):
        document.update({"status": "unavailable", "reason": "a subject root is not an accessible Git checkout"})
    else:
        if document["identities"]["bend"]["observed"] != PINNED_BEND_COMMIT or document["identities"]["semaprax"]["observed"] != args.semaprax_commit:
            document.update({"status": "unavailable", "reason": "a subject root differs from its pinned commit"})
        elif not args.bun.is_file() or not args.semaprax.is_file():
            document.update({"status": "unavailable", "reason": "a pinned executable is unavailable"})
        else:
            fixtures = {
                "bend": root / "fixtures/bend-two-value-boolean-v1.bend",
                "semaprax": root / "fixtures/semaprax-two-value-boolean-v1.spx",
            }
            if not all(path.is_file() for path in fixtures.values()):
                document.update({"status": "unavailable", "reason": "a Boolean success fixture is unavailable"})
            else:
                args.artifact_dir.mkdir(parents=True, exist_ok=True)
                document["fixtures"] = {name: {"path": str(path.resolve()), "sha256": digest(path)} for name, path in fixtures.items()}
                document["executables"] = {"bun_sha256": digest(args.bun), "semaprax_sha256": digest(args.semaprax)}
                environment = dict(os.environ, BEND_NO_TELEMETRY="1")
                routes = {
                    "bend_normal": ([str(args.bun), str(args.bend_root / "bend2/main.ts"), str(fixtures["bend"])], "0\n1\n"),
                    "bend_verdict": ([str(args.bun), str(args.bend_root / "bend2/main.ts"), str(fixtures["bend"]), "--verdict"], "ALL PROOFS CHECK\n"),
                    "semaprax_runtime": ([str(args.semaprax), "run", str(fixtures["semaprax"])], "0\n"),
                }
                for name, (command, exact) in routes.items():
                    cold = invoke(args.artifact_dir, name, "cold", 0, command, environment)
                    cold = expected(cold, args.artifact_dir / f"{name}-cold-0.stdout", exact) if "stdout" in cold else cold
                    warm = []
                    for ordinal in range(1, args.warm_samples + 1):
                        row = invoke(args.artifact_dir, name, "warm", ordinal, command, environment)
                        warm.append(expected(row, args.artifact_dir / f"{name}-warm-{ordinal}.stdout", exact) if "stdout" in row else row)
                    document["paths"][name] = summarize(cold, warm)
                document["status"] = "completed" if all(row["status"] == "completed" for row in document["paths"].values()) else "unavailable"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(canonical(document))
    return 0 if document["status"] == "completed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
