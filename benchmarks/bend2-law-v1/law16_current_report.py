#!/usr/bin/env python3
"""Render the current LAW-16 evidence state from authenticated local capsules."""
import argparse
import importlib.util
import json
import pathlib

ROOT = pathlib.Path(__file__).parent


def module(name):
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


def render():
    remaining = REMAINING.review()
    rss = RSS.review(ROOT / "evidence/law16-boolean-negation-peak-rss-v1")
    nonproof = NONPROOF.review(ROOT / "evidence/law16-boolean-negation-nonproof-process-v1")
    proof_verdict = PROOFVERDICT.review(ROOT / "evidence/law16-boolean-negation-proof-verdict-v1")
    pilot = PILOT.review(ROOT / "evidence/law16-boolean-negation-agent-pilot-v1")
    campaign = CAMPAIGN.review(ROOT / "evidence/law16-boolean-negation-agent-campaign-v1")
    process = PROCESS.review(ROOT / "evidence/law16-boolean-negation-process-v1")
    effort = read(ROOT / "evidence/law16-effort-summary-v1.json")
    annotations = read(ROOT / "evidence/law16-annotation-summary-v1.json")
    nonproof_identity = read(
        ROOT / "evidence/law16-boolean-negation-nonproof-process-v1/identity.json"
    )
    proof_identity = read(
        ROOT / "evidence/law16-boolean-negation-proof-verdict-v1/identity.json"
    )

    return {
        "schema": SCHEMA,
        "status": "incomplete",
        "scope": "local pinned historical Boolean-negation evidence only",
        "matched_boolean": {
            "semantic_contract": process["semantic_contract"],
            "process_routes": process["process_states"],
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
            "peak_rss": rss["routes"],
            "timing_interpretation": (
                "ordinary Bend, Bend verdict, SEMAPRAX check, and installed-Z3 p50/p95 "
                "and RSS are separate local process-provisioning observations; no cross-route "
                "ratio or winner"
            ),
        },
        "measurement_provenance": {
            "identity_scope": "pinned historical local executable observations; not current-head claims",
            "hardware": {
                "status": "unavailable",
                "reason": "no retained machine-model or CPU-topology record binds these samples",
            },
            "operating_system": {
                "status": "unavailable",
                "reason": "no retained OS-version record binds these samples",
            },
            "backend": {
                "status": "observed",
                "bend": "ordinary Bend and separate Bend --verdict routes",
                "semaprax": "separate SEMAPRAX check and installed-Z3 source-proof routes",
                "z3_sha256": proof_identity["z3_sha256"],
            },
            "flags": {
                "status": "partially_observed",
                "bend_nonproof_environment": {"BEND_NO_TELEMETRY": "1"},
                "compiler_optimization": {
                    "status": "unavailable",
                    "reason": "no retained compiler optimization-flag record",
                },
            },
            "tool_identities": {
                "bend_commit": nonproof_identity["tools"]["bend_commit"],
                "bend_main_sha256": nonproof_identity["tools"]["bend_main"]["sha256"],
                "bun_sha256": nonproof_identity["tools"]["bun"]["sha256"],
                "semaprax_sha256": proof_identity["semaprax_sha256"],
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
                "status": "unavailable",
                "reason": "no retained normalized annotation and changed-proof-byte summary binds Boolean candidates to a fixed seed",
            },
            "historical_bounded_balance_v2": {
                "status": "retained_source_evidence_only",
                "source": "evidence/law16-annotation-summary-v1.json",
                "scope": "historical bounded-balance campaign; not evidence for the required checked-u32 cell",
                "rows": annotations["rows"],
                "nonclaims": annotations["nonclaims"],
            },
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
            "cold_cache": "unavailable: no retained reproducible clean cache isolation",
            "Lean": "unavailable: no retained admitted Lean export/kernel route",
            "cost": "unavailable: no Codex JSON monetary charge event",
            "project_sized": "unavailable: Boolean microcell is not project-sized/incremental evidence",
            "list_refactor_lawbreaking": remaining,
        },
        "closure": (
            "no: the matched Boolean cell and ten agent pairs do not satisfy checked-u32, Lean, "
            "cold-cache, project-sized, list/refactor/incremental, or cost acceptance requirements"
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
