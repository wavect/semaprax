#!/usr/bin/env python3
"""Run the saved RI-13 applications and retain distinct measurement classes."""

import argparse
import csv
import hashlib
import json
import os
import platform
import statistics
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
ROUTES = ("direct_rust", "handwritten_adapter", "generated_semaprax")
ALLOCATION_COLUMNS = (
    "allocation_calls",
    "deallocation_calls",
    "reallocation_calls",
    "allocated_bytes",
    "deallocated_bytes",
)
M3_COPY_LEDGER_SCHEMA = "semaprax.ri13.m3-copy-ledger.v1"


def percentile(values, percent):
    values = sorted(values)
    offset = (len(values) - 1) * percent / 100
    lower = int(offset)
    upper = min(lower + 1, len(values) - 1)
    return values[lower] + (values[upper] - values[lower]) * (offset - lower)


def digest(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def run(command, environment, expected_stdout):
    started = time.perf_counter_ns()
    completed = subprocess.run(
        command,
        cwd=ROOT,
        env=environment,
        text=True,
        capture_output=True,
        check=False,
    )
    elapsed_ns = time.perf_counter_ns() - started
    if completed.returncode:
        raise RuntimeError(
            f"{' '.join(command)} exited {completed.returncode}:\n{completed.stderr}"
        )
    if expected_stdout not in completed.stdout:
        raise RuntimeError(
            f"{' '.join(command)} did not emit {expected_stdout!r}:\n{completed.stdout}"
        )
    return {
        "command": command,
        "elapsed_ns": elapsed_ns,
        "stdout_sha256": digest(completed.stdout),
        "stderr_sha256": digest(completed.stderr),
    }, completed.stdout


def parse_m3_samples(text):
    rows = list(csv.DictReader(text.splitlines()))
    required = {"route", "iteration", "elapsed_ns", "body_bytes", *ALLOCATION_COLUMNS}
    if not rows or set(rows[0]) != required:
        raise ValueError(
            "M3 measure output must have exactly the timing and allocator-request columns; "
            "run this harness with the RI-13 allocator instrumentation applied"
        )
    by_route = {route: [] for route in ROUTES}
    for row in rows:
        route = row["route"]
        if route not in by_route:
            raise ValueError(f"unknown M3 route {route!r}")
        if int(row["body_bytes"]) != 2 or int(row["elapsed_ns"]) <= 0:
            raise ValueError(f"invalid M3 route sample {row!r}")
        by_route[route].append({key: int(value) for key, value in row.items() if key != "route"})
    counts = {route: len(samples) for route, samples in by_route.items()}
    if len(set(counts.values())) != 1 or not next(iter(counts.values())):
        raise ValueError(f"unbalanced M3 route samples {counts}")
    result = {"raw_csv_sha256": digest(text), "samples_per_route": next(iter(counts.values())), "routes": {}}
    for route, samples in by_route.items():
        elapsed = [sample["elapsed_ns"] for sample in samples]
        body_bytes = {sample["body_bytes"] for sample in samples}
        if len(body_bytes) != 1:
            raise ValueError(f"M3 route {route} changed response body size")
        result["routes"][route] = {
            "body_bytes_per_sample": body_bytes.pop(),
            "mean_ns": round(statistics.mean(elapsed), 1),
            "p50_ns": round(percentile(elapsed, 50), 1),
            "p90_ns": round(percentile(elapsed, 90), 1),
            "p99_ns": round(percentile(elapsed, 99), 1),
            "serialized_calls_per_second": round(1e9 / statistics.mean(elapsed), 2),
            "allocator_requests": {
                column: round(statistics.mean(sample[column] for sample in samples), 1)
                for column in ALLOCATION_COLUMNS
            },
        }
    return result


def parse_m3_batch_samples(text):
    rows = list(csv.DictReader(text.splitlines()))
    required = {"route", "iteration", "operations", "elapsed_ns", "body_bytes", *ALLOCATION_COLUMNS}
    if not rows or set(rows[0]) != required:
        raise ValueError("M3 batch output must have exactly the batch timing and allocator-request columns")
    by_route = {route: [] for route in ROUTES}
    for row in rows:
        route = row["route"]
        if route not in by_route:
            raise ValueError(f"unknown M3 batch route {route!r}")
        sample = {key: int(value) for key, value in row.items() if key != "route"}
        if sample["operations"] <= 1 or sample["body_bytes"] != 2 or sample["elapsed_ns"] <= 0:
            raise ValueError(f"invalid M3 batch sample {row!r}")
        by_route[route].append(sample)
    counts = {route: len(samples) for route, samples in by_route.items()}
    if len(set(counts.values())) != 1 or not next(iter(counts.values())):
        raise ValueError(f"unbalanced M3 batch samples {counts}")
    result = {"raw_csv_sha256": digest(text), "samples_per_route": next(iter(counts.values())), "routes": {}}
    for route, samples in by_route.items():
        operations = {sample["operations"] for sample in samples}
        body_bytes = {sample["body_bytes"] for sample in samples}
        if len(operations) != 1 or len(body_bytes) != 1:
            raise ValueError(f"M3 batch route {route} changed its operation or body size")
        elapsed = [sample["elapsed_ns"] for sample in samples]
        operation_count = operations.pop()
        result["routes"][route] = {
            "operations_per_sample": operation_count,
            "total_operations": operation_count * len(samples),
            "body_bytes_per_operation": body_bytes.pop(),
            "mean_batch_ns": round(statistics.mean(elapsed), 1),
            "p50_batch_ns": round(percentile(elapsed, 50), 1),
            "p90_batch_ns": round(percentile(elapsed, 90), 1),
            "p99_batch_ns": round(percentile(elapsed, 99), 1),
            "normalized_operations_per_second": round(operation_count * 1e9 / statistics.mean(elapsed), 2),
            "allocator_requests_per_batch": {
                column: round(statistics.mean(sample[column] for sample in samples), 1)
                for column in ALLOCATION_COLUMNS
            },
        }
    return result


def m3_copy_ledger(measurement):
    """Render the byte facts the scalar M3 route can establish exactly.

    Its generated boundary has an `i64` input and result, so no byte carrier
    crosses that boundary. The CSV establishes response payload bytes per
    route, while copies inside reqwest and decoding remain unobserved.
    """
    samples = measurement["samples_per_route"]
    routes = {}
    for route, values in measurement["routes"].items():
        body_bytes = values["body_bytes_per_sample"]
        routes[route] = {
            "samples": samples,
            "response_wire_bytes": samples * body_bytes,
            "generated_boundary_copied_bytes": 0,
            "generated_boundary_shape": "i64-to-i64",
            "host_callback_payload_copied_bytes": 0,
            "host_callback_payload_shape": "i64-to-Future<Result<i64,String>>",
            "host_callback_capture_copied_bytes": None,
            "foreign_response_body_copied_bytes": None,
            "foreign_response_body_shape": "reqwest Response::text to parsed i64",
        }
    return {
        "schema": M3_COPY_LEDGER_SCHEMA,
        "routes": routes,
        "unmeasured_copy_domains": [
            "reqwest response buffering",
            "HTTP decoding",
            "Response::text UTF-8 handling",
            "host callback captures (reqwest Client and endpoint String)",
        ],
        "exact_copy_domains": [
            "generated i64 boundary",
            "host callback invocation payload",
        ],
    }


def cargo_command(manifest, binary):
    return [
        "cargo",
        "run",
        "--locked",
        "--offline",
        "--quiet",
        "--manifest-path",
        manifest,
        "--bin",
        binary,
    ]


def current_text(command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def self_test():
    header = ["route", "iteration", "elapsed_ns", "body_bytes", *ALLOCATION_COLUMNS]
    rows = [header]
    for iteration in range(2):
        for route in ROUTES:
            rows.append([route, str(iteration), "100", "2", "1", "2", "0", "16", "16"])
    report = parse_m3_samples("\n".join(",".join(row) for row in rows))
    assert report["samples_per_route"] == 2
    assert report["routes"]["generated_semaprax"]["body_bytes_per_sample"] == 2
    assert report["routes"]["generated_semaprax"]["allocator_requests"]["allocated_bytes"] == 16
    assert m3_copy_ledger(report)["routes"]["generated_semaprax"] == {
        "samples": 2,
        "response_wire_bytes": 4,
        "generated_boundary_copied_bytes": 0,
        "generated_boundary_shape": "i64-to-i64",
        "host_callback_payload_copied_bytes": 0,
        "host_callback_payload_shape": "i64-to-Future<Result<i64,String>>",
        "host_callback_capture_copied_bytes": None,
        "foreign_response_body_copied_bytes": None,
        "foreign_response_body_shape": "reqwest Response::text to parsed i64",
    }
    batch_header = ["route", "iteration", "operations", "elapsed_ns", "body_bytes", *ALLOCATION_COLUMNS]
    batch_rows = [batch_header]
    for iteration in range(2):
        for route in ROUTES:
            batch_rows.append([route, str(iteration), "64", "6400", "2", "8", "8", "0", "128", "128"])
    batch = parse_m3_batch_samples("\n".join(",".join(row) for row in batch_rows))
    assert batch["routes"]["generated_semaprax"]["total_operations"] == 128
    assert batch["routes"]["generated_semaprax"]["normalized_operations_per_second"] == 10_000_000.0
    print("ri13-combined-measure-self-test-ok")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="receipt JSON path")
    parser.add_argument(
        "--target-dir",
        type=Path,
        default=ROOT / "target" / "ri13-combined-app",
        help="private Cargo target directory for every application",
    )
    parser.add_argument(
        "--fresh-target",
        action="store_true",
        help="refuse an existing target directory to make this a clean-target build receipt",
    )
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    if arguments.self_test:
        self_test()
        return
    target = arguments.target_dir.resolve()
    target_existed = target.exists()
    if arguments.fresh_target and target_existed:
        raise SystemExit(f"refusing existing --fresh-target directory: {target}")
    target.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment.update(
        {
            "CARGO_TARGET_DIR": str(target),
            "CARGO_BUILD_JOBS": "1",
            "CARGO_INCREMENTAL": "0",
            "CARGO_PROFILE_DEV_DEBUG": "0",
        }
    )
    if not environment.get("CLANG"):
        raise SystemExit("CLANG must name the explicit compiler for the M1 generated C consumers")

    stages = []
    for name, command, expected in [
        (
            "m1_prepare",
            cargo_command("examples/ri13-m1-regex-url/prepare/Cargo.toml", "prepare"),
            "ri13-m1-prepared:",
        ),
        (
            "m1_consumer",
            cargo_command("examples/ri13-m1-regex-url/consumer/Cargo.toml", "consumer"),
            "ri13-m1-regex-url-ok",
        ),
        (
            "m2_prepare",
            cargo_command("examples/ri13-m2-record-iterator/Cargo.toml", "prepare"),
            "ri13-m2-prepared:",
        ),
        (
            "m2_consumer",
            cargo_command("examples/ri13-m2-record-iterator/Cargo.toml", "consumer"),
            "ri13-m2-record-iterator-ok",
        ),
        (
            "m3_prepare",
            cargo_command("examples/ri13-m3-local-http/Cargo.toml", "prepare"),
            "ri13-m3-prepared",
        ),
        (
            "m3_consumer",
            cargo_command("examples/ri13-m3-local-http/Cargo.toml", "consumer"),
            "ri13-m3-local-http-ok",
        ),
    ]:
        result, _ = run(command, environment, expected)
        result["stage"] = name
        stages.append(result)

    measure_command = cargo_command("examples/ri13-m3-local-http/Cargo.toml", "measure")
    measure_result, samples = run(measure_command, environment, "generated_semaprax")
    measure_result["stage"] = "m3_route_measurement"
    route_measurement = parse_m3_samples(samples)
    batch_command = cargo_command("examples/ri13-m3-local-http/Cargo.toml", "measure") + ["--", "batch"]
    batch_result, batch_samples = run(batch_command, environment, "generated_semaprax")
    batch_result["stage"] = "m3_batch_throughput_measurement"
    batch_measurement = parse_m3_batch_samples(batch_samples)
    report = {
        "schema": "semaprax.ri13.combination-measurement.v1",
        "checkout": current_text(["git", "rev-parse", "HEAD"]),
        "target_dir": str(target),
        "target_preexisted": target_existed,
        "host": {"platform": platform.platform(), "python": sys.version.split()[0]},
        "toolchain": {"cargo": current_text(["cargo", "--version"]), "clang": environment["CLANG"]},
        "full_build_and_consumer_stages": stages,
        "route_measurement_command": measure_result,
        "route_timing_and_allocator_requests": route_measurement,
        "batch_throughput_measurement_command": batch_result,
        "batch_throughput": batch_measurement,
        "m3_copy_ledger": m3_copy_ledger(route_measurement),
        "limits": [
            "M1, M2 and M3 remain separately admitted source profiles; this receipt does not claim one linked Project.",
            "Build-and-consumer stage elapsed times include Cargo work and process startup; they are not route latency.",
            "Scalar route timings include loopback HTTP and two-byte response parsing; the separate 64-operation batch rows retain their own normalized throughput and remain local evidence.",
            "Allocator values count current-thread requests and do not infer copies. The ledger records exact zero-byte scalar generated and host-callback payload boundaries, response wire bytes, and explicit unavailable cells for reqwest/HTTP/text/capture copies.",
        ],
    }
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(arguments.output)


if __name__ == "__main__":
    main()
