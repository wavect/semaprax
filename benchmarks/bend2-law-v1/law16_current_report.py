#!/usr/bin/env python3
"""Render the current LAW-16 evidence state from authenticated local capsules."""
import argparse
import hashlib
import importlib.util
import json
import pathlib
import statistics
import sys

ROOT = pathlib.Path(__file__).parent


def module(name):
    if str(ROOT) not in sys.path:
        sys.path.insert(0, str(ROOT))
    spec = importlib.util.spec_from_file_location(name, ROOT / f"{name}.py")
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


PILOT = module("law16_boolean_negation_agent_pilot")
CAMPAIGN = module("law16_boolean_negation_agent_campaign_capsule")
PROCESS = module("law16_boolean_negation_process_capsule")
REMAINING = module("law16_remaining_cells_admission")
RSS = module("law16_boolean_negation_rss_capsule")
NONPROOF = module("law16_boolean_negation_nonproof_capsule")
PROOFVERDICT = module("law16_boolean_negation_proof_verdict_capsule")
FULL_U32_EQUAL_SPEC = module("full_u32_equal_spec")
GUARDED_I64_BALANCE = module("law16_guarded_i64_balance_smt")
GUARDED_I64_PROFILE = module("full_u32_guarded_i64_profile_v2")
BEND_U32_SORT = module("law16_bend_u32_sort_proof")
COST_PROVENANCE = module("law16_boolean_negation_cost_provenance")
BOOLEAN_ANNOTATIONS = module("law16_annotation_summary")
CACHE_ISOLATION = module("law16_cache_isolation_probe")
BOOLEAN_REFACTOR = module("law16_boolean_refactor_cell")
GUEST_CACHE = module("law16_guest_cache")
PROJECT_INCREMENTAL = module("law16_project_incremental_cell")
NATIVE_PHASES = module("law16_boolean_native_phases")
SCHEMA = "semaprax.bend2-law-benchmark.current-report.v1"


def read(path):
    return json.loads(path.read_text())


def boolean_synthesis_effort(pilot, campaign):
    """Summarize only the retained agent-turn token evidence."""
    totals = {
        "bend2": {"agent_turns": 0, "chargeable_tokens": 0},
        "semaprax-scalar-v1": {"agent_turns": 0, "chargeable_tokens": 0},
    }
    for language, lane in pilot["lanes"].items():
        totals[language]["agent_turns"] += 1
        totals[language]["chargeable_tokens"] += lane["chargeable_tokens"]
    for pair in campaign["pairs"]:
        for language, lane in pair.items():
            if language == "ordinal":
                continue
            totals[language]["agent_turns"] += 1
            totals[language]["chargeable_tokens"] += lane["chargeable_tokens"]
    return {
        "status": "observed",
        "scope": "Boolean-negation agent edits only",
        "phase": "one isolated agent turn per language and ordinal; not kernel or Z3 checking",
        "matched_pairs": 10,
        "per_language": totals,
        "cost": {
            "status": "unavailable",
            "reason": "retained Codex JSON events contain no monetary charge",
        },
    }


def timing_variation(routes):
    """Summarize within-cell spread after capsule reviewers authenticate samples."""
    groups = {}
    for group, entries in routes.items():
        group_rows = {}
        for label, path in entries.items():
            receipt = read(path)
            states = {}
            for state in ("fresh_process", "repeat_process"):
                samples = receipt["cells"][state]["samples"]
                elapsed = [sample["elapsed_ns"] for sample in samples]
                if len(elapsed) != 30 or any(not isinstance(value, int) or value < 0 for value in elapsed):
                    raise ValueError(f"{group}/{label}/{state} does not contain 30 valid elapsed-time samples")
                median = statistics.median(elapsed)
                states[state] = {
                    "count": len(elapsed),
                    "median_ns": median,
                    "mad_ns": statistics.median(abs(value - median) for value in elapsed),
                }
            group_rows[label] = states
        groups[group] = group_rows
    return {
        "method": "median absolute deviation from the within-cell median, in nanoseconds",
        "interpretation": "descriptive sample spread only; not a confidence interval or cross-route comparison",
        "cells": groups,
    }
