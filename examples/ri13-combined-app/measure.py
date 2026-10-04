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
COPY_COLUMNS = (
    "foreign_response_body_copied_bytes",
    "host_callback_captured_bytes",
)
M3_COPY_LEDGER_SCHEMA = "semaprax.ri13.m3-copy-ledger.v1"
LINKED_COPY_LEDGER_SCHEMA = "semaprax.ri13.linked-copy-ledger.v1"
LINKED_COPY_PREFIX = "ri13-linked-copy-ledger:"
M1_TASKS = ("regex_scan", "url_parse_view")
M1_BATCH_COLUMNS = (
    "task", "route", "iteration", "operations", "elapsed_ns",
    "allocation_calls", "allocated_bytes", "borrowed_input_bytes",
    "adapter_copy_events", "adapter_copied_bytes", "owner_live_count",
    "view_live_count", "string_live_count",
)
M2_TASKS = ("generic_record", "stateful_callback")
M2_BATCH_COLUMNS = (
    "task", "route", "iteration", "operations", "elapsed_ns",
    "allocation_calls", "allocated_bytes", "adapter_buffer_copied_bytes",
)


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
    required = {
        "route",
        "iteration",
        "elapsed_ns",
        "body_bytes",
        *COPY_COLUMNS,
        *ALLOCATION_COLUMNS,
    }
    if not rows or set(rows[0]) != required:
        raise ValueError(
            "M3 measure output must have exactly the timing, copied-byte, and allocator "
            "columns; run this harness with the RI-13 instrumentation applied"
        )
    by_route = {route: [] for route in ROUTES}
    for row in rows:
        route = row["route"]
        if route not in by_route:
            raise ValueError(f"unknown M3 route {route!r}")
        if int(row["body_bytes"]) != 2 or int(row["elapsed_ns"]) <= 0:
            raise ValueError(f"invalid M3 route sample {row!r}")
        if int(row["foreign_response_body_copied_bytes"]) != 2:
            raise ValueError(f"M3 route did not copy its exact two-byte response {row!r}")
        expected_callback_bytes = 2 if route == "generated_semaprax" else 0
        if int(row["host_callback_captured_bytes"]) != expected_callback_bytes:
            raise ValueError(f"M3 callback copy accounting differs from its route {row!r}")
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
            "foreign_response_body_copied_bytes_per_sample": samples[0][
                "foreign_response_body_copied_bytes"
            ],
            "host_callback_captured_bytes_per_sample": samples[0][
                "host_callback_captured_bytes"
            ],
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




def parse_m1_batch_samples(text):
    """Parse matched fixed-workload M1 batches without attributing foreign copies."""
    rows = list(csv.DictReader(text.splitlines()))
    if not rows or tuple(rows[0]) != M1_BATCH_COLUMNS:
        raise ValueError("M1 measure output must retain its exact batch columns")
    grouped = {task: {route: [] for route in ROUTES} for task in M1_TASKS}
    for row in rows:
        task = row.get("task")
        route = row.get("route")
        if task not in grouped or route not in ROUTES:
            raise ValueError(f"unknown M1 batch row {row!r}")
        sample = {key: int(value) for key, value in row.items() if key not in {"task", "route"}}
        if sample["operations"] != 4096 or sample["elapsed_ns"] <= 0:
            raise ValueError(f"M1 batch changed its reviewed held-export workload {row!r}")
        if sample["borrowed_input_bytes"] != sample["operations"] * 28:
            raise ValueError(f"M1 batch changed its exact borrowed input accounting {row!r}")
        if any(sample[key] != 0 for key in (
            "adapter_copy_events", "adapter_copied_bytes", "owner_live_count",
            "view_live_count", "string_live_count",
        )):
            raise ValueError(f"M1 batch changed generated adapter or cleanup accounting {row!r}")
        if sample["allocation_calls"] < 0 or sample["allocated_bytes"] < 0:
            raise ValueError(f"M1 batch has invalid allocator accounting {row!r}")
        grouped[task][route].append(sample)
    result = {"raw_csv_sha256": digest(text), "tasks": {}}
    for task, by_route in grouped.items():
        counts = {route: len(samples) for route, samples in by_route.items()}
        if len(set(counts.values())) != 1 or not next(iter(counts.values())):
            raise ValueError(f"unbalanced M1 {task} batch samples {counts}")
        routes = {}
        for route, samples in by_route.items():
            elapsed = [sample["elapsed_ns"] for sample in samples]
            routes[route] = {
                "operations_per_sample": 4096,
                "total_operations": 4096 * len(samples),
                "borrowed_input_bytes_per_batch": 4096 * 28,
                "adapter_copy_events_per_batch": 0,
                "adapter_copied_bytes_per_batch": 0,
                "post_run_live_counts": {"owner": 0, "view": 0, "string": 0},
                "mean_batch_ns": round(statistics.mean(elapsed), 1),
                "p50_batch_ns": round(percentile(elapsed, 50), 1),
                "p90_batch_ns": round(percentile(elapsed, 90), 1),
                "p99_batch_ns": round(percentile(elapsed, 99), 1),
                "normalized_operations_per_second": round(4096 * 1e9 / statistics.mean(elapsed), 2),
                "allocator_requests_per_batch": {
                    key: round(statistics.mean(sample[key] for sample in samples), 1)
                    for key in ("allocation_calls", "allocated_bytes")
                },
                "foreign_target_copied_bytes": {
                    "status": "unavailable",
                    "reason": "Regex::is_match and Url::parse internal copies are outside the generated adapter boundary",
                },
            }
        result["tasks"][task] = {
            "samples_per_route": next(iter(counts.values())),
            "routes": routes,
        }
    return result

