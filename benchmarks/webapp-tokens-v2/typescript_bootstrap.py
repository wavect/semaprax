#!/usr/bin/env python3
"""Prepare dependency-only tooling for a new, explicitly opted-in campaign.

No npm install, cache metadata, application source, or historical replay is
involved. Preparation and validation make no model requests.
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import stat
import subprocess
from pathlib import Path
from typing import Any

import dependency_bundle as dependencies

SCHEMA = "semaprax.teamdesk.typescript-bootstrap.v1"
CORE_PACKAGES = ("react", "react-dom", "react-router-dom", "typescript", "vite",
                 "@vitejs/plugin-react", "@types/node", "@types/react", "@types/react-dom")
MODULES = ("typescript_bootstrap.py", "dependency_bundle.py")


def load(path: Path) -> dict[str, Any]:
    dependencies.digest(path)
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("bootstrap JSON must be an object")
    return value


def module_hashes() -> dict[str, str]:
    return {name: dependencies.digest(Path(__file__).with_name(name)) for name in MODULES}


def runtime_identity(node_binary: str) -> dict[str, Any]:
    executable = Path(shutil.which(node_binary) or node_binary).resolve(strict=True)
    result = subprocess.run([str(executable), "-p",
                             "JSON.stringify({versions:process.versions,platform:process.platform,arch:process.arch})"],
                            check=True, capture_output=True, text=True, timeout=30)
    return {"node_sha256": dependencies.digest(executable), "node": json.loads(result.stdout),
            "host_platform": platform.system(), "host_machine": platform.machine()}


def node_inventory(root: Path) -> dict[str, list[dict[str, Any]]]:
    inventory = dependencies.dependency_inventory(root, allow_other=True)
    selected = {key: [row for row in rows if Path(row["path"]).parts[0] == "node_modules"]
                for key, rows in inventory.items()}
    if not selected["files"]:
        raise ValueError("bootstrap requires a populated node_modules tree")
    for row in selected["symlinks"]:
        target = (root / row["path"]).resolve(strict=True)
        try:
            target.relative_to((root / "node_modules").resolve(strict=True))
        except ValueError as error:
            raise ValueError("bootstrap link must stay inside node_modules") from error
    return selected


def package_versions(root: Path) -> dict[str, str]:
    return {name: load(root / "node_modules" / name / "package.json")["version"]
            for name in CORE_PACKAGES}


def qualification_inputs(summary_path: Path, reference: Path) -> dict[str, Any]:
    summary = load(summary_path)
    arm = summary.get("arms", {}).get("typescript", {})
    qualification = arm.get("qualification", {})
    if (summary.get("schema") != "semaprax.teamdesk.reference.qualification.v1"
            or summary.get("qualified") is not True or qualification.get("passed") is not True
            or qualification.get("cases") != 912 or qualification.get("failures") != []
            or qualification.get("missingCases") != [] or qualification.get("missingGroups") != []):
        raise ValueError("bootstrap requires a qualified 912-case TypeScript reference")
    report_path = Path(arm["report_file"])
    if dependencies.digest(report_path) != arm["report_sha256"]:
        raise ValueError("bootstrap qualification report hash drift")
    report = load(report_path)
    before = {row["name"]: row["sha256"] for row in report["candidate_before"]}
    hashes = {name: dependencies.digest(reference / name)
              for name in ("package.json", "package-lock.json")}
    if (report.get("schema") != "semaprax.teamdesk.acceptance.v1"
            or report.get("qualification") != qualification
            or report.get("arm") != "typescript" or report.get("spec_sha256") != summary["spec_sha256"]
            or any(before.get(name) != value for name, value in hashes.items())):
        raise ValueError("bootstrap package/lock differs from qualified reference")
    return {"summary_path": str(summary_path.resolve()), "summary_sha256": dependencies.digest(summary_path),
            "report_path": str(report_path.resolve()), "report_sha256": arm["report_sha256"],
            "reference_path": str(reference.resolve()), "package_files_sha256": hashes,
            "spec_sha256": summary["spec_sha256"], "gate_source_commit": summary["gate_source"],
            "qualified_node_version": report["node"]}


def prepare(reference: Path, summary: Path, destination: Path, node_binary: str) -> dict[str, Any]:
    reference = reference.resolve(strict=True)
    provenance = qualification_inputs(summary, reference)
    original = node_inventory(reference)
    versions = package_versions(reference)
    runtime = runtime_identity(node_binary)
    if "v" + runtime["node"]["versions"]["node"] != provenance["qualified_node_version"]:
        raise ValueError("bootstrap runtime differs from qualified Node version")
    lock = load(reference / "package-lock.json")
    if any(lock.get("packages", {}).get(f"node_modules/{name}", {}).get("version") != version
           for name, version in versions.items()):
        raise ValueError("bootstrap installed core versions differ from reference lock")
    for relative, entry in lock.get("packages", {}).items():
        if not relative:
            continue
        path = Path(relative)
        if path.is_absolute() or ".." in path.parts or path.parts[0] != "node_modules":
            raise ValueError("bootstrap lock has unsupported package path")
        installed = reference / path / "package.json"
        if not installed.exists() and entry.get("optional") is True:
            continue
        if load(installed).get("version") != entry.get("version"):
            raise ValueError("bootstrap installed dependency differs from locked version")
    if destination.exists() or destination.is_symlink():
        raise ValueError("bootstrap destination must be new")
    destination.mkdir(parents=True)
    bundle = destination / "bundle"
    bundle.mkdir()
    shutil.copytree(reference / "node_modules", bundle / "node_modules", symlinks=True)
    # Owner write permission belongs to the private tooling contract. Preserve
    # executable bits and hash the normalized bundle separately from its origin.
    for current, dirs, files in os.walk(bundle / "node_modules", followlinks=False):
        os.chmod(current, stat.S_IMODE(Path(current).stat().st_mode) | stat.S_IRWXU)
        for name in files:
            path = Path(current) / name
            if not path.is_symlink():
                os.chmod(path, stat.S_IMODE(path.stat().st_mode) | stat.S_IRUSR | stat.S_IWUSR)
    if node_inventory(reference) != original or qualification_inputs(summary, reference) != provenance:
        raise ValueError("bootstrap reference drifted during preparation")
    inventory = dependencies.dependency_inventory(bundle)
    if any(row["path"].split("/")[0] != "node_modules"
           for rows in inventory.values() for row in rows):
        raise ValueError("bootstrap contains non-dependency paths")
    receipt = {"schema": SCHEMA, "bundle_path": str(bundle.resolve()), "inventory": inventory,
               "inventory_sha256": dependencies.dependency_fingerprint(inventory),
               "source_inventory_sha256": dependencies.dependency_fingerprint(original),
               "modules_sha256": module_hashes(), "runtime": runtime,
               "packages": versions, "qualification": provenance,
               "copy_policy": "private writable node_modules only; no app or package scripts",
               "historical_replay": False, "historical_dependency_byte_identity_verified": False}
    (destination / "receipt.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    return receipt


def validate(receipt_path: Path, node_binary: str, *, spec_sha256: str | None = None,
             summary_sha256: str | None = None) -> dict[str, Any]:
    receipt = load(receipt_path)
    if (receipt.get("schema") != SCHEMA or receipt.get("historical_replay") is not False
            or receipt.get("historical_dependency_byte_identity_verified") is not False):
        raise ValueError("unsupported future-campaign bootstrap receipt")
    if receipt.get("modules_sha256") != module_hashes() or receipt.get("runtime") != runtime_identity(node_binary):
        raise ValueError("bootstrap helper, runtime or platform drift")
    q = receipt["qualification"]
    if (spec_sha256 is not None and q["spec_sha256"] != spec_sha256
            or summary_sha256 is not None and q["summary_sha256"] != summary_sha256):
        raise ValueError("bootstrap qualification differs from campaign")
    if qualification_inputs(Path(q["summary_path"]), Path(q["reference_path"])) != q:
        raise ValueError("bootstrap qualification provenance drift")
    bundle = Path(receipt["bundle_path"])
    inventory = dependencies.dependency_inventory(bundle)
    if (set(path.name for path in bundle.iterdir()) != {"node_modules"}
            or inventory != receipt["inventory"]
            or dependencies.dependency_fingerprint(inventory) != receipt["inventory_sha256"]
            or node_inventory(bundle) != inventory or package_versions(bundle) != receipt["packages"]
            or any(not row["mode"] & stat.S_IWUSR for row in inventory["files"])):
        raise ValueError("bootstrap closed dependency inventory drift")
    if dependencies.dependency_fingerprint(node_inventory(Path(q["reference_path"]))) != receipt["source_inventory_sha256"]:
        raise ValueError("bootstrap reference dependency drift")
    return receipt


def provision(receipt_path: Path, candidate: Path, node_binary: str, *, spec_sha256: str,
              summary_sha256: str) -> dict[str, Any]:
    receipt = validate(receipt_path, node_binary, spec_sha256=spec_sha256, summary_sha256=summary_sha256)
    candidate.mkdir(parents=True, exist_ok=True)
    fingerprint = dependencies.copy_dependency_bundle(Path(receipt["bundle_path"]), candidate, receipt["inventory"])
    validate(receipt_path, node_binary, spec_sha256=spec_sha256, summary_sha256=summary_sha256)
    return {"receipt_sha256": dependencies.digest(receipt_path), "initial_inventory_sha256": fingerprint,
            "supplied_paths": ["node_modules"], "application_source_supplied": False,
            "package_manifest_supplied": False, "packages": receipt["packages"]}


def prompt_note(receipt: dict[str, Any]) -> str:
    versions = ", ".join(f"{name} {version}" for name, version in sorted(receipt["packages"].items()))
    return ("A private writable node_modules dependency tree is already present in the candidate: " + versions +
            ". Use node_modules/.bin/tsc and node_modules/.bin/vite locally; no npm install is required. "
            "Author your own package.json, configuration, scripts, server, React UI and tests. "
            "No application implementation or package scripts are supplied.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--qualification-summary", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--node-binary", default="node")
    args = parser.parse_args()
    receipt = prepare(args.reference, args.qualification_summary, args.destination, args.node_binary)
    print(json.dumps({"receipt": str(args.destination / "receipt.json"),
                      "inventory_sha256": receipt["inventory_sha256"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
