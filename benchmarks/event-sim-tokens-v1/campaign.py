#!/usr/bin/env python3
"""Prepare or run a matched, evidence-gated ShiftSim campaign."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import time
import tomllib
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import live_campaign_common as common
import cli_typescript_bootstrap as ts_bootstrap

BENCHMARK = Path(__file__).resolve().parent
sys.path.insert(0, str(BENCHMARK))
from corpus_io import ESCAPED_KEYS_AND_IDENTIFIERS, render_request

REPO = BENCHMARK.parents[1]
MODEL = "claude-sonnet-5-5"
EFFORT = "medium"
ARMS = ("semaprax", "typescript")
MIN_TRIALS_PER_ARM = 5
SEED_FILES = ("/benchmarks/event-sim-tokens-v1/SPEC.md",)
CALIBRATION_PROMPT = "This is a context calibration request. Reply with exactly READY; do not use tools or read files."
PRICE_BOOK_DATE = "2026-10-07"
PRICE_BOOK_SOURCE = "https://platform.claude.com/docs/en/models/sonnet-5-5/overview"
QUALIFICATION_EVIDENCE_SCHEMA = "semaprax.event-sim-qualification-evidence.v2"
QUALIFICATION_EVIDENCE_SCHEMA_V3 = "semaprax.event-sim-qualification-evidence.v3"
QUALIFICATION_EVIDENCE_SCHEMA_V4 = "semaprax.event-sim-qualification-evidence.v4"
QUALIFICATION_BUILD_RECEIPT_SCHEMA = "semaprax.event-sim-qualification-build-receipt.v1"
ACCEPTANCE_REPORT_SCHEMA = "semaprax.event-sim.acceptance-report.v1"
NATIVE_PROJECT_SCHEMA = "semaprax.project.v24"
NATIVE_PROJECT_PROFILE = "language-command-io.stream.v2"
NATIVE_INPUT_ROUTE = "argv-utf8+stdin-stream.v1"
NATIVE_COMMAND_RESULT_TYPE = "i64"
NATIVE_PROCESS_STATUS_RANGE = [0, 255]
NATIVE_PROJECT_ROUTE = {
    "project_schema": NATIVE_PROJECT_SCHEMA,
    "project_profile": NATIVE_PROJECT_PROFILE,
    "input_route": NATIVE_INPUT_ROUTE,
    "command_result_type": NATIVE_COMMAND_RESULT_TYPE,
    "process_status_range": NATIVE_PROCESS_STATUS_RANGE,
}
AUTHORING_PROFILE_V24 = "semaprax-project-v24-stream-v2"
AUTHORING_PROFILE_V27 = "semaprax-project-v27-stream-data-v1"
AUTHORING_PROFILE_V30 = "semaprax-project-v30-owned-data-v1"
AUTHORING_PROFILE_CATALOG_V31 = "semaprax-project-v31-collection-record-v1"
AUTHORING_PROFILE_CATALOG_V32 = "semaprax-project-v32-nested-outcome-v1"
STREAM_DATA_CAPABILITIES = [
    "process.args.read", "process.stderr.write", "process.stdin.read", "process.stdout.write",
]
NATIVE_PROJECT_ROUTE_V27 = {
    "project_schema": "semaprax.project.v27",
    "project_profile": "language-command-io.stream-data.v1",
    "input_route": NATIVE_INPUT_ROUTE,
    "command_result_type": NATIVE_COMMAND_RESULT_TYPE,
    "process_status_range": NATIVE_PROCESS_STATUS_RANGE,
}
AUTHORING_PROFILES = {
    AUTHORING_PROFILE_V24: {
        "rounds": (1, 2), "qualification_schema": QUALIFICATION_EVIDENCE_SCHEMA,
        "campaign_schema": "semaprax.event-sim-campaign.v1",
        "codex_campaign_schema": "semaprax.event-sim-codex-campaign.v1",
        "prompt_schema": "semaprax.event-sim-prompt.v1", "route": NATIVE_PROJECT_ROUTE,
    },
    AUTHORING_PROFILE_V27: {
        "rounds": (3,), "qualification_schema": QUALIFICATION_EVIDENCE_SCHEMA_V3,
        "campaign_schema": "semaprax.event-sim-campaign.v2",
        "codex_campaign_schema": "semaprax.event-sim-codex-campaign.v2",
        "prompt_schema": "semaprax.event-sim-prompt.v2", "route": NATIVE_PROJECT_ROUTE_V27,
    },
    AUTHORING_PROFILE_V30: {
        "rounds": (4,), "qualification_schema": QUALIFICATION_EVIDENCE_SCHEMA_V4,
        "campaign_schema": "semaprax.event-sim-campaign.v3",
        "codex_campaign_schema": "semaprax.event-sim-codex-campaign.v3",
        "prompt_schema": "semaprax.event-sim-prompt.v3",
        "route": {**NATIVE_PROJECT_ROUTE_V27, "project_schema": "semaprax.project.v30",
                  "project_profile": "language-command-io.owned-data.v1"},
    },
}
PINNED_AUTHORING_PROFILES = (AUTHORING_PROFILE_V27, AUTHORING_PROFILE_V30)
# Route-only registry for explicit profiles consumed by the shared manifest
# reader. Catalog v31/v32 routes are not ShiftSim campaign/qualification profiles.
AUTHORING_MANIFEST_ROUTES = {
    profile: data["route"] for profile, data in AUTHORING_PROFILES.items()
}
AUTHORING_MANIFEST_ROUTES[AUTHORING_PROFILE_CATALOG_V31] = {
    **NATIVE_PROJECT_ROUTE_V27,
    "project_schema": "semaprax.project.v31",
    "project_profile": "language-command-io.collection-record.v1",
}
AUTHORING_MANIFEST_ROUTES[AUTHORING_PROFILE_CATALOG_V32] = {
    **NATIVE_PROJECT_ROUTE_V27,
    "project_schema": "semaprax.project.v32",
    "project_profile": "language-command-io.nested-outcome.v1",
}
SPEC_RELATIVE = "benchmarks/event-sim-tokens-v1/SPEC.md"
CORPUS_RELATIVE = "benchmarks/event-sim-tokens-v1/acceptance/corpus.json"
ORACLE_RELATIVE = "benchmarks/event-sim-tokens-v1/oracle.py"
FROZEN_BENCHMARK_SHA256 = {
    SPEC_RELATIVE: "5a8631fc59f55d145bfabb62c8edd3f86164114e3d27b69422031b664b529e00",
    CORPUS_RELATIVE: "3c285999cfcf6a905e885d636ac55ba0ccb0f5999ef5b20ac3a8c17a2e023587",
    ORACLE_RELATIVE: "bdaeb7910f525271445493f3fe651f09c572ed28b132d01c608de5712d7fba01",
}
CURRENT_PRICE_USD_PER_MTOK = {**common.PRICE_USD_PER_MTOK, "cache_read": 0.1}


def sha_text(value: str) -> str:
    import hashlib
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def sha_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def blob_at_commit(repo: Path, commit: str, relative: str) -> bytes:
    result = subprocess.run(["git", "show", f"{commit}:{relative}"], cwd=repo,
                            capture_output=True, check=False)
    if result.returncode:
        raise ValueError(f"pinned compiler commit lacks required file: {relative}")
    return result.stdout


def round_identity(repo: Path, commit: str, round_number: int) -> dict[str, str]:
    if isinstance(round_number, bool) or round_number not in (1, 2, 3, 4):
        raise ValueError(f"unsupported ShiftSim round: {round_number}")
    hashes = {path: sha_bytes(blob_at_commit(repo, commit, path))
              for path in FROZEN_BENCHMARK_SHA256}
    if hashes != FROZEN_BENCHMARK_SHA256:
        raise ValueError(f"round {round_number} requires the unchanged frozen SPEC, corpus, and oracle")
    return hashes


def select_authoring_profile(round_number: int, requested: str | None) -> tuple[str, dict[str, Any]]:
    expected = {3: AUTHORING_PROFILE_V27, 4: AUTHORING_PROFILE_V30}.get(round_number, AUTHORING_PROFILE_V24)
    if requested is None:
        if round_number in (3, 4):
            raise ValueError(f"round {round_number} requires --authoring-profile {expected}")
        requested = expected
    if requested not in AUTHORING_PROFILES:
        raise ValueError(f"unsupported ShiftSim authoring profile: {requested}")
    if requested != expected or round_number not in AUTHORING_PROFILES[requested]["rounds"]:
        raise ValueError(f"round {round_number} requires authoring profile {expected}")
    return requested, AUTHORING_PROFILES[requested]


def _bound_file(reference: Any, evidence_path: Path, label: str) -> tuple[Path, bytes, str]:
    if not isinstance(reference, dict) or not isinstance(reference.get("path"), str):
        raise ValueError(f"qualification evidence must reference {label}")
    path = Path(reference["path"])
    if not path.is_absolute():
        path = evidence_path.parent / path
    if path.is_symlink():
        raise ValueError(f"qualification {label} must not be a symlink")
    path = path.resolve(strict=True)
    if not path.is_file():
        raise ValueError(f"qualification {label} must be a regular file")
    data = path.read_bytes()
    digest = sha_bytes(data)
    if reference.get("sha256") != digest:
        raise ValueError(f"qualification {label} hash does not match the pinned file")
    return path, data, digest


def _sha256_value(value: Any, label: str) -> str:
    if (not isinstance(value, str) or len(value) != 64
            or any(character not in "0123456789abcdef" for character in value)):
        raise ValueError(f"{label} must be a lowercase SHA-256 digest")
    return value


def _manifest_route(data: bytes, authoring_profile: str = AUTHORING_PROFILE_V27) -> dict[str, Any]:
    expected = AUTHORING_MANIFEST_ROUTES[authoring_profile]
    try:
        manifest = tomllib.loads(data.decode("utf-8"))
    except (UnicodeError, tomllib.TOMLDecodeError) as error:
        raise ValueError("candidate manifest is not valid UTF-8 TOML") from error
    package, command, exports, capabilities = (
        manifest.get(name) for name in ("package", "command", "exports", "capabilities"))
    if not all(isinstance(table, dict) for table in (package, command, exports, capabilities)):
        raise ValueError("candidate manifest lacks package, command, exports, or capabilities table")
    function = command.get("function")
    route = {
        "schema": manifest.get("schema"), "profile": package.get("profile"),
        "input": command.get("input"), "function": function,
        "exports_web": exports.get("web"), "capabilities": capabilities.get("required"),
    }
    if (route["schema"] != "semaprax.manifest.v1"
            or route["profile"] != expected["project_profile"]
            or route["input"] != expected["input_route"]
            or not isinstance(function, str) or not function
            or route["exports_web"] != [function]
            or capabilities != {"required": STREAM_DATA_CAPABILITIES}):
        label = {
            AUTHORING_PROFILE_V27: "v27 stream-data",
            AUTHORING_PROFILE_V30: "v30 owned-data",
        }.get(authoring_profile, authoring_profile)
        raise ValueError(f"candidate manifest does not match the exact {label} command route")
    return route


def _validate_closed_inventory_document(inventory: Any) -> list[dict[str, Any]]:
    files = inventory.get("files") if isinstance(inventory, dict) else None
    if (not isinstance(inventory, dict)
            or inventory.get("schema") != "semaprax.closed-authored-inventory.v1"
            or not isinstance(files, list)):
        raise ValueError("candidate source inventory must use the closed authored inventory schema")
    paths = []
    for row in files:
        if (not isinstance(row, dict) or not isinstance(row.get("path"), str) or not row.get("path")
                or Path(row["path"]).is_absolute() or ".." in Path(row["path"]).parts
                or isinstance(row.get("bytes"), bool) or not isinstance(row.get("bytes"), int)
                or row["bytes"] < 0):
            raise ValueError("candidate source inventory contains an invalid file row")
        _sha256_value(row.get("sha256"), "candidate source file hash")
        paths.append(row["path"])
    if paths != sorted(set(paths)):
        raise ValueError("candidate source inventory paths must be unique and sorted")
    encoded = json.dumps(files, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
    if inventory.get("sha256") != sha_bytes(encoded):
        raise ValueError("candidate source inventory closed digest does not match its file rows")
    return files


def candidate_authoring_admission(candidate: Path, arm: str, authoring_profile: str) -> dict[str, Any]:
    if arm != "semaprax" or authoring_profile not in PINNED_AUTHORING_PROFILES:
        return {"status": "not_applicable", "authoring_profile": authoring_profile}
    path = candidate / "semaprax.toml"
    try:
        if path.is_symlink() or not path.is_file():
            version = "v27" if authoring_profile == AUTHORING_PROFILE_V27 else "v30"
            raise ValueError(f"{version} SEMAPRAX candidate must contain a regular semaprax.toml")
        data = path.read_bytes()
        route = _manifest_route(data, authoring_profile)
        return {"status": "passed", "path": str(path), "sha256": sha_bytes(data), "route": route}
    except (OSError, ValueError) as error:
        return {"status": "failed", "path": str(path), "error": str(error)}


def provider_quota_failure(observed: dict[str, Any]) -> dict[str, Any] | None:
    result = observed.get("result_event")
    if not isinstance(result, dict) or result.get("is_error") is not True:
        return None
    if (result.get("api_error_status") != 429
            and result.get("api_error") != "usage_limit_reached"):
        return None
    return {"status": "provider_quota_or_rate_limit", "api_error_status": result.get("api_error_status"),
            "api_error": result.get("api_error"), "source": "provider_result_event"}


def acceptance_case_request(case: dict[str, Any], kind: str) -> bytes:
    return render_request(case, kind)


def validate_qualification_evidence(
    evidence_path: Path,
    repo: Path,
    commit: str,
    semaprax_binary_sha256: str | None,
    authoring_profile: str = AUTHORING_PROFILE_V24,
) -> dict[str, Any]:
    """Bind a scored campaign to reviewed native acceptance evidence."""
    if authoring_profile == AUTHORING_PROFILE_V30 and (evidence_path.is_symlink() or not evidence_path.is_file()):
        raise ValueError("v30 qualification evidence must be a regular JSON file")
    evidence_path = evidence_path.expanduser().resolve(strict=True)
    if not evidence_path.is_file():
        raise ValueError("qualification evidence must be a regular JSON file")
    try:
        evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("qualification evidence is not valid UTF-8 JSON") from error
    profile = AUTHORING_PROFILES.get(authoring_profile)
    if profile is None:
        raise ValueError(f"unsupported ShiftSim authoring profile: {authoring_profile}")
    expected_schema = profile["qualification_schema"]
    if not isinstance(evidence, dict) or evidence.get("schema") != expected_schema:
        raise ValueError(f"qualification evidence schema must be {expected_schema}")

    expected_spec = sha_bytes(blob_at_commit(repo, commit, SPEC_RELATIVE))
    expected_corpus_bytes = blob_at_commit(repo, commit, CORPUS_RELATIVE)
    expected_corpus = sha_bytes(expected_corpus_bytes)
    local_corpus = (BENCHMARK / "acceptance" / "corpus.json").read_bytes()
    if sha_bytes(local_corpus) != expected_corpus:
        raise ValueError("local acceptance corpus differs from the pinned compiler commit")
    if evidence.get("spec_sha256") != expected_spec:
        raise ValueError("qualification evidence SPEC hash does not match the pinned compiler commit")
    if evidence.get("acceptance_corpus_sha256") != expected_corpus:
        raise ValueError("qualification evidence corpus hash does not match the pinned compiler commit")
    if evidence.get("compiler_source_commit") != commit:
        raise ValueError("qualification evidence compiler source commit does not match --base-ref")
    binary_hash = evidence.get("compiler_binary_sha256")
    if not isinstance(binary_hash, str) or len(binary_hash) != 64 or any(c not in "0123456789abcdef" for c in binary_hash):
        raise ValueError("qualification evidence must contain a lowercase SHA-256 compiler binary hash")
    if semaprax_binary_sha256 is not None and binary_hash != semaprax_binary_sha256:
        raise ValueError("qualification evidence compiler binary hash does not match --semaprax-bin")
    route = evidence.get("native_project_route")
    if route != profile["route"]:
        raise ValueError("qualification evidence must identify the native streaming Project and input routes")

    report_ref = evidence.get("acceptance_report")
    if not isinstance(report_ref, dict) or not isinstance(report_ref.get("path"), str):
        raise ValueError("qualification evidence must reference a per-case acceptance report")
    report_path = Path(report_ref["path"])
    if not report_path.is_absolute():
        report_path = evidence_path.parent / report_path
    report_path = report_path.resolve(strict=True)
    if not report_path.is_file():
        raise ValueError("qualification acceptance report must be a regular file")
    report_bytes = report_path.read_bytes()
    report_hash = sha_bytes(report_bytes)
    if report_ref.get("sha256") != report_hash:
        raise ValueError("qualification acceptance report hash does not match the pinned report")
    try:
        report = json.loads(report_bytes.decode("utf-8"))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("qualification acceptance report is not valid UTF-8 JSON") from error
    if (not isinstance(report, dict) or report.get("schema") != ACCEPTANCE_REPORT_SCHEMA
            or report.get("status") != "passed" or report.get("corpus_sha256") != expected_corpus):
        raise ValueError("qualification acceptance report does not prove the pinned corpus passed")

    corpus = json.loads(expected_corpus_bytes.decode("utf-8"))
    expected_cases = [("valid", case) for case in corpus["valid"]]
    expected_cases.extend(("invalid", case) for case in corpus["invalid"])
    rows = report.get("cases")
    if not isinstance(rows, list) or len(rows) != len(expected_cases):
        raise ValueError("qualification report must contain every pinned acceptance case")
    for row, (kind, case) in zip(rows, expected_cases):
        request = acceptance_case_request(case, kind)
        expected_output = (
            json.dumps(case["expected"], ensure_ascii=False, separators=(",", ":")).encode("utf-8") + b"\n"
            if kind == "valid" else b""
        )
        expected_exit = 0 if kind == "valid" else 2
        if (not isinstance(row, dict) or row.get("name") != case.get("name")
                or row.get("kind") != kind or row.get("status") != "passed"
                or row.get("input_encoding") != case.get("request_encoding", "compact")
                or row.get("input_bytes") != len(request) or row.get("input_sha256") != sha_bytes(request)
                or row.get("expected_exit_code") != expected_exit or row.get("exit_code") != expected_exit
                or row.get("stdout_sha256") != sha_bytes(expected_output)
                or row.get("expected_stdout_sha256") != sha_bytes(expected_output)):
            raise ValueError(f"qualification acceptance case did not pass exactly: {case.get('name')}")
        if kind == "invalid" and row.get("stderr_one_diagnostic_line") is not True:
            raise ValueError(f"qualification invalid case lacks exactly one diagnostic line: {case.get('name')}")
    if report.get("valid_cases") != len(corpus["valid"]) or report.get("invalid_cases") != len(corpus["invalid"]):
        raise ValueError("qualification acceptance report case counts do not match the pinned corpus")
    rows_by_name = {row.get("name"): row for row in rows if isinstance(row, dict)}
    large_whitespace = rows_by_name.get("large-leading-whitespace")
    compact_max = rows_by_name.get("max-cardinality-compact")
    escaped_max = rows_by_name.get("max-cardinality-escaped-keys-and-ids")
    cases_by_name = {case["name"]: case for case in corpus["valid"]}
    compact_case = cases_by_name.get("max-cardinality-compact")
    escaped_case = cases_by_name.get("max-cardinality-escaped-keys-and-ids")
    if (large_whitespace is None or large_whitespace.get("leading_whitespace_bytes") != 65_537
            or large_whitespace.get("input_bytes", 0) <= 65_536):
        raise ValueError("qualification evidence must pass the >65,536-byte whitespace case")
    if (compact_max is None or compact_max.get("input_bytes", 65_537) > 65_536
            or escaped_max is None or escaped_max.get("input_encoding") != ESCAPED_KEYS_AND_IDENTIFIERS
            or escaped_max.get("input_bytes", 0) <= 65_536
            or compact_case is None or escaped_case is None
            or compact_case.get("input") != escaped_case.get("input")
            or len(compact_case.get("input", {}).get("servers", [])) != 8
            or len(compact_case.get("input", {}).get("patients", [])) != 256
            or compact_max.get("expected_stdout_sha256") != escaped_max.get("expected_stdout_sha256")):
        raise ValueError("qualification evidence must pass escaped and compact maximum-cardinality requests")

    source_binding = {}
    if authoring_profile in PINNED_AUTHORING_PROFILES:
        source = evidence.get("candidate_source")
        if not isinstance(source, dict):
            version = "v3" if authoring_profile == AUTHORING_PROFILE_V27 else "v4"
            raise ValueError(f"{version} qualification evidence must bind candidate source inventory and manifest")
        inventory_path, inventory_bytes, inventory_hash = _bound_file(
            source.get("inventory"), evidence_path, "candidate source inventory")
        manifest_path, manifest_bytes, manifest_hash = _bound_file(
            source.get("manifest"), evidence_path, "candidate manifest")
        try:
            inventory = json.loads(inventory_bytes.decode("utf-8"))
        except (UnicodeError, json.JSONDecodeError) as error:
            raise ValueError("candidate source inventory is not valid UTF-8 JSON") from error
        files = _validate_closed_inventory_document(inventory)
        manifest_rows = [row for row in files or []
                         if isinstance(row, dict) and row.get("path") == "semaprax.toml"]
        if (len(manifest_rows) != 1 or manifest_rows[0].get("sha256") != manifest_hash
                or manifest_rows[0].get("bytes") != len(manifest_bytes)):
            raise ValueError("candidate source inventory must bind semaprax.toml exactly")
        _manifest_route(manifest_bytes, authoring_profile)
        receipt_path, receipt_bytes, receipt_hash = _bound_file(
            evidence.get("qualification_build_receipt"), evidence_path,
            "qualification build receipt")
        native_path, _native_bytes, native_hash = _bound_file(
            evidence.get("qualified_native_binary"), evidence_path,
            "qualified native binary")
        try:
            receipt = json.loads(receipt_bytes.decode("utf-8"))
        except (UnicodeError, json.JSONDecodeError) as error:
            raise ValueError("qualification build receipt is not valid UTF-8 JSON") from error
        subject = evidence.get("qualification_subject")
        expected_subject = {
            "compiler_source_commit": commit,
            "compiler_binary_sha256": binary_hash,
            "closed_authored_inventory_sha256": inventory["sha256"],
            "candidate_manifest_sha256": manifest_hash,
            "native_binary_sha256": native_hash,
        }
        if subject != expected_subject:
            version = "v3" if authoring_profile == AUTHORING_PROFILE_V27 else "v4"
            raise ValueError(f"{version} qualification subject does not bind compiler, source, manifest, and native binary")
        if (not isinstance(receipt, dict)
                or receipt.get("schema") != QUALIFICATION_BUILD_RECEIPT_SCHEMA
                or receipt.get("qualification_subject") != expected_subject
                or receipt.get("acceptance_report_sha256") != report_hash):
            raise ValueError("qualification build receipt does not bind the accepted report to the same subject")
        source_binding = {
            "candidate_source_inventory_path": str(inventory_path),
            "candidate_source_inventory_sha256": inventory_hash,
            "closed_authored_inventory_sha256": inventory["sha256"],
            "candidate_manifest_path": str(manifest_path),
            "candidate_manifest_sha256": manifest_hash,
            "qualification_build_receipt_path": str(receipt_path),
            "qualification_build_receipt_sha256": receipt_hash,
            "qualified_native_binary_path": str(native_path),
            "qualified_native_binary_sha256": native_hash,
            "qualification_subject": expected_subject,
        }

    return {
        "status": "evidence_gate_passed",
        "scored_trials_allowed": True,
        "issue_611_status": "open; no issue closure is asserted by this evidence",
        "evidence_path": str(evidence_path),
        "evidence_sha256": sha_bytes(evidence_path.read_bytes()),
        "spec_sha256": expected_spec,
        "acceptance_corpus_sha256": expected_corpus,
        "acceptance_report_path": str(report_path),
        "acceptance_report_sha256": report_hash,
        "acceptance_cases_passed": len(rows),
        "compiler_source_commit": commit,
        "compiler_binary_sha256": binary_hash,
        "native_project_route": route,
        "authoring_profile": authoring_profile,
        **source_binding,
    }


def resolve_commit(repo: Path, ref: str) -> str:
    result = subprocess.run(["git", "rev-parse", "--verify", f"{ref}^{{commit}}"], cwd=repo,
                            text=True, capture_output=True, check=False)
    if result.returncode:
        raise ValueError(f"base ref does not resolve to a commit: {ref}")
    return result.stdout.strip()


def copy_qualification_artifacts(qualification: dict[str, Any], artifacts: Path) -> None:
    """Copy validated evidence without allowing plan-to-launch drift."""
    bindings = (
        ("evidence_path", "evidence_sha256", "evidence_artifact", "qualification-evidence.json"),
        ("acceptance_report_path", "acceptance_report_sha256", "acceptance_report_artifact",
         "qualification-acceptance-report.json"),
        ("candidate_source_inventory_path", "candidate_source_inventory_sha256",
         "candidate_source_inventory_artifact", "qualification-candidate-source-inventory.json"),
        ("candidate_manifest_path", "candidate_manifest_sha256", "candidate_manifest_artifact",
         "qualification-candidate-semaprax.toml"),
        ("qualification_build_receipt_path", "qualification_build_receipt_sha256",
         "qualification_build_receipt_artifact", "qualification-build-receipt.json"),
        ("qualified_native_binary_path", "qualified_native_binary_sha256",
         "qualified_native_binary_artifact", "qualification-native-binary"),
    )
    for path_key, hash_key, artifact_key, name in bindings:
        source_value = qualification.get(path_key)
        if source_value is None:
            continue
        source = Path(source_value)
        expected = qualification.get(hash_key)
        if expected is None or source.is_symlink() or not source.is_file() or common.digest(source) != expected:
            raise ValueError(f"qualified artifact drifted before copy: {path_key}")
        destination = artifacts / name
        shutil.copyfile(source, destination)
        if common.digest(source) != expected or common.digest(destination) != expected:
            raise ValueError(f"qualified artifact drifted during copy: {path_key}")
        qualification[artifact_key] = str(destination)


def generate_v3_qualification(
    candidate: Path,
    semaprax_bin: Path,
    repo: Path,
    compiler_commit: str,
    output: Path,
    timeout: int,
    *, authoring_profile: str = AUTHORING_PROFILE_V27,
) -> dict[str, Any]:
    """Build and accept one immutable subject; historical callers retain v27/v3."""
    if authoring_profile not in PINNED_AUTHORING_PROFILES:
        raise ValueError("qualification builder requires an explicit pinned authoring profile")
    profile = AUTHORING_PROFILES[authoring_profile]
    candidate = candidate.expanduser().resolve(strict=True)
    semaprax_bin = semaprax_bin.expanduser().resolve(strict=True)
    repo = repo.expanduser().resolve(strict=True)
    output = output.expanduser().resolve()
    if timeout <= 0:
        raise ValueError("qualification timeout must be positive")
    if output.exists():
        raise ValueError(f"qualification output must not already exist: {output}")
    if not semaprax_bin.is_file() or semaprax_bin.is_symlink():
        raise ValueError("qualification compiler must be a regular file")
    compiler_hash = common.digest(semaprax_bin)
    for protected in (repo, candidate):
        try:
            output.relative_to(protected)
        except ValueError:
            continue
        raise ValueError("qualification output must be outside the repository and candidate")
    admission = candidate_authoring_admission(candidate, "semaprax", authoring_profile)
    if admission["status"] != "passed":
        raise ValueError(admission.get("error", "candidate failed pinned manifest admission"))
    inventory = closed_authored_inventory(candidate)
    output.mkdir(parents=True)
    inventory_path = output / "candidate-source-inventory.json"
    inventory_path.write_text(json.dumps(inventory, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    manifest_path = output / "candidate-semaprax.toml"
    shutil.copyfile(candidate / "semaprax.toml", manifest_path)
    manifest_hash = common.digest(manifest_path)
    if manifest_hash != admission["sha256"] or closed_authored_inventory(candidate) != inventory:
        raise ValueError("candidate source changed while qualification artifacts were staged")
    native_path = output / "qualified-native-binary"
    env = trial_environment(semaprax_bin)
    for command in (
        [str(semaprax_bin), "check", "--manifest-path", str(candidate / "semaprax.toml")],
        [str(semaprax_bin), "build", "--manifest-path", str(candidate / "semaprax.toml"),
         "--target", "native", "--output", str(native_path)],
    ):
        try:
            completed = subprocess.run(command, cwd=candidate, capture_output=True, check=False,
                                       timeout=timeout, env=env)
        except subprocess.TimeoutExpired as error:
            if (closed_authored_inventory(candidate) != inventory or semaprax_bin.is_symlink()
                    or not semaprax_bin.is_file() or common.digest(semaprax_bin) != compiler_hash):
                raise ValueError("candidate source or pinned compiler changed during timed-out qualification build") from error
            raise ValueError("qualification compiler command timed out") from error
        if completed.returncode:
            raise ValueError(f"qualification compiler command failed: {common.bounded_text(completed.stderr)}")
        if (closed_authored_inventory(candidate) != inventory or semaprax_bin.is_symlink()
                or not semaprax_bin.is_file() or common.digest(semaprax_bin) != compiler_hash):
            raise ValueError("candidate source or pinned compiler changed during qualification build")
    if native_path.is_symlink() or not native_path.is_file():
        raise ValueError("qualification build did not produce a regular native binary")
    native_hash = common.digest(native_path)
    report_path = output / "acceptance-report.json"
    runner = BENCHMARK / "acceptance" / "run.py"
    try:
        accepted = subprocess.run(
            [sys.executable, str(runner), "--command-json", json.dumps([str(native_path)]),
             "--report-json", str(report_path)],
            cwd=candidate, capture_output=True, check=False, timeout=timeout, env=env)
    except subprocess.TimeoutExpired as error:
        if (closed_authored_inventory(candidate) != inventory or native_path.is_symlink()
                or not native_path.is_file() or common.digest(native_path) != native_hash
                or semaprax_bin.is_symlink() or not semaprax_bin.is_file()
                or common.digest(semaprax_bin) != compiler_hash):
            raise ValueError("qualification source, compiler, or native binary changed during timed-out acceptance") from error
        raise ValueError("qualification acceptance timed out") from error
    if accepted.returncode:
        raise ValueError(f"qualification acceptance failed: {common.bounded_text(accepted.stderr)}")
    if report_path.is_symlink() or not report_path.is_file():
        raise ValueError("qualification acceptance did not produce a regular report")
    if (closed_authored_inventory(candidate) != inventory or native_path.is_symlink()
            or not native_path.is_file() or common.digest(native_path) != native_hash
            or semaprax_bin.is_symlink() or not semaprax_bin.is_file()
            or common.digest(semaprax_bin) != compiler_hash):
        raise ValueError("qualification source, compiler, or native binary changed during acceptance")
    report_hash = common.digest(report_path)
    subject = {
        "compiler_source_commit": compiler_commit,
        "compiler_binary_sha256": compiler_hash,
        "closed_authored_inventory_sha256": inventory["sha256"],
        "candidate_manifest_sha256": manifest_hash,
        "native_binary_sha256": native_hash,
    }
    receipt_path = output / "qualification-build-receipt.json"
    receipt_path.write_text(json.dumps({
        "schema": QUALIFICATION_BUILD_RECEIPT_SCHEMA,
        "qualification_subject": subject,
        "acceptance_report_sha256": report_hash,
    }, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    evidence_path = output / "qualification-evidence.json"
    evidence = {
        "schema": profile["qualification_schema"],
        "spec_sha256": sha_bytes(blob_at_commit(repo, compiler_commit, SPEC_RELATIVE)),
        "acceptance_corpus_sha256": sha_bytes(blob_at_commit(repo, compiler_commit, CORPUS_RELATIVE)),
        "compiler_source_commit": compiler_commit,
        "compiler_binary_sha256": compiler_hash,
        "native_project_route": profile["route"],
        "acceptance_report": {"path": str(report_path), "sha256": report_hash},
        "candidate_source": {
            "inventory": {"path": str(inventory_path), "sha256": common.digest(inventory_path)},
            "manifest": {"path": str(manifest_path), "sha256": manifest_hash},
        },
        "qualification_subject": subject,
        "qualification_build_receipt": {
            "path": str(receipt_path), "sha256": common.digest(receipt_path)},
        "qualified_native_binary": {"path": str(native_path), "sha256": native_hash},
    }
    evidence_path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    validate_qualification_evidence(
        evidence_path, repo, compiler_commit, compiler_hash, authoring_profile)
    return {"status": "qualified", "evidence": str(evidence_path),
            "qualification_subject": subject}


def trial_environment(semaprax_bin: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["SEMAPRAX_BIN"] = str(semaprax_bin)
    env["PATH"] = str(semaprax_bin.parent) + os.pathsep + env.get("PATH", "")
    return env


def require_authoring_eligibility(settings: dict[str, Any], semaprax_bin: Path) -> None:
    """V30 dispatch replays the full closed qualification, never a version guess."""
    if settings.get("authoring_profile") != AUTHORING_PROFILE_V30:
        return
    qualification = settings.get("qualification", {})
    if (settings.get("round") != 4 or qualification.get("scored_trials_allowed") is not True
            or settings.get("native_project_route") != AUTHORING_PROFILES[AUTHORING_PROFILE_V30]["route"]
            or not isinstance(qualification.get("evidence_path"), str)
            or not isinstance(settings.get("qualification_repository"), str)
            or not isinstance(settings.get("repository_commit"), str)):
        raise ValueError("v30 authoring requires fresh source/binary/profile-bound all-15 qualification")
    if semaprax_bin.is_symlink() or not semaprax_bin.is_file():
        raise ValueError("v30 authoring compiler must remain a regular file")
    fresh = validate_qualification_evidence(
        Path(qualification["evidence_path"]), Path(settings["qualification_repository"]),
        settings["repository_commit"], common.digest(semaprax_bin), AUTHORING_PROFILE_V30)
    copied_bindings = {
        "evidence_artifact": "evidence_sha256",
        "acceptance_report_artifact": "acceptance_report_sha256",
        "candidate_source_inventory_artifact": "candidate_source_inventory_sha256",
        "candidate_manifest_artifact": "candidate_manifest_sha256",
        "qualification_build_receipt_artifact": "qualification_build_receipt_sha256",
        "qualified_native_binary_artifact": "qualified_native_binary_sha256",
    }
    if fresh != {key: value for key, value in qualification.items() if key not in copied_bindings}:
        raise ValueError("v30 qualification changed after the immutable campaign plan")
    for key, hash_key in copied_bindings.items():
        if key in qualification:
            copy = Path(qualification[key])
            if copy.is_symlink() or not copy.is_file() or common.digest(copy) != fresh[hash_key]:
                raise ValueError("v30 retained qualification copy changed before dispatch")


def prompt_for(arm: str, candidate: Path, semaprax_bin: Path,
               authoring_profile: str = AUTHORING_PROFILE_V24) -> str:
    if authoring_profile == AUTHORING_PROFILE_V30:
        historical = prompt_for(arm, candidate, semaprax_bin, AUTHORING_PROFILE_V27)
        if arm == "typescript":
            return historical
        old_guidance = (
            "Private helpers may borrow `Vec<T>` only when T is a Copy scalar (i64, i32, u8, usize, char, f32, f64, or "
            "bool). Do not expose owned or returned Vec values, borrowed Vec values at public roots, "
            "or any Vec in the command ABI. ")
        new_guidance = (
            "Private helpers may use the v30 admitted owned leaf collections and flat records; "
            "the external command ABI still contains no Vec or nominal value. Read "
            "`$SEMAPRAX_BIN help language author:owned-data` for exact shapes, operations, "
            "ownership, and unchanged Bytes allocation/clone loop restrictions. Built-in checked "
            "source derivation is optional; all generated source remains subject to ordinary checks. ")
        return historical.replace("native Project v27 manifest", "native Project v30 manifest").replace(
            "`language-command-io.stream-data.v1`", "`language-command-io.owned-data.v1`").replace(
            old_guidance, new_guidance)
    language = "SEMAPRAX native Project" if arm == "semaprax" else "TypeScript on Node.js"
    prompt = f"""Implement a complete, idiomatic ShiftSim application in {language} according to
