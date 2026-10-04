#!/usr/bin/env python3
"""Validate and summarize one pair of RI-13 nontrivial M3 batch CSVs."""

import argparse
import csv
import hashlib
import json
import statistics
from pathlib import Path


SCHEMA = "semaprax.ri13.m3-nontrivial-batch-summary.v1"
ROUTES = ("direct_rust", "handwritten_adapter", "generated_semaprax")
FIELDS = (
    "route",
    "iteration",
    "operations",
    "elapsed_ns",
    "body_bytes",
    "foreign_response_body_copied_bytes",
    "host_callback_captured_bytes",
    "allocation_calls",
    "deallocation_calls",
    "reallocation_calls",
    "allocated_bytes",
    "deallocated_bytes",
)
ALLOCATION_FIELDS = FIELDS[-5:]
SAMPLES_PER_ROUTE = 5
OPERATIONS_PER_BATCH = 16
BODY_BYTES_PER_OPERATION = 4096
FOREIGN_COPIED_BYTES_PER_BATCH = OPERATIONS_PER_BATCH * BODY_BYTES_PER_OPERATION
INVESTIGATION_THRESHOLD = 0.90


def integer(row, field, path):
    try:
        value = int(row[field])
    except (KeyError, TypeError, ValueError) as error:
        raise ValueError(f"{path}: invalid {field}") from error
    if value < 0:
        raise ValueError(f"{path}: negative {field}")
    return value


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def load(path):
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            if tuple(reader.fieldnames or ()) != FIELDS:
                raise ValueError(f"{path}: unexpected CSV fields")
            rows = list(reader)
    except OSError as error:
        raise ValueError(f"cannot read {path}") from error
    if len(rows) != len(ROUTES) * SAMPLES_PER_ROUTE:
        raise ValueError(f"{path}: missing or extra batch rows")

    samples = {route: {} for route in ROUTES}
    for row in rows:
        route = row.get("route")
        if route not in samples:
            raise ValueError(f"{path}: unknown route {route!r}")
        iteration = integer(row, "iteration", path)
        if iteration >= SAMPLES_PER_ROUTE or iteration in samples[route]:
            raise ValueError(f"{path}: duplicate or out-of-range {route} iteration")
        if integer(row, "operations", path) != OPERATIONS_PER_BATCH:
            raise ValueError(f"{path}: unexpected operations per batch")
        if integer(row, "body_bytes", path) != BODY_BYTES_PER_OPERATION:
            raise ValueError(f"{path}: unexpected body bytes")
        if integer(row, "elapsed_ns", path) == 0:
            raise ValueError(f"{path}: zero elapsed time")
        if integer(row, "foreign_response_body_copied_bytes", path) != FOREIGN_COPIED_BYTES_PER_BATCH:
            raise ValueError(f"{path}: foreign response copy total changed")
        callback_copies = integer(row, "host_callback_captured_bytes", path)
        expected_callback_copies = FOREIGN_COPIED_BYTES_PER_BATCH if route == "generated_semaprax" else 0
        if callback_copies != expected_callback_copies:
            raise ValueError(f"{path}: callback copy total changed for {route}")
        samples[route][iteration] = {
            "elapsed_ns": integer(row, "elapsed_ns", path),
            **{field: integer(row, field, path) for field in ALLOCATION_FIELDS},
        }
    if any(set(iterations) != set(range(SAMPLES_PER_ROUTE)) for iterations in samples.values()):
        raise ValueError(f"{path}: each route must have exactly five iterations")
    return samples


def route_summary(samples):
    result = {}
    for route, iterations in samples.items():
        values = list(iterations.values())
        elapsed = statistics.median(value["elapsed_ns"] for value in values)
        result[route] = {
            "p50_elapsed_ns": elapsed,
            "p50_elapsed_ms": round(elapsed / 1_000_000, 6),
            "p50_allocator_requests": {
                field: statistics.median(value[field] for value in values)
                for field in ALLOCATION_FIELDS
            },
        }
    return result


def summarize(baseline_path, candidate_path):
    baseline = route_summary(load(baseline_path))
    candidate = route_summary(load(candidate_path))
    before = baseline["generated_semaprax"]
    after = candidate["generated_semaprax"]
    direct = candidate["direct_rust"]
    ratio = direct["p50_elapsed_ns"] / after["p50_elapsed_ns"]
    return {
        "schema": SCHEMA,
        "workload": {
            "routes": list(ROUTES),
            "samples_per_route": SAMPLES_PER_ROUTE,
            "operations_per_batch": OPERATIONS_PER_BATCH,
            "body_bytes_per_operation": BODY_BYTES_PER_OPERATION,
            "foreign_response_body_copied_bytes_per_batch": FOREIGN_COPIED_BYTES_PER_BATCH,
        },
        "inputs": [
            {"label": "baseline", "path": str(baseline_path), "sha256": digest(baseline_path)},
            {"label": "candidate", "path": str(candidate_path), "sha256": digest(candidate_path)},
        ],
        "runs": {"baseline": baseline, "candidate": candidate},
        "comparison": {
            "generated_p50_elapsed_reduction_fraction": round(
                1 - after["p50_elapsed_ns"] / before["p50_elapsed_ns"], 4
            ),
            "generated_p50_allocation_calls_reduction_fraction": round(
                1
                - after["p50_allocator_requests"]["allocation_calls"]
                / before["p50_allocator_requests"]["allocation_calls"],
                4,
            ),
            "candidate_generated_to_direct_normalized_throughput_ratio": round(ratio, 4),
            "investigation_threshold": INVESTIGATION_THRESHOLD,
            "status": "investigation_required" if ratio < INVESTIGATION_THRESHOLD else "threshold_not_triggered",
        },
        "limitations": [
            "Both inputs are five-sample local Darwin arm64 loopback measurements, not a portability or production result.",
            "The generated route includes per-operation registration and checked-source preparation; this summary does not attribute elapsed time to individual operations.",
            "The copy counters cover only the fixture-owned response capture and generated callback subset; foreign internal copies remain unmeasured.",
        ],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args(argv)
    try:
        result = summarize(arguments.baseline, arguments.candidate)
    except ValueError as error:
        parser.error(str(error))
    arguments.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
