#!/usr/bin/env python3
"""Summarize exact local M3 samples without treating loopback as ABI overhead."""

import csv
import json
import random
import statistics
import sys
from pathlib import Path


ROUTES = ("direct_rust", "handwritten_adapter", "generated_semaprax")
EXPECTED = 90


def percentile(values, percent):
    values = sorted(values)
    offset = (len(values) - 1) * percent / 100
    lower = int(offset)
    upper = min(lower + 1, len(values) - 1)
    return values[lower] + (values[upper] - values[lower]) * (offset - lower)


def summarize(path):
    rows = list(csv.DictReader(path.open(newline="", encoding="utf-8")))
    assert len(rows) == EXPECTED * len(ROUTES), "missing or extra samples"
    samples = {route: {} for route in ROUTES}
    for row in rows:
        route = row["route"]
        assert route in samples
        iteration = int(row["iteration"])
        assert 0 <= iteration < EXPECTED and iteration not in samples[route]
        assert int(row["body_bytes"]) == 2
        elapsed_ns = int(row["elapsed_ns"])
        assert elapsed_ns > 0
        samples[route][iteration] = elapsed_ns
    assert all(len(values) == EXPECTED for values in samples.values())

    result = {"input": str(path), "samples_per_route": EXPECTED, "routes": {}}
    for route in ROUTES:
        durations = list(samples[route].values())
        mean_ns = statistics.mean(durations)
        result["routes"][route] = {
            "mean_ns": round(mean_ns, 1),
            "p50_ns": round(percentile(durations, 50), 1),
            "p90_ns": round(percentile(durations, 90), 1),
            "p99_ns": round(percentile(durations, 99), 1),
            "serialized_calls_per_second": round(1e9 / mean_ns, 2),
        }

    manual = samples["handwritten_adapter"]
    generated = samples["generated_semaprax"]
    ratio = statistics.mean(manual.values()) / statistics.mean(generated.values())
    rng = random.Random(371)
    ratios = []
    for _ in range(1000):
        chosen = [rng.randrange(EXPECTED) for _ in range(EXPECTED)]
        manual_mean = statistics.mean(manual[i] for i in chosen)
        generated_mean = statistics.mean(generated[i] for i in chosen)
        ratios.append(manual_mean / generated_mean)
    result["generated_vs_handwritten_throughput_ratio"] = round(ratio, 4)
    result["paired_bootstrap_95_percent_ratio"] = [
        round(percentile(ratios, 2.5), 4),
        round(percentile(ratios, 97.5), 4),
    ]
    result["investigate_over_10_percent_slower"] = ratio < 0.9
    result["interpretation"] = (
        "This tiny loopback HTTP comparison includes source preparation; "
        "it does not establish the nontrivial batch-work threshold."
    )
    return result


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: summarize.py samples.csv")
    print(json.dumps(summarize(Path(sys.argv[1])), sort_keys=True, indent=2))
