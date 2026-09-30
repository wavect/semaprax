#!/usr/bin/env python3
"""Account for #326's frozen control cells without authorizing or running them.

This is a read-only projection of the existing specialization_protocol v1,
not a second protocol, provider adapter, result importer or review attestation.
Exit 3 means the complete accounting was emitted but the experiment is blocked;
exit 2 means even the input accounting could not be authenticated.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import sys

SUITE = pathlib.Path(__file__).resolve().parent.parent
if str(SUITE) not in sys.path:
    sys.path.insert(0, str(SUITE))

import runnable_adapter as v1
import runnable_v3_provenance as acquisition
from agent import specialization_protocol as protocol

SCHEMA = "benchmark.cross_language.agent.specialization_accounting.v1"
ORIGIN_COMMIT = "ebe4235ebc2b9621f7fd174f8bf7647744dc88c5"
REVIEWED_BRANCH_COMMIT = "6a7a341273c30b81cf53f3fa9ef94dc7930b4c72"
FROZEN_PLAN_SHA256 = "sha256:cfed39a0f84f0c44dce9d27b978878798fc6ec4733f035bb756a3f3ea65a6ef4"
CURRENT_PLAN_SHA256 = "sha256:13e15167643e375335cdfdb5fc287aca6c93df35ee35d75728f71ed8c6fec26c"
BLOCKED_EXIT = 3

# Repository-owned input identities, not caller-selected hashes or approvals.
# The two retained JSON files have the exact Git blob identities from ebe4235e:
# protocol 457e56e77d68155c562068ede4e1f9c5fecd6a6f,
# tasks    ae30aba4943a2e52bf056edcd7bb5f7d680114a8.
# Code/oracle bytes below are read as data, not executed by this acquisition.
INPUTS = {
    "frozen_protocol": ("agent/provenance/specialization-held-out-v1.protocol.json", 2872,
                        "3165375ffa9ce3b20e650be8dd261263d6d59bcb105868364580cf9b1c35c62a"),
    "frozen_tasks": ("agent/provenance/specialization-held-out-v1.tasks.json", 13789,
                     "f5cd390280bbdd82533fa6953d114ecf66a67d65c66d6571ccc6acb05a1113f1"),
    "current_protocol": ("agent/specialization-protocol.example.json", 2872,
                         "5cd05297bee0d040399b9a42adee7adb3beb3dc181e92a8997a3614d144e0745"),
    "current_tasks": ("tasks.json", 26607,
                      "cd6cef4d71810f1f1c9187b5750fa2dc8a0c9a1fe4dee32a9a70b51f19e3700f"),
    "plan_builder": ("agent/specialization_protocol.py", 16696,
                     "7428264909d186a2e1005d90cf8e41491bd57102dec638cec7c152a8a1e46bd4"),
    "current_oracle": ("run.py", 28079,
                       "29b9955133f70efabf9f240f5254a5654e0aec21ef7acf4e12b975b45322d5aa"),
    "live_transport": ("agent/live_transport.py", 3927,
                       "7c3b9edcda76185c1b7b8f1771dc39a9469f5e07fc2129ca74c1fc76d402c21b"),
}
FROZEN_TASK_IDS = (
    "module-import-refactor-v1", "booking-window-conflict-v1",
    "cold-chain-release-gate-v1", "stable-dispatch-order-v1",
    "owned-byte-sentinel-balance-v1", "stale-edit-preservation-v1",
    "telemetry-overflow-diagnosis-v1", "clean-install-calculator-v1",
    "concurrent-delta-merge-v1",
)

# Missing authority is recorded, never manufactured from a task request, a
# fixture identity, an environment variable, or the existence of this report.
BLOCKERS = {
    "base_model_not_approved": "The retained provider/model/revision are offline-fixture values, not an approved real model.",
    "controls_not_approved": "Guidance, action schema, sampling, tool configuration and equal budgets need an exact reviewed artifact set.",
    "adaptation_and_split_not_approved": "No development-only adaptation method/dataset and frozen held-out custody plan is approved; no adapted arm is added.",
    "oracle_and_source_not_approved": "The frozen oracle digest differs from current run.py. Corpus/oracle materialization and equivalence need review; no silent re-pin.",
    "leakage_review_missing": "No independent leakage review or attestation is present. Metadata-only accounting cannot attest to absence of experimental leakage.",
    "credential_spend_egress_authority_missing": "Explicit provider credential use, training/inference spend and endpoint/data-egress ceilings are not authorized.",
    "independent_reviewer_custody_missing": "No independent blinded reviewer/data custodian or control-comparison review record is assigned.",
    "live_transport_unimplemented": "The existing LiveTransport.complete still refuses; a reviewed and verified real-provider integration is required after authorization.",
}


def acquisition_available() -> bool:
    """Require the existing no-follow reader's primitives without a fallback."""
    return (all(getattr(os, name, 0) for name in
                ("O_NOFOLLOW", "O_DIRECTORY", "O_CLOEXEC", "O_NONBLOCK"))
            and os.open in getattr(os, "supports_dir_fd", set()))


