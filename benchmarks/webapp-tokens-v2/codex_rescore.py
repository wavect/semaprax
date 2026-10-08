#!/usr/bin/env python3
"""Offline, separately pinned rescoring of a completed Codex TeamDesk campaign.

This never edits the original campaign or its accounting.  It only accepts a
terminal receipt, copies closed candidate archives into a new output root, and
records a new gate result beside the original status and paid-wall evidence.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import math
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

BENCHMARK = Path(__file__).resolve().parent
REPO = BENCHMARK.parents[1]
sys.path.insert(0, str(BENCHMARK))
sys.path.insert(0, str(BENCHMARK.parent))
import codex_campaign as campaign
from dependency_bundle import dependency_inventory, copy_dependency_bundle, dependency_fingerprint

SCHEMA = "semaprax.teamdesk.codex-rescore.v1"
TERMINAL_SCHEMA = "semaprax.codex-campaign-terminal.v1"
DEPENDENCY_SCHEMA = "semaprax.rescore.dependencies.v1"
ARCHIVE_EXCLUDED = set(campaign.ARCHIVE_EXCLUDED_DIRS)


def digest(path: Path) -> str:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"expected regular file: {path}")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"expected regular JSON file: {path}")
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"JSON root must be an object: {path}")
    return value


def rel_file_inventory(root: Path) -> dict[str, str]:
    if root.is_symlink() or not root.is_dir():
        raise ValueError("candidate archive must be a real directory")
    root = root.resolve(strict=True)
    inventory: dict[str, str] = {}
    for current, directories, files in os.walk(root, followlinks=False):
        current_path = Path(current)
        if current_path.is_symlink():
            raise ValueError("candidate archive contains a symlink directory")
        for name in directories:
            if (current_path / name).is_symlink():
                raise ValueError("candidate archive contains a symlink directory")
        directories[:] = sorted(name for name in directories if name not in ARCHIVE_EXCLUDED)
        for name in sorted(files):
            path = current_path / name
            if path.is_symlink():
                raise ValueError("candidate archive contains a symlink file")
            if not path.is_file():
                raise ValueError("candidate archive contains a non-regular file")
            relative = path.relative_to(root).as_posix()
            if any(part in ARCHIVE_EXCLUDED for part in Path(relative).parts):
                continue
            inventory[relative] = digest(path)
    return inventory


def copy_closed_archive(source: Path, destination: Path, expected: dict[str, str]) -> dict[str, str]:
    actual = rel_file_inventory(source)
    if actual != expected:
        raise ValueError("archived candidate closed inventory or hashes differ from original result")
    if destination.exists() or destination.is_symlink():
        raise ValueError("rescore candidate destination must be absent")
    destination.mkdir(parents=True)
    for relative, wanted in actual.items():
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / relative, target, follow_symlinks=False)
        if digest(target) != wanted:
            raise ValueError("candidate copy hash differs from archived source")
    copied = rel_file_inventory(destination)
    if copied != actual:
        raise ValueError("candidate copy inventory differs from archived source")
    return copied


def dependency_entries(receipt_path: Path, original_trials: list[dict[str, Any]]) -> tuple[dict[str, Any], str]:
    receipt = load_json(receipt_path)
    entries = receipt.get("entries")
    wanted = {f"typescript-{number:02d}" for number in range(1, campaign.MIN_TRIALS_PER_ARM + 1)}
    if receipt.get("schema") != DEPENDENCY_SCHEMA or not isinstance(entries, dict) or set(entries) != wanted:
        raise ValueError("dependency receipt does not bind every TypeScript trial")
    originals = {(row["arm"], row["number"]): row for row in original_trials}
    for key, entry in entries.items():
        row = originals[("typescript", int(key[-2:]))]
        expected = row.get("candidate_files_sha256")
        if not isinstance(entry, dict) or not isinstance(expected, dict):
            raise ValueError("dependency receipt entry lacks original candidate binding")
        bundle = Path(str(entry.get("bundle_path", "")))
        package_json, package_lock = expected.get("package.json"), expected.get("package-lock.json")
        if (bundle.is_symlink() or not bundle.is_dir() or not isinstance(package_json, str)
                or entry.get("package_json_sha256") != package_json
                or entry.get("package_lock_sha256") != package_lock
                or entry.get("historical_byte_identity_verified") is not False
                or not isinstance(entry.get("recovery_origin"), (str, dict))
                or not entry.get("recovery_origin")
                or not isinstance(entry.get("inventory"), dict)
                or dependency_inventory(bundle) != entry["inventory"]):
            raise ValueError("dependency receipt entry differs from its original candidate or closed bundle")
    return receipt, digest(receipt_path)


def terminal_trials(results: dict[str, Any], terminal: dict[str, Any], results_sha: str, campaign_sha: str) -> list[dict[str, Any]]:
    campaign_status = results.get("campaign_status")
    receipt_status, exit_code = terminal.get("status"), terminal.get("process_exit_code")
    ordinary = receipt_status == "complete" and campaign_status == "complete" and exit_code == 0
    finalized_interruption = (receipt_status == "finalized" and campaign_status == "interrupted"
        and terminal.get("campaign_status") == campaign_status and exit_code in {0, 2}
        and terminal.get("actual_process_exit_code") == exit_code
        and terminal.get("unlaunched_trial_order") == []
        and results.get("unlaunched_trial_order") == [])
    if (terminal.get("schema") != TERMINAL_SCHEMA or not (ordinary or finalized_interruption)
            or terminal.get("campaign_sha256") != campaign_sha
            or terminal.get("results_sha256") != results_sha):
        raise ValueError("terminal receipt does not bind a finalized completed ten-trial campaign")
    trials = results.get("trials")
    if campaign_status not in {"complete", "interrupted"} or not isinstance(trials, list) or len(trials) != 10:
        raise ValueError("original results are not a finalized ten-trial campaign")
    seen: set[tuple[str, int]] = set()
    counts = {arm: 0 for arm in campaign.ARMS}
    for row in trials:
        if not isinstance(row, dict) or row.get("arm") not in campaign.ARMS or type(row.get("number")) is not int:
            raise ValueError("original trial lacks an arm/number identity")
        identity = (row["arm"], row["number"])
        if identity in seen:
            raise ValueError("original trials contain a duplicate arm/number")
        seen.add(identity); counts[row["arm"]] += 1
    if counts != {arm: campaign.MIN_TRIALS_PER_ARM for arm in campaign.ARMS}:
        raise ValueError("original campaign must contain exactly five trials per arm")
    claimed = terminal.get("trial_ids") if receipt_status == "finalized" else terminal.get("trials")
    expected = {(arm, number) for arm in campaign.ARMS for number in range(1, campaign.MIN_TRIALS_PER_ARM + 1)}
    claimed_ids = [(x.get("arm"), x.get("number")) for x in claimed if isinstance(x, dict)] if isinstance(claimed, list) else []
    if not isinstance(claimed, list) or len(claimed_ids) != len(claimed) or len(set(claimed_ids)) != 10 or set(claimed_ids) != expected or seen != expected:
        raise ValueError("terminal receipt does not bind every original trial identity")
    return trials


def snapshot_gate(repo: Path, output: Path, original: dict[str, Any], clarification: Path,
                  qualification_path: Path, gate_source: str) -> dict[str, Any]:
    old_seed = original.get("seed_files_sha256", {})
    spec_sha = old_seed.get(campaign.FROZEN_SPEC)
    contract_relative = "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md"
    old_contract = old_seed.get(contract_relative)
    if not isinstance(spec_sha, str) or not isinstance(old_contract, str):
        raise ValueError("original campaign lacks frozen SPEC and CONTRACT identities")
    clarification_sha = digest(clarification)
    receipt_path = qualification_path.resolve(strict=True)
    receipt = load_json(receipt_path)
    gate_commit = campaign.validate_qualification_receipt(repo, receipt, spec_sha)
    if campaign.resolve_commit(repo, gate_source) != gate_commit:
        raise ValueError("new gate source does not match the qualification receipt")
    runner_files = tuple(campaign.ACCEPTANCE_SOURCE_FILES)
    hashes: dict[str, str] = {}
    destination = output / "gate-source"
    if destination.exists():
        raise ValueError("new gate snapshot destination must be absent")
    for relative in runner_files:
        source = repo / relative
        hashes[relative] = digest(source)
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        qualified = campaign._git_file(repo, gate_commit, relative)
        if source.read_bytes() != qualified:
            raise ValueError(f"new gate differs from qualified commit: {relative}")
        shutil.copy2(source, target, follow_symlinks=False)
        if digest(target) != hashes[relative]:
            raise ValueError("new gate snapshot hash differs while copying")
    contract_source = repo / contract_relative
    contract_hash = digest(contract_source)
    if contract_source.read_bytes() != campaign._git_file(repo, gate_commit, contract_relative):
        raise ValueError("new contract differs from qualified commit")
    contract_snapshot = output / "gate-metadata" / "CONTRACT.md"
    contract_snapshot.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(contract_source, contract_snapshot, follow_symlinks=False)
    if digest(contract_snapshot) != contract_hash:
        raise ValueError("new contract snapshot hash differs while copying")
    snapshot_receipt = destination / "qualification-receipt.json"
    snapshot_clarification = destination / "clarification.md"
    shutil.copy2(receipt_path, snapshot_receipt, follow_symlinks=False)
    shutil.copy2(clarification, snapshot_clarification, follow_symlinks=False)
    return {
        "path": "gate-source", "files_sha256": hashes, "runner_files_sha256": dict(hashes),
        "contract_document": {"repo_path": contract_relative, "sha256": contract_hash,
                              "snapshot_path": "gate-metadata/CONTRACT.md"},
        "qualification_receipt": {"path": str(receipt_path), "sha256": digest(receipt_path),
                                    "snapshot_path": "gate-source/qualification-receipt.json",
                                    "gate_source_commit": gate_commit, "required_cases": 912},
        "original_frozen_inputs": {"spec_sha256": spec_sha, "contract_sha256": old_contract},
        "clarification": {"path": str(clarification), "sha256": clarification_sha,
                            "snapshot_path": "gate-source/clarification.md"},
    }


def verify_gate_unchanged(repo: Path, output: Path, gate: dict[str, Any]) -> None:
    for relative, wanted in gate["files_sha256"].items():
        if digest(repo / relative) != wanted or digest(output / gate["path"] / relative) != wanted:
            raise ValueError(f"new gate source drifted: {relative}")
    contract = gate["contract_document"]
    if (digest(repo / contract["repo_path"]) != contract["sha256"]
            or digest(output / contract["snapshot_path"]) != contract["sha256"]):
        raise ValueError("new contract source drifted")
    receipt = gate["qualification_receipt"]
    clarification = gate["clarification"]
    if (digest(Path(receipt["path"])) != receipt["sha256"]
            or digest(output / receipt["snapshot_path"]) != receipt["sha256"]
            or digest(Path(clarification["path"])) != clarification["sha256"]
            or digest(output / clarification["snapshot_path"]) != clarification["sha256"]):
        raise ValueError("new qualification receipt or clarification drifted")


def contract_groups(contract: Path) -> set[str]:
    match = re.search(r"export const COVERAGE = \[(.*?)\];", contract.read_text(encoding="utf-8"), re.S)
    if match is None:
        raise ValueError("snapshotted gate contract lacks the coverage inventory")
    groups = set(re.findall(r"'([^']+)'", match.group(1)))
    if not groups:
        raise ValueError("snapshotted gate contract has an empty coverage inventory")
    return groups


def reference_case_groups(receipt: dict[str, Any], arm: str, settings: dict[str, Any], compiler_sha: str) -> dict[str, str]:
    """Load the receipt-bound full reference report, never its spotlight summary."""
    arm_receipt = receipt.get("arms", {}).get(arm)
    if not isinstance(arm_receipt, dict):
        raise ValueError(f"qualification receipt lacks {arm} reference binding")
    report_path = Path(str(arm_receipt.get("report_file", "")))
    report_sha = arm_receipt.get("report_sha256")
    if (not isinstance(report_sha, str) or report_path.is_symlink() or not report_path.is_file()
            or digest(report_path) != report_sha):
        raise ValueError(f"qualification receipt {arm} report hash binding differs")
    report = load_json(report_path)
    checks, gate_rows, qualification = report.get("checks"), report.get("gate"), report.get("qualification", {})
    if (not isinstance(checks, list) or not isinstance(gate_rows, list)
            or not all(isinstance(row, dict) and isinstance(row.get("id"), str) and row["id"]
                       and isinstance(row.get("group"), str) and row["group"] for row in checks)
            or not all(isinstance(row, dict) and isinstance(row.get("name"), str) and row["name"]
                       and isinstance(row.get("sha256"), str) and row["sha256"] for row in gate_rows)):
        raise ValueError(f"qualification receipt {arm} report is not a full accepted reference: malformed rows")
    gate_prefix = "benchmarks/webapp-tokens-v2/acceptance/"
    expected_gate = {name.removeprefix(gate_prefix): value for name, value in
                     settings["harness_source_snapshot"]["files_sha256"].items() if name.startswith(gate_prefix)}
    reported_gate = {row["name"]: row["sha256"] for row in gate_rows}
    if not (report.get("schema") == "semaprax.teamdesk.acceptance.v1" and report.get("arm") == arm
            and report.get("spec_sha256") == settings["qualification"]["spec_sha256"]
            and len(checks) == 912 and len(gate_rows) == len(expected_gate)
            and reported_gate == expected_gate and qualification.get("passed") is True
            and qualification.get("cases") == 912 and qualification.get("missingCases") == []
            and qualification.get("missingGroups") == [] and qualification.get("failures") == []):
        raise ValueError(f"qualification receipt {arm} report is not a full accepted reference")
    cases = {row.get("id"): row.get("group") for row in checks if isinstance(row, dict)}
    if (len(cases) != 912 or any(not isinstance(case, str) or not isinstance(group, str) or not group
                                 for case, group in cases.items())
            or any(row.get("status") != "passed" for row in checks if isinstance(row, dict))):
        raise ValueError(f"qualification receipt {arm} report lacks exact passed case/group inventory")
    if arm == "semaprax":
        compiler = report.get("compiler", {})
        if compiler.get("source_sha") != settings["compiler_source_commit"] or compiler.get("sha256") != compiler_sha:
            raise ValueError("qualification receipt SEMAPRAX compiler binding differs")
    return cases


def report_is_accepted(report: dict[str, Any], arm: str, settings: dict[str, Any], compiler_sha: str,
                       required_cases: dict[str, str], required_groups: set[str]) -> bool:
    checks = report.get("checks")
    gate_rows = report.get("gate")
    if (not isinstance(checks, list) or not isinstance(gate_rows, list)
            or not all(isinstance(row, dict) and isinstance(row.get("id"), str) and row["id"]
                       and isinstance(row.get("group"), str) and row["group"] for row in checks)
            or not all(isinstance(row, dict) and isinstance(row.get("name"), str) and row["name"]
                       and isinstance(row.get("sha256"), str) and row["sha256"] for row in gate_rows)):
        return False
    gate_prefix = "benchmarks/webapp-tokens-v2/acceptance/"
    expected_gate = {name.removeprefix(gate_prefix): value for name, value in
                     settings["harness_source_snapshot"]["files_sha256"].items() if name.startswith(gate_prefix)}
    reported_gate = {row["name"]: row["sha256"] for row in gate_rows}
    identifiers = [row["id"] for row in checks]
    qualification = report.get("qualification", {})
    compiler = report.get("compiler", {})
    return (report.get("schema") == "semaprax.teamdesk.acceptance.v1" and report.get("arm") == arm
            and report.get("spec_sha256") == settings["qualification"]["spec_sha256"]
            and reported_gate == expected_gate and len(gate_rows) == len(expected_gate)
            and qualification.get("passed") is True and qualification.get("cases") == 912
            and qualification.get("missingCases") == [] and qualification.get("missingGroups") == []
            and qualification.get("failures") == [] and len(identifiers) == len(set(identifiers)) == 912
            and {row["id"]: row["group"] for row in checks} == required_cases
            and all(isinstance(row, dict) and row.get("status") == "passed"
                    and isinstance(row.get("group"), str) and row["group"] in required_groups for row in checks)
            and {row["group"] for row in checks} == required_groups
            and (arm != "semaprax" or (compiler.get("source_sha") == settings["compiler_source_commit"]
                                         and compiler.get("sha256") == compiler_sha)))


def gate_inventory_shapes(gate: dict[str, Any]) -> bool:
    runner = set(campaign.ACCEPTANCE_SOURCE_FILES)
    recorded, recorded_runner, contract = (gate.get("files_sha256"), gate.get("runner_files_sha256"),
                                             gate.get("contract_document"))
    return (isinstance(recorded, dict) and isinstance(recorded_runner, dict) and isinstance(contract, dict)
            and set(recorded) == runner and set(recorded_runner) == runner
            and all(isinstance(value, str) for value in recorded.values())
            and all(recorded_runner[name] == recorded[name] for name in runner)
            and contract.get("repo_path") == "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md"
            and isinstance(contract.get("sha256"), str) and contract.get("snapshot_path") == "gate-metadata/CONTRACT.md")


def validate_gate_inventory(repo: Path, gate: dict[str, Any], campaign_record: dict[str, Any], gate_commit: str) -> None:
    if not gate_inventory_shapes(gate):
        raise ValueError("sidecar gate inventory differs from the admitted acceptance closure")
    seed = campaign_record.get("seed_files_sha256", {})
    frozen = gate.get("original_frozen_inputs", {})
    if (not isinstance(seed, dict) or frozen.get("spec_sha256") != seed.get(campaign.FROZEN_SPEC)
            or frozen.get("contract_sha256") != seed.get("benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md")):
        raise ValueError("sidecar frozen inputs differ from the original campaign seed")
    for relative, wanted in gate["files_sha256"].items():
        if hashlib.sha256(campaign._git_file(repo, gate_commit, relative)).hexdigest() != wanted:
            raise ValueError(f"sidecar gate inventory differs from its qualified commit: {relative}")
    contract = gate["contract_document"]
    if hashlib.sha256(campaign._git_file(repo, gate_commit, contract["repo_path"])).hexdigest() != contract["sha256"]:
        raise ValueError("sidecar contract differs from its qualified commit")


def rescore_settings(campaign_record: dict[str, Any], output: Path, gate: dict[str, Any]) -> dict[str, Any]:
    settings = dict(campaign_record)
    settings["artifacts"] = str(output)
    settings["harness_source_snapshot"] = {"path": gate["path"], "files_sha256": gate["runner_files_sha256"]}
    settings["qualification"] = {"spec_sha256": gate["original_frozen_inputs"]["spec_sha256"], "required_cases": 912}
    return settings


def retained_candidate_matches(source: dict[str, Any], row: dict[str, Any]) -> bool:
    archive, expected = source.get("candidate_archive"), source.get("candidate_files_sha256")
    return (isinstance(archive, str) and isinstance(expected, dict)
            and row.get("candidate_files_sha256") == expected and rel_file_inventory(Path(archive)) == expected)


def scored_wall_matches(row: dict[str, Any]) -> bool:
    acceptance = row.get("new_acceptance")
    return isinstance(acceptance, dict) and row.get("new_acceptance_wall_seconds") == acceptance.get("seconds")


def validate_sidecar(path: Path, repo: Path) -> dict[str, Any]:
    sidecar = load_json(path)
    if sidecar.get("schema") != SCHEMA:
        raise ValueError("rescore sidecar schema differs")
    # Older sidecars remain readable under their original receipt contract.
    # Newly produced sidecars also retain the relocated copy implementation.
    copy_module = sidecar.get("dependency_copy_module")
    if copy_module is not None:
        if (not isinstance(copy_module, dict)
                or copy_module.get("snapshot_path") != "tooling-source/dependency_bundle.py"
                or digest(path.parent / copy_module["snapshot_path"]) != copy_module.get("sha256")):
            raise ValueError("rescore dependency copy module hash drifted")
    original, gate, rows = sidecar.get("original"), sidecar.get("gate"), sidecar.get("trials")
    compiler = sidecar.get("compiler")
    jobs, wall = sidecar.get("rescore_jobs"), sidecar.get("rescore_wall_seconds")
    if (type(jobs) is not int or jobs not in (1, 2) or type(wall) not in (int, float)
            or (isinstance(wall, float) and not math.isfinite(wall)) or wall < 0):
        raise ValueError("rescore sidecar has an invalid jobs or wall-time setting")
    if not isinstance(original, dict) or not isinstance(gate, dict) or not isinstance(rows, list) or not isinstance(compiler, dict):
        raise ValueError("rescore sidecar has an invalid top-level inventory")
    original_path, campaign_path, terminal_path = (Path(str(original.get(key, "")))
        for key in ("results", "campaign", "terminal_receipt"))
    if (digest(original_path) != original.get("results_sha256")
            or digest(campaign_path) != original.get("campaign_sha256")
            or digest(terminal_path) != original.get("terminal_receipt_sha256")):
        raise ValueError("original campaign evidence hash drifted")
    original_results, campaign_record, terminal = load_json(original_path), load_json(campaign_path), load_json(terminal_path)
    if original_results.get("campaign") != campaign_record:
        raise ValueError("original results no longer bind the supplied campaign record")
    original_trials = terminal_trials(original_results, terminal, digest(original_path), digest(campaign_path))
    dependency = sidecar.get("dependency_receipt")
    dependency_receipt = None
    dependency_sha = None
    if dependency is not None:
        if not isinstance(dependency, dict):
            raise ValueError("sidecar dependency receipt binding is malformed")
        dependency_path = Path(str(dependency.get("path", "")))
        if digest(dependency_path) != dependency.get("sha256"):
            raise ValueError("sidecar dependency receipt hash drifted")
        dependency_receipt, dependency_sha = dependency_entries(dependency_path, original_trials)
        if dependency_sha != dependency.get("sha256"):
            raise ValueError("sidecar dependency receipt differs from its hash binding")
    if (compiler.get("source_sha") != campaign_record.get("compiler_source_commit")
            or compiler.get("sha256") != campaign_record.get("source_binary_sha256")
            or digest(Path(str(compiler.get("path", "")))) != compiler.get("sha256")):
        raise ValueError("sidecar compiler identity differs from the original campaign")
    if not gate_inventory_shapes(gate):
        raise ValueError("sidecar gate inventory differs from the admitted acceptance closure")
    verify_gate_unchanged(repo, path.parent, gate)
    expected_settings = rescore_settings(campaign_record, path.parent, gate)
    if sidecar.get("settings") != expected_settings:
        raise ValueError("sidecar settings differ from the original campaign and permitted rescore overrides")
    receipt = load_json(Path(str(gate["qualification_receipt"]["path"])))
    gate_commit = campaign.validate_qualification_receipt(repo, receipt, gate["original_frozen_inputs"]["spec_sha256"])
    if gate_commit != gate["qualification_receipt"].get("gate_source_commit"):
        raise ValueError("sidecar qualification receipt gate binding differs")
    validate_gate_inventory(repo, gate, campaign_record, gate_commit)
    if (receipt.get("compiler_source_commit") != compiler.get("source_sha")
            or receipt.get("compiler_binary_sha256") != compiler.get("sha256")):
        raise ValueError("sidecar qualification receipt compiler binding differs")
    required_groups = contract_groups(path.parent / gate["path"] / "benchmarks/webapp-tokens-v2/acceptance/contract.mjs")
    if gate.get("required_groups") != sorted(required_groups):
        raise ValueError("sidecar required group inventory differs from snapshotted contract")
    required = {arm: reference_case_groups(receipt, arm, expected_settings, compiler["sha256"])
                for arm in campaign.ARMS}
    if any(set(cases.values()) != required_groups for cases in required.values()):
        raise ValueError("qualified reference report group inventory differs from snapshotted gate")
    expected = {(arm, number) for arm in campaign.ARMS for number in range(1, campaign.MIN_TRIALS_PER_ARM + 1)}
    actual = {(row.get("arm"), row.get("number")) for row in rows if isinstance(row, dict)}
    if len(rows) != 10 or actual != expected:
        raise ValueError("rescore sidecar has duplicate or missing trial identities")
    originals = {(row["arm"], row["number"]): row for row in original_trials}
    for row in rows:
        identity = (row["arm"], row["number"])
        source = originals[identity]
        for key, source_key in (("original_status", "status"), ("original_acceptance", "acceptance"),
                                ("original_paid_wall_seconds", "elapsed_seconds"), ("original_cost", "list_price"),
                                ("original_resource_assessment", "resource_assessment")):
            if row.get(key) != source.get(source_key):
                raise ValueError(f"sidecar altered original paid field {key} for {identity}")
        status = row.get("rescore_status")
        if status != "unscorable" and not retained_candidate_matches(source, row):
            raise ValueError("scored attempt candidate inventory differs from the archived original")
        if status not in {"accepted", "not_accepted", "unscorable"}:
            raise ValueError("rescore sidecar has an invalid new status")
        if status == "unscorable":
            if (not isinstance(row.get("reason"), str) or row.get("new_acceptance") is not None
                    or (identity[0] != "typescript" and row.get("dependency_setup") is not None)):
                raise ValueError("unscorable original attempt lacks a retained reason")
            continue
        if identity[0] == "typescript":
            if dependency_receipt is None:
                raise ValueError("TypeScript attempt was scored without a dependency receipt")
            entry = dependency_receipt["entries"][f"{identity[0]}-{identity[1]:02d}"]
            setup = row.get("dependency_setup")
            if (not isinstance(setup, dict) or setup.get("entry") != f"{identity[0]}-{identity[1]:02d}"
                    or setup.get("bundle_path") != str(Path(entry["bundle_path"]).resolve())
                    or setup.get("bundle_inventory_sha256") != dependency_fingerprint(entry["inventory"])
                    or setup.get("copy_sha256") != dependency_fingerprint(entry["inventory"])
                    or setup.get("receipt_sha256") != dependency_sha):
                raise ValueError("TypeScript dependency setup differs from the receipt-bound bundle")
            candidate = path.parent / "candidates" / f"{identity[0]}-{identity[1]:02d}"
            if dependency_inventory(candidate, allow_other=True) != entry["inventory"]:
                raise ValueError("TypeScript dependency copy drifted after rescoring")
        elif row.get("dependency_setup") is not None:
            raise ValueError("SEMAPRAX attempt unexpectedly has a dependency setup")
        acceptance = row.get("new_acceptance")
        if (not isinstance(acceptance, dict) or not isinstance(row.get("reason"), str)
                or not scored_wall_matches(row)):
            raise ValueError("scored attempt lacks its gate result, exact wall time, or reason")
        report_path, report_hash = row.get("new_report_path"), row.get("new_report_sha256")
        report: dict[str, Any] | None = None
        if report_path is not None or report_hash is not None:
            if not isinstance(report_path, str) or not isinstance(report_hash, str) or digest(Path(report_path)) != report_hash:
                raise ValueError("scored attempt report bytes differ from its recorded hash")
            report = load_json(Path(report_path))
            if acceptance.get("report") != report:
                raise ValueError("scored attempt embeds a report different from its on-disk report")
        accepted = report is not None and report_is_accepted(report, row["arm"], sidecar["settings"], compiler["sha256"], required[row["arm"]], required_groups)
        if acceptance.get("accepted") is not accepted or (status == "accepted") is not accepted:
            raise ValueError("sidecar accepted status differs from its exact report contract")
    return sidecar


def rescore(args: argparse.Namespace) -> dict[str, Any]:
    repo = Path(args.repo).resolve(strict=True)
    original_path = Path(args.original_results)
    artifacts = Path(args.artifacts)
    compiler = Path(args.semaprax_bin)
    dependency_value = getattr(args, "dependency_receipt", None)
    dependency_path = Path(dependency_value) if dependency_value else None
    terminal_path, qualification_path, clarification = (Path(value) for value in
        (args.terminal_receipt, args.qualification_receipt, args.clarification))
    output = Path(args.output).resolve()
    if (original_path.is_symlink() or not original_path.is_file() or artifacts.is_symlink() or not artifacts.is_dir()
            or compiler.is_symlink() or not compiler.is_file() or terminal_path.is_symlink() or not terminal_path.is_file()
            or qualification_path.is_symlink() or not qualification_path.is_file() or clarification.is_symlink()
            or not clarification.is_file() or (dependency_path is not None
            and (dependency_path.is_symlink() or not dependency_path.is_file()))
            or output.exists() or output.is_symlink()):
        raise ValueError("rescore inputs must be genuine files/directories and output must be absent")
    original_path, artifacts, compiler = (value.resolve(strict=True) for value in (original_path, artifacts, compiler))
    terminal_path, qualification_path, clarification = (value.resolve(strict=True) for value in (terminal_path, qualification_path, clarification))
    if dependency_path is not None:
        dependency_path = dependency_path.resolve(strict=True)
    for protected in (artifacts, original_path, compiler, terminal_path, qualification_path, clarification, dependency_path):
        if protected is None:
            continue
        try:
            output.relative_to(protected if protected.is_dir() else protected.parent)
        except ValueError:
            continue
        raise ValueError("rescore output must be outside original artifacts and all inputs")
    if not compiler.is_file() or compiler.is_symlink() or digest(compiler) != args.compiler_sha256:
        raise ValueError("compiler differs from the explicitly pinned regular binary")
    original = load_json(original_path)
    campaign_row = original.get("campaign")
    if not isinstance(campaign_row, dict) or campaign_row.get("compiler_source_commit") != args.compiler_source_sha:
        raise ValueError("original campaign compiler source identity differs")
    if campaign_row.get("source_binary_sha256") != args.compiler_sha256:
        raise ValueError("original campaign compiler binary identity differs")
    campaign_path = artifacts / "campaign.json"
    if campaign_path.is_symlink() or not campaign_path.is_file():
        raise ValueError("original campaign record must be a regular file")
    terminal = load_json(terminal_path)
    trials = terminal_trials(original, terminal, digest(original_path), digest(campaign_path))
    dependency_receipt: dict[str, Any] | None = None
    dependency_receipt_sha: str | None = None
    if dependency_path is not None:
        dependency_receipt, dependency_receipt_sha = dependency_entries(dependency_path, trials)
    for original_row in trials:
        archive = original_row.get("candidate_archive")
        if isinstance(archive, str):
            archive_path = Path(archive)
            if archive_path.is_symlink() or not archive_path.is_dir():
                raise ValueError("original candidate archive must be a real directory")
            try:
                output.relative_to(archive_path.resolve(strict=True))
            except ValueError:
                pass
            else:
                raise ValueError("rescore output must be outside original candidate archives")
    output.mkdir(parents=True)
    copy_module_source = BENCHMARK / "dependency_bundle.py"
    copy_module = {"snapshot_path": "tooling-source/dependency_bundle.py", "sha256": digest(copy_module_source)}
    copy_module_target = output / copy_module["snapshot_path"]
    copy_module_target.parent.mkdir()
    shutil.copy2(copy_module_source, copy_module_target, follow_symlinks=False)
    if digest(copy_module_target) != copy_module["sha256"]:
        raise ValueError("rescore dependency copy module drifted while snapshotting")
    gate = snapshot_gate(repo, output, campaign_row, clarification, qualification_path, args.gate_source)
    validate_gate_inventory(repo, gate, campaign_row, gate["qualification_receipt"]["gate_source_commit"])
    receipt = load_json(qualification_path)
    if (receipt.get("compiler_source_commit") != args.compiler_source_sha
            or receipt.get("compiler_binary_sha256") != args.compiler_sha256):
        raise ValueError("qualification receipt compiler identity differs from the original campaign")
    required_groups = contract_groups(output / gate["path"] / "benchmarks/webapp-tokens-v2/acceptance/contract.mjs")
    gate["required_groups"] = sorted(required_groups)
    settings = rescore_settings(campaign_row, output, gate)
    required_cases = {arm: reference_case_groups(receipt, arm, settings, args.compiler_sha256) for arm in campaign.ARMS}
    if any(set(cases.values()) != required_groups for cases in required_cases.values()):
        raise ValueError("qualified reference report group inventory differs from snapshotted gate")
    rows: list[dict[str, Any]] = []
    def score_one(original_row: dict[str, Any]) -> dict[str, Any]:
        arm, number = original_row["arm"], original_row["number"]
        row: dict[str, Any] = {"arm": arm, "number": number, "original_status": original_row.get("status"),
                               "original_acceptance": original_row.get("acceptance"),
                               "original_paid_wall_seconds": original_row.get("elapsed_seconds"),
                               "original_cost": original_row.get("list_price"),
                               "original_resource_assessment": original_row.get("resource_assessment"),
                               "rescore_status": "unscorable", "dependency_setup": None}
        archive = original_row.get("candidate_archive")
        expected = original_row.get("candidate_files_sha256")
        if not isinstance(archive, str) or not isinstance(expected, dict) or not all(isinstance(k, str) and isinstance(v, str) for k, v in expected.items()):
            row["reason"] = "original attempt has no closed archived candidate inventory"
            return row
        entry = None
        if arm == "typescript":
            if dependency_receipt is None:
                row["reason"] = "TypeScript dependency bundle is unavailable for offline replay"
                return row
            entry = dependency_receipt["entries"][f"{arm}-{number:02d}"]
        verify_gate_unchanged(repo, output, gate)
        candidate = output / "candidates" / f"{arm}-{number:02d}"
        copied = copy_closed_archive(Path(archive), candidate, expected)
        if entry is not None:
            bundle = Path(entry["bundle_path"])
            copied_dependency = copy_dependency_bundle(bundle, candidate, entry["inventory"])
            row["dependency_setup"] = {"entry": f"{arm}-{number:02d}", "bundle_path": str(bundle.resolve()),
                                       "bundle_inventory_sha256": dependency_fingerprint(entry["inventory"]),
                                       "copy_sha256": copied_dependency,
                                       "receipt_sha256": dependency_receipt_sha}
        evidence = output / "acceptance" / f"{arm}-{number:02d}"
        evidence.mkdir(parents=True)
        try:
            result = campaign.check_candidate(candidate, evidence, arm, settings, compiler)
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            verify_gate_unchanged(repo, output, gate)
            if (rel_file_inventory(candidate) != copied or rel_file_inventory(Path(archive)) != expected
                    or (entry is not None and dependency_inventory(candidate, allow_other=True) != entry["inventory"]
                        or entry is not None and dependency_inventory(Path(entry["bundle_path"])) != entry["inventory"])):
                raise ValueError("candidate copy or original archive changed during failed rescoring")
            row.update({"rescore_status": "not_accepted", "reason": f"new gate execution failed: {error}",
                        "candidate_files_sha256": copied, "new_acceptance": {"accepted": False, "report": {}},
                        "new_acceptance_wall_seconds": None, "new_report_path": None, "new_report_sha256": None})
            return row
        verify_gate_unchanged(repo, output, gate)
        if (rel_file_inventory(candidate) != copied or rel_file_inventory(Path(archive)) != expected
                or (entry is not None and dependency_inventory(candidate, allow_other=True) != entry["inventory"]
                    or entry is not None and dependency_inventory(Path(entry["bundle_path"])) != entry["inventory"])):
            raise ValueError("candidate copy or original archive changed during rescoring")
        report_path = evidence / "report.json"
        report = load_json(report_path) if report_path.is_file() and not report_path.is_symlink() else None
        accepted = (result.get("accepted") is True and report is not None
                    and report_is_accepted(report, arm, settings, args.compiler_sha256, required_cases[arm], required_groups))
        result["accepted"] = accepted
        result["report"] = report if report is not None else {}
        row.update({"rescore_status": "accepted" if accepted else "not_accepted",
                    "reason": "accepted" if accepted else str(result.get("failure") or "new gate did not produce an accepted report"),
                    "candidate_files_sha256": copied, "new_acceptance": result,
                    "new_acceptance_wall_seconds": result.get("seconds"),
                    "new_report_path": str(report_path) if report is not None else None,
                    "new_report_sha256": digest(report_path) if report is not None else None})
        return row
    rescore_started = time.monotonic()
    with ThreadPoolExecutor(max_workers=args.jobs) as executor:
        rows = list(executor.map(score_one, trials))
    rescore_wall_seconds = round(time.monotonic() - rescore_started, 3)
    verify_gate_unchanged(repo, output, gate)
    if digest(copy_module_source) != copy_module["sha256"] or digest(copy_module_target) != copy_module["sha256"]:
        raise ValueError("rescore dependency copy module drifted during scoring")
    sidecar = {"schema": SCHEMA, "dependency_copy_module": copy_module, "rescore_jobs": args.jobs, "rescore_wall_seconds": rescore_wall_seconds, "claim": "separate rescoring evidence; original paid results and costs are retained", "original": {"results": str(original_path), "results_sha256": digest(original_path), "campaign": str(campaign_path), "campaign_sha256": digest(campaign_path), "terminal_receipt": str(terminal_path), "terminal_receipt_sha256": digest(terminal_path)}, "dependency_receipt": None if dependency_path is None else {"path": str(dependency_path), "sha256": dependency_receipt_sha}, "gate": gate, "compiler": {"path": str(compiler), "sha256": args.compiler_sha256, "source_sha": args.compiler_source_sha}, "settings": settings, "trials": rows}
    (output / "rescore.json").write_text(json.dumps(sidecar, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return sidecar


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=str(REPO)); parser.add_argument("--original-results")
    parser.add_argument("--artifacts"); parser.add_argument("--terminal-receipt")
    parser.add_argument("--semaprax-bin"); parser.add_argument("--compiler-sha256")
    parser.add_argument("--compiler-source-sha"); parser.add_argument("--clarification")
    parser.add_argument("--qualification-receipt"); parser.add_argument("--gate-source")
    parser.add_argument("--output"); parser.add_argument("--dependency-receipt"); parser.add_argument("--jobs", type=int, choices=(1, 2), default=1); parser.add_argument("--validate-sidecar")
    try:
        args = parser.parse_args()
        repo = Path(args.repo).resolve(strict=True)
        if args.validate_sidecar:
            print(json.dumps(validate_sidecar(Path(args.validate_sidecar).resolve(strict=True), repo), indent=2, sort_keys=True))
            return 0
        required = ("original_results", "artifacts", "terminal_receipt", "semaprax_bin", "compiler_sha256",
                    "compiler_source_sha", "clarification", "qualification_receipt", "gate_source", "output")
        if any(getattr(args, name) is None for name in required):
            raise ValueError("rescore requires original results/artifacts/terminal receipt/compiler/gate/clarification/output bindings")
        print(json.dumps(rescore(args), indent=2, sort_keys=True))
        return 0
    except (OSError, ValueError, json.JSONDecodeError, shutil.Error, subprocess.SubprocessError) as error:
        print(f"codex rescore error: {error}", file=sys.stderr); return 2


if __name__ == "__main__":
    raise SystemExit(main())
