#!/usr/bin/env python3
"""Verify the pins and route boundaries of the LAW16 guarded-i64 v2 capsule."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PROFILE = ROOT / "fixtures/full-u32-guarded-i64-profile-v2.json"


def sha256(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def verify(profile_path: Path = PROFILE, root: Path = ROOT) -> list[str]:
    profile = json.loads(profile_path.read_text())
    errors: list[str] = []
    if profile.get("status") != "supplemental_controls_and_matched_sort_law_route_no_timing":
        errors.append("profile status drifted")
    evidence = profile["reproduction"]["retained_evidence"]
    report_path = root / evidence
    expected_report = profile["reproduction"]["report_sha256"]
    if sha256(report_path) != expected_report:
        errors.append("retained controls report hash mismatch")
    report = json.loads(report_path.read_text())
    if report.get("status") != "supplemental_controls_pass":
        errors.append("bounded controls did not pass")
    if len(report.get("cases", [])) != 12 or len(report.get("domain_controls", [])) != 4:
        errors.append("expected 12 paired source controls and 4 domain refusals")
    rows = report.get("cases", []) + report.get("domain_controls", [])
    if not all(row.get("expected_outcome_observed") is True for row in rows):
        errors.append("one or more bounded control outcomes did not match")
    for route in profile["routes"]:
        source_rows = [route["source"]] if "source" in route else route.get("sources", {}).values()
        for source in source_rows:
            source_path = ((root / source["path"]) if source["path"].startswith("fixtures/")
                           else root.parent.parent / source["path"])
            if sha256(source_path) != source["sha256"]:
                errors.append(f"source hash mismatch: {source['path']}")
    lean_capsule_path = root / "evidence/law16-i64-list-proof-v1/capsule.json"
    capsule = json.loads(lean_capsule_path.read_text())
    if sha256(lean_capsule_path) != profile["routes"][2]["capsule_sha256"]:
        errors.append("LAW15 Lean capsule hash mismatch")
    if capsule.get("status") != "supplemental_i64_list_profile_proved":
        errors.append("LAW15 Lean capsule does not report its proved status")
    if capsule.get("coverage", {}).get("laws") != ["sort_sorted", "sort_permutation", "sort_multiplicity"]:
        errors.append("LAW15 Lean theorem law inventory drifted")
    if capsule.get("coverage", {}).get("declarations") != ["law15.collection.insert", "law15.collection.sort"]:
        errors.append("LAW15 Lean declaration identities drifted")
    if profile["routes"][1]["disposition"] != "paired_concrete_controls_pass_theorem_match_unsupported":
        errors.append("LAW16 list route was promoted beyond its concrete/model evidence")
    if profile["routes"][2]["identity_boundary"] != (
        "does not authenticate fixtures/full-u32-encoding-v1/sort.spx or law16.insert/law16.sort; no cross-source aliasing"
    ):
        errors.append("LAW15/LAW16 source identity boundary changed")

    bend_route = profile["routes"][3]
    bend_capsule_path = root / bend_route["bend"]["proof_capsule"]
    if sha256(bend_capsule_path) != bend_route["bend"]["proof_capsule_sha256"]:
        errors.append("Bend universal proof capsule hash mismatch")
    try:
        proof_module_path = root / "law16_bend_u32_sort_proof.py"
        spec = importlib.util.spec_from_file_location("law16_bend_u32_sort_profile_v2", proof_module_path)
        proof_module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(proof_module)
        bend_capsule = proof_module.verify(bend_capsule_path)
    except (OSError, ValueError, AttributeError) as error:
        errors.append(f"Bend universal proof evidence failed verification: {error}")
        bend_capsule = {}
    coverage = bend_capsule.get("coverage", {})
    if bend_capsule.get("status") != "supplemental_bend_u32_sort_source_proved":
        errors.append("Bend source proof status drifted")
    if coverage.get("domain") != "every finite List<U32>, every queried U32":
        errors.append("Bend universal proof domain drifted")
    if coverage.get("sortedness") != "sorted_sort(xs): SortedB(Sort.sort(xs), 0)":
        errors.append("Bend universal sortedness law drifted")
    if coverage.get("multiplicity") != "count_sort(probe,xs): count(probe,Sort.sort(xs)) == count(probe,xs)":
        errors.append("Bend universal multiplicity law drifted")
    if bend_route["bend"]["source"]["sha256"] != bend_capsule.get("source", {}).get("sha256"):
        errors.append("Bend source identity differs from its proof capsule")
    if bend_route["bend"]["proof_source"]["sha256"] != bend_capsule.get("proof", {}).get("sha256"):
        errors.append("Bend proof source identity differs from its proof capsule")
    if bend_route["semaprax"]["source"]["sha256"] != capsule.get("source", {}).get("sha256"):
        errors.append("LAW15 source identity differs from its proof capsule")
    if bend_route["disposition"] != "matched_universal_semantic_laws_under_u32_embedding_no_timing":
        errors.append("cross-source route disposition changed")
    if bend_route["semaprax"]["subject"] != "law15.collection.sort" or bend_route["bend"]["subject"] != "Sort.sort":
        errors.append("cross-source route source identities were relabeled")
    if bend_route["algorithm_boundary"]["claim"] != (
        "independent source algorithms prove aligned postconditions; no source/body identity or translation-equivalence claim"
    ):
        errors.append("cross-source route overstates algorithm identity")
    if bend_route["trusted_computing_bases"]["bend"] == bend_route["trusted_computing_bases"]["semaprax"]:
        errors.append("cross-source route collapses distinct proof TCBs")
    if bend_route["scope_limit"] != (
        "supplemental cross-source semantic theorem comparison for the U32 subdomain only; not certification of fixtures/full-u32-encoding-v1/sort.spx, law16.insert, or law16.sort; no timing or LAW16 source-pair admission"
    ):
        errors.append("cross-source route scope boundary changed")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", type=Path, default=PROFILE)
    args = parser.parse_args()
    errors = verify(args.profile)
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print("PASS: pins, bounded controls, and source-distinct theorem routes verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
