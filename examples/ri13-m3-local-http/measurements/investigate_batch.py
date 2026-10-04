#!/usr/bin/env python3
"""Render a bounded investigation for an RI-13 M3 batch receipt.

The report retains measurements. This tool only derives ratios and per-operation
allocator-request differences, then binds the source observations used to
explain why the present generated route repeats preparation for every call.
"""

import argparse
import hashlib
import json
import pathlib


SCHEMA = "semaprax.ri13.m3-batch-investigation.v1"
REPORT_SCHEMA = "semaprax.ri13.combination-measurement.v1"
ROUTES = ("direct_rust", "handwritten_adapter", "generated_semaprax")
ALLOCATION_COLUMNS = (
    "allocation_calls",
    "deallocation_calls",
    "reallocation_calls",
    "allocated_bytes",
    "deallocated_bytes",
)
THRESHOLD = 0.90


def canonical(value):
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def load(path):
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read batch receipt {path}") from error
    if not isinstance(value, dict) or value.get("schema") != REPORT_SCHEMA:
        raise ValueError("batch receipt has an unsupported schema")
    return value


def batch_routes(report):
    batch = report.get("batch_throughput")
    command = report.get("batch_throughput_measurement_command")
    if not isinstance(batch, dict) or not isinstance(command, dict):
        raise ValueError("receipt lacks a batch throughput measurement")
    if command.get("command", [])[-2:] != ["--", "batch"]:
        raise ValueError("batch receipt does not bind the measure batch selector")
    routes = batch.get("routes")
    if not isinstance(routes, dict) or set(routes) != set(ROUTES):
        raise ValueError("batch receipt routes are not exact")
    for name, row in routes.items():
        if not isinstance(row, dict):
            raise ValueError(f"batch route {name} is malformed")
        if row.get("operations_per_sample") != 64 or row.get("body_bytes_per_operation") != 2:
            raise ValueError(f"batch route {name} changed the reviewed workload")
        if not isinstance(row.get("normalized_operations_per_second"), (int, float)) or row["normalized_operations_per_second"] <= 0:
            raise ValueError(f"batch route {name} lacks throughput")
        allocations = row.get("allocator_requests_per_batch")
        if not isinstance(allocations, dict) or set(allocations) != set(ALLOCATION_COLUMNS):
            raise ValueError(f"batch route {name} lacks allocator request observations")
        if any(not isinstance(value, (int, float)) or value < 0 for value in allocations.values()):
            raise ValueError(f"batch route {name} has invalid allocator request observations")
    return routes


def source_observations(path):
    try:
        source = path.read_text()
    except OSError as error:
        raise ValueError(f"cannot read measured route source {path}") from error
    required = (
        "for _ in 0..BATCH_OPERATIONS",
        "execute(route, runtime, client, &endpoint, revision)",
        "generated::register(Arc::clone(revision)",
        ".call_typed(seed, 10_000)",
    )
    if not all(token in source for token in required):
        raise ValueError("measured route source lacks the reviewed repeated-registration shape")
    return {
        "path": str(path),
        "sha256": digest(path),
        "observations": [
            "each 64-operation batch iteration calls execute once per operation",
            "the generated execute branch registers a one-shot callback per operation",
            "the generated execute branch consumes that registration through call_typed per operation",
        ],
    }


def investigate(report_path, source_path):
    report = load(report_path)
    routes = batch_routes(report)
    handwritten = routes["handwritten_adapter"]
    generated = routes["generated_semaprax"]
    ratio = generated["normalized_operations_per_second"] / handwritten["normalized_operations_per_second"]
    operations = generated["operations_per_sample"]
    allocation_delta = {
        column: round(
            (generated["allocator_requests_per_batch"][column] - handwritten["allocator_requests_per_batch"][column])
            / operations,
            3,
        )
        for column in ALLOCATION_COLUMNS
    }
    return {
        "schema": SCHEMA,
        "status": "investigation_required" if ratio < THRESHOLD else "threshold_not_triggered",
        "receipt": {"path": str(report_path), "sha256": digest(report_path), "checkout": report.get("checkout")},
        "workload": {"operations_per_sample": operations, "body_bytes_per_operation": generated["body_bytes_per_operation"]},
        "measured": {
            "generated_operations_per_second": generated["normalized_operations_per_second"],
            "handwritten_operations_per_second": handwritten["normalized_operations_per_second"],
            "direct_operations_per_second": routes["direct_rust"]["normalized_operations_per_second"],
            "generated_to_handwritten_throughput_ratio": round(ratio, 4),
            "investigation_threshold": THRESHOLD,
            "generated_minus_handwritten_allocator_requests_per_operation": allocation_delta,
        },
        "inspected_source": source_observations(source_path),
        "limitations": [
            "The receipt measures total route time and allocator requests; it does not time individual registration, checked-prefix, clone, or resume operations.",
            "The source observations explain the repeated setup shape but do not assign an exact fraction of the measured regression to any one operation.",
            "This local investigation is not a performance pass, a cross-platform result, or a nontrivial-batch threshold result.",
        ],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--receipt", required=True, type=pathlib.Path)
    parser.add_argument("--measure-source", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        result = investigate(args.receipt, args.measure_source)
    except ValueError as error:
        parser.error(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(canonical(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
