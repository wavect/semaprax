#!/usr/bin/env python3
"""Audit LAW-16 issue #392 acceptance criteria against retained evidence."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).parent
PROJECT_ROOT = ROOT.parent.parent
SCHEMA = "semaprax.bend2-law-benchmark.law16-closure-audit.v1"
ISSUE = {
    "url": "https://github.com/wavect/semaprax/issues/392",
    "number": 392,
    "title": "[P2] Benchmark law-driven development against pinned Bend 2 with equal semantics and trust",
    "state_at_capture": "open",
    "updated_at": "2026-10-04T13:17:24Z",
    "api_body_bytes": 6383,
    "api_body_sha256": "d296c0e16e6ee8edab12c934e41f22ef7bfa177d72ccf6e1ed7ab4bf059adcdd",
    "embedded_content_sha256": "b5787613eb5c292353384e8fc7475e9c6ade4ad5f2476e75d5b1116b8985e222",
}

ACCEPTANCE_TEXT = (
    "A single documented command or harness sequence reproduces each available benchmark cell with raw artifacts and pinned identities.",
    "Equal-spec checks reject the empty-sort/no-op-transfer loopholes on both sides.",
    "Normal Bend checking and verdict-kernel results are never conflated.",
    "Missing tools, unsupported targets, timed-out proofs, and differing numeric domains cannot count as a win.",
    "Cold/warm results and proof synthesis/check/compile/run times are separate.",
    "Agent outcomes include fixed budgets, repeated trials, rejected-law-gaming cases, and token/cost provenance.",
    "The final report may honestly show Semaprax losing a dimension; passing this ticket does not require a predetermined winner.",
)

REQUIRED_IMPLEMENTATION_TEXT = (
    "Pin both source commits, toolchains, dependencies, hardware, OS, backend, optimization flags, target numeric semantics, and inputs. Record Bend's ordinary checker and `--verdict` separately; also separate Semaprax SMT, external Lean, and runtime/test-only paths.",
    "Define matched tasks: scalar contract bug, structured balance transfer, supported list theorem, law-preserving refactor, law-breaking agent edit, and a project-sized incremental edit. State unsupported cells rather than substituting a weaker task.",
    "Equalize law strength: sorting includes permutation/multiplicity and sortedness; balances include intended state change, not only conservation; compare the same domain/overflow semantics. Bend Nat/U32 and Semaprax checked integers are not interchangeable by default.",
    "Measure cold/warm checking wall time, p50/p95, peak memory, proof-writing effort, explicit annotations, generated/changed proof bytes, law inventory coverage, rejection of seeded spec-gaming attacks, and cache invalidation work. Use at least 30 timed repetitions per microbenchmark configuration, separate process/tool provisioning, retain raw samples, and treat tiny/noisy differences as inconclusive. Set `BEND_NO_TELEMETRY=1` for offline/timing trials and record it, because the reviewed CLI performs an opt-out daily version check.",
    "For agent experiments, pin model/configuration/tool access/budget, preregister at least 10 matched independent trials per admitted task/language/model (label a smaller run as a pilot), record success criteria and failures, and export token/cost events through existing telemetry. Separate proof synthesis cost from kernel checking and compilation/runtime cost.",
    "Keep runtime throughput and GPU scaling as a separate benchmark family. Do not compare CPU Semaprax to GPU Bend without explicit comparable work/hardware; do not claim law support creates GPU parity.",
    "Store machine-readable raw results plus a transparent report, including confidence/variation, nonresults, actual execution evidence, and the trusted computing base. Publish superiority only for measured dimensions; retain adverse results.",
)


def module(name: str, path: pathlib.Path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def evidence(path: str) -> dict:
    selected = ROOT / path
    if not selected.is_file() or selected.is_symlink():
        raise ValueError(f"required audit evidence is unavailable: {path}")
    return {"path": path, "bytes": selected.stat().st_size, "sha256": "sha256:" + digest(selected)}


def verify_i64_list_capsule() -> dict:
    """Check retained capsule hashes and output markers without rerunning Lean."""
    route = module("law16_i64_list_proof_capsule_for_audit", ROOT / "law16_i64_list_proof_capsule.py")
    capsule_path = ROOT / "evidence/law16-i64-list-proof-v1"
    result = json.loads((capsule_path / "capsule.json").read_text())
    if result.get("schema") != route.SCHEMA or result.get("status") != "supplemental_i64_list_profile_proved":
        raise ValueError("supplemental i64 list capsule schema or status drifted")
    for key in ("source", "proof_module", "harness"):
        ref = result[key]
        path = PROJECT_ROOT / ref["path"]
        if not path.is_file() or path.stat().st_size != ref["bytes"] or "sha256:" + digest(path) != ref["sha256"]:
            raise ValueError(f"supplemental i64 list capsule input drifted: {key}")
    for attack in result["attacks"].values():
        path = PROJECT_ROOT / attack["path"]
        if not path.is_file() or path.stat().st_size != attack["bytes"] or "sha256:" + digest(path) != attack["sha256"]:
            raise ValueError("supplemental i64 list attack input drifted")
    raw = {}
    for stream, ref in result["raw"].items():
        path = capsule_path / "raw" / ref["path"]
        if not path.is_file() or path.is_symlink() or path.stat().st_size != ref["bytes"] or "sha256:" + digest(path) != ref["sha256"]:
            raise ValueError(f"supplemental i64 list raw {stream} stream drifted")
        raw[stream] = path.read_bytes()
    output = (raw["stdout"] + raw["stderr"]).decode("utf-8", "replace")
    if not all(marker in output for marker in (
        f"{route.TEST} ... ok",
        "empty-output: count differs for value 2 on input [1, 2]",
        "duplicate-element: count differs for value 2 on input [1, 2]",
    )):
        raise ValueError("supplemental i64 list kernel-test markers drifted")
    binary_path = pathlib.Path(result["test_binary"]["path"])
    if not binary_path.is_absolute():
        binary_path = PROJECT_ROOT / binary_path
    if result.get("command") != route.command(binary_path):
        raise ValueError("supplemental i64 list test command drifted")
    if result.get("kernel", {}).get("pin") != route.PIN or result.get("kernel", {}).get("version") != route.VERSION:
        raise ValueError("supplemental i64 list Lean pin drifted")
    if result.get("coverage", {}).get("laws") != ["sort_sorted", "sort_permutation", "sort_multiplicity"]:
        raise ValueError("supplemental i64 list law coverage drifted")
    if result.get("coverage", {}).get("declarations") != ["law15.collection.insert", "law15.collection.sort"]:
        raise ValueError("supplemental i64 list proof declaration scope drifted")
    return {
        "status": "retained_test_output_and_source_hashes_verified",
        "capsule": "evidence/law16-i64-list-proof-v1/capsule.json",
        "profile": result["profile"],
        "coverage": result["coverage"],
        "attacks": ["empty_sort", "duplicate_multiplicity"],
        "lean_pin": result["kernel"]["pin"],
        "lean_version": result["kernel"]["version"],
        "test_binary_sha256": result["test_binary"]["sha256"],
        "test_binary_association": "local path/hash recorded; not a build attestation",
        "exit_code": None,
        "exit_code_retained": False,
        "capture_status": result["status"],
        "raw_success_markers": "test success and both rejection witness strings are present",
        "original_law16_cell": result["original_law16_cell"],
        "nonclaims": result["nonclaims"],
        "evidence": [
            evidence("evidence/law16-i64-list-proof-v1/capsule.json"),
            evidence("evidence/law16-i64-list-proof-v1/raw/kernel-test.stdout"),
            evidence("evidence/law16-i64-list-proof-v1/raw/kernel-test.stderr"),
        ],
    }


def report_data() -> dict:
    report = module("law16_current_report_for_closure_audit", ROOT / "law16_current_report.py")
    return report.render()


def verify_guarded_i64_v2() -> dict:
    """Summarize retained full-U32/guarded-i64 controls with their scoped limits."""
    profile_path = ROOT / "fixtures/full-u32-guarded-i64-profile-v2.json"
    profile = json.loads(profile_path.read_text())
    report_path = ROOT / profile["reproduction"]["retained_evidence"]
    control = json.loads(report_path.read_text())
    verifier = module("law16_guarded_i64_v2_for_closure_audit", ROOT / "full_u32_guarded_i64_profile_v2.py")
    errors = verifier.verify(profile_path)
    if errors:
        raise ValueError("guarded-i64 v2 profile verification failed: " + "; ".join(errors))
    cases = control["cases"]
    outcomes = {}
    for task in ("balance", "sort"):
        rows = [row for row in cases if row["task"] == task]
        outcomes[task] = {
            "cases": len(rows),
            "expected_outcomes_observed": all(row["expected_outcome_observed"] for row in rows),
            "candidate_attack_pairs_per_route": 1,
            "routes": ["bend_normal", "bend_verdict", "semaprax_native"],
        }
    return {
        "status": profile["status"],
        "numeric_domain": profile["numeric_domain"],
        "profile_sha256": "sha256:" + digest(profile_path),
        "control_report_sha256": "sha256:" + digest(report_path),
        "verification": "profile verifier passed: source pins, 12 paired controls, 4 domain refusals, and LAW15 capsule identity",
        "paired_controls": outcomes,
        "representation_model": {"claim": control["representation_bridge"]["claim"], "exit_code": control["representation_bridge"]["exit_code"], "source": control["representation_bridge"]["source"]},
        "bounded_sort_model": {"claim": control["sort_equal_spec_model"]["claim"], "exit_code": control["sort_equal_spec_model"]["exit_code"], "source": control["sort_equal_spec_model"]["source"]},
        "law15_lean_route": {"disposition": profile["routes"][2]["disposition"], "declarations": profile["routes"][2]["declarations"], "identity_boundary": profile["routes"][2]["identity_boundary"]},
        "supplemental_bend_lean_comparison": {"disposition": profile["routes"][3]["disposition"], "alignment": profile["routes"][3]["domain_and_law_alignment"], "algorithm_boundary": profile["routes"][3]["algorithm_boundary"], "scope_limit": profile["routes"][3]["scope_limit"]},
        "original_manifest_status": profile["base_profile"]["native_u32_source_status"],
        "build_association": profile["reproduction"]["build_association"],
        "claim_limit": profile["reproduction"]["claim_limit"],
        "evidence": [evidence("fixtures/full-u32-guarded-i64-profile-v2.json"), evidence("evidence/law16-guarded-i64-profile-controls-v2/report.json")],
    }


def render() -> dict:
    report = report_data()
    list_proof = verify_i64_list_capsule()
    profile_v2 = verify_guarded_i64_v2()
    current_report = "evidence/law16-current-report-v1.json"
    common = [evidence(current_report)]
    criteria = [
        {
            "id": "AC1",
            "text": ACCEPTANCE_TEXT[0],
            "status": "partial",
            "assessment": "The benchmark README documents one pinned replay harness for retained verification and fresh capture routes. The retained replay authenticates the available capsules, including the recovered bounded-balance raw outputs. Fresh capture has not been exercised end to end, so this is not a complete reproduction of the benchmark cells.",
            "evidence": common + [evidence("README.md"), evidence("law16_replay.py"), evidence("law16_current_report.py")],
        },
        {
            "id": "AC2",
            "text": ACCEPTANCE_TEXT[1],
            "status": "partial",
            "assessment": "Supplemental guarded-i64/U32 v2 retains one candidate/attack pair for balance and sort across Bend ordinary, Bend verdict, and SEMAPRAX native routes, plus four SEMAPRAX domain refusals. A separate Bend source theorem proves sortedness and multiplicity for all finite U32 lists, while Lean proves the aligned laws for LAW15 collection.sort over all finite List<i64>. These authenticate distinct source algorithms and trusted computing bases. No LAW16 source theorem or source translation/lowering certificate is established; original checked-u32 syntax remains unsupported.",
            "evidence": common + [
                evidence("evidence/full-u32-encoding-controls-v1/report.json"),
                evidence("full_u32_equal_spec.py"),
                evidence("fixtures/full-u32-encoding-v1/sort-equal-spec.smt2"),
                evidence("evidence/law16-guarded-i64-balance-smt-v1/result.json"),
                evidence("law16_guarded_i64_balance_smt.py"),
                evidence("evidence/law16-i64-list-proof-v1/capsule.json"),
                evidence("fixtures/full-u32-guarded-i64-profile-v2.json"),
                evidence("evidence/law16-guarded-i64-profile-controls-v2/report.json"),
            ],
        },
        {
            "id": "AC3",
            "text": ACCEPTANCE_TEXT[2],
            "status": "met",
            "assessment": "Ordinary Bend, Bend verdict, SEMAPRAX check, and installed-Z3 routes have separate evidence fields and route-specific controls; the report makes no cross-route timing comparison.",
            "evidence": common + [
                evidence("evidence/law16-boolean-negation-nonproof-process-v1/bend-ordinary.json"),
                evidence("evidence/law16-boolean-negation-proof-verdict-v1/successful/bend-verdict.json"),
                evidence("test_full_u32_encoding_controls.py"),
            ],
        },
        {
            "id": "AC4",
            "text": ACCEPTANCE_TEXT[3],
            "status": "met",
            "assessment": "Unavailable/non-admitted routes remain explicit nonresults; infrastructure failure cannot count as attack rejection in the supplemental control tests; differing numeric profiles and source-proof limits are recorded. No superiority result is published.",
            "evidence": common + [
                evidence("test_full_u32_encoding_controls.py"),
                evidence("evidence/law16-checked-u32-nonadmission-v1/review.json"),
                evidence("evidence/full-u32-encoding-controls-v1/report.json"),
            ],
        },
        {
            "id": "AC5",
            "text": ACCEPTANCE_TEXT[4],
            "status": "partial",
            "assessment": "Process timing, proof/check routes, agent synthesis tokens, and runtime controls are separated. The retained fresh/repeat process observations do not establish OS-cache cold/warm state; native compile and run remain combined and unmeasured as separate timings.",
            "evidence": common + [
                evidence("evidence/law16-boolean-negation-process-v2/manifest.json"),
                evidence("evidence/law16-boolean-negation-process-v2/provenance.json"),
                evidence("evidence/law16-effort-summary-v1.json"),
                evidence("evidence/full-u32-encoding-controls-v1/report.json"),
            ],
        },
        {
            "id": "AC6",
            "text": ACCEPTANCE_TEXT[5],
            "status": "partial",
            "assessment": "Ten fixed-budget matched Boolean agent pairs include candidate/attack outcomes and retained token counters. No monetary charge event is present, and the other planned task cells have no corresponding ten-trial agent campaigns because they are unsupported or unobserved.",
            "evidence": common + [
                evidence("evidence/law16-boolean-negation-agent-pilot-v1/review.json"),
                evidence("evidence/law16-boolean-negation-agent-campaign-v1/review.json"),
                evidence("evidence/law16-effort-summary-v1.json"),
            ],
        },
        {
            "id": "AC7",
            "text": ACCEPTANCE_TEXT[6],
            "status": "met",
            "assessment": "The current report keeps LAW-16 incomplete, makes no winner or superiority claim, retains adverse/no-op and unsupported results, and reports per-cell timing variation without cross-route ranking.",
            "evidence": common + [evidence("LAW16-REPORT.md"), evidence("evidence/law16-i64-list-proof-v1/capsule.json")],
        },
    ]

    requirement_assessments = (
        (
            "partial",
            "Local process and proof routes bind source/tool pins, host facts, backend/flags, numeric profile, and inputs. A pinned Lean kernel test capture is retained for the separate LAW15 collection List<i64> profile; no corresponding proof is retained for a LAW16 declaration, and the test-binary association is not a build attestation.",
        ),
        (
            "partial",
            "Boolean, balance, and sort controls are retained, including a supplemental LAW15 List<i64> Lean proof test. The v2 balance/sort source controls are one candidate/attack pair per route, not the required theorem task campaigns. The planned LAW16 list identities, refactor, law-breaking agent edit, and project-sized incremental cells remain unsupported or unavailable.",
        ),
        (
            "partial",
            "Supplemental controls pair full-U32 Bend values with guarded-i64 SEMAPRAX source, with explicit state-preserving failure cases and all-U32 representation bitvector checks. Separate Bend and Lean source theorems establish aligned universal sort laws over the U32 subdomain for distinct algorithms. They do not prove source translation/lowering or the original LAW16 identities. The checked-u32 source profile remains unadmitted.",
        ),
        (
            "partial",
            "Available process cells have 30 samples, p50/p95, peak RSS, and descriptive MAD; proof synthesis tokens, Boolean annotation/changed-byte counts, and route checks are separately reported. Cold-cache isolation, monetary cost, and separate native compile/run timings are unavailable.",
        ),
        (
            "partial",
            "Ten fixed-budget matched agent pairs cover the Boolean task and retain candidate/attack outcomes and token counters. No monetary charge event exists, and unsupported planned tasks have no matched agent trials.",
        ),
        (
            "met",
            "The law-checking report does not mix runtime throughput or GPU scaling into its comparison, and it makes no CPU-versus-GPU parity claim.",
        ),
        (
            "met",
            "Machine-readable evidence and a human report retain route identities, raw-evidence reviews, variation, nonresults, adverse controls, and trust boundaries. The report publishes no superiority claim.",
        ),
    )
    required = [
        {"id": f"R{index}", "text": text, "status": status, "assessment": assessment}
        for index, (text, (status, assessment)) in enumerate(
            zip(REQUIRED_IMPLEMENTATION_TEXT, requirement_assessments, strict=True), start=1
        )
    ]

    unsupported = report["unavailable_or_unsupported"]
    cells = [
        {
            "id": "checked_u32_source_syntax",
            "classification": "unsupported",
            "blocking_requirements": ["AC2", "R2", "R3"],
            "status": unsupported["checked_u32"],
            "evidence": [evidence("evidence/law16-checked-u32-nonadmission-v1/review.json")],
        },
        {
            "id": "supported_list_theorem",
            "classification": "supplemental_profile_only",
            "blocking_requirements": ["AC2", "R2", "R3"],
            "status": "Retained Bend and Lean source theorems prove aligned universal sortedness and multiplicity laws over the U32 subdomain, with separate empty/duplicate attack controls. They authenticate distinct algorithms and source identities; neither proves the original law16.* declarations. The separate U32 SMT model remains limited to four-element lists.",
            "evidence": [evidence("evidence/bend-u32-sort-universal-v1/capsule.json"), evidence("evidence/law16-i64-list-proof-v1/capsule.json"), evidence("fixtures/full-u32-encoding-v1/sort-equal-spec.smt2"), evidence("evidence/law16-checked-u32-nonadmission-v1/review.json")],
        },
        {
            "id": "law16_list_source_theorem",
            "classification": "unavailable_for_law16_identity",
            "blocking_requirements": ["AC2", "R2", "R3"],
            "status": "the retained generic list proof explicitly names LAW15 identities and states that the original LAW16 cell is unchanged",
            "evidence": [evidence("evidence/law16-i64-list-proof-v1/capsule.json")],
        },
        {
            "id": "law_preserving_refactor",
            "classification": "unsupported",
            "blocking_requirements": ["R2"],
            "status": "no matched refactor-equivalence route; checked-u32 parser admission blocks the planned fixture",
            "evidence": [evidence("evidence/law16-checked-u32-nonadmission-v1/review.json")],
        },
        {
            "id": "law_breaking_agent_edit",
            "classification": "unsupported",
            "blocking_requirements": ["AC6", "R2", "R5"],
            "status": "no retained matched law-inventory preservation route; checked-u32 parser admission blocks the planned fixture",
            "evidence": [evidence("evidence/law16-checked-u32-nonadmission-v1/review.json")],
        },
        {
            "id": "original_structured_balance_source_proof",
            "classification": "unsupported_by_this_source_profile",
            "blocking_requirements": ["AC2", "R2", "R3"],
            "status": "guarded-i64 scalar source postconditions are retained; original structured checked-u32 source profile is not admitted",
            "evidence": [evidence("evidence/law16-guarded-i64-balance-smt-v1/result.json")],
        },
        {
            "id": "cold_cache_isolation",
            "classification": "unavailable",
            "blocking_requirements": ["AC5", "R4"],
            "status": unsupported["cold_cache"],
            "evidence": [evidence("evidence/law16-boolean-negation-process-v2/provenance.json")],
        },
        {
            "id": "external_lean_export_kernel",
            "classification": "supplemental_route_available_but_not_law16_cell",
            "blocking_requirements": ["R1", "R2"],
            "status": "a retained pinned Lean kernel route covers LAW15 collection.sort over List<i64>; this supplemental capsule does not prove a LAW16 declaration",
            "evidence": [evidence("evidence/law16-i64-list-proof-v1/capsule.json")],
        },
        {
            "id": "project_sized_incremental_cell",
            "classification": "unavailable",
            "blocking_requirements": ["R2", "R4"],
            "status": unsupported["project_sized"],
            "evidence": common,
        },
        {
            "id": "agent_monetary_cost_events",
            "classification": "unavailable",
            "blocking_requirements": ["AC6", "R5"],
            "status": unsupported["cost"],
            "evidence": common,
        },
    ]

    audited_commit = subprocess.check_output(
        ["git", "-C", str(ROOT.parent.parent), "rev-parse", "HEAD"], text=True
    ).strip()
    return {
        "schema": SCHEMA,
        "audit_status": "open_requirements_remain",
        "issue_state_at_capture": ISSUE["state_at_capture"],
        "closure": "not_satisfied",
        "audited_repository_commit": audited_commit,
        "issue": {
            **ISSUE,
            "acceptance_criteria": [
                {"id": f"AC{index}", "text": text, "checked_at_capture": False}
                for index, text in enumerate(ACCEPTANCE_TEXT, start=1)
            ],
            "required_implementation": [
                {"id": f"R{index}", "text": text}
                for index, text in enumerate(REQUIRED_IMPLEMENTATION_TEXT, start=1)
            ],
        },
        "acceptance_assessment": criteria,
        "required_implementation_assessment": required,
        "supplemental_i64_list_proof": list_proof,
        "supplemental_guarded_i64_profile_v2": profile_v2,
        "current_report_reconciliation": {
            "field": "unavailable_or_unsupported.Lean",
            "existing_report_value": unsupported["Lean"],
            "audit_update": "The current report includes the LAW15 collection List<i64> Lean proof test. It is supplemental and does not close or prove the original LAW16 list cell.",
        },
        "unmet_requirements": [
            {
                "id": "AC1",
                "reason": "A single replay sequence authenticates retained capsules, but fresh capture has not been exercised end to end.",
                "kind": "reproducibility_harness_gap",
            },
            {
                "id": "AC5",
                "reason": "No cache-isolated cold observation exists; fresh/repeat process states cannot substitute for it.",
                "kind": "missing_measurement",
            },
            {
                "id": "AC6",
                "reason": "No monetary cost event exists, and the ten-trial agent evidence covers only the Boolean task.",
                "kind": "missing_agent_telemetry_and_task_coverage",
            },
        ],
        "declared_unsupported_or_unavailable_cells": cells,
        "report_status": report["status"],
        "report_closure": report["closure"],
        "evidence_basis": [
            evidence("LAW16-REPORT.md"),
            evidence("README.md"),
            evidence("law16_current_report.py"),
            evidence(current_report),
        ],
        "nonclaims": [
            "this audit does not close issue #392",
            "unsupported or unavailable cells are not wins and are not silently replaced by supplemental controls",
            "the supplemental guarded-i64 balance source proof is not proof of lowering or app execution",
            "the no-op source-proof refusal is not classified as a counterexample",
            "the four-element U32 sort model check is not a source-level unbounded-list theorem",
            "full-domain representation bitvector checks do not establish source translation or lowering",
            "supplemental guarded-i64 v2 source controls do not change original checked-u32 manifest admission",
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if not args.output.parent.is_dir():
        parser.error("output parent must already exist")
    try:
        value = render()
    except (OSError, ValueError, json.JSONDecodeError, KeyError) as error:
        parser.error(str(error))
    args.output.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