def parse_m2_batch_samples(text):
    """Parse the existing matched M2 record/callback batch comparison."""
    rows = list(csv.DictReader(text.splitlines()))
    if not rows or tuple(rows[0]) != M2_BATCH_COLUMNS:
        raise ValueError("M2 measure output must retain its exact batch columns")
    grouped = {
        task: {route: [] for route in ROUTES}
        for task in M2_TASKS
    }
    for row in rows:
        task = row.get("task")
        route = row.get("route")
        if task not in grouped or route not in ROUTES:
            raise ValueError(f"unknown M2 batch row {row!r}")
        sample = {key: int(value) for key, value in row.items() if key not in {"task", "route"}}
        if sample["operations"] <= 1 or sample["elapsed_ns"] <= 0:
            raise ValueError(f"invalid M2 batch row {row!r}")
        if sample["adapter_buffer_copied_bytes"] != 0:
            raise ValueError(f"M2 scalar adapter copied bytes {row!r}")
        grouped[task][route].append(sample)
    result = {"raw_csv_sha256": digest(text), "tasks": {}}
    for task, by_route in grouped.items():
        counts = {route: len(samples) for route, samples in by_route.items()}
        if len(set(counts.values())) != 1 or not next(iter(counts.values())):
            raise ValueError(f"unbalanced M2 {task} batch samples {counts}")
        routes = {}
        for route, samples in by_route.items():
            operations = {sample["operations"] for sample in samples}
            if len(operations) != 1:
                raise ValueError(f"M2 {task} changed operations for {route}")
            operation_count = operations.pop()
            elapsed = [sample["elapsed_ns"] for sample in samples]
            routes[route] = {
                "operations_per_sample": operation_count,
                "total_operations": operation_count * len(samples),
                "mean_batch_ns": round(statistics.mean(elapsed), 1),
                "p50_batch_ns": round(percentile(elapsed, 50), 1),
                "p90_batch_ns": round(percentile(elapsed, 90), 1),
                "p99_batch_ns": round(percentile(elapsed, 99), 1),
                "normalized_operations_per_second": round(
                    operation_count * 1e9 / statistics.mean(elapsed), 2
                ),
                "allocator_requests_per_batch": {
                    key: round(statistics.mean(sample[key] for sample in samples), 1)
                    for key in ("allocation_calls", "allocated_bytes")
                },
                "adapter_buffer_copied_bytes_per_batch": 0,
            }
        result["tasks"][task] = {
            "samples_per_route": next(iter(counts.values())),
            "routes": routes,
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
    crosses that boundary. The CSV observes the fixture's explicit `Bytes` to
    callback-owned `Vec<u8>` copy. Copies inside reqwest and HTTP decoding
    remain unobserved.
    """
    samples = measurement["samples_per_route"]
    routes = {}
    for route, values in measurement["routes"].items():
        body_bytes = values["body_bytes_per_sample"]
        foreign_response_body_copied_bytes = values[
            "foreign_response_body_copied_bytes_per_sample"
        ]
        host_callback_captured_bytes = values["host_callback_captured_bytes_per_sample"]
        routes[route] = {
            "samples": samples,
            "response_wire_bytes": samples * body_bytes,
            "foreign_response_body_copied_bytes": samples
            * foreign_response_body_copied_bytes,
            "host_callback_captured_bytes": samples * host_callback_captured_bytes,
            "generated_boundary_copied_bytes": 0,
            "generated_boundary_shape": "i64-to-i64",
            "host_callback_payload_copied_bytes": 0,
            "host_callback_payload_shape": "i64-to-Future<Result<i64,String>>",
            "host_callback_capture_copied_bytes": None,
            "foreign_response_body_shape": "Response::bytes to callback-owned Vec<u8>",
        }
    return {
        "schema": M3_COPY_LEDGER_SCHEMA,
        "routes": routes,
        "unmeasured_copy_domains": [
            "reqwest response buffering before the fixture's Bytes-to-Vec copy",
            "HTTP decoding before the fixture's Bytes-to-Vec copy",
            "UTF-8 validation after the fixture's Vec copy",
            "host callback captures (reqwest Client and endpoint String)",
        ],
        "exact_copy_domains": [
            "generated i64 boundary",
            "host callback invocation payload",
            "fixture Bytes-to-Vec response copy",
            "generated host callback subset of the fixture response copy",
        ],
    }


def parse_linked_copy_ledger(text):
    rows = [
        line.removeprefix(LINKED_COPY_PREFIX)
        for line in text.splitlines()
        if line.startswith(LINKED_COPY_PREFIX)
    ]
    if len(rows) != 1:
        raise ValueError("linked consumer must emit exactly one copied-byte ledger")
    try:
        ledger = json.loads(rows[0])
    except json.JSONDecodeError as error:
        raise ValueError("linked copied-byte ledger is not JSON") from error
    if set(ledger) != {"schema", "m1", "m2"} or ledger["schema"] != LINKED_COPY_LEDGER_SCHEMA:
        raise ValueError("linked copied-byte ledger has an unsupported schema")
    regex = ledger["m1"].get("regex_result_owner")
    url = ledger["m1"].get("url_owner_view")
    record = ledger["m2"].get("serde_record")
    callback = ledger["m2"].get("iterator_callback")
    if not all(isinstance(value, dict) for value in (regex, url, record, callback)):
        raise ValueError("linked copied-byte ledger has missing route evidence")
    foreign_regex_bytes = regex.get("foreign_target_copied_bytes")
    if (
        regex.get("status") != "measured"
        or regex.get("adapter_copy_events") != 0
        or regex.get("adapter_copied_bytes") != 0
        or regex.get("adapter_borrowed_scan_input_bytes") != 28
        or regex.get("borrow_matches_target") is not True
        or not isinstance(foreign_regex_bytes, dict)
        or foreign_regex_bytes.get("status") != "unavailable"
        or not isinstance(foreign_regex_bytes.get("reason"), str)
    ):
        raise ValueError("linked Regex owner evidence is not an exact zero-copy observation")
    foreign_url_bytes = url.get("foreign_target_copied_bytes")
    if (
        url.get("status") != "measured"
        or url.get("adapter_copy_events") != 0
        or url.get("adapter_copied_bytes") != 0
        or url.get("borrow_matches_target") is not True
        or not isinstance(foreign_url_bytes, dict)
        or foreign_url_bytes.get("status") != "unavailable"
        or not isinstance(foreign_url_bytes.get("reason"), str)
    ):
        raise ValueError("linked Url ownership evidence must retain the unavailable foreign byte count")
    if (
        record.get("input_json_bytes") != 25
        or record.get("output_json_bytes") != 25
        or record.get("generated_mirror_string_clone_copied_bytes") != 3
        or record.get("generated_mirror_to_record_transferred_string_bytes") != 6
        or record.get("generated_mirror_to_record_copied_string_bytes") != 0
        or record.get("generated_mirror_to_record_pointers_preserved") is not True
    ):
        raise ValueError("linked Serde record evidence changed its exact fixture transfer")
    deserialization = record.get("deserialize_owned_string_copied_bytes")
    if not isinstance(deserialization, dict) or deserialization.get("status") != "unavailable" or not isinstance(deserialization.get("reason"), str):
        raise ValueError("linked Serde deserialization must retain its unavailable byte count")
    if callback != {"fn_invocations": 1, "fn_mut_invocations": 1, "scalar_argument_result_copied_bytes": 0}:
        raise ValueError("linked iterator callback evidence changed its scalar copy accounting")
    return ledger


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


def m3_negative_control_command():
    return [
        "cargo",
        "test",
        "--locked",
        "--offline",
        "--quiet",
        "--test",
        "project",
        "ri13_m3::saved_m3_application_runs_offline_and_refuses_timeout_and_stale_binding_mutants",
        "--",
        "--ignored",
        "--exact",
    ]


def current_text(command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def self_test():
    header = [
        "route",
        "iteration",
        "elapsed_ns",
        "body_bytes",
        *COPY_COLUMNS,
        *ALLOCATION_COLUMNS,
    ]
    rows = [header]
    for iteration in range(2):
        for route in ROUTES:
            rows.append(
                [
                    route,
                    str(iteration),
                    "100",
                    "2",
                    "2",
                    "2" if route == "generated_semaprax" else "0",
                    "1",
                    "2",
                    "0",
                    "16",
                    "16",
                ]
            )
    report = parse_m3_samples("\n".join(",".join(row) for row in rows))
    assert report["samples_per_route"] == 2
    assert report["routes"]["generated_semaprax"]["body_bytes_per_sample"] == 2
    assert report["routes"]["generated_semaprax"]["allocator_requests"]["allocated_bytes"] == 16
    assert m3_copy_ledger(report)["routes"]["generated_semaprax"] == {
        "samples": 2,
        "response_wire_bytes": 4,
        "foreign_response_body_copied_bytes": 4,
        "host_callback_captured_bytes": 4,
        "generated_boundary_copied_bytes": 0,
        "generated_boundary_shape": "i64-to-i64",
        "host_callback_payload_copied_bytes": 0,
        "host_callback_payload_shape": "i64-to-Future<Result<i64,String>>",
        "host_callback_capture_copied_bytes": None,
        "foreign_response_body_shape": "Response::bytes to callback-owned Vec<u8>",
    }
    batch_header = ["route", "iteration", "operations", "elapsed_ns", "body_bytes", *ALLOCATION_COLUMNS]
    batch_rows = [batch_header]
    for iteration in range(2):
        for route in ROUTES:
            batch_rows.append([route, str(iteration), "64", "6400", "2", "8", "8", "0", "128", "128"])
    batch = parse_m3_batch_samples("\n".join(",".join(row) for row in batch_rows))
    assert batch["routes"]["generated_semaprax"]["total_operations"] == 128
    assert batch["routes"]["generated_semaprax"]["normalized_operations_per_second"] == 10_000_000.0
    m1_rows = [list(M1_BATCH_COLUMNS)]
    for task in M1_TASKS:
        for iteration in range(2):
            for route in ROUTES:
                m1_rows.append([
                    task, route, str(iteration), "4096", "409600", "16", "512",
                    str(4096 * 28), "0", "0", "0", "0", "0",
                ])
    m1 = parse_m1_batch_samples("\n".join(",".join(row) for row in m1_rows))
    assert m1["tasks"]["regex_scan"]["routes"]["generated_semaprax"]["total_operations"] == 8192
    assert m1["tasks"]["url_parse_view"]["routes"]["direct_rust"]["normalized_operations_per_second"] == 10_000_000.0
    tampered_m1 = [row.copy() for row in m1_rows]
    tampered_m1[1][7] = "1"
    try:
        parse_m1_batch_samples("\n".join(",".join(row) for row in tampered_m1))
    except ValueError:
        pass
    else:
        raise AssertionError("M1 borrowed-input drift must fail the batch gate")
    m2_rows = [list(M2_BATCH_COLUMNS)]
    for task in M2_TASKS:
        for iteration in range(2):
            for route in ROUTES:
                m2_rows.append([task, route, str(iteration), "32", "3200", "4", "96", "0"])
    m2 = parse_m2_batch_samples("\n".join(",".join(row) for row in m2_rows))
    assert m2["tasks"]["generic_record"]["routes"]["generated_semaprax"]["total_operations"] == 64
    assert m2["tasks"]["stateful_callback"]["routes"]["direct_rust"]["normalized_operations_per_second"] == 10_000_000.0
    tampered_m2 = [row.copy() for row in m2_rows]
    tampered_m2[1][-1] = "1"
    try:
        parse_m2_batch_samples("\n".join(",".join(row) for row in tampered_m2))
    except ValueError:
        pass
    else:
        raise AssertionError("nonzero M2 scalar copy bytes must fail the batch gate")
    assert m3_negative_control_command() == [
        "cargo", "test", "--locked", "--offline", "--quiet", "--test", "project",
        "ri13_m3::saved_m3_application_runs_offline_and_refuses_timeout_and_stale_binding_mutants",
        "--", "--ignored", "--exact",
    ]
    linked_row = (
        "ri13-linked-copy-ledger:{\"schema\":\"semaprax.ri13.linked-copy-ledger.v1\",\"m1\":{\"regex_result_owner\":{\"status\":\"measured\",\"adapter_copy_events\":0,\"adapter_copied_bytes\":0,\"adapter_borrowed_scan_input_bytes\":28,\"borrow_matches_target\":true,\"foreign_target_copied_bytes\":{\"status\":\"unavailable\",\"reason\":\"no counter\"}},\"url_owner_view\":{\"status\":\"measured\",\"adapter_copy_events\":0,\"adapter_copied_bytes\":0,\"borrow_matches_target\":true,\"foreign_target_copied_bytes\":{\"status\":\"unavailable\",\"reason\":\"no counter\"}}},\"m2\":{\"serde_record\":{\"input_json_bytes\":25,\"output_json_bytes\":25,\"generated_mirror_string_clone_copied_bytes\":3,\"generated_mirror_to_record_transferred_string_bytes\":6,\"generated_mirror_to_record_copied_string_bytes\":0,\"generated_mirror_to_record_pointers_preserved\":true,\"deserialize_owned_string_copied_bytes\":{\"status\":\"unavailable\",\"reason\":\"no counter\"}},\"iterator_callback\":{\"fn_invocations\":1,\"fn_mut_invocations\":1,\"scalar_argument_result_copied_bytes\":0}}}"
    )
    linked = parse_linked_copy_ledger(linked_row)
    assert linked["m2"]["serde_record"]["generated_mirror_string_clone_copied_bytes"] == 3
    assert linked["m1"]["regex_result_owner"]["adapter_borrowed_scan_input_bytes"] == 28
    assert linked["m1"]["url_owner_view"]["adapter_copied_bytes"] == 0
    tampered_linked = json.loads(linked_row.removeprefix(LINKED_COPY_PREFIX))
    tampered_linked["m1"]["url_owner_view"]["adapter_copied_bytes"] = 1
    try:
        parse_linked_copy_ledger(LINKED_COPY_PREFIX + json.dumps(tampered_linked))
    except ValueError:
        pass
    else:
        raise AssertionError("nonzero Url adapter bytes must fail the exact zero-copy ledger")
    tampered_linked = json.loads(linked_row.removeprefix(LINKED_COPY_PREFIX))
    tampered_linked["m2"]["serde_record"]["generated_mirror_to_record_copied_string_bytes"] = 1
    try:
        parse_linked_copy_ledger(LINKED_COPY_PREFIX + json.dumps(tampered_linked))
    except ValueError:
        pass
    else:
        raise AssertionError("nonzero generated mirror-to-record bytes must fail the exact transfer ledger")
    tampered_linked = json.loads(linked_row.removeprefix(LINKED_COPY_PREFIX))
    tampered_linked["m1"]["regex_result_owner"]["adapter_borrowed_scan_input_bytes"] = 27
    try:
        parse_linked_copy_ledger(LINKED_COPY_PREFIX + json.dumps(tampered_linked))
    except ValueError:
        pass
    else:
        raise AssertionError("changed Regex borrowed scan bytes must fail the exact ledger")
    try:
        parse_linked_copy_ledger(
            "ri13-linked-copy-ledger:{\"schema\":\"semaprax.ri13.linked-copy-ledger.v1\",\"m1\":{},\"m2\":{}}"
        )
    except ValueError:
        pass
    else:
        raise AssertionError("linked ledger accepted missing unavailable evidence")
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
    linked_copy_ledger = None
    for name, command, expected in [
        (
            "m1_prepare",
            cargo_command("examples/ri13-m1-regex-url/prepare/Cargo.toml", "semaprax-ri13-m1-prepare"),
            "ri13-m1-prepared:",
        ),
        (
            "m1_consumer",
            cargo_command("examples/ri13-m1-regex-url/consumer/Cargo.toml", "semaprax-ri13-m1-consumer"),
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
        (
            "m3_negative_controls",
            m3_negative_control_command(),
            "test result: ok. 1 passed",
        ),
        (
            "linked_prepare",
            cargo_command(
                "examples/ri13-combined-app/linked/prepare/Cargo.toml", "prepare"
            ),
            "ri13-linked-prepared:",
        ),
        (
            "linked_consumer",
            cargo_command("examples/ri13-combined-app/linked/Cargo.toml", "consumer"),
            "ri13-linked-project-ok",
        ),
    ]:
        result, stdout = run(command, environment, expected)
        result["stage"] = name
        stages.append(result)
        if name == "linked_consumer":
            linked_copy_ledger = parse_linked_copy_ledger(stdout)

    if linked_copy_ledger is None:
        raise RuntimeError("linked consumer did not produce copied-byte evidence")

    m1_batch_command = cargo_command("examples/ri13-m1-regex-url/consumer/Cargo.toml", "measure")
    m1_batch_result, m1_batch_samples = run(
        m1_batch_command, environment, "url_parse_view,generated_semaprax"
    )
    m1_batch_result["stage"] = "m1_batch_throughput_measurement"
    m1_batch_measurement = parse_m1_batch_samples(m1_batch_samples)

    m2_batch_command = cargo_command("examples/ri13-m2-record-iterator/Cargo.toml", "measure")
    m2_batch_result, m2_batch_samples = run(
        m2_batch_command, environment, "stateful_callback,generated_semaprax"
    )
    m2_batch_result["stage"] = "m2_batch_throughput_measurement"
    m2_batch_measurement = parse_m2_batch_samples(m2_batch_samples)

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
        "m1_batch_throughput_measurement_command": m1_batch_result,
        "m1_batch_throughput": m1_batch_measurement,
        "m2_batch_throughput_measurement_command": m2_batch_result,
        "m2_batch_throughput": m2_batch_measurement,
        "batch_throughput_measurement_command": batch_result,
        "batch_throughput": batch_measurement,
        "m3_copy_ledger": m3_copy_ledger(route_measurement),
        "linked_copy_ledger": linked_copy_ledger,
        "limits": [
            "The first six stages retain separately admitted M1, M2, and M3 profiles. The final two stages prepare and execute the distinct held linked Project, without claiming that it is one public SDK profile.",
            "Build-and-consumer stage elapsed times include Cargo work and process startup; they are not route latency.",
            "M1 retains matched 4096-operation repeats of the held scalar Regex and Url exports across direct Rust, handwritten adapters, and generated Semaprax. It does not measure varying scan inputs, and Regex/Url foreign implementation copies remain unavailable.",
            "M2 retains matched 32-operation generic-record and stateful-callback batches for direct Rust, handwritten adapters, and generated Semaprax.",
            "Scalar route timings include loopback HTTP and two-byte response parsing; the separate 64-operation batch rows retain their own normalized throughput and remain local evidence.",
            "Allocator values count current-thread requests and do not infer copies. The ledger records exact zero-byte scalar generated and host-callback payload boundaries, response wire bytes, and explicit unavailable cells for reqwest/HTTP/text/capture copies.",
            "The linked ledger measures only the generated Regex owner carrier, generated Serde mirror clone, and scalar iterator callback boundaries. Url::parse and serde_json deserialization retain unavailable copied-byte states.",
        ],
    }
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(arguments.output)


if __name__ == "__main__":
    main()
