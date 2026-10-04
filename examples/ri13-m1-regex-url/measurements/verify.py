#!/usr/bin/env python3
"""Verify the fixed-workload local RI-13 M1 batch investigation."""

import csv
import hashlib
import json
import statistics
from pathlib import Path


ROOT = Path(__file__).resolve().parent
RAW = ROOT / "darwin-arm64-93c9e2599-2026-10-04.csv"
RAW_SHA256 = "e41d1c66120711d3147a5cd6ec2e0a2c937486c7af3866654c087d67868a666a"
SCHEMA = "semaprax.ri13.m1-batch-investigation.v1"
THRESHOLD = 0.90
TASKS = ("regex_scan", "url_parse_view")
ROUTES = ("direct_rust", "handwritten_adapter", "generated_semaprax")
COLUMNS = (
    "task", "route", "iteration", "operations", "elapsed_ns",
    "allocation_calls", "allocated_bytes", "borrowed_input_bytes",
    "adapter_copy_events", "adapter_copied_bytes", "owner_live_count",
    "view_live_count", "string_live_count",
)


def median(rows, field):
    return statistics.median(row[field] for row in rows)


def load():
    if hashlib.sha256(RAW.read_bytes()).hexdigest() != RAW_SHA256:
        raise ValueError("M1 raw CSV digest differs from the recorded local run")
    with RAW.open(newline="", encoding="utf-8") as source:
        reader = csv.DictReader(source)
        if tuple(reader.fieldnames or ()) != COLUMNS:
            raise ValueError("M1 raw CSV columns differ from the reviewed batch schema")
        rows = []
        for raw in reader:
            try:
                row = {key: raw[key] if key in {"task", "route"} else int(raw[key]) for key in COLUMNS}
            except (KeyError, TypeError, ValueError) as error:
                raise ValueError("M1 raw CSV has a non-integer measurement field") from error
            rows.append(row)
    return rows


def summarize(rows):
    tasks = {}
    for task in TASKS:
        routes = {}
        for route in ROUTES:
            samples = [row for row in rows if row["task"] == task and row["route"] == route]
            if len(samples) != 5 or sorted(row["iteration"] for row in samples) != list(range(5)):
                raise ValueError(f"M1 raw CSV does not retain five ordered {task}/{route} samples")
            if any(row["operations"] != 4096 or row["elapsed_ns"] <= 0 for row in samples):
                raise ValueError(f"M1 raw CSV changed the reviewed {task}/{route} workload")
            if any(row["borrowed_input_bytes"] != 4096 * 28 for row in samples):
                raise ValueError(f"M1 raw CSV changed {task}/{route} borrowed input accounting")
            if any(row[key] != 0 for row in samples for key in (
                "adapter_copy_events", "adapter_copied_bytes", "owner_live_count",
                "view_live_count", "string_live_count",
            )):
                raise ValueError(f"M1 raw CSV changed {task}/{route} adapter or cleanup accounting")
            routes[route] = {
                "median_elapsed_ns": median(samples, "elapsed_ns"),
                "median_allocation_calls": median(samples, "allocation_calls"),
                "median_allocated_bytes": median(samples, "allocated_bytes"),
            }
        generated = routes["generated_semaprax"]["median_elapsed_ns"]
        tasks[task] = {
            "routes": routes,
            "generated_to_direct_throughput_ratio": round(
                routes["direct_rust"]["median_elapsed_ns"] / generated, 4
            ),
            "generated_to_handwritten_throughput_ratio": round(
                routes["handwritten_adapter"]["median_elapsed_ns"] / generated, 4
            ),
        }
    if any(
        task["generated_to_direct_throughput_ratio"] < THRESHOLD
        or task["generated_to_handwritten_throughput_ratio"] < THRESHOLD
        for task in tasks.values()
    ):
        status = "investigation_required"
    else:
        status = "threshold_not_triggered"
    return {
        "schema": SCHEMA,
        "checkout": "93c9e2599",
        "raw_csv": {"path": RAW.name, "sha256": f"sha256:{RAW_SHA256}"},
        "workload": {
            "batch_api": "checked-export-repeat.v1",
            "operations_per_sample": 4096,
            "borrowed_input_bytes_per_batch": 4096 * 28,
            "adapter_copied_bytes_per_batch": 0,
            "post_run_live_counts": {"owner": 0, "view": 0, "string": 0},
        },
        "investigation_threshold": THRESHOLD,
        "status": status,
        "tasks": tasks,
        "limitations": [
            "The batch repeats held scalar exports and does not accept varying foreign input.",
            "Regex and Url internal copy counts remain unavailable.",
            "This local debug-build result is not a cross-platform or threshold-pass claim.",
        ],
    }


if __name__ == "__main__":
    print(json.dumps(summarize(load()), indent=2, sort_keys=True))
