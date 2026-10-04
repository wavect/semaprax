#!/usr/bin/env python3
"""Render a digest-bound LAW-16 report without inventing a comparison result.

The runner's receipt remains the source of truth.  This renderer retains its
per-path raw samples and law-gaming observations, derives variation only from
those samples, and makes every unavailable or failed path visible.  It has no
winner, ratio, score, or cross-path aggregation.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import pathlib
from datetime import datetime, timezone


INPUT_SCHEMA = "semaprax.bend2-law-benchmark.result.v1"
SCHEMA = "semaprax.bend2-law-benchmark.report.v1"
PATHS = ("bend_normal", "bend_verdict", "semaprax_smt", "semaprax_lean", "semaprax_runtime")


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest(path: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def read_result(path: pathlib.Path) -> dict:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read benchmark result {path}") from error
    if not isinstance(value, dict) or value.get("schema") != INPUT_SCHEMA:
        raise ValueError("benchmark result has unsupported schema")
    if not isinstance(value.get("cells"), list) or not isinstance(value.get("identities"), dict):
        raise ValueError("benchmark result lacks cells or pinned identities")
    return value


def variation(samples: object) -> dict:
    if not isinstance(samples, list) or not samples or any(not isinstance(value, (int, float)) for value in samples):
        raise ValueError("completed path lacks numeric raw warm samples")
    values = [float(value) for value in samples]
    mean = sum(values) / len(values)
    variance = sum((value - mean) ** 2 for value in values) / len(values)
    return {
        "sample_count": len(values),
        "min_ms": min(values),
        "max_ms": max(values),
        "mean_ms": round(mean, 3),
        "population_stddev_ms": round(math.sqrt(variance), 3),
        "raw_samples_ms": samples,
    }


def path_report(path: str, observed: object) -> dict:
    if not isinstance(observed, dict) or observed.get("status") not in {"ok", "unavailable", "failed"}:
        raise ValueError(f"{path} has an invalid path status")
    row = {
        "path": path,
        "status": observed["status"],
        "cold": observed.get("cold"),
        "law_gaming_attacks": observed.get("attacks", {}),
    }
    if observed["status"] == "ok":
        warm = observed.get("warm")
        if not isinstance(warm, dict):
            raise ValueError(f"{path} completed without a warm summary")
        samples = warm.get("samples_ms")
        row["warm"] = {
            "p50_ms": warm.get("p50_ms"),
            "p95_ms": warm.get("p95_ms"),
            "variation": variation(samples),
        }
    else:
        row["reason"] = observed.get("reason", "path did not produce admitted timing evidence")
        if "warm" in observed:
            row["warm"] = observed["warm"]
    return row


def render(result_path: pathlib.Path) -> dict:
    result = read_result(result_path)
    cells = []
    for cell in result["cells"]:
        if not isinstance(cell, dict) or not isinstance(cell.get("id"), str) or not isinstance(cell.get("paths"), dict):
            raise ValueError("benchmark result has malformed cell evidence")
        if set(cell["paths"]) != set(PATHS):
            raise ValueError("benchmark result does not retain every distinct execution path")
        cells.append({
            "id": cell["id"],
            "numeric_domain": cell.get("numeric_domain"),
            "laws": cell.get("laws"),
            "paths": [path_report(path, cell["paths"][path]) for path in PATHS],
        })
    return {
        "schema": SCHEMA,
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "benchmark_result": {"path": str(result_path.resolve()), "sha256": digest(result_path), "status": result.get("status")},
        "pinned_subjects": result["identities"],
        "environment": result.get("environment"),
        "configuration": result.get("configuration"),
        "artifacts": {
            "manifest": result.get("manifest"),
            "commands_sha256": result.get("commands_sha256"),
            "fixture_digests": result.get("fixture_digests"),
        },
        "trusted_computing_base": [
            "the exact pinned Bend checkout and its ordinary checker",
            "the exact pinned Bend checkout and its separately invoked verdict kernel",
            "the exact pinned SEMAPRAX checkout and each declared SMT, Lean, or runtime command",
            "the recorded local operating system, toolchains, dependencies, hardware, and command wrappers",
            "the report renderer, which validates receipt shape and derives variation from retained raw samples",
        ],
        "cells": cells,
        "nonclaims": [
            "no cross-path timing ratio, score, winner, or superiority claim",
            "ordinary Bend checking and Bend verdict evidence remain separate",
            "unavailable, timed-out, or failed paths are nonresults and cannot establish a win",
            "reported variation describes retained local samples only and does not establish cross-host significance",
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--benchmark-result", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        document = render(args.benchmark_result)
    except ValueError as error:
        parser.error(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(canonical(document))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
