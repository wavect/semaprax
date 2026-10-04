#!/usr/bin/env python3
"""Verify the pins and route boundaries of the LAW16 guarded-i64 v2 capsule."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PROFILE = ROOT / "fixtures/full-u32-guarded-i64-profile-v2.json"


def sha256(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def verify(profile_path: Path = PROFILE, root: Path = ROOT) -> list[str]:
    profile = json.loads(profile_path.read_text())
    errors: list[str] = []
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
    capsule = json.loads((root / "evidence/law16-i64-list-proof-v1/capsule.json").read_text())
    if sha256(root / "evidence/law16-i64-list-proof-v1/capsule.json") != profile["routes"][2]["capsule_sha256"]:
        errors.append("LAW15 Lean capsule hash mismatch")
    if capsule.get("status") != "supplemental_i64_list_profile_proved":
        errors.append("LAW15 Lean capsule does not report its proved status")
    if profile["routes"][1]["disposition"] != "paired_concrete_controls_pass_theorem_match_unsupported":
        errors.append("LAW16 list route was promoted beyond its concrete/model evidence")
    if profile["routes"][2]["disposition"] != "source_authenticated_semaprax_theorem_proved_bend_comparison_incomparable":
        errors.append("LAW15 route was incorrectly presented as a matched Bend theorem")
    if profile["routes"][2]["identity_boundary"] != (
        "does not authenticate fixtures/full-u32-encoding-v1/sort.spx or law16.insert/law16.sort; no cross-source aliasing"
    ):
        errors.append("LAW15/LAW16 source identity boundary changed")
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
    print("PASS: pins, bounded controls, and incomparable theorem routes verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