def render():
    remaining = REMAINING.review()
    try:
        historical_rss = {"status": "authenticated", "review": RSS.review(ROOT / "evidence/law16-boolean-negation-peak-rss-v1")}
    except ValueError as error:
        historical_rss = {"status": "unavailable", "reason": str(error), "scope": "historical absolute-path evidence is not rebound to this checkout"}
    rss = RSS.review_current(ROOT / "evidence/law16-boolean-negation-peak-rss-v2")
    nonproof = NONPROOF.review(ROOT / "evidence/law16-boolean-negation-nonproof-process-v1")
    proof_verdict = PROOFVERDICT.review(ROOT / "evidence/law16-boolean-negation-proof-verdict-v1")
    pilot = PILOT.review(ROOT / "evidence/law16-boolean-negation-agent-pilot-v1")
    campaign = CAMPAIGN.review(ROOT / "evidence/law16-boolean-negation-agent-campaign-v1")
    process = PROCESS.review(ROOT / "evidence/law16-boolean-negation-process-v1")
    process_v2 = PROCESS.review(ROOT / "evidence/law16-boolean-negation-process-v2")
    full_u32_controls = read(ROOT / "evidence/full-u32-encoding-controls-v1/report.json")
    full_u32_equal_spec = FULL_U32_EQUAL_SPEC.profile()
    balance_source_proof = GUARDED_I64_BALANCE.verify_capsule(
        ROOT / "evidence/law16-guarded-i64-balance-smt-v1"
    )
    balance_source_proof_result = read(
        ROOT / "evidence/law16-guarded-i64-balance-smt-v1/result.json"
    )
    profile_errors = GUARDED_I64_PROFILE.verify()
    if profile_errors:
        raise ValueError(f"guarded-i64 profile drifted: {profile_errors}")
    guarded_i64_profile = read(ROOT / "fixtures/full-u32-guarded-i64-profile-v2.json")
    bend_u32_sort = BEND_U32_SORT.verify(
        ROOT / "evidence/bend-u32-sort-universal-v1/capsule.json"
    )
    law15_list = read(ROOT / "evidence/law16-i64-list-proof-v1/capsule.json")
    cost_provenance = COST_PROVENANCE.capture(ROOT / "evidence")
    if cost_provenance != read(ROOT / "evidence/law16-boolean-negation-agent-cost-provenance-v1.json"):
        raise ValueError("Boolean agent cost provenance receipt drifted")
    claude_plan_path = ROOT / "fixtures/law16-claude-cost-pilot-plan-v1.json"
    claude_plan = read(claude_plan_path)
    claude_pilot = read(ROOT / "evidence/law16-claude-cost-pilot-v1.json")
    if (
        claude_pilot.get("schema") != "semaprax.bend2-law-benchmark.claude-cost-pilot.v1"
        or claude_plan.get("schema") != "semaprax.bend2-law-benchmark.claude-cost-pilot-plan.v1"
        or claude_pilot.get("plan", {}).get("sha256") != "sha256:" + hashlib.sha256(claude_plan_path.read_bytes()).hexdigest()
        or claude_pilot.get("provider", {}).get("model_id") != claude_plan.get("provider", {}).get("model_id")
        or claude_pilot.get("campaign_admission") is not False
    ):
        raise ValueError("Claude cost pilot plan or nonadmission receipt drifted")
    cache_probe = CACHE_ISOLATION.review()
    guest_cache = GUEST_CACHE.review(ROOT / "evidence/law16-guest-cache-thirty-v1")
    project_incremental = PROJECT_INCREMENTAL.verify(ROOT / "evidence/law16-project-incremental-cell-v1")
    native_phases = NATIVE_PHASES.review(ROOT / "evidence/law16-native-phase-thirty-v1")
    boolean_refactor = BOOLEAN_REFACTOR.verify(ROOT / "evidence/law16-boolean-refactor-cell-v1")
    claude_boolean_pilot = read(ROOT / "evidence/law16-claude-boolean-pilot-v1/capsule.json")
    if claude_boolean_pilot.get("schema") != "semaprax.bend2-law-benchmark.claude-boolean-pilot-capsule.v1" or claude_boolean_pilot.get("campaign_admission") is not False:
        raise ValueError("Claude Boolean pilot nonadmission receipt drifted")
    process_v2_provenance = read(ROOT / "evidence/law16-boolean-negation-process-v2/provenance.json")
    effort = read(ROOT / "evidence/law16-effort-summary-v1.json")
    annotations = read(ROOT / "evidence/law16-annotation-summary-v1.json")
    boolean_annotations = BOOLEAN_ANNOTATIONS.summarize()
    if boolean_annotations != read(ROOT / "evidence/law16-boolean-negation-annotation-summary-v1.json"):
        raise ValueError("Boolean annotation receipt drifted")
    nonproof_identity = read(
        ROOT / "evidence/law16-boolean-negation-nonproof-process-v1/identity.json"
    )
    proof_identity = read(
        ROOT / "evidence/law16-boolean-negation-proof-verdict-v1/identity.json"
    )
    variation = timing_variation({
        "historical_process_v1": {
            f"{lane}_{kind}": ROOT / f"evidence/law16-boolean-negation-process-v1/{lane}/{kind}/receipt.json"
            for lane in ("bend", "semaprax") for kind in ("candidate", "attack")
        },
        "process_v2": {
            f"{lane}_{kind}": ROOT / f"evidence/law16-boolean-negation-process-v2/{lane}/{kind}/receipt.json"
            for lane in ("bend", "semaprax") for kind in ("candidate", "attack")
        },
        "ordinary_check_v1": {
            "bend_ordinary": ROOT / "evidence/law16-boolean-negation-nonproof-process-v1/bend-ordinary.json",
            "semaprax_check": ROOT / "evidence/law16-boolean-negation-nonproof-process-v1/semaprax-check.json",
        },
        "proof_verdict_v1": {
            "bend_verdict": ROOT / "evidence/law16-boolean-negation-proof-verdict-v1/successful/bend-verdict.json",
            "semaprax_z3": ROOT / "evidence/law16-boolean-negation-proof-verdict-v1/successful/semaprax-z3.json",
        },
    })

    return {
        "schema": SCHEMA,
        "status": "incomplete",
        "scope": "local pinned Boolean-negation evidence and separate guarded-U32 theorem/control routes",
        "matched_boolean": {
            "semantic_contract": process["semantic_contract"],
            "process_routes": process_v2["process_states"],
            "historical_process_routes": process["process_states"],
            "process_provenance": {
                "source": "evidence/law16-boolean-negation-process-v2/provenance.json",
                "command_count": process_v2_provenance["command_count"],
                "process_states": process_v2_provenance["process_states"],
                "cold_cache": process_v2_provenance["cold_cache"],
            },
            "ordinary_and_nonproof_process_routes": nonproof["routes"],
            "bounded_proof_and_verdict_process_routes": proof_verdict["routes"],
            "proof_path_nonresult": proof_verdict["path_admission_nonresult"],
            "candidate_and_attack_controls": process["observations"],
            "agent_pairs": {
                "pilot_pairs": 1,
                "continuation_pairs": campaign["aggregate"]["pairs"],
                "total_pairs": 10,
                "bend_candidate_acceptances": 10,
                "bend_attack_rejections": 10,
                "semaprax_candidate_discharges": 10,
                "semaprax_attack_rejections": 10,
                "pilot_tokens": {
                    key: {
                        "chargeable_tokens": value["chargeable_tokens"],
                        "cached_input_tokens": value["cached_input_tokens"],
                    }
                    for key, value in pilot["lanes"].items()
                },
                "cost_usage": {
                    "status": "unavailable",
                    "reason": "all retained Codex JSON events omit monetary charge",
                },
            },
            "cost_provenance": {
                "source": "evidence/law16-boolean-negation-agent-cost-provenance-v1.json",
                "status": cost_provenance["status"],
                "matched_pairs": cost_provenance["scope"]["matched_pairs"],
                "trials": cost_provenance["scope"]["trials"],
                "aggregate_token_usage": cost_provenance["aggregate_token_usage"],
                "cost_usage": cost_provenance["cost_usage"],
                "nonclaims": cost_provenance["nonclaims"],
            },
            "claude_cost_probe": {
                "source": "evidence/law16-claude-cost-pilot-v1.json",
                "status": claude_pilot["status"],
                "provider": claude_pilot["provider"],
                "provider_cost_usd": claude_pilot["provider_cost_usd"],
                "provider_usage": claude_pilot["provider_usage"],
                "result": claude_pilot["result"],
                "campaign_admission": claude_pilot["campaign_admission"],
                "raw_provider_stream_retained": False,
                "nonclaims": claude_pilot["nonclaims"],
            },
            "peak_rss": rss["routes"],
            "timing_variation": variation,
            "historical_peak_rss": historical_rss,
            "timing_interpretation": (
                "ordinary Bend, Bend verdict, SEMAPRAX check, and installed-Z3 p50/p95 "
                "and RSS are separate local process-provisioning observations; no cross-route "
                "ratio or winner"
            ),
        },
        "supplemental_full_u32_encoding_controls": {
            "status": full_u32_controls["status"],
            "source": "evidence/full-u32-encoding-controls-v1/report.json",
            "original_manifest_unchanged": full_u32_controls["original_manifest_unchanged"],
            "representation_profile": full_u32_controls["representation_profile"],
            "numeric_domain": full_u32_controls["numeric_domain"],
            "candidate_and_attack_routes": len(full_u32_controls["cases"]),
            "domain_boundary_controls": len(full_u32_controls["domain_controls"]),
            "build_commit_association": full_u32_controls["build_commit_association"],
            "nonclaims": full_u32_controls["nonclaims"],
            "equal_spec_profile": {
                "source": "full_u32_equal_spec.py",
                "schema": full_u32_equal_spec["schema"],
                "profile": full_u32_equal_spec["profile"],
                "universal_model_checks": full_u32_equal_spec["universal_model_checks"],
                "sort_result_interpretation": {
                    "source": "fixtures/full-u32-encoding-v1/sort-equal-spec.smt2",
                    "test": "test_full_u32_equal_spec.py::test_universal_sort_model_checks_sortedness_and_multiplicity",
                    "scope": "all U32 values for each element of a list of exactly four elements and every queried U32 value",
                    "answers": [
                        "unsat: no length-four output violates sortedness or exact multiplicity",
                        "sat: a nonempty four-value input with query equal to one element witnesses that zero output multiplicity cannot preserve input multiplicity",
                    ],
                    "claim_boundary": "bounded model-level equal-spec result; not a source-translation or lowering certificate and not an unbounded-list theorem",
                },
            },
        },
        "supplemental_guarded_i64_balance_source_proof": {
            "status": balance_source_proof["status"],
            "source": "evidence/law16-guarded-i64-balance-smt-v1/result.json",
            "semantic_scope": balance_source_proof_result["semantic_scope"],
            "positive_smt_discharges": balance_source_proof["positive_smt_discharges"],
            "selected_postconditions": [
                {
                    "name": row["name"],
                    "declaration_id": row["declaration_id"],
                    "ensures_index": row["ensures_index"],
                    "status": row["status"],
                }
                for row in balance_source_proof_result["positive_cases"]
            ],
            "no_op_negative": {
                "status": balance_source_proof["no_op_negative"],
                "diagnostic": balance_source_proof_result["negative_control"]["diagnostic"],
                "solver_outcome_classification": "unclaimed; refusal does not distinguish counterexample, unknown, or another refusal reason",
            },
            "tool_and_build_identity": {
                "source_commit": balance_source_proof_result["semaprax"]["source_commit"],
                "semaprax_sha256": balance_source_proof_result["semaprax"]["sha256"],
                "z3_version": balance_source_proof_result["z3"]["version"],
                "z3_sha256": balance_source_proof_result["z3"]["sha256"],
                "platform": balance_source_proof_result["build"]["platform"],
                "target_profile": balance_source_proof_result["build"]["target_profile"],
            },
            "full_u32_original": balance_source_proof["full_u32_original"],
            "overall_law16": balance_source_proof["overall_law16"],
            "nonclaims": balance_source_proof_result["nonclaims"],
        },
        "supplemental_guarded_i64_profile_v2": {
            "source": "fixtures/full-u32-guarded-i64-profile-v2.json",
            "status": guarded_i64_profile["status"],
            "numeric_domain": guarded_i64_profile["numeric_domain"],
            "routes": guarded_i64_profile["routes"],
            "nonclaims": guarded_i64_profile["nonclaims"],
        },
        "supplemental_universal_list_theorems": {
            "bend": {
                "source": "evidence/bend-u32-sort-universal-v1/capsule.json",
                "status": bend_u32_sort["status"],
                "coverage": bend_u32_sort["coverage"],
                "negative_control": "empty universal count law refused; a separate concrete count mismatch was kernel-checked",
            },
            "semaprax": {
                "source": "evidence/law16-i64-list-proof-v1/capsule.json",
                "status": law15_list["status"],
                "coverage": law15_list["coverage"],
                "original_law16_cell": law15_list["original_law16_cell"],
            },
            "comparison_scope": "aligned universal sortedness and exact multiplicity over the U32 subset; distinct source algorithms and proof TCBs; no matched timing or original law16.* source certificate",
        },
        "measurement_provenance": {
            "identity_scope": "machine-local observations bound to the process-v2 raw samples; not current-head claims",
            "hardware": {
                "status": "observed",
                "source": "evidence/law16-boolean-negation-process-v2/provenance.json",
                "observation": process_v2["measurement_provenance"]["host"]["hardware"],
            },
            "operating_system": {
                "status": "observed",
                "source": "evidence/law16-boolean-negation-process-v2/provenance.json",
                "observation": {
                    "system": process_v2["measurement_provenance"]["host"]["system"],
                    "machine": process_v2["measurement_provenance"]["host"]["machine"],
                    "release": process_v2["measurement_provenance"]["host"]["release"],
                    "detail": process_v2["measurement_provenance"]["host"]["operating_system"],
                },
            },
            "backend": {
                "status": "observed",
                "source": "evidence/law16-boolean-negation-process-v2/provenance.json",
                "observation": process_v2_provenance["backend_and_flags"],
            },
            "flags": {
                "status": "observed",
                "source": "evidence/law16-boolean-negation-process-v2/provenance.json",
                "observation": process_v2_provenance["backend_and_flags"],
                "compiler_optimization": {
                    "status": "unavailable",
                    "reason": "the observed process cell invokes retained executables and does not compile them",
                },
            },
            "tool_identities": {
                "status": "observed",
                "source": "evidence/law16-boolean-negation-process-v2/provenance.json",
                "observation": process_v2_provenance["toolchain"],
            },
            "process_state_and_cache_boundary": {
                "states": process_v2_provenance["process_states"],
                "cold_cache": process_v2_provenance["cold_cache"],
            },
        },
        "proof_effort": {
            "boolean_agent_synthesis": boolean_synthesis_effort(pilot, campaign),
            "historical_bounded_balance_v2": {
                "status": "retained_source_evidence_only",
                "source": "evidence/law16-effort-summary-v1.json",
                "scope": "historical bounded-balance campaign; not evidence for the required checked-u32 cell",
                "rows": effort["rows"],
                "phase_separation": effort["phase_separation"],
                "nonclaims": effort["nonclaims"],
            },
        },
        "annotations_and_changed_bytes": {
            "matched_boolean": {
                "status": "retained_source_evidence_only",
                "source": "evidence/law16-boolean-negation-annotation-summary-v1.json",
                "scope": "ten matched Boolean final sources versus their fixed seed; byte distance is not semantic effort",
                "matched_pairs": boolean_annotations["matched_pairs"],
                "rows": boolean_annotations["rows"],
                "nonclaims": boolean_annotations["nonclaims"],
            },
            "historical_bounded_balance_v2": {
                "status": "retained_source_evidence_only",
                "source": "evidence/law16-annotation-summary-v1.json",
                "scope": "historical bounded-balance campaign; not evidence for the required checked-u32 cell",
                "rows": annotations["rows"],
                "nonclaims": annotations["nonclaims"],
            },
        },
        "cache_isolation_probe": {
            "source": "evidence/law16-cache-isolation-probe-v1/receipt.json",
            "status": cache_probe["cold_state"]["status"],
            "reason": cache_probe["cold_state"]["reason"],
            "checking_measurements": cache_probe["checking_measurements"],
            "container_state": cache_probe["container_state"],
            "nonclaims": cache_probe["nonclaims"],
        },
        "guest_file_cache_cold_warm": {
            "source": "evidence/law16-guest-cache-thirty-v1/receipt.json",
            **guest_cache,
            "scope": GUEST_CACHE.GUEST_SCOPE,
        },
        "supplemental_project_incremental_cell": {
            "source": "evidence/law16-project-incremental-cell-v1/result.json",
            **project_incremental,
            "scope": "local three-module SEMAPRAX compiler cache control; no matched Bend route or project-sized timing comparison",
        },
        "supplemental_boolean_native_phases": {
            "source": "evidence/law16-native-phase-thirty-v1/receipt.json",
            **native_phases,
            "scope": "30 local samples each for separate Bend check/C emission/Clang compile/run and SEMAPRAX check/combined native build/run phases; no proof, cache isolation, or cross-route comparison",
        },
        "supplemental_boolean_refactor_cell": {
            "source": "evidence/law16-boolean-refactor-cell-v1/result.json",
            **boolean_refactor,
            "scope": "matched scalar Boolean refactor and law-breaking edit; original checked-u32 cells remain unadmitted",
        },
        "claude_boolean_pilot": {
            "source": "evidence/law16-claude-boolean-pilot-v1/capsule.json",
            "status": claude_boolean_pilot["status"],
            "attempts": claude_boolean_pilot["attempts"],
            "campaign_admission": claude_boolean_pilot["campaign_admission"],
            "nonclaims": claude_boolean_pilot["nonclaims"],
        },
        "pins_and_trust": {
            "observation_identity": "local historical pins, retained as exact executable/tool evidence",
            "bend": "local historical commit 947db722640c86247849343657bf2f7ef01cb7f1; verdict output is retained tool evidence",
            "semaprax": "local historical executable commit 9a9db7a8117ac8d292b24ffd5671ec3333272290; installed-Z3 source proof discharges selected app.negate ensures[0]",
            "boundaries": [
                "Bend verdict and SEMAPRAX installed-Z3 have distinct trusted computing bases",
                "SEMAPRAX source proof does not prove lowering or execution",
                "fresh/repeat paths do not isolate OS, executable, solver, or tool caches",
            ],
        },
        "unavailable_or_unsupported": {
            "checked_u32": "unsupported_by_pinned_parser: SPX-P003 admits i32, u8, usize literal suffixes, not u32",
            "cold_cache": "partial: 30 guest file-page-cache cold/warm pairs per ordinary Bend and SEMAPRAX check route; host/Rosetta cache state and proof/verdict cold/warm routes remain unmeasured",
            "Lean": "supplemental LAW15 collection source theorem physically checked by Lean; no Boolean or original law16.* Lean export",
            "cost": "unavailable for admitted agent trials: Codex JSON has no monetary charge event; a separate Claude probe recorded cost but failed before any source outcome",
            "project_sized": "partial: three-module SEMAPRAX calculator core edit and signature negative control exercised; no matched Bend route, large project, or incremental timing comparison",
            "list_refactor_lawbreaking": remaining,
        },
        "closure": (
            "no: the matched Boolean cell and supplemental U32 semantic theorem comparison do not satisfy "
            "the original checked-u32 source admission, full cold-cache and proof-route coverage, matched project-sized incremental/refactor, "
            "or monetary cost-event acceptance requirements"
        ),
        "nonclaims": [
            "no cross-route timing ratio, winner, or superiority claim",
            "no full LAW-16 closure",
            "no checked-u32 substitute from i32/i64/u8/usize",
        ],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.output.exists() or not args.output.parent.is_dir():
        parser.error("output must be new")
    args.output.write_text(json.dumps(render(), indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