`benchmarks/event-sim-tokens-v1/SPEC.md`. The whole implementation belongs under
`{candidate}`. Do not change compiler or benchmark files. The checkout contains
only the public specification; the independent acceptance corpus and oracle are
not in your workspace.

Create executable `build.sh`, `run.sh`, and `test.sh`. `build.sh` must compile
or validate the implementation without network access. `run.sh` must accept
one JSON request on stdin, emit the exact report plus one newline on stdout,
and produce the specified status-2 diagnostic for invalid requests. `test.sh`
must run your own automated tests and fail nonzero on errors. The SEMAPRAX arm
must use a native Project v24 manifest and the compiler at `{semaprax_bin}`
(also available as `$SEMAPRAX_BIN`), with Project profile
`language-command-io.stream.v2`, input `argv-utf8+stdin-stream.v1`, and an
explicit command function returning `i64` process status in the range 0..255.
Return 0 for valid requests and 2 for invalid requests, writing the specified
diagnostic to stderr and nothing to stdout for invalid input. Do not use a
failed contract or runtime failure to represent ordinary invalid input. Build
the native command with `$SEMAPRAX_BIN build --manifest-path semaprax.toml
--target native --output dist/shiftsim`, and have `run.sh` execute that native
binary. `semaprax run` executes the ordinary Project entry and is not the
selected command process adapter. Streaming operations are documented by
`$SEMAPRAX_BIN help language`. The TypeScript arm must use Node from `PATH` and
provide the same stdin and process status behavior. Built-in
language/compiler/runtime help is available to either arm.
The specification explicitly has no raw-input byte limit for JSON whitespace;
the hidden acceptance corpus includes a valid request over 65,536 bytes due to
leading whitespace.