def _snapshot() -> dict[str, bytes]:
    if not acquisition_available():
        raise acquisition.Error("specialization_accounting_acquisition_unavailable")
    result = {}
    for role, (relative, size, expected) in INPUTS.items():
        data = acquisition.read_regular(SUITE / relative, size)
        if len(data) != size or acquisition.digest(data) != expected:
            raise acquisition.Error(f"specialization_accounting_input_drift:{role}")
        result[role] = data
    return result


def _cell_key(row: dict) -> tuple:
    return row["variant"], row["task"], row["language"], row["split"], row["trial"]


def _unexecuted(row: dict, reason_ids: list[str], classification: str) -> dict:
    # Absence is not correctness=false, free inference, or zero-millisecond work.
    return dict(row, classification=classification, reason_ids=reason_ids.copy(),
                metrics={name: None for name in protocol.REQUIRED_METRICS},
                trial_wall_ms=None, outcome_artifact_sha256=None)


def _task_inventory(data: bytes, planned: set[str], language: str) -> list[dict]:
    rows = []
    for task in json.loads(data)["tasks"]:
        if task["id"] in planned:
            selection = "in_original_matrix"
        elif task["split"] != "held_out":
            selection = "not_held_out"
        elif language not in task["languages"]:
            selection = "no_declared_evaluation_language"
        else:
            selection = "outside_original_matrix"
        rows.append({"task": task["id"], "split": task["split"],
                     "declared_languages": list(task["languages"]), "selection": selection})
    return rows


