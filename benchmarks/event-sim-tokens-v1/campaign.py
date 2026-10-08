#!/usr/bin/env python3
"""Prepare or run a matched, evidence-gated ShiftSim campaign."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
import tomllib
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import live_campaign_common as common

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
    if isinstance(round_number, bool) or round_number not in (1, 2, 3):
        raise ValueError(f"unsupported ShiftSim round: {round_number}")
    hashes = {path: sha_bytes(blob_at_commit(repo, commit, path))
              for path in FROZEN_BENCHMARK_SHA256}
    if hashes != FROZEN_BENCHMARK_SHA256:
        raise ValueError(f"round {round_number} requires the unchanged frozen SPEC, corpus, and oracle")
    return hashes


def select_authoring_profile(round_number: int, requested: str | None) -> tuple[str, dict[str, Any]]:
    expected = AUTHORING_PROFILE_V27 if round_number == 3 else AUTHORING_PROFILE_V24
    if requested is None:
        if round_number == 3:
            raise ValueError(f"round 3 requires --authoring-profile {AUTHORING_PROFILE_V27}")
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


def _manifest_route(data: bytes) -> dict[str, Any]:
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
            or route["profile"] != NATIVE_PROJECT_ROUTE_V27["project_profile"]
            or route["input"] != NATIVE_PROJECT_ROUTE_V27["input_route"]
            or not isinstance(function, str) or not function
            or route["exports_web"] != [function]
            or capabilities != {"required": STREAM_DATA_CAPABILITIES}):
        raise ValueError("candidate manifest does not match the exact v27 stream-data command route")
    return route


def candidate_authoring_admission(candidate: Path, arm: str, authoring_profile: str) -> dict[str, Any]:
    if arm != "semaprax" or authoring_profile != AUTHORING_PROFILE_V27:
        return {"status": "not_applicable", "authoring_profile": authoring_profile}
    path = candidate / "semaprax.toml"
    try:
        if path.is_symlink() or not path.is_file():
            raise ValueError("v27 SEMAPRAX candidate must contain a regular semaprax.toml")
        data = path.read_bytes()
        route = _manifest_route(data)
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
    if authoring_profile == AUTHORING_PROFILE_V27:
        source = evidence.get("candidate_source")
        if not isinstance(source, dict):
            raise ValueError("v3 qualification evidence must bind candidate source inventory and manifest")
        inventory_path, inventory_bytes, inventory_hash = _bound_file(
            source.get("inventory"), evidence_path, "candidate source inventory")
        manifest_path, manifest_bytes, manifest_hash = _bound_file(
            source.get("manifest"), evidence_path, "candidate manifest")
        try:
            inventory = json.loads(inventory_bytes.decode("utf-8"))
        except (UnicodeError, json.JSONDecodeError) as error:
            raise ValueError("candidate source inventory is not valid UTF-8 JSON") from error
        files = inventory.get("files") if isinstance(inventory, dict) else None
        manifest_rows = [row for row in files or []
                         if isinstance(row, dict) and row.get("path") == "semaprax.toml"]
        if len(manifest_rows) != 1 or manifest_rows[0].get("sha256") != manifest_hash:
            raise ValueError("candidate source inventory must bind semaprax.toml exactly")
        _manifest_route(manifest_bytes)
        source_binding = {
            "candidate_source_inventory_path": str(inventory_path),
            "candidate_source_inventory_sha256": inventory_hash,
            "candidate_manifest_path": str(manifest_path),
            "candidate_manifest_sha256": manifest_hash,
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


def trial_environment(semaprax_bin: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["SEMAPRAX_BIN"] = str(semaprax_bin)
    env["PATH"] = str(semaprax_bin.parent) + os.pathsep + env.get("PATH", "")
    return env


def prompt_for(arm: str, candidate: Path, semaprax_bin: Path,
               authoring_profile: str = AUTHORING_PROFILE_V24) -> str:
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
    evidence_argument = getattr(args, "qualification_evidence", None)
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
        "arms": list(ARMS),
        "trial_order": rounds,
        "native_project_route": profile["route"],
        "qualification": qualification,
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
    workspace = artifacts / "worktrees" / "calibration"
    error = common.add_seed_worktree(seed_repo, workspace, seed_commit, SEED_FILES)
    row: dict[str, Any] = {"status": "failed", "failure": error}
    if error:
        return row
    prompt = CALIBRATION_PROMPT
    stream, stderr = artifacts / "calibration.jsonl", artifacts / "calibration.stderr.txt"
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


def check_program(
    candidate: Path,
    timeout: int,
    env: dict[str, str],
    qualification_mode: str = "preflight_only",
) -> dict[str, Any]:
    result: dict[str, Any] = {"build": {"status": "missing"}, "candidate_tests": {"status": "not_run"},
                              "independent_acceptance": {"status": "not_run"}, "accepted": False}
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
        if row["status"] != "passed":
            return result
    runner = BENCHMARK / "acceptance" / "run.py"
    command = [sys.executable, str(runner), "--command-json", json.dumps(["/bin/sh", str(candidate / "run.sh")])]
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
    result["accepted"] = row["status"] == "passed"
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
    prompt = prompt_for(arm, candidate, semaprax_bin, settings.get("authoring_profile", AUTHORING_PROFILE_V24))
    prompts = artifacts / "prompts"
    prompts.mkdir(exist_ok=True)
    (prompts / f"{label}.txt").write_text(prompt, encoding="utf-8")
    row["prompt_sha256"] = sha_text(prompt)
    transcripts = artifacts / "transcripts"
    transcripts.mkdir(exist_ok=True)
    stream, stderr = transcripts / f"{label}.jsonl", transcripts / f"{label}.stderr.txt"
    process = run_process(common.claude_command(settings, prompt), workspace,
                          trial_environment(semaprax_bin), stream, stderr, settings["timeout_seconds"])
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
            row["acceptance"] = check_program(
                candidate, settings["timeout_seconds"], trial_environment(semaprax_bin), qualification_mode,
            )
            row["acceptance_elapsed_seconds"] = round(time.monotonic() - started, 3)
            row["status"] = "accepted" if row["acceptance"]["accepted"] else "not_accepted"
            if row["status"] != "accepted":
                row["failure"] = "candidate failed build or acceptance checks"
    try:
        row["final_candidate_source_metrics"] = common.authored_source_metrics(
            candidate, settings.get("authored_source_tokenizer")
        )
    except (OSError, RuntimeError, UnicodeError, json.JSONDecodeError) as error:
        row["final_candidate_source_metrics"] = {
            "status": "measurement_failed", "total_tokens": None, "files": [],
            "tokenizer": settings.get("authored_source_tokenizer"), "error": str(error),
        }
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
    hashes, omitted = common.archive_candidate(candidate, archive)
    row["candidate_archive"] = str(archive)
    row["candidate_source_sha256"] = hashes
    row["candidate_archive_excluded_paths"] = omitted
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
            field: sum(usage.get(field) for usage in usage_rows if isinstance(usage.get(field), int))
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
            "aggregate_attempt_wall_seconds": sum(row.get("elapsed_seconds", 0) or 0 for row in selected),
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
            "turns": sum(row.get("observed", {}).get("turns_with_usage", 0) for row in selected),
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
    for action in ("plan", "run", "preflight"):
        p = sub.add_parser(action)
        p.add_argument("--repo", default=str(REPO))
        p.add_argument("--base-ref", required=True)
        p.add_argument("--round", type=int, choices=(1, 2, 3), default=1,
                       help="round 3 qualifies the frozen task for the explicit v27 authoring profile")
        p.add_argument("--authoring-profile", choices=tuple(AUTHORING_PROFILES), default=None)
        p.add_argument("--artifacts", required=True)
        if action != "preflight":
            p.add_argument("--trials-per-arm", type=int, default=MIN_TRIALS_PER_ARM)
        p.add_argument("--model", default=MODEL)
        p.add_argument("--effort", default=EFFORT)
        p.add_argument("--timeout-seconds", type=int, default=1800)
        p.add_argument("--max-budget-usd", type=float, default=None)
        p.add_argument("--tokenizer-dir", default=None)
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
        assert semaprax_bin is not None
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
            evidence_path = Path(qualification["evidence_path"])
            report_path = Path(qualification["acceptance_report_path"])
            evidence_copy = artifacts / "qualification-evidence.json"
            report_copy = artifacts / "qualification-acceptance-report.json"
            shutil.copyfile(evidence_path, evidence_copy)
            shutil.copyfile(report_path, report_copy)
            settings["qualification"]["evidence_artifact"] = str(evidence_copy)
            settings["qualification"]["acceptance_report_artifact"] = str(report_copy)
            for key, artifact_name in (
                ("candidate_source_inventory_path", "qualification-candidate-source-inventory.json"),
                ("candidate_manifest_path", "qualification-candidate-semaprax.toml"),
            ):
                if qualification.get(key):
                    copied = artifacts / artifact_name
                    shutil.copyfile(qualification[key], copied)
                    settings["qualification"][key.replace("_path", "_artifact")] = str(copied)
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