Do not access material outside the public specification and your candidate
directory, except built-in language/compiler/runtime help. Finish by listing files written and stating whether the implementation
is complete.
"""
    if authoring_profile == AUTHORING_PROFILE_V24:
        return prompt
    if authoring_profile != AUTHORING_PROFILE_V27:
        raise ValueError(f"unsupported ShiftSim authoring profile: {authoring_profile}")
    prompt = prompt.replace("native Project v24 manifest", "native Project v27 manifest")
    prompt = prompt.replace("`language-command-io.stream.v2`", "`language-command-io.stream-data.v1`")
    prompt = prompt.replace("`$SEMAPRAX_BIN help language`", "`$SEMAPRAX_BIN help language projects`")
    marker = "The TypeScript arm must use Node from `PATH`"
    extension = (
        "For the SEMAPRAX arm, keep the external command ABI exactly `fn() -> i64` and the "
        "manifest capabilities exactly `process.args.read`, `process.stderr.write`, "
        "`process.stdin.read`, and `process.stdout.write` in sorted order. Private helpers may "
        "borrow `Vec<T>` only when T is a Copy scalar (i64, i32, u8, usize, char, f32, f64, or "
        "bool). Do not expose owned or returned Vec values, borrowed Vec values at public roots, "
        "or any Vec in the command ABI. "
    )
    return prompt.replace(marker, extension + marker)


def retain_fixed_harness_context(row: dict[str, Any], settings: dict[str, Any], prompt: str) -> None:
    if settings.get("authoring_profile") == AUTHORING_PROFILE_V30:
        row["fixed_harness_context"] = {
            "prompt_sha256": sha_text(prompt), "prompt_utf8_bytes": len(prompt.encode("utf-8")),
            "tokens": None, "actual_billed_usd": None,
            "scope": "full retained harness/task prompt including paths and any TS bootstrap note; excludes authored/generated source",
        }


def plan(args: argparse.Namespace, semaprax_binary_sha256: str | None = None) -> dict[str, Any]:
    repo = Path(args.repo).resolve(strict=True)
    commit = resolve_commit(repo, args.base_ref)
    round_number = getattr(args, "round", 1)
    benchmark_hashes = round_identity(repo, commit, round_number)
    authoring_profile, profile = select_authoring_profile(
        round_number, getattr(args, "authoring_profile", None))
    artifacts = Path(args.artifacts).expanduser().resolve()
    try:
        artifacts.relative_to(repo)
    except ValueError:
        pass
    else:
        raise ValueError("artifact directory must be outside the repository")
    if artifacts.exists():
        raise ValueError(f"artifact path must not already exist: {artifacts}")
    if args.trials_per_arm < MIN_TRIALS_PER_ARM:
        raise ValueError(f"at least {MIN_TRIALS_PER_ARM} trials per arm are required")
    if args.timeout_seconds <= 0 or (args.max_budget_usd is not None and args.max_budget_usd <= 0):
        raise ValueError("timeout and optional max budget must be positive")
    if args.model != MODEL or args.effort != EFFORT:
        raise ValueError(f"matched ShiftSim pins --model {MODEL} and --effort {EFFORT}")
    tokenizer = common.tokenizer_metadata(getattr(args, "tokenizer_dir", None))
    typescript_tooling = None
    receipt_arg = getattr(args, "typescript_bootstrap_receipt", None)
    if receipt_arg:
        receipt_path = Path(receipt_arg).expanduser()
        if receipt_path.is_symlink():
            raise ValueError("TypeScript bootstrap receipt must not be a symlink")
        resolved_receipt = receipt_path.resolve(strict=True)
        if receipt_path.absolute() != resolved_receipt:
            raise ValueError("TypeScript bootstrap receipt path must not traverse symlinked parents")
        receipt_path = resolved_receipt
        receipt = ts_bootstrap.validate(receipt_path, getattr(args, "node_binary", "node"),
                                        getattr(args, "npm_binary", "npm"))
        typescript_tooling = {"receipt_path": str(receipt_path), "receipt_sha256": common.digest(receipt_path),
                              "inventory_sha256": receipt["inventory_sha256"], "runtime": receipt["runtime"],
                              "package_json_sha256": receipt["package_json_sha256"],
                              "package_lock_sha256": receipt["package_lock_sha256"],
                              "packages": receipt["packages"], "helper_sha256": receipt["helper_sha256"],
                              "dependency_helper_sha256": receipt["dependency_helper_sha256"],
                              "node_binary": getattr(args, "node_binary", "node"),
                              "npm_binary": getattr(args, "npm_binary", "npm")}
    evidence_argument = getattr(args, "qualification_evidence", None)
    if authoring_profile == AUTHORING_PROFILE_V30 and evidence_argument is None:
        raise ValueError("v30 authoring requires fresh source/binary/profile-bound all-15 qualification")
    if evidence_argument is not None:
        if semaprax_binary_sha256 is None:
            binary_argument = getattr(args, "semaprax_bin", None)
            if binary_argument is None:
                raise ValueError("qualification evidence requires --semaprax-bin so its binary hash can be bound")
            binary_path = Path(binary_argument).expanduser().resolve(strict=True)
            if not binary_path.is_file():
                raise ValueError("semaprax-bin must be a regular file")
            semaprax_binary_sha256 = common.digest(binary_path)
        qualification = validate_qualification_evidence(
            Path(evidence_argument), repo, commit, semaprax_binary_sha256, authoring_profile,
        )
    else:
        qualification = {
            "status": "preflight_not_qualified",
            "scored_trials_allowed": False,
            "blocking_issue": 611,
            "issue_611_status": "open",
            "reason": "no pinned native streaming qualification evidence was supplied",
        }
    rounds = [arm for i in range(args.trials_per_arm)
              for arm in (ARMS if i % 2 == 0 else tuple(reversed(ARMS)))]
    return {
        "schema": profile["campaign_schema"],
        "created_at_utc": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "benchmark": "event-sim-tokens-v1",
        "round": round_number,
        "authoring_profile": authoring_profile,
        "prompt_schema": profile["prompt_schema"],
        "benchmark_inputs_sha256": benchmark_hashes,
        "seed_files_sha256": {SPEC_RELATIVE: benchmark_hashes[SPEC_RELATIVE]},
        "repository_commit": commit,
        "artifacts": str(artifacts),
        "model": args.model,
        "effort": args.effort,
        "trials_per_arm": args.trials_per_arm,
        "attempt_denominator": 2 * args.trials_per_arm,
        "timeout_seconds": args.timeout_seconds,
        "max_budget_usd": args.max_budget_usd,
        "authored_source_tokenizer": tokenizer,
        "typescript_bootstrap": typescript_tooling,
        "typescript_setup_context_tokens": None,
        "arms": list(ARMS),
        "trial_order": rounds,
        "native_project_route": profile["route"],
        "qualification": qualification,
        **({"qualification_repository": str(repo), "language_setup": {
            "schema": "semaprax.event-sim-language-setup.v1",
            "semaprax": {"authoring_profile": authoring_profile,
                         "qualification_schema": QUALIFICATION_EVIDENCE_SCHEMA_V4,
                         "fixed_harness_context_tokens": None},
            "typescript": {"prompt_condition": AUTHORING_PROFILE_V27,
                           "node_bootstrap": "unchanged strong baseline",
                           "fixed_harness_context_tokens": None},
            "comparison": "SEM profile/help context changes from round 3; TS prompt is identical; no causal cost claim",
            "accounting": "retain exact per-arm prompt bytes and hash separately from authored/generated source; tokens and billing unknown",
        }} if authoring_profile == AUTHORING_PROFILE_V30 else {}),
        "price_book": {
            "date": "2026-10-08" if round_number >= 2 else PRICE_BOOK_DATE,
            "source_url": PRICE_BOOK_SOURCE,
            "per_million_tokens": CURRENT_PRICE_USD_PER_MTOK if round_number >= 2 else common.PRICE_USD_PER_MTOK,
            "claim": "list-price estimate; provider result cost is reported separately and is not a billing receipt",
        },
        "calibration": {
            "mode": "matched empty-task one-turn calibration before trials",
            "prompt": CALIBRATION_PROMPT,
            "prompt_sha256": sha_text(CALIBRATION_PROMPT),
            "usage_convention": "diagnostic only; never subtracted from raw trial usage",
        },
        "seed_checkout": {
            "mode": "new parentless one-commit repository and detached worktree per calibration/trial",
            "included_files": list(SEED_FILES),
            "candidate_path": "benchmarks/event-sim-tokens-v1/candidate/",
            "excluded": "all other source files, hidden acceptance corpus/oracle, and original repository history/objects",
        },
    }


def single_arm_preflight_plan(
    args: argparse.Namespace,
    arm: str,
    semaprax_binary_sha256: str | None = None,
) -> dict[str, Any]:
    """Make a one-trial exploratory plan without relaxing scored-run limits."""
    if arm not in ARMS:
        raise ValueError(f"preflight arm must be one of: {', '.join(ARMS)}")
    plan_args = argparse.Namespace(**vars(args))
    plan_args.trials_per_arm = MIN_TRIALS_PER_ARM
    settings = plan(plan_args, semaprax_binary_sha256)
    settings.update({
        "campaign_kind": "single_arm_preflight",
        "trials_per_arm": 1,
        "attempt_denominator": 1,
        "arms": [arm],
        "trial_order": [arm],
        "qualification": {
            "status": "preflight_not_qualified",
            "scored_trials_allowed": False,
            "blocking_issue": 611,
            "issue_611_status": "open",
            "reason": "a single-arm preflight is exploratory and cannot be scored as a matched comparison",
        },
    })
    return settings


def run_process(command: list[str], cwd: Path, env: dict[str, str], stdout: Path, stderr: Path,
                timeout: int) -> dict[str, Any]:
    return common.run_claude(command, cwd, env, stdout, stderr, timeout)


def launch_calibration(seed_repo: Path, artifacts: Path, seed_commit: str,
                       settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    require_authoring_eligibility(settings, semaprax_bin)
    workspace = artifacts / "worktrees" / "calibration"
    error = common.add_seed_worktree(seed_repo, workspace, seed_commit, SEED_FILES)
    row: dict[str, Any] = {"status": "failed", "failure": error}
    if error:
        return row
    prompt = CALIBRATION_PROMPT
    stream, stderr = artifacts / "calibration.jsonl", artifacts / "calibration.stderr.txt"
    require_authoring_eligibility(settings, semaprax_bin)
    process = run_process(common.claude_command(settings, prompt), workspace,
                          trial_environment(semaprax_bin), stream, stderr, settings["timeout_seconds"])
    usage = common.stream_usage(stream, settings["model"]) if stream.exists() else {"usage": {}, "models_observed": []}
    row.update(process)
    row["observed"] = usage
    row["observed_model_id"] = usage["models_observed"][0] if len(usage["models_observed"]) == 1 else None
    row["status"] = "ready" if process["process_exit_code"] == 0 and row["observed_model_id"] else "failed"
    row["list_price_estimate_usd"] = common.rate_card_estimate_details(
        usage.get("usage", {}), settings.get("price_book", {}).get("per_million_tokens"),
    )["usd"]
    common.save_json(artifacts / "calibration.json", row)
    return row


def closed_authored_inventory(candidate: Path, *, exclude_verified_node_modules: bool = False,
                              additional_suffixes: frozenset[str] = frozenset()) -> dict[str, Any]:
    """Hash retained authored files without following candidate-controlled links."""
    if not candidate.exists() and not candidate.is_symlink():
        files: list[dict[str, Any]] = []
        encoded = json.dumps(files, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
        return {"schema": "semaprax.closed-authored-inventory.v1", "files": files,
                "sha256": sha_bytes(encoded)}
    if candidate.is_symlink() or not candidate.is_dir():
        raise ValueError("candidate must be a regular directory")
    files = []
    for directory, names, filenames in os.walk(candidate, topdown=True, followlinks=False):
        root = Path(directory)
        retained_names = []
        for name in sorted(names):
            path = root / name
            relative = path.relative_to(candidate)
            excluded_names = common.AUTHORED_EXCLUDED_DIRS - (
                {"node_modules", ".cache"} if exclude_verified_node_modules else set())
            if (name in excluded_names
                    or (name == "node_modules" and exclude_verified_node_modules
                        and relative.as_posix() == "node_modules")):
                if path.is_symlink() or not stat.S_ISDIR(path.stat(follow_symlinks=False).st_mode):
                    raise ValueError(f"excluded candidate directory must not bridge outside: {path.relative_to(candidate)}")
                continue
            if path.is_symlink():
                raise ValueError(f"retained candidate path must not be a symlink: {path.relative_to(candidate)}")
            if not stat.S_ISDIR(path.stat(follow_symlinks=False).st_mode):
                raise ValueError(f"retained candidate path must be a directory: {path.relative_to(candidate)}")
            retained_names.append(name)
        names[:] = retained_names
        for name in sorted(filenames):
            path = root / name
            relative = path.relative_to(candidate)
            mode = path.stat(follow_symlinks=False).st_mode
            if path.is_symlink() or not stat.S_ISREG(mode):
                raise ValueError(f"retained candidate file must be regular: {relative}")
            if path.suffix.lower() not in common.AUTHORED_SUFFIXES | additional_suffixes and name not in common.AUTHORED_SPECIAL_NAMES:
                continue
            if path.suffix.lower() in {".c", ".h"}:
                continue
            if path.suffix.lower() in {".js", ".mjs", ".cjs"} and any(
                    path.with_suffix(suffix).is_file() for suffix in (".ts", ".tsx", *sorted(additional_suffixes))):
                continue
            files.append({"path": relative.as_posix(), "bytes": path.stat().st_size,
                          "sha256": common.digest(path)})
    files.sort(key=lambda row: row["path"])
    encoded = json.dumps(files, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
    return {"schema": "semaprax.closed-authored-inventory.v1", "files": files,
            "sha256": sha_bytes(encoded)}


def _phase_source_and_binary_guard(
    candidate: Path,
    expected_inventory: dict[str, Any],
    native_binary: Path | None,
    native_binary_sha256: str | None,
    compiler: Path | None = None,
    compiler_sha256: str | None = None,
    *, exclude_verified_node_modules: bool = False,
    additional_suffixes: frozenset[str] = frozenset(),
) -> tuple[bool, dict[str, Any]]:
    try:
        observed = closed_authored_inventory(candidate, exclude_verified_node_modules=exclude_verified_node_modules,
                                             additional_suffixes=additional_suffixes)
        binary_regular = (native_binary is None or (
            not native_binary.is_symlink() and native_binary.is_file()
            and stat.S_ISREG(native_binary.stat(follow_symlinks=False).st_mode)))
        binary_hash = common.digest(native_binary) if native_binary is not None and binary_regular else None
        compiler_regular = (compiler is None or (
            not compiler.is_symlink() and compiler.is_file()
            and stat.S_ISREG(compiler.stat(follow_symlinks=False).st_mode)))
        compiler_hash = common.digest(compiler) if compiler is not None and compiler_regular else None
        passed = (observed == expected_inventory and binary_hash == native_binary_sha256
                  and compiler_hash == compiler_sha256)
        return passed, {"status": "passed" if passed else "failed", "inventory": observed,
                        "native_binary_regular": binary_regular,
                        "native_binary_sha256": binary_hash,
                        "compiler_binary_regular": compiler_regular,
                        "compiler_binary_sha256": compiler_hash}
    except (OSError, ValueError) as error:
        return False, {"status": "failed", "error": str(error)}


def check_program(
    candidate: Path,
    timeout: int,
    env: dict[str, str],
    qualification_mode: str = "preflight_only",
    authoring_profile: str = AUTHORING_PROFILE_V24,
    harness_output: Path | None = None,
    compiler_binary_sha256: str | None = None,
    expected_inventory: dict[str, Any] | None = None,
    arm: str | None = None,
    *, exclude_verified_node_modules: bool = False,
) -> dict[str, Any]:
    result: dict[str, Any] = {"build": {"status": "missing"}, "candidate_tests": {"status": "not_run"},
                              "independent_acceptance": {"status": "not_run"}, "accepted": False}
    v27 = authoring_profile in PINNED_AUTHORING_PROFILES  # Includes v30; legacy route mechanics stay identical.
    version = "v30" if authoring_profile == AUTHORING_PROFILE_V30 else "v27"
    if v27 and arm not in ARMS:
        result["route_admission"] = {"status": "failed", "error": f"{version} checks require an explicit arm"}
        return result
    semaprax_v27 = v27 and arm == "semaprax"
    try:
        observed_inventory = closed_authored_inventory(candidate, exclude_verified_node_modules=exclude_verified_node_modules) if v27 else None
    except (OSError, ValueError) as error:
        result["source_consistency"] = {"status": "failed", "error": str(error)}
        return result
    if v27:
        if expected_inventory is not None and observed_inventory != expected_inventory:
            result["source_consistency"] = {"status": "failed", "inventory": observed_inventory}
            return result
        initial_inventory = expected_inventory or observed_inventory
        result["closed_authored_inventory"] = initial_inventory
        candidate_runner = candidate / "run.sh"
        if candidate_runner.is_symlink() or not candidate_runner.is_file():
            result["candidate_runner"] = {"status": "missing", "path": str(candidate_runner)}
            return result
        result["candidate_runner"] = {
            "status": "present", "used_for_acceptance": arm == "typescript"}
    else:
        initial_inventory = None
        candidate_runner = candidate / "run.sh"
        if candidate_runner.is_symlink() or not candidate_runner.is_file():
            result["candidate_runner"] = {"status": "missing", "path": str(candidate_runner)}
            return result
        result["candidate_runner"] = {"status": "present", "used_for_acceptance": True}
    native_binary = None
    native_binary_hash = None
    compiler = None
    if v27 and arm == "typescript" and ((candidate / "semaprax.toml").exists()
                                         or (candidate / "semaprax.toml").is_symlink()):
        result["typescript_route"] = {
            "status": "failed", "error": f"{version} TypeScript candidate must not contain a SEMAPRAX manifest"}
        return result
    if semaprax_v27:
        compiler = Path(env.get("SEMAPRAX_BIN", ""))
        if (not compiler.is_file() or compiler.is_symlink()
                or common.digest(compiler) != compiler_binary_sha256):
            result["pinned_compiler"] = {"status": "failed"}
            return result
        if harness_output is None:
            result["pinned_compiler"] = {"status": "failed", "error": "missing harness output path"}
            return result
        harness_output.parent.mkdir(parents=True, exist_ok=False)
        manifest = candidate / "semaprax.toml"
        commands = (
            ("pinned_compiler_check", [str(compiler), "check", "--manifest-path", str(manifest)]),
            ("pinned_native_build", [str(compiler), "build", "--manifest-path", str(manifest),
                                     "--target", "native", "--output", str(harness_output)]),
        )
        for key, command in commands:
            started = time.monotonic()
            try:
                proc = subprocess.run(command, cwd=candidate, capture_output=True, check=False,
                                      timeout=timeout, env=env)
                result[key] = {"status": "passed" if proc.returncode == 0 else "failed",
                    "exit_code": proc.returncode, "seconds": round(time.monotonic() - started, 3),
                    "stdout": common.bounded_text(proc.stdout), "stderr": common.bounded_text(proc.stderr)}
            except subprocess.TimeoutExpired as exc:
                result[key] = {"status": "timeout", "seconds": round(time.monotonic() - started, 3),
                    "stdout": common.bounded_text(exc.stdout or b""), "stderr": common.bounded_text(exc.stderr or b"")}
            passed, guard = _phase_source_and_binary_guard(
                candidate, initial_inventory, None, None, compiler, compiler_binary_sha256,
                exclude_verified_node_modules=exclude_verified_node_modules)
            result[f"{key}_source_consistency"] = guard
            if result[key]["status"] != "passed" or not passed:
                return result
        native_binary = harness_output
        if native_binary.is_symlink() or not native_binary.is_file():
            result["pinned_native_build"]["status"] = "failed"
            result["pinned_native_build"]["error"] = "compiler did not produce a regular native binary"
            return result
        native_binary_hash = common.digest(native_binary)
    for key, script in (("build", "build.sh"), ("candidate_tests", "test.sh")):
        path = candidate / script
        if not path.is_file():
            result[key] = {"status": "missing", "path": str(path)}
            return result
        started = time.monotonic()
        try:
            proc = subprocess.run(["/bin/sh", str(path)], cwd=candidate, capture_output=True,
                                  check=False, timeout=timeout, env=env)
            row = {"status": "passed" if proc.returncode == 0 else "failed",
                   "exit_code": proc.returncode, "seconds": round(time.monotonic() - started, 3),
                   "stdout": common.bounded_text(proc.stdout), "stderr": common.bounded_text(proc.stderr)}
        except subprocess.TimeoutExpired as exc:
            row = {"status": "timeout", "seconds": round(time.monotonic() - started, 3),
                   "stdout": common.bounded_text(exc.stdout or b""), "stderr": common.bounded_text(exc.stderr or b"")}
        result[key] = row
        if v27:
            passed, guard = _phase_source_and_binary_guard(
                candidate, initial_inventory, native_binary, native_binary_hash,
                compiler if semaprax_v27 else None,
                compiler_binary_sha256 if semaprax_v27 else None,
                exclude_verified_node_modules=exclude_verified_node_modules)
            result[f"{key}_source_consistency"] = guard
            if row["status"] != "passed" or not passed:
                return result
        elif row["status"] != "passed":
            return result
    runner = BENCHMARK / "acceptance" / "run.py"
    if semaprax_v27:
        accepted_command = [str(native_binary)]
    elif v27:
        accepted_command = ["/bin/sh", str(candidate / "run.sh")]
        result["typescript_route"] = {
            "status": "passed", "command": accepted_command,
            "semaprax_manifest_refused": True}
    else:
        accepted_command = ["/bin/sh", str(candidate / "run.sh")]
    command = [sys.executable, str(runner), "--command-json", json.dumps(accepted_command)]
    started = time.monotonic()
    try:
        proc = subprocess.run(command, cwd=candidate, capture_output=True, check=False, timeout=timeout, env=env)
        row = {"status": "passed" if proc.returncode == 0 else "failed", "exit_code": proc.returncode,
               "seconds": round(time.monotonic() - started, 3), "stdout": common.bounded_text(proc.stdout),
               "stderr": common.bounded_text(proc.stderr)}
    except subprocess.TimeoutExpired as exc:
        row = {"status": "timeout", "stdout": common.bounded_text(exc.stdout or b""),
               "stderr": common.bounded_text(exc.stderr or b"")}
    result["independent_acceptance"] = row
    if v27:
        passed, guard = _phase_source_and_binary_guard(
            candidate, initial_inventory, native_binary, native_binary_hash,
            compiler if semaprax_v27 else None,
                compiler_binary_sha256 if semaprax_v27 else None,
                exclude_verified_node_modules=exclude_verified_node_modules)
        result["acceptance_source_consistency"] = guard
        result["native_binary"] = {"path": str(native_binary) if native_binary is not None else None,
                                   "sha256": native_binary_hash,
                                   "unchanged_after_acceptance": passed}
        if semaprax_v27:
            result["pinned_compiler"] = {"path": str(compiler),
                                         "sha256": compiler_binary_sha256,
                                         "unchanged_after_acceptance": passed}
    else:
        passed = True
    if v27:
        result["source_consistency"] = {"status": "passed" if passed else "failed"}
    result["accepted"] = row["status"] == "passed" and passed
    result["qualification_mode"] = qualification_mode
    return result


def seeded_spec_integrity(workspace: Path, settings: dict[str, Any]) -> dict[str, Any]:
    relative = "benchmarks/event-sim-tokens-v1/SPEC.md"
    expected = settings.get("seed_files_sha256", {}).get(relative)
    path = workspace / relative
    observed = common.digest(path) if path.is_file() else None
    status = "passed" if expected is not None and observed == expected else "failed"
    return {"status": status, "path": relative, "expected_sha256": expected,
            "observed_sha256": observed}


def launch_trial(seed_repo: Path, artifacts: Path, seed_commit: str, trial: dict[str, Any],
                 settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    require_authoring_eligibility(settings, semaprax_bin)
    arm, number = trial["arm"], trial["number"]
    label = f"{arm}-{number:02d}"
    workspace = artifacts / "worktrees" / label
    error = common.add_seed_worktree(seed_repo, workspace, seed_commit, SEED_FILES)
    qualification = settings.get("qualification", {})
    qualification_mode = (
        "evidence_gated_scored" if qualification.get("scored_trials_allowed") is True else "preflight_only"
    )
    row: dict[str, Any] = {
        **trial, "workspace": str(workspace), "status": "failed", "failure": error,
        "qualification_mode": qualification_mode,
    }
    if error:
        return row
    candidate = workspace / "benchmarks" / "event-sim-tokens-v1" / "candidate"
    tooling = settings.get("typescript_bootstrap") if arm == "typescript" else None
    env = trial_environment(semaprax_bin)
    if tooling:
        setup_started = time.monotonic()
        try:
            receipt = ts_bootstrap.verify_plan(tooling)
            row["supplied_tooling"] = ts_bootstrap.stage(Path(tooling["receipt_path"]), candidate,
                                                          tooling["node_binary"], tooling["npm_binary"])
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            row.update({"failure": f"TypeScript bootstrap refused before prompt: {error}",
                        "typescript_setup": {"status": "failed", "elapsed_seconds": round(time.monotonic() - setup_started, 3),
                                              "context_tokens": None},
                        "workspace_retained_for_review": True, "worktree_removed_after_archive": False})
            return row
        row["typescript_setup"] = {"status": "ready", "elapsed_seconds": round(time.monotonic() - setup_started, 3),
                                   "context_tokens": None}
        node_path = Path(shutil.which(tooling["node_binary"]) or tooling["node_binary"]).resolve(strict=True)
        env["PATH"] = os.pathsep.join((str(candidate / "node_modules/.bin"), str(node_path.parent), env.get("PATH", "")))
    prompt = prompt_for(arm, candidate, semaprax_bin, settings.get("authoring_profile", AUTHORING_PROFILE_V24))
    if tooling:
        prompt += "\n\n" + ts_bootstrap.prompt_note(receipt)
    prompts = artifacts / "prompts"
    prompts.mkdir(exist_ok=True)
    (prompts / f"{label}.txt").write_text(prompt, encoding="utf-8")
    row["prompt_sha256"] = sha_text(prompt)
    retain_fixed_harness_context(row, settings, prompt)
    transcripts = artifacts / "transcripts"
    transcripts.mkdir(exist_ok=True)
    stream, stderr = transcripts / f"{label}.jsonl", transcripts / f"{label}.stderr.txt"
    require_authoring_eligibility(settings, semaprax_bin)
    process = run_process(common.claude_command(settings, prompt), workspace,
                          env, stream, stderr, settings["timeout_seconds"])
    row.update(process)
    row["transcript"], row["stderr_path"] = str(stream), str(stderr)
    usage = common.stream_usage(stream, settings["model"]) if stream.exists() else {
        "models_observed": [], "usage": {field: None for field in common.ALL_USAGE_FIELDS},
        "turns_with_usage": 0, "legacy_net_input": {"net_input_tokens": None},
    }
    row["observed"] = usage
    row["provider_input_plus_cache_tokens_raw"] = common.input_tokens_total(usage["usage"])
    row["list_price_estimate_usd"] = common.rate_card_estimate_details(
        usage["usage"], settings.get("price_book", {}).get("per_million_tokens"),
    )["usd"]
    row["provider_quota"] = provider_quota_failure(usage)
    row["provider_reported_api_equivalent_total_cost_usd"] = usage.get("provider_reported_api_equivalent_total_cost_usd")
    row["provider_receipt_actual_usd"] = None
    model_ok = common.observed_model_matches(usage.get("models_observed"), settings.get("observed_model_id"))
    spec_integrity = seeded_spec_integrity(workspace, settings)
    row["seeded_spec_integrity"] = spec_integrity
    if tooling:
        intact, evidence = ts_bootstrap.verify_staged(
            candidate, receipt["inventory"], evidence_path=artifacts / "dependency-evidence" / label)
        row["dependency_tree_integrity"] = evidence
        if not intact:
            row.update({"status": "not_accepted", "failure": "staged TypeScript dependency tree changed",
                        "final_candidate_source_metrics": {"status": "measurement_failed", "total_tokens": None,
                            "files": [], "tokenizer": settings.get("authored_source_tokenizer")},
                        "workspace_retained_for_review": True, "worktree_removed_after_archive": False})
            return row
    if settings.get("authoring_profile") in PINNED_AUTHORING_PROFILES:
        try:
            row["closed_authored_inventory_after_model"] = closed_authored_inventory(
                candidate, exclude_verified_node_modules=bool(tooling))
        except (OSError, ValueError) as error:
            row.update({"status": "not_accepted", "failure": str(error),
                        "workspace_retained_for_review": True, "worktree_removed_after_archive": False})
            return row
    if process["timed_out"]:
        row["failure"] = "trial hit wall-clock timeout"
    elif process["process_exit_code"] != 0:
        row["failure"] = f"Claude Code exited with {process['process_exit_code']}"
    elif not model_ok:
        row["failure"] = "observed model missing or differs from calibration model"
    elif spec_integrity["status"] != "passed":
        row["failure"] = "trial changed the frozen public benchmark specification"
    else:
        admission = candidate_authoring_admission(
            candidate, arm, settings.get("authoring_profile", AUTHORING_PROFILE_V24))
        row["authoring_admission"] = admission
        if admission["status"] == "failed":
            row["status"] = "not_accepted"
            row["failure"] = "candidate failed exact authoring-profile admission"
        else:
            started = time.monotonic()
            if settings.get("authoring_profile") in PINNED_AUTHORING_PROFILES:
                row["acceptance"] = check_program(
                    candidate, settings["timeout_seconds"], env, qualification_mode,
                    settings["authoring_profile"], artifacts / "harness-native" / label / "shiftsim",
                    qualification.get("compiler_binary_sha256") or settings.get("semaprax_binary_sha256"),
                    row.get("closed_authored_inventory_after_model"), arm,
                    exclude_verified_node_modules=bool(tooling))
            else:
                row["acceptance"] = check_program(
                    candidate, settings["timeout_seconds"], env, qualification_mode,
                    exclude_verified_node_modules=bool(tooling))
            row["acceptance_elapsed_seconds"] = round(time.monotonic() - started, 3)
            row["status"] = "accepted" if row["acceptance"]["accepted"] else "not_accepted"
            if row["status"] != "accepted":
                row["failure"] = "candidate failed build or acceptance checks"
    try:
        if tooling:
            intact, evidence = ts_bootstrap.verify_staged(
                candidate, receipt["inventory"], evidence_path=artifacts / "dependency-evidence" / f"{label}-before-metrics")
            row["dependency_tree_integrity_before_metrics"] = evidence
            if not intact:
                row.update({"status": "not_accepted", "failure": "staged TypeScript dependency tree changed before source measurement",
                            "final_candidate_source_metrics": {"status": "measurement_failed", "total_tokens": None,
                                "files": [], "tokenizer": settings.get("authored_source_tokenizer")},
                            "workspace_retained_for_review": True, "worktree_removed_after_archive": False})
                return row
        row["final_candidate_source_metrics"] = common.authored_source_metrics(
            candidate, settings.get("authored_source_tokenizer"),
            exclude_verified_node_modules=bool(tooling)
        )
    except (OSError, RuntimeError, UnicodeError, json.JSONDecodeError) as error:
        row["final_candidate_source_metrics"] = {
            "status": "measurement_failed", "total_tokens": None, "files": [],
            "tokenizer": settings.get("authored_source_tokenizer"), "error": str(error),
        }
    if settings.get("authoring_profile") in PINNED_AUTHORING_PROFILES:
        acceptance = row.get("acceptance", {})
        native = acceptance.get("native_binary", {})
        native_path = Path(native["path"]) if native.get("path") else None
        pinned = acceptance.get("pinned_compiler", {})
        compiler_path = Path(pinned["path"]) if pinned.get("path") else None
        expected_inventory = acceptance.get(
            "closed_authored_inventory", row.get("closed_authored_inventory_after_model"))
        passed, guard = _phase_source_and_binary_guard(
            candidate, expected_inventory, native_path, native.get("sha256"),
            compiler_path, pinned.get("sha256"), exclude_verified_node_modules=bool(tooling))
        row["source_consistency_after_metrics"] = guard
        if not passed:
            row.update({"status": "not_accepted", "failure": "candidate source, compiler, or harness native binary changed",
                        "workspace_retained_for_review": True, "worktree_removed_after_archive": False})
            if isinstance(row.get("acceptance"), dict):
                row["acceptance"]["accepted"] = False
            return row
    archive = artifacts / "candidates" / label
    archive.parent.mkdir(parents=True, exist_ok=True)
    spec_integrity_before_archive = seeded_spec_integrity(workspace, settings)
    row["seeded_spec_integrity_before_archive"] = spec_integrity_before_archive
    if spec_integrity_before_archive["status"] != "passed":
        if row["status"] == "accepted":
            row["status"] = "not_accepted"
        row["failure"] = "trial changed the frozen public benchmark specification; workspace retained for review"
        row["workspace_retained_for_review"] = True
        row["worktree_removed_after_archive"] = False
        return row
    if tooling:
        intact, evidence = ts_bootstrap.verify_staged(
            candidate, receipt["inventory"], evidence_path=artifacts / "dependency-evidence" / f"{label}-before-archive")
        row["dependency_tree_integrity_before_archive"] = evidence
        if not intact:
            row.update({"status": "not_accepted", "failure": "staged TypeScript dependency tree changed before archive",
                        "final_candidate_source_metrics": {"status": "measurement_failed", "total_tokens": None,
                            "files": [], "tokenizer": settings.get("authored_source_tokenizer")},
                        "workspace_retained_for_review": True, "worktree_removed_after_archive": False})
            return row
    hashes, omitted = common.archive_candidate(candidate, archive,
                                               exclude_verified_node_modules=bool(tooling))
    row["candidate_archive"] = str(archive)
    row["candidate_source_sha256"] = hashes
    row["candidate_archive_excluded_paths"] = omitted
    if settings.get("authoring_profile") in PINNED_AUTHORING_PROFILES:
        acceptance = row.get("acceptance", {})
        native = acceptance.get("native_binary", {})
        native_path = Path(native["path"]) if native.get("path") else None
        pinned = acceptance.get("pinned_compiler", {})
        compiler_path = Path(pinned["path"]) if pinned.get("path") else None
        expected_inventory = acceptance.get(
            "closed_authored_inventory", row.get("closed_authored_inventory_after_model"))
        passed, guard = _phase_source_and_binary_guard(
            candidate, expected_inventory, native_path, native.get("sha256"),
            compiler_path, pinned.get("sha256"), exclude_verified_node_modules=bool(tooling))
        try:
            archived_inventory = closed_authored_inventory(archive)
            archive_matches = archived_inventory == expected_inventory
        except (OSError, ValueError) as error:
            archived_inventory = {"status": "failed", "error": str(error)}
            archive_matches = False
        guard["archived_inventory"] = archived_inventory
        guard["archive_matches"] = archive_matches
        passed = passed and archive_matches
        row["source_consistency_after_archive"] = guard
        if not passed:
            row.update({"status": "not_accepted", "failure": "candidate source, compiler, or harness native binary changed",
                        "workspace_retained_for_review": True, "worktree_removed_after_archive": False})
            if isinstance(row.get("acceptance"), dict):
                row["acceptance"]["accepted"] = False
            return row
    # Preserve the complete authored candidate before deleting its modified worktree.
    removed = subprocess.run(["git", "worktree", "remove", "--force", str(workspace)],
                             cwd=seed_repo, text=True, capture_output=True, check=False)
    row["worktree_removed_after_archive"] = removed.returncode == 0
    if removed.returncode:
        row["worktree_cleanup_error"] = common.bounded_text(removed.stderr)
        row["workspace_retained_for_review"] = True
    return row


def summarize(
    rows: list[dict[str, Any]],
    calibration: dict[str, Any] | None = None,
    qualification: dict[str, Any] | None = None,
) -> dict[str, Any]:
    qualification = qualification or {
        "status": "preflight_not_qualified", "scored_trials_allowed": False,
        "blocking_issue": 611, "issue_611_status": "open",
    }
    arms: dict[str, Any] = {}
    for arm in ARMS:
        selected = [row for row in rows if row.get("arm") == arm]
        accepted = sum(row.get("status") == "accepted" for row in selected)
        costs = [row.get("list_price_estimate_usd") for row in selected]
        complete_cost_total = sum(costs) if costs and all(isinstance(value, (int, float)) for value in costs) else None
        provider_costs = [row.get("provider_reported_api_equivalent_total_cost_usd") for row in selected]
        provider_cost_total = (
            sum(provider_costs) if provider_costs and all(isinstance(value, (int, float)) for value in provider_costs)
            else None
        )
        usage_rows = [row.get("observed", {}).get("usage", {}) for row in selected]
        raw_usage_totals = {
            field: (
                sum(usage.get(field) for usage in usage_rows if isinstance(usage.get(field), int))
                if usage_rows else None
            )
            for field in common.ALL_USAGE_FIELDS
        }
        raw_usage_incomplete = {
            field: sum(not isinstance(usage.get(field), int) for usage in usage_rows)
            for field in common.ALL_USAGE_FIELDS
        }
        arms[arm] = {
            "attempts": len(selected),
            "accepted_trials": accepted,
            "accepted_per_attempt": accepted,
            "all_attempt_wall_seconds": [row.get("elapsed_seconds") for row in selected],
            "aggregate_attempt_wall_seconds": (
                sum(row["elapsed_seconds"] for row in selected)
                if selected and all(
                    isinstance(row.get("elapsed_seconds"), (int, float)) for row in selected
                )
                else None
            ),
            "list_price_estimate_usd_per_attempt": [row.get("list_price_estimate_usd") for row in selected],
            "list_price_estimate_total_usd_including_failures": complete_cost_total,
            "list_price_estimate_usd_per_accepted_task_including_failures": (
                complete_cost_total / accepted if complete_cost_total is not None and accepted else None
            ),
            "provider_cost_per_attempt": [row.get("provider_reported_api_equivalent_total_cost_usd") for row in selected],
            "provider_reported_api_equivalent_total_cost_known_subtotal_usd": provider_cost_total,
            "provider_reported_api_equivalent_usd_per_accepted_task_including_failures": (
                provider_cost_total / accepted if provider_cost_total is not None and accepted else None
            ),
            "raw_usage_per_attempt": [row.get("observed", {}).get("usage") for row in selected],
            "raw_usage_known_subtotal_by_bucket": raw_usage_totals,
            "raw_usage_attempts_missing_bucket": raw_usage_incomplete,
            "turns": (
                sum(row.get("observed", {}).get("turns_with_usage", 0) for row in selected)
                if selected else None
            ),
            "turns_with_usage_per_attempt": [
                row.get("observed", {}).get("turns_with_usage", 0) for row in selected
            ],
            "raw_input_plus_cache_tokens_per_attempt": [row.get("provider_input_plus_cache_tokens_raw") for row in selected],
            "legacy_net_input_tokens_per_attempt": [row.get("observed", {}).get("legacy_net_input", {}).get("net_input_tokens") for row in selected],
            "final_authored_source_token_proxy_per_attempt": [
                row.get("final_candidate_source_metrics", {}).get("total_tokens") for row in selected
            ],
        }
    return {
        "qualification": qualification,
        "qualification_note": (
            "Pinned native acceptance evidence passed; this only gates scored trials and does not close issue 611."
            if qualification.get("scored_trials_allowed") is True else
            "Without pinned native acceptance evidence, acceptance outcomes are preflight-only and must not be scored."
        ),
        "attempt_denominator": len(rows),
        "arms": arms,
        "calibration": calibration,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    for qualification_action in ("qualify-v3", "qualify-v4"):
        qualify = sub.add_parser(qualification_action, help="build and accept one closed native qualification subject")
        qualify.add_argument("--repo", default=str(REPO))
        qualify.add_argument("--compiler-source-ref", required=True)
        qualify.add_argument("--candidate", required=True)
        qualify.add_argument("--semaprax-bin", required=True)
        qualify.add_argument("--output", required=True)
        qualify.add_argument("--timeout-seconds", type=int, default=1800)
    for action in ("plan", "run", "preflight"):
        p = sub.add_parser(action)
        p.add_argument("--repo", default=str(REPO))
        p.add_argument("--base-ref", required=True)
        p.add_argument("--round", type=int, choices=(1, 2, 3, 4), default=1,
                       help="round 3 selects v27; round 4 requires explicit qualified v30 owned-data setup")
        p.add_argument("--authoring-profile", choices=tuple(AUTHORING_PROFILES), default=None)
        p.add_argument("--artifacts", required=True)
        if action != "preflight":
            p.add_argument("--trials-per-arm", type=int, default=MIN_TRIALS_PER_ARM)
        p.add_argument("--model", default=MODEL)
        p.add_argument("--effort", default=EFFORT)
        p.add_argument("--timeout-seconds", type=int, default=1800)
        p.add_argument("--max-budget-usd", type=float, default=None)
        p.add_argument("--tokenizer-dir", default=None)
        p.add_argument("--typescript-bootstrap-receipt", default=None)
        p.add_argument("--node-binary", default="node"); p.add_argument("--npm-binary", default="npm")
        if action in ("plan", "run"):
            p.add_argument("--qualification-evidence", default=None,
                           help="pinned native streaming acceptance evidence; enables scored trials only when valid")
        if action in ("run", "preflight"):
            p.add_argument("--semaprax-bin", required=True)
        else:
            p.add_argument("--semaprax-bin", default=None,
                           help="compiler binary to bind when planning evidence-gated scored trials")
        if action == "preflight":
            p.add_argument("--arm", choices=ARMS, required=True,
                           help="run one exploratory trial for this arm; results are never scored")
    args = parser.parse_args()
    try:
        if args.action in ("qualify-v3", "qualify-v4"):
            repo = Path(args.repo).resolve(strict=True)
            result = generate_v3_qualification(
                Path(args.candidate), Path(args.semaprax_bin), repo,
                resolve_commit(repo, args.compiler_source_ref), Path(args.output),
                args.timeout_seconds, authoring_profile=(AUTHORING_PROFILE_V30
                    if args.action == "qualify-v4" else AUTHORING_PROFILE_V27))
            print(json.dumps(result, indent=2, sort_keys=True))
            return 0
        semaprax_bin = None
        binary_hash = None
        if args.action in ("run", "preflight") or args.semaprax_bin is not None:
            semaprax_bin = Path(args.semaprax_bin).expanduser().resolve(strict=True)
            if not semaprax_bin.is_file():
                raise ValueError("semaprax-bin must be a regular file")
            binary_hash = common.digest(semaprax_bin)
        if args.action == "preflight":
            settings = single_arm_preflight_plan(args, args.arm, binary_hash)
        else:
            settings = plan(args, binary_hash)
        if args.action == "plan":
            print(json.dumps(settings, indent=2))
            return 0
        ts_bootstrap.verify_plan(settings.get("typescript_bootstrap"))
        assert semaprax_bin is not None
        require_authoring_eligibility(settings, semaprax_bin)
        artifacts = Path(settings["artifacts"])
        artifacts.mkdir(parents=True)
        seed_repo = artifacts / "seed-repository"
        seed = common.create_seed_repository(Path(args.repo).resolve(strict=True), settings["repository_commit"],
                                             seed_repo, SEED_FILES)
        settings.update(seed)
        settings["semaprax_binary"] = str(semaprax_bin)
        settings["semaprax_binary_sha256"] = binary_hash
        qualification = settings["qualification"]
        if qualification.get("scored_trials_allowed") is True:
            copy_qualification_artifacts(qualification, artifacts)
        version = subprocess.run(["claude", "--version"], text=True, capture_output=True, check=False)
        settings["claude_version"] = version.stdout.strip() if version.returncode == 0 else None
        node = subprocess.run(["node", "--version"], text=True, capture_output=True, check=False)
        settings["node_version"] = node.stdout.strip() if node.returncode == 0 else None
        common.save_json(artifacts / "campaign.json", settings)
        campaign_started = time.monotonic()
        calibration = launch_calibration(seed_repo, artifacts, seed["seed_repository_commit"], settings, semaprax_bin)
        settings["observed_model_id"] = calibration.get("observed_model_id")
        settings["calibration_result"] = calibration
        common.save_json(artifacts / "campaign.json", settings)
        rows = []
        if calibration.get("status") == "ready":
            numbers = {arm: 0 for arm in ARMS}
            for ordinal, arm in enumerate(settings["trial_order"]):
                numbers[arm] += 1
                row = launch_trial(seed_repo, artifacts, seed["seed_repository_commit"],
                                   {"arm": arm, "number": numbers[arm]}, settings, semaprax_bin)
                rows.append(row)
                quota_stop = settings["round"] >= 2 and row.get("provider_quota") is not None
                common.save_json(artifacts / "results.json", {
                    "campaign": settings, "calibration": calibration, "trials": rows,
                    "summary": summarize(rows, calibration, settings["qualification"]),
                    "campaign_status": "interrupted_provider_quota" if quota_stop else (
                        "complete" if len(rows) == len(settings["trial_order"]) else "in_progress"
                    ),
                    "unlaunched_trial_order": settings["trial_order"][ordinal + 1:],
                    "campaign_elapsed_wall_seconds": round(time.monotonic() - campaign_started, 3),
                })
                print(f"{arm} {numbers[arm]}/{settings['trials_per_arm']}: {row.get('status', 'failed')}", flush=True)
                if quota_stop:
                    print("provider quota/rate limit interrupted the matched campaign; remaining trials were not launched", file=sys.stderr)
                    return 2
        else:
            common.save_json(artifacts / "results.json", {
                "campaign": settings, "calibration": calibration, "trials": [],
                "summary": summarize([], calibration, settings["qualification"]),
                "campaign_elapsed_wall_seconds": round(time.monotonic() - campaign_started, 3),
            })
            print("calibration failed; no trials launched", file=sys.stderr)
            return 2
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"ShiftSim campaign error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