def report() -> dict:
    """Authenticate public metadata; account for every original and added cell.

    No caller-selected root, filter, budget, approval, result or provider exists.
    The same v1 builder makes both schedules. Restoring historical input bytes
    does not roll back today's corpus or make a runnable source/oracle capsule.
    """
    sources = _snapshot()
    frozen = protocol.build_plan(json.loads(sources["frozen_protocol"]), sources["frozen_tasks"])
    current = protocol.build_plan(json.loads(sources["current_protocol"]), sources["current_tasks"])
    if protocol.sha256(protocol.canonical_bytes(frozen)) != FROZEN_PLAN_SHA256:
        raise acquisition.Error("specialization_frozen_plan_drift")
    if protocol.sha256(protocol.canonical_bytes(current)) != CURRENT_PLAN_SHA256:
        raise acquisition.Error("specialization_current_plan_drift")
    expected = [(variant, task, "semaprax-project", "held_out", repeat)
                for variant in protocol.REQUIRED_VARIANTS for task in FROZEN_TASK_IDS
                for repeat in range(1, 4)]
    if [_cell_key(row) for row in frozen["rows"]] != expected or len(expected) != 81:
        raise acquisition.Error("specialization_frozen_denominator_drift")
    expected_set = set(expected)
    current_keys = [_cell_key(row) for row in current["rows"]]
    if len(current_keys) != 90 or len(set(current_keys)) != 90 or not expected_set.issubset(current_keys):
        raise acquisition.Error("specialization_current_denominator_drift")
    added = [row for row in current["rows"] if _cell_key(row) not in expected_set]
    if (len(added) != 9 or {row["task"] for row in added} != {"iterative-repair-workflow-v1"}):
        raise acquisition.Error("specialization_added_cells_drift")
    reasons = list(BLOCKERS)
    rows = [_unexecuted(row, reasons, "unexecuted_authorization_and_review_missing")
            for row in frozen["rows"]]
    unreviewed = {"status": "not_recorded", "reviewer": None,
                  "independence_verified": False, "record_sha256": None}
    source_records = [
        {"role": role, "path": "benchmarks/cross-language-v1/" + relative,
         "bytes": size, "sha256": protocol.sha256(sources[role]),
         "source_commit": ORIGIN_COMMIT if role.startswith("frozen_") else REVIEWED_BRANCH_COMMIT,
         "origin_path": "benchmarks/cross-language-v1/" + (
             "tasks.json" if role == "frozen_tasks" else
             "agent/specialization-protocol.example.json" if role == "frozen_protocol" else relative)}
        for role, (relative, size, _) in INPUTS.items()
    ]
    result = {
        "schema": SCHEMA, "record_kind": "unexecuted_cell_accounting",
        "status": "blocked_not_authorized", "execution": "not_attempted",
        "is_model_result": False, "issue_closable": False,
        "non_claim": "This metadata-only accounting is not an approved experiment, model trial, leakage review or independent control comparison.",
        "issue": 326, "source_records": source_records,
        "frozen_plan_sha256": FROZEN_PLAN_SHA256,
        "current_plan_sha256": CURRENT_PLAN_SHA256,
        "protocol_id": frozen["protocol_id"],
        "authorization": {"status": "not_authorized", "record_sha256": None,
                          "original_requirements": frozen["authorization_required"]},
        "proposed_base_model": frozen["base_model"],
        "proposed_tool_configuration": frozen["tool_configuration"],
        "proposed_resource_policy": frozen["resource_policy"],
        "proposed_variants": frozen["variants"],
        "oracle": {"frozen_proposal": frozen["acceptance_oracle"],
                   "current_runner_sha256": protocol.sha256(sources["current_oracle"]),
                   "current_matches_frozen": protocol.sha256(sources["current_oracle"]) == frozen["acceptance_oracle"]["sha256"],
                   "status": "not_approved_requires_source_and_oracle_review"},
        "adaptation": {"status": "not_approved", "arm_in_original_matrix": False,
                       "method": None, "dataset_manifest_sha256": None,
                       "owner_declared_development_tasks": [
                           task["id"] for task in json.loads(sources["frozen_tasks"])["tasks"]
                           if task["split"] == "development"]},
        "blockers": [{"id": key, "reason": value, "resolution_record_sha256": None}
                     for key, value in BLOCKERS.items()],
        "counts": {"controls": 3, "held_out_tasks": 9, "repeats": 3,
                   "planned_cells": len(rows), "unexecuted_cells": len(rows),
                   "actual_model_trials": 0, "observed_outcomes": 0},
        "metrics": list(protocol.REQUIRED_METRICS), "rows": rows,
        "original_task_inventory": _task_inventory(sources["frozen_tasks"], set(FROZEN_TASK_IDS), "semaprax-project"),
        "current_task_inventory": _task_inventory(sources["current_tasks"], set(FROZEN_TASK_IDS), "semaprax-project"),
        "current_example_expansion": {
            "planned_cells": len(current["rows"]), "in_original_matrix": 81,
            "additional_cells": len(added), "authorized_expansion": False,
            "reason": "Today's example includes iterative-repair-workflow-v1. Its nine cells are not silently added to the original experiment or removed from the current corpus.",
            "rows": [_unexecuted(row, ["outside_original_frozen_matrix"] + reasons,
                                 "outside_original_frozen_matrix") for row in added]},
        "control_summary": [{"variant": variant, "planned_cells": 27,
                             "unexecuted_cells": 27, "actual_model_trials": 0,
                             "accepted_outcome_rate": None, "mean_cost_usd": None,
                             "mean_trial_wall_ms": None, "status": "not_estimable"}
                            for variant in protocol.REQUIRED_VARIANTS],
        "control_comparisons": [{"left": left, "right": right, "paired_model_trials": 0,
                                 "status": "not_estimable", "effect": None, "uncertainty": None,
                                 "reason": "No actual model outcomes; neither zero effect nor equivalence is established."}
                                for left, right in (("guided", "base"), ("constrained", "base"),
                                                    ("constrained", "guided"))],
        "independent_reviews": {"control_comparison": unreviewed.copy(),
                                "leakage": unreviewed.copy()},
        "data_custody": {"status": "unassigned", "custodian": None, "record_sha256": None},
        "acceptance": {"every_original_cell_classified": True, "approved_matrix_executed": False,
                       "independent_control_comparison_recorded": False,
                       "independent_leakage_review_recorded": False},
    }
    if len(protocol.canonical_bytes(result)) > v1.MAX_RESULT_BYTES:
        raise acquisition.Error("specialization_accounting_result_exceeds_bound")
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args(argv)  # No spend, approval, result, task filter or execution flags.
    try:
        data = protocol.canonical_bytes(report())
    except (acquisition.Error, OSError, ValueError, KeyError, TypeError) as error:
        failure = {"schema": SCHEMA, "status": "unavailable", "execution": "not_attempted",
                   "issue_closable": False, "reason": str(error)}
        sys.stderr.write(protocol.canonical_bytes(failure).decode("ascii"))
        return 2
    sys.stdout.write(data.decode("ascii"))
    return BLOCKED_EXIT


if __name__ == "__main__":
    raise SystemExit(main())
