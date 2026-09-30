#!/usr/bin/env python3
"""Report the fixed #322 official comparison scope; never execute a toolchain.

Implemented adapters and local-fixture execution are not independent official
admission. The existing v3 TypeScript profile is the only supported official
lane. This read-only projection preserves the frozen 13 x 14 denominator and
its original flags/reasons; it cannot admit a toolchain or produce a score.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import sys

SUITE = pathlib.Path(__file__).resolve().parent
if str(SUITE) not in sys.path:
    sys.path.insert(0, str(SUITE))

import runnable_adapter as v1
import runnable_v3_corrections as corrections
import runnable_v3_provenance as provenance

SCHEMA = "benchmark.cross_language.supported_scope.v1"
OFFICIAL_PROFILE = "benchmark.cross_language.runnable_adapter.v3"
SUPPORTED_ADAPTER = "typescript"
PREFIX = "benchmarks/cross-language-v1/"

# Implementation-owned scope, not caller-selectable admission or new origin
# metadata. Unsupported lanes remain in every task's denominator.
EXCLUSIONS = {
    "semaprax": "Repository compiler fixture; no independent official-toolchain admission in the comparison v3 profile.",
    "semaprax-project": "Repository Project fixture; no independent official-toolchain admission in the comparison v3 profile.",
    "rust": "v1 local_fixture only; official rustc distribution, runtime dependencies and host authority are not admitted by v3.",
    "c": "v2 local_fixture only; official Clang distribution, runtime dependencies and host authority are not admitted by v3.",
    "python": "v2 local_fixture only; official CPython distribution, runtime dependencies and host authority are not admitted by v3.",
    "swift": "v2 local_fixture only; official Swift distribution, runtime dependencies and host authority are not admitted by v3.",
    "java": "v2 local_fixture only; official JDK distribution, runtime dependencies and host authority are not admitted by v3.",
    "zero": "Historical Zero revision is not an admitted official comparison subject; official provenance and runnable ports are not admitted by v3.",
    "ntnt": "No official NTNT toolchain provenance or runnable task ports are admitted in this comparison profile.",
    "aver": "No official Aver toolchain provenance or runnable task ports are admitted in this comparison profile.",
    "vera": "No official Vera toolchain provenance or runnable task ports are admitted in this comparison profile.",
    "hale": "No official Hale toolchain provenance or runnable task ports are admitted in this comparison profile.",
    "moonbit": "No pinned offline official MoonBit toolchain provenance or runnable task ports are admitted in this comparison profile.",
}


def acquisition_available() -> bool:
    """Require the existing POSIX acquisition primitives; never use a fallback."""
    return (all(getattr(os, name, 0) for name in
                ("O_NOFOLLOW", "O_DIRECTORY", "O_CLOEXEC", "O_NONBLOCK"))
            and os.open in getattr(os, "supports_dir_fd", set()))


def _project_inventory(manifest: dict, sources: dict[str, bytes]) -> list[dict]:
    """Join only authenticated inventory bytes, preserving declaration order."""
    tasks = json.loads(sources[PREFIX + "tasks.json"])["tasks"]
    adapters = json.loads(sources[PREFIX + "adapters.json"])["adapters"]
    task_ids = [task["id"] for task in tasks]
    adapter_ids = [adapter["id"] for adapter in adapters]
    if (len(task_ids) != 13 or len(set(task_ids)) != 13
            or task_ids != manifest["task_ids"] or len(adapter_ids) != 14
            or len(set(adapter_ids)) != 14
            or set(adapter_ids) != set(EXCLUSIONS) | {SUPPORTED_ADAPTER}):
        raise provenance.Error("supported_scope_denominator_mismatch")
    expected = [
        {"task_id": task["id"], "adapter_id": adapter["id"],
         "declared": adapter["id"] in task["languages"],
         "implemented": adapter["implemented"],
         "blocked_reason": adapter.get("blocked_reason")}
        for task in tasks for adapter in adapters
    ]
    if manifest["comparison_inventory"] != expected:
        raise provenance.Error("supported_scope_inventory_mismatch")
    result = []
    for row in expected:
        supported = row["adapter_id"] == SUPPORTED_ADAPTER
        if supported and (row["declared"] is not True or row["implemented"] is not True
                          or row["blocked_reason"] is not None):
            raise provenance.Error("supported_scope_source_lane_incomplete")
        reason = None if supported else EXCLUSIONS[row["adapter_id"]]
        if not supported and not row["declared"]:
            reason += " No public/hidden port is declared for this task/adapter slot."
        result.append(dict(row, official_support="supported" if supported else "not_supported",
                           support_reason=reason,
                           official_profile=OFFICIAL_PROFILE if supported else None))
    return result


def report() -> dict:
    """Acquire the fixed corpus and correction through existing no-follow gates.

    There is deliberately no root, policy, subset, toolchain or expected-hash
    argument. Availability of a tool on PATH cannot alter the supported set.
    Admission of input bytes is not admission or execution of a runtime.
    """
    if not acquisition_available():
        raise provenance.Error("supported_scope_acquisition_unavailable")
    manifest, sources = provenance.source_snapshot()
    # Verify the already-approved effective TypeScript source as well as the
    # unchanged baseline; never mistake the historical stale-edit formula for
    # the corrected subject. No candidate is compiled or scored here.
    corrections.admit(manifest, sources)
    inventory = _project_inventory(manifest, sources)
    adapter_ids = [row["adapter_id"] for row in inventory[:14]]
    result = {
        "schema": SCHEMA,
        "status": "scope_inventory",
        "execution": "not_attempted",
        "runtime_availability": "not_probed",
        "decision": {"issue": 322, "date": "2026-09-30",
                     "kind": "retain_existing_official_scope", "newly_admitted_lanes": []},
        "source_origin": provenance.SOURCE_COMMIT,
        "source_manifest_sha256": provenance.SOURCE_HASH,
        "source_correction_sha256": corrections.HASH,
        "supported_adapter_ids": [SUPPORTED_ADAPTER],
        "not_supported_adapter_ids": [name for name in adapter_ids if name != SUPPORTED_ADAPTER],
        "supported_profile": {
            "schema": OFFICIAL_PROFILE,
            "node": "22.12.0", "typescript": "5.8.3",
            "host": {"platform": "darwin-arm64", "version": "26.5.1", "build": "25F80"},
            "runtime_gate": "test_runnable_adapter_v3.py",
        },
        "counts": {"tasks": 13, "adapters": 14, "slots": 182,
                   "supported_slots": 13, "not_supported_slots": 169},
        "comparison_inventory": inventory,
    }
    if len(provenance.canonical(result)) > v1.MAX_RESULT_BYTES:
        raise provenance.Error("supported_scope_result_exceeds_bound")
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args(argv)  # No filtering or caller-defined support policy.
    try:
        data = provenance.canonical(report())
    except (provenance.Error, OSError, ValueError, KeyError, TypeError) as error:
        failure = {"schema": SCHEMA, "status": "unavailable", "execution": "not_attempted",
                   "reason": str(error)}
        print(provenance.canonical(failure).decode("ascii"), file=sys.stderr, end="")
        return 2
    sys.stdout.write(data.decode("ascii"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
