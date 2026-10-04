#!/usr/bin/env python3
"""Bind one combined RI-13 receipt to its scoped batch investigation.

M1 and M2 have no matched direct/handwritten batch benchmark in the combined
fixture.  This tool makes that absence explicit instead of deriving a ratio
from Cargo stage time or a different workload.
"""

import argparse
import hashlib
import json
from pathlib import Path


SCHEMA = "semaprax.ri13.combined-throughput-investigation.v1"
COMBINED_SCHEMA = "semaprax.ri13.combination-measurement.v1"
M3_SCHEMA = "semaprax.ri13.m3-batch-investigation.v1"
ROUTES = ("direct_rust", "handwritten_adapter", "generated_semaprax")
STAGES = (
    "m1_prepare", "m1_consumer", "m2_prepare", "m2_consumer",
    "m3_prepare", "m3_consumer", "m3_negative_controls", "linked_prepare", "linked_consumer",
)
THRESHOLD = 0.90


def canonical(value):
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def load(path, schema, label):
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {label} {path}") from error
    if not isinstance(value, dict) or value.get("schema") != schema:
        raise ValueError(f"{label} has an unsupported schema")
    return value


def batch_routes(receipt):
    stages = receipt.get("full_build_and_consumer_stages")
    if (
        not isinstance(stages, list)
        or len(stages) != len(STAGES)
        or not all(isinstance(row, dict) for row in stages)
        or [row.get("stage") for row in stages] != list(STAGES)
    ):
        raise ValueError("combined receipt does not bind the exact M1/M2/M3 stages")
    command = receipt.get("batch_throughput_measurement_command")
    batch = receipt.get("batch_throughput")
    routes = batch.get("routes") if isinstance(batch, dict) else None
    if not isinstance(command, dict) or command.get("command", [])[-2:] != ["--", "batch"]:
        raise ValueError("combined receipt does not bind the M3 batch selector")
    if not isinstance(routes, dict) or set(routes) != set(ROUTES):
        raise ValueError("combined receipt has no exact M3 comparison routes")
    for name, row in routes.items():
        if not isinstance(row, dict) or row.get("operations_per_sample") != 64 or row.get("body_bytes_per_operation") != 2:
            raise ValueError(f"combined receipt changed the reviewed M3 workload for {name}")
        if not isinstance(row.get("normalized_operations_per_second"), (int, float)) or row["normalized_operations_per_second"] <= 0:
            raise ValueError(f"combined receipt lacks normalized throughput for {name}")
    return routes


def investigate(receipt_path, m3_path):
    receipt = load(receipt_path, COMBINED_SCHEMA, "combined receipt")
    m3 = load(m3_path, M3_SCHEMA, "M3 investigation")
    routes = batch_routes(receipt)
    m3_receipt = m3.get("receipt")
    if not isinstance(m3_receipt, dict) or m3_receipt.get("sha256") != digest(receipt_path):
        raise ValueError("M3 investigation is not bound to this combined receipt")
    measured = m3.get("measured")
    if not isinstance(measured, dict):
        raise ValueError("M3 investigation lacks measured ratios")
    generated = routes["generated_semaprax"]["normalized_operations_per_second"]
    handwritten = routes["handwritten_adapter"]["normalized_operations_per_second"]
    direct = routes["direct_rust"]["normalized_operations_per_second"]
    if (measured.get("generated_operations_per_second"), measured.get("handwritten_operations_per_second"), measured.get("direct_operations_per_second")) != (generated, handwritten, direct):
        raise ValueError("M3 investigation throughput differs from the combined receipt")
    ratio = generated / direct
    return {
        "schema": SCHEMA,
        "receipt": {"path": str(receipt_path), "sha256": digest(receipt_path), "checkout": receipt.get("checkout")},
        "m3_investigation": {"path": str(m3_path), "sha256": digest(m3_path), "status": m3.get("status")},
        "profiles": {
            "m1": {
                "status": "unavailable",
                "reason": "the combined receipt has M1 prepare/consumer stages but no matched direct and handwritten batch routes",
            },
            "m2": {
                "status": "unavailable",
                "reason": "the combined receipt has M2 prepare/consumer stages but no matched direct and handwritten batch routes",
            },
            "m3": {
                "status": "investigation_required" if ratio < THRESHOLD else "threshold_not_triggered",
                "workload": {"operations_per_sample": 64, "body_bytes_per_operation": 2},
                "normalized_operations_per_second": {
                    "direct_rust": direct,
                    "handwritten_adapter": handwritten,
                    "generated_semaprax": generated,
                },
                "generated_to_direct_throughput_ratio": round(ratio, 4),
                "generated_to_handwritten_throughput_ratio": round(generated / handwritten, 4),
                "investigation_threshold": THRESHOLD,
            },
        },
        "limitations": [
            "This is a local investigation record, not a performance pass or cross-platform claim.",
            "M1 and M2 remain unavailable until the same workload has matched direct and handwritten measurements.",
            "The M3 source-bound investigation explains repeated registration but does not attribute an exact share of route time to an operation.",
        ],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--m3-investigation", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args(argv)
    try:
        result = investigate(arguments.receipt, arguments.m3_investigation)
    except ValueError as error:
        parser.error(str(error))
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(canonical(result), encoding="utf-8")


if __name__ == "__main__":
    main()
