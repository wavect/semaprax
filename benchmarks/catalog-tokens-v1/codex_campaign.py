#!/usr/bin/env python3
"""Matched Catalog23 owned-data cohort; model dispatch exists only under run."""
from __future__ import annotations
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
from typing import Any

BENCHMARK = Path(__file__).resolve().parent
REPO = BENCHMARK.parents[1]
CLI_ADAPTER = REPO / "benchmarks/cli-tokens-v1/codex_campaign.py"
sys.path.insert(0, str(CLI_ADAPTER.parent))
spec = importlib.util.spec_from_file_location("catalog_shared_codex_accounting", CLI_ADAPTER)
if spec is None or spec.loader is None:
    raise RuntimeError("cannot load shared Codex accounting")
codex = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = codex
spec.loader.exec_module(codex)
sys.path.insert(0, str(BENCHMARK))
import support as catalog
import compiler_input_capture as compiler_capture
MODEL, EFFORT, TIMEOUT_SECONDS = codex.MODEL, codex.EFFORT, codex.TIMEOUT_SECONDS
ARMS, MIN_TRIALS_PER_ARM = catalog.ARMS, catalog.MIN_TRIALS_PER_ARM
HARNESS_SOURCE_FILES = tuple(sorted(set((*codex.HARNESS_SOURCE_FILES,
    "benchmarks/cli_typescript_bootstrap.py", "benchmarks/webapp-tokens-v2/dependency_bundle.py",
    "benchmarks/event-sim-tokens-v1/campaign.py", "benchmarks/event-sim-tokens-v1/corpus_io.py",
    "benchmarks/catalog-tokens-v1/SPEC.md", "benchmarks/catalog-tokens-v1/acceptance/corpus.json",
    "benchmarks/catalog-tokens-v1/acceptance/run.py", "benchmarks/catalog-tokens-v1/oracle.py",
    "benchmarks/catalog-tokens-v1/support.py", "benchmarks/catalog-tokens-v1/codex_campaign.py"))))


def plan(args: argparse.Namespace) -> dict[str, Any]:
    binary = catalog.regular(Path(args.semaprax_bin))
    repo = Path(args.repo).resolve(strict=True)
    commit = catalog.resolve_commit(repo, args.base_ref)
    compiler_commit = catalog.resolve_commit(repo, args.compiler_source_ref)
    if compiler_commit != commit:
        raise ValueError("catalog source/base compiler commits must be identical")
    if (args.model, args.effort, args.timeout_seconds) != (MODEL, EFFORT, TIMEOUT_SECONDS):
        raise ValueError("catalog pins gpt-6.1-sol/medium and1800 seconds for both arms")
    if isinstance(args.trials_per_arm, bool) or args.trials_per_arm < 5:
        raise ValueError("catalog requires at least five trials per arm")
    if args.authoring_profile != catalog.AUTHORING_PROFILE_V30:
        raise ValueError("catalog requires explicit v30 authoring-profile selection")
    if args.max_budget_usd is not None:
        raise ValueError("Codex has no strict per-attempt monetary cap")
    artifacts = Path(args.artifacts).resolve()
    if artifacts.exists() or artifacts.is_relative_to(repo):
        raise ValueError("catalog artifacts must be a new external directory")
    inputs = catalog.frozen_inputs(repo, commit)
    qualification = catalog.validate_qualification_evidence(Path(args.qualification_evidence), repo,
        commit, catalog.common.digest(binary))
    if not args.typescript_bootstrap_receipt:
        raise ValueError("strong Node/TypeScript baseline requires its pinned dependency-only receipt")
    receipt_path = catalog.regular(Path(args.typescript_bootstrap_receipt))
    receipt = catalog.ts_bootstrap.validate(receipt_path, args.node_binary, args.npm_binary)
    tooling = {"receipt_path": str(receipt_path), "receipt_sha256": catalog.common.digest(receipt_path),
        "node_binary": args.node_binary, "npm_binary": args.npm_binary,
        **{key: receipt[key] for key in ("inventory_sha256", "package_json_sha256", "package_lock_sha256",
                                       "helper_sha256", "dependency_helper_sha256", "runtime", "packages")}}
    capabilities = codex.capabilities(args.codex_binary)
    if capabilities["status"] != "ready":
        raise ValueError("Codex isolation controls unavailable")
    return {"schema": "semaprax.catalog-codex-campaign.v1", "benchmark": "catalog-tokens-v1",
        "cohort": "catalog-owned-data-v1", "authoring_profile": args.authoring_profile,
        "repository_commit": commit, "compiler_source_commit": compiler_commit,
        "source_binary_sha256": catalog.common.digest(binary), "native_project_route": catalog.ROUTE,
        "qualification_repository": str(repo), "qualification": qualification,
        "benchmark_inputs_sha256": inputs, "seed_files_sha256": {catalog.SPEC_RELATIVE: inputs[catalog.SPEC_RELATIVE]},
        "arms": list(ARMS), "trial_order": [arm for i in range(args.trials_per_arm)
            for arm in (ARMS if i % 2 == 0 else tuple(reversed(ARMS)))],
        "trials_per_arm": args.trials_per_arm, "attempt_denominator": 2 * args.trials_per_arm,
        "artifacts": str(artifacts), "model": MODEL, "effort": EFFORT,
        "model_requested": MODEL, "effort_requested": EFFORT, "timeout_seconds": TIMEOUT_SECONDS,
        "codex_binary": args.codex_binary, "codex_capabilities": capabilities,
        "codex_version": subprocess.run([args.codex_binary, "--version"], capture_output=True,
            text=True, check=True).stdout.strip(),
        "resource_policy": codex.resources.policy(),
        "harness_source_files_sha256": codex.harness_source_inventory(REPO, HARNESS_SOURCE_FILES),
        "typescript_bootstrap": tooling, "typescript_setup_context_tokens": None,
        "authored_source_tokenizer": catalog.common.tokenizer_metadata(args.tokenizer_dir),
        "calibration": {"prompt": catalog.CALIBRATION_PROMPT, "usage": "separate; never subtracted from raw trial usage"},
        "codex_execution": {"stable_context_tokens": None, "actual_billed_usd": None},
        "price_book": {"source": "https://developers.openai.com/api/docs/models/gpt-6.1-sol",
            "standard_short_context_usd_per_million": dict(codex.PRICE_USD_PER_MTOK),
            "conditional": True, "actual_billed_usd": None},
        "language_setup": {"claim": "new independent cohort; per-arm full prompt retained separately; no historical or live savings claim",
            "fixed_harness_context_tokens": None, "reference_solution_supplied": False},
        "seed_checkout": {"included_files": list(catalog.SEED_FILES),
            "excluded": "compiler source, hidden23 corpus/oracle, manual reference candidates, history and generated application source"}}


def command_for(settings: dict[str, Any], prompt: str) -> list[str]:
    command = codex.codex_command(prompt, settings.get("model", MODEL), settings.get("effort", EFFORT))
    command[0] = settings["codex_binary"]
    return command


def workspace_guard(workspace: Path, settings: dict[str, Any]) -> dict[str, Any]:
    """Recheck immutable SPEC and the candidate-only write boundary at each phase."""
    candidate = Path("benchmarks/catalog-tokens-v1/candidate")
    public = Path(catalog.SPEC_RELATIVE)
    allowed_dirs = set(public.parents) | set(candidate.parents)
    expected = settings.get("seed_files_sha256", {}).get(public.as_posix())
    unexpected = []
    try:
        spec_path = workspace / public
        spec_ok = (expected is not None and not spec_path.is_symlink() and spec_path.is_file()
                   and catalog.common.digest(spec_path) == expected)
        for path in workspace.rglob("*"):
            relative = path.relative_to(workspace)
            if relative.parts[0] == ".git":
                continue
            if relative == candidate:
                if path.is_symlink() or not path.is_dir():
                    unexpected.append(relative.as_posix())
            elif candidate in relative.parents:
                continue
            elif relative == public:
                continue
            elif relative in allowed_dirs and path.is_dir() and not path.is_symlink():
                continue
            else:
                unexpected.append(relative.as_posix())
        return {"status": "passed" if spec_ok and not unexpected else "failed",
                "spec_integrity": "passed" if spec_ok else "failed", "outside_candidate_paths": sorted(unexpected)}
    except OSError as error:
        return {"status": "failed", "error": str(error)}


def cleanup_workspace(seed_repo: Path, workspace: Path, settings: dict[str, Any], row: dict[str, Any]) -> None:
    guard = workspace_guard(workspace, settings)
    row["workspace_integrity_before_cleanup"] = guard
    if guard["status"] != "passed":
        invalidate(row, "workspace integrity failed before cleanup")
        return
    removed = subprocess.run(["git", "worktree", "remove", "--force", str(workspace)],
                             cwd=seed_repo, capture_output=True, text=True, check=False)
    row["worktree_removed_after_archive"] = removed.returncode == 0
    if removed.returncode:
        row["workspace_retained_for_review"] = True
        row["worktree_cleanup_error"] = catalog.common.bounded_text(removed.stderr)


def invalidate(row: dict[str, Any], reason: str) -> None:
    row.update({"status": "not_accepted", "failure": reason, "workspace_retained_for_review": True,
                "worktree_removed_after_archive": False})
    if isinstance(row.get("acceptance"), dict):
        row["acceptance"]["accepted"] = False
        row["acceptance"]["invalidated"] = reason


def observe(workspace: Path, artifacts: Path, label: str, stream: Path) -> dict[str, Any]:
    usage = codex.parse_exec_jsonl(stream)
    copied = codex.copy_task_rollout(usage["thread_ids"], workspace,
                                     artifacts / "transcripts" / f"{label}.rollout.jsonl")
    trace = codex.trace_usage(usage, copied) if copied else {"reconciled": False, "model_requests": []}
    return {"rollout_trace": str(copied) if copied else None,
            "observed": {**usage, **trace, "stable_context_tokens": None},
            "list_price": codex.list_price_estimate(trace.get("model_requests", [])),
            "provider_receipt_actual_usd": None}


@codex.resources.guarded_attempt
def launch_trial(seed_repo: Path, artifacts: Path, seed_commit: str, trial: dict[str, Any],
                 settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    """Persist one attempt, reconcile exact task telemetry, and recheck each boundary."""
    catalog.require_authoring_eligibility(settings, semaprax_bin)
    catalog.common.require_compiler_binding(settings, semaprax_bin)
    label = f"{trial['arm']}-{trial['number']:02d}"
    workspace = artifacts / "worktrees" / label
    row: dict[str, Any] = {**trial, "workspace": str(workspace), "status": "failed", "failure": None,
                           "qualification_mode": "evidence_gated_scored"}
    error = catalog.common.add_seed_worktree(seed_repo, workspace, seed_commit, catalog.SEED_FILES)
    if error:
        row.update({"failure": error, "runner_error": True, "workspace_retained_for_review": True})
        return row
    candidate = workspace / "benchmarks/catalog-tokens-v1/candidate"
    tooling = settings.get("typescript_bootstrap") if trial["arm"] == "typescript" else None
    env = catalog.trial_environment(semaprax_bin)
    receipt = None
    if tooling:
        setup_started = time.monotonic()
        try:
            receipt = catalog.ts_bootstrap.verify_plan(tooling)
            row["supplied_tooling"] = catalog.ts_bootstrap.stage(Path(tooling["receipt_path"]), candidate,
                                                                  tooling["node_binary"], tooling["npm_binary"])
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            row.update({"failure": f"TypeScript bootstrap refused before prompt: {error}",
                        "typescript_setup": {"status": "failed", "elapsed_seconds": round(time.monotonic() - setup_started, 3),
                                              "context_tokens": None},
                        "workspace_retained_for_review": True})
            return row
        row["typescript_setup"] = {"status": "ready", "elapsed_seconds": round(time.monotonic() - setup_started, 3),
                                   "context_tokens": None}
        node_path = Path(shutil.which(tooling["node_binary"]) or tooling["node_binary"]).resolve(strict=True)
        env["PATH"] = os.pathsep.join((str(candidate / "node_modules/.bin"), str(node_path.parent), env.get("PATH", "")))
        env["CATALOG_NODE_BINARY"] = str(node_path)
        env["CATALOG_NODE_SHA256"] = tooling["runtime"]["node_binary_sha256"]
        env["CATALOG_TSC_JS"] = str(candidate / "node_modules/typescript/bin/tsc")
    stream, stderr = (artifacts / "transcripts" / f"{label}.{suffix}" for suffix in ("jsonl", "stderr.txt"))
    stream.parent.mkdir(exist_ok=True)
    row.update({"transcript": str(stream), "stderr_path": str(stderr)})
    try:
        with compiler_capture.authoring_compiler(trial["arm"], candidate, workspace, artifacts,
                label, settings, semaprax_bin, row) as authoring_binary:
            prompt = catalog.prompt_for(trial["arm"], candidate, authoring_binary,
                settings.get("authoring_profile", catalog.AUTHORING_PROFILE_V30))
            if tooling:
                prompt += "\n\n" + catalog.ts_bootstrap.prompt_note(receipt)
            (artifacts / "prompts").mkdir(exist_ok=True)
            (artifacts / "prompts" / f"{label}.txt").write_text(prompt, encoding="utf-8")
            row["prompt_sha256"] = catalog.sha_text(prompt)
            catalog.retain_fixed_harness_context(row, settings, prompt)
            authoring_env = {**env, "SEMAPRAX_BIN": str(authoring_binary)} if trial["arm"] == "semaprax" else env
            catalog.require_authoring_eligibility(settings, semaprax_bin)
            catalog.common.require_compiler_binding(settings, semaprax_bin)
            row.update(codex.run_codex(command_for(settings, prompt), workspace,
                                       authoring_env, stream, stderr, settings["timeout_seconds"]))
        row.update(observe(workspace, artifacts, label, stream))
        observed = row["observed"]
        row["telemetry_valid"] = (observed.get("reconciled") is True
            and observed.get("model_observed") == MODEL and observed.get("effort_observed") == EFFORT
            and observed.get("invalid_stream_lines") == 0)
        guard = workspace_guard(workspace, settings)
        row["workspace_integrity_before_acceptance"] = guard
        if tooling:
            intact, evidence = catalog.ts_bootstrap.verify_staged(
                candidate, receipt["inventory"], evidence_path=artifacts / "dependency-evidence" / label)
            row["dependency_tree_integrity"] = evidence
            if not intact:
                invalidate(row, "staged TypeScript dependency tree changed")
                row["dependency_tree_integrity"] = evidence
                row["final_candidate_source_metrics"] = {"status": "measurement_failed", "total_tokens": None,
                    "files": [], "tokenizer": settings.get("authored_source_tokenizer")}
                return row
        if settings.get("authoring_profile") in catalog.PINNED_AUTHORING_PROFILES:
            try:
                row["closed_authored_inventory_after_model"] = catalog.closed_authored_inventory(
                    candidate, exclude_verified_node_modules=bool(tooling))
            except (OSError, ValueError) as error:
                invalidate(row, str(error))
                return row
        if row["timed_out"]:
            row["failure"] = "trial hit wall-clock timeout"
        elif row["process_exit_code"] != 0:
            row["failure"] = f"Codex exited with {row['process_exit_code']}"
        elif not observed.get("reconciled"):
            row["failure"] = "missing or unreconciled task-owned rollout usage"
        elif observed.get("model_observed") != MODEL or observed.get("effort_observed") != EFFORT:
            row["failure"] = "observed rollout model or effort differs from pinned request"
        elif not row["telemetry_valid"]:
            row["failure"] = "invalid Codex event stream"
        elif guard["status"] != "passed":
            invalidate(row, "workspace integrity failed before acceptance")
        else:
            admission = catalog.candidate_authoring_admission(
                candidate, trial["arm"], settings.get("authoring_profile", catalog.AUTHORING_PROFILE_V30))
            row["authoring_admission"] = admission
            if admission["status"] == "failed":
                row.update({"status": "not_accepted",
                            "failure": "candidate failed exact authoring-profile admission"})
            else:
                started = time.monotonic()
                if settings.get("authoring_profile") in catalog.PINNED_AUTHORING_PROFILES:
                    row["acceptance"] = catalog.check_program(candidate, settings["timeout_seconds"],
                        env, "evidence_gated_scored",
                        settings["authoring_profile"],
                        artifacts / "harness-native" / label / "catalog",
                        settings["qualification"].get("compiler_binary_sha256"),
                        row.get("closed_authored_inventory_after_model"), trial["arm"],
                        exclude_verified_node_modules=bool(tooling))
                else:
                    row["acceptance"] = catalog.check_program(candidate, settings["timeout_seconds"],
                        env, "evidence_gated_scored", exclude_verified_node_modules=bool(tooling))
                row["acceptance_elapsed_seconds"] = round(time.monotonic() - started, 3)
                row["status"] = "accepted" if row["acceptance"]["accepted"] else "not_accepted"
                if row["status"] != "accepted":
                    row["failure"] = "candidate failed build or acceptance checks"
    except (OSError, RuntimeError, ValueError, UnicodeError) as error:
        row.update({"failure": str(error), "runner_error": True, "status": "failed"})
    try:
        if tooling:
            intact, evidence = catalog.ts_bootstrap.verify_staged(
                candidate, receipt["inventory"], evidence_path=artifacts / "dependency-evidence" / f"{label}-before-metrics")
            row["dependency_tree_integrity_before_metrics"] = evidence
            if not intact:
                invalidate(row, "staged TypeScript dependency tree changed before source measurement")
                row["dependency_tree_integrity_before_metrics"] = evidence
                return row
        row["final_candidate_source_metrics"] = catalog.common.authored_source_metrics(
            candidate, settings.get("authored_source_tokenizer"),
            exclude_verified_node_modules=bool(tooling),
            additional_suffixes=catalog.ADDITIONAL_AUTHORED_SUFFIXES)
    except (OSError, RuntimeError, ValueError, UnicodeError) as error:
        row["final_candidate_source_metrics"] = {"status": "measurement_failed", "total_tokens": None,
            "files": [], "tokenizer": settings.get("authored_source_tokenizer"), "error": str(error)}
    if settings.get("authoring_profile") in catalog.PINNED_AUTHORING_PROFILES:
        acceptance = row.get("acceptance", {})
        native = acceptance.get("native_binary", {})
        native_path = Path(native["path"]) if native.get("path") else None
        pinned = acceptance.get("pinned_compiler", {})
        compiler_path = Path(pinned["path"]) if pinned.get("path") else None
        expected_inventory = acceptance.get(
            "closed_authored_inventory", row.get("closed_authored_inventory_after_model"))
        passed, phase_guard = catalog._phase_source_and_binary_guard(
            candidate, expected_inventory, native_path, native.get("sha256"),
            compiler_path, pinned.get("sha256"), exclude_verified_node_modules=bool(tooling),
            expected_runtime=acceptance.get("runtime_artifacts"))
        row["source_consistency_after_metrics"] = phase_guard
        if not passed:
            invalidate(row, "candidate source, compiler, or harness native binary changed")
            return row
    guard = workspace_guard(workspace, settings)
    row["workspace_integrity_before_archive"] = guard
    if guard["status"] != "passed":
        invalidate(row, "workspace integrity failed before archive")
        return row
    if tooling:
        intact, evidence = catalog.ts_bootstrap.verify_staged(
            candidate, receipt["inventory"], evidence_path=artifacts / "dependency-evidence" / f"{label}-before-archive")
        row["dependency_tree_integrity_before_archive"] = evidence
        if not intact:
            invalidate(row, "staged TypeScript dependency tree changed before archive")
            row["dependency_tree_integrity_before_archive"] = evidence
            row["final_candidate_source_metrics"] = {"status": "measurement_failed", "total_tokens": None,
                "files": [], "tokenizer": settings.get("authored_source_tokenizer")}
            return row
    archive = artifacts / "candidates" / label
    archive.parent.mkdir(parents=True, exist_ok=True)
    try:
        hashes, omitted = catalog.common.archive_candidate(
            candidate, archive, exclude_verified_node_modules=bool(tooling))
        row.update({"candidate_archive": str(archive), "candidate_source_sha256": hashes,
                    "candidate_archive_excluded_paths": omitted})
    except (OSError, RuntimeError, ValueError) as error:
        row.update({"runner_error": True, "workspace_retained_for_review": True,
                    "failure": f"candidate archive failed: {error}", "status": "failed"})
        if isinstance(row.get("acceptance"), dict):
            row["acceptance"]["accepted"] = False
        return row
    if settings.get("authoring_profile") in catalog.PINNED_AUTHORING_PROFILES:
        acceptance = row.get("acceptance", {})
        native = acceptance.get("native_binary", {})
        native_path = Path(native["path"]) if native.get("path") else None
        pinned = acceptance.get("pinned_compiler", {})
        compiler_path = Path(pinned["path"]) if pinned.get("path") else None
        expected_inventory = acceptance.get(
            "closed_authored_inventory", row.get("closed_authored_inventory_after_model"))
        passed, phase_guard = catalog._phase_source_and_binary_guard(
            candidate, expected_inventory, native_path, native.get("sha256"),
            compiler_path, pinned.get("sha256"), exclude_verified_node_modules=bool(tooling),
            expected_runtime=acceptance.get("runtime_artifacts"))
        try:
            archived_inventory = catalog.closed_authored_inventory(archive)
            archive_matches = archived_inventory == expected_inventory
        except (OSError, ValueError) as error:
            archived_inventory = {"status": "failed", "error": str(error)}
            archive_matches = False
        phase_guard["archived_inventory"] = archived_inventory
        phase_guard["archive_matches"] = archive_matches
        passed = passed and archive_matches
        row["source_consistency_after_archive"] = phase_guard
        if not passed:
            invalidate(row, "candidate source, compiler, or harness native binary changed")
            return row
    cleanup_workspace(seed_repo, workspace, settings, row)
    return row


@codex.resources.guarded_attempt
def launch_calibration(seed_repo: Path, artifacts: Path, seed_commit: str,
                       settings: dict[str, Any], semaprax_bin: Path) -> dict[str, Any]:
    """A separate tool-free READY request, with the same isolation and cleanup."""
    catalog.require_authoring_eligibility(settings, semaprax_bin)
    catalog.common.require_compiler_binding(settings, semaprax_bin)
    workspace = artifacts / "worktrees" / "calibration"
    row: dict[str, Any] = {"status": "failed", "failure": None, "separate_from_trials": True,
                           "subtracted_from_trials": False}
    error = catalog.common.add_seed_worktree(seed_repo, workspace, seed_commit, catalog.SEED_FILES)
    if error:
        row["failure"] = error
        return row
    stream, stderr = (artifacts / "transcripts" / f"calibration.{suffix}" for suffix in ("jsonl", "stderr.txt"))
    stream.parent.mkdir(parents=True, exist_ok=True)
    row.update({"transcript": str(stream), "stderr_path": str(stderr)})
    try:
        catalog.require_authoring_eligibility(settings, semaprax_bin)
        catalog.common.require_compiler_binding(settings, semaprax_bin)
        row.update(codex.run_codex(command_for(settings, catalog.CALIBRATION_PROMPT), workspace,
                                   catalog.trial_environment(semaprax_bin), stream, stderr, settings["timeout_seconds"]))
        row.update(observe(workspace, artifacts, "calibration", stream))
        observed = row["observed"]
        events = [json.loads(line) for line in stream.read_text().splitlines() if line]
        messages = [event.get("item") for event in events if isinstance(event, dict)]
        replies = [item.get("text", "").strip() for item in messages
                   if isinstance(item, dict) and item.get("type") == "agent_message"]
        tools = observed.get("tool_item_counts", {})
        row["status"] = "ready" if (not row["timed_out"] and row["process_exit_code"] == 0
            and observed.get("reconciled") and observed.get("model_observed") == MODEL
            and observed.get("effort_observed") == EFFORT and replies == ["READY"]
            and observed.get("invalid_stream_lines") == 0
            and all(kind == "agent_message" or not count for kind, count in tools.items())) else "failed"
        if row["status"] == "failed":
            row["failure"] = "calibration failed tool-free READY, model/effort, or telemetry checks"
    except (OSError, RuntimeError, ValueError, UnicodeError) as error:
        row["failure"] = str(error)
    cleanup_workspace(seed_repo, workspace, settings, row)
    return row


def run_campaign(args: argparse.Namespace) -> dict[str, Any]:
    settings = plan(args)
    catalog.require_authoring_eligibility(settings, Path(args.semaprax_bin))
    catalog.common.require_compiler_binding(settings, Path(args.semaprax_bin))
    catalog.ts_bootstrap.verify_plan(settings.get("typescript_bootstrap"))
    artifacts, binary = Path(settings["artifacts"]), Path(args.semaprax_bin).expanduser().resolve()
    artifacts.mkdir(parents=True)
    snapshot = codex.snapshot_harness_sources(REPO, artifacts, settings["harness_source_files_sha256"],
        settings["seed_files_sha256"]["benchmarks/catalog-tokens-v1/SPEC.md"],
        relative_files=HARNESS_SOURCE_FILES, spec_path="benchmarks/catalog-tokens-v1/SPEC.md")
    settings["harness_source_snapshot"] = snapshot
    qualification = settings["qualification"]
    catalog.copy_qualification_artifacts(qualification, artifacts)
    seed = catalog.common.create_seed_repository(Path(args.repo).resolve(), settings["repository_commit"],
                                                   artifacts / "seed-repository", catalog.SEED_FILES)
    settings.update(seed); settings["semaprax_binary"] = str(binary)
    (artifacts / "campaign.json").write_text(json.dumps(settings, indent=2, sort_keys=True) + "\n")
    calibration = launch_calibration(artifacts / "seed-repository", artifacts, seed["seed_repository_commit"], settings, binary)
    (artifacts / "calibration.json").write_text(json.dumps(calibration, indent=2, sort_keys=True) + "\n")
    rows: list[dict[str, Any]] = []; remaining = list(settings["trial_order"])
    if calibration["status"] == "ready":
        numbers = {arm: 0 for arm in ARMS}
        while remaining:
            arm = remaining.pop(0); numbers[arm] += 1
            row = launch_trial(artifacts / "seed-repository", artifacts, seed["seed_repository_commit"],
                               {"arm": arm, "number": numbers[arm]}, settings, binary)
            rows.append(row)
            (artifacts / "attempts").mkdir(exist_ok=True)
            (artifacts / "attempts" / f"{arm}-{numbers[arm]:02d}.json").write_text(
                json.dumps(row, indent=2, sort_keys=True) + "\n")
            # A nonzero Codex exit can include quota exhaustion; do not retry or launch later paid attempts.
            if (row.get("resource_assessment", {}).get("contaminated")
                    or row.get("runner_error") or row.get("workspace_retained_for_review")
                    or (not row.get("timed_out") and (
                        row.get("process_exit_code") not in (0, None)
                        or row.get("telemetry_valid") is False))):
                break
    report = {"campaign": settings, "calibration": calibration, "trials": rows,
              "attempt_denominator": settings["attempt_denominator"], "recorded_attempts": len(rows),
              "unlaunched_trial_order": remaining,
              "campaign_status": "complete" if not remaining and calibration["status"] == "ready" else "interrupted"}
    (artifacts / "results.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    qualification = sub.add_parser("qualify", help="unpaid real native build and original23 acceptance")
    qualification.add_argument("--repo", default=str(REPO))
    qualification.add_argument("--compiler-source-ref", required=True)
    qualification.add_argument("--semaprax-bin", required=True)
    qualification.add_argument("--candidate", required=True)
    qualification.add_argument("--output", required=True)
    qualification.add_argument("--timeout-seconds", type=int, default=1800)
    for action in ("plan", "run"):
        p = sub.add_parser(action)
        p.add_argument("--repo", default=str(REPO)); p.add_argument("--base-ref", required=True)
        p.add_argument("--compiler-source-ref", required=True); p.add_argument("--semaprax-bin", required=True)
        p.add_argument("--qualification-evidence", required=True); p.add_argument("--artifacts", required=True)
        p.add_argument("--authoring-profile", required=True, choices=(catalog.AUTHORING_PROFILE_V30,))
        p.add_argument("--trials-per-arm", type=int, default=5)
        p.add_argument("--model", default=MODEL); p.add_argument("--effort", default=EFFORT)
        p.add_argument("--timeout-seconds", type=int, default=1800); p.add_argument("--max-budget-usd", type=float, default=None)
        p.add_argument("--tokenizer-dir", default=None); p.add_argument("--codex-binary", default="codex")
        p.add_argument("--typescript-bootstrap-receipt", required=True)
        p.add_argument("--node-binary", default="node"); p.add_argument("--npm-binary", default="npm")
        if action == "run": p.add_argument("--acknowledge-paid-attempts", action="store_true")
    args = parser.parse_args()
    try:
        if args.action == "qualify":
            repo = Path(args.repo).resolve(strict=True)
            result = catalog.qualify(Path(args.candidate), Path(args.semaprax_bin), repo,
                catalog.resolve_commit(repo, args.compiler_source_ref), Path(args.output), args.timeout_seconds)
        elif args.action == "plan":
            result = plan(args)
        else:
            if not args.acknowledge_paid_attempts:
                raise ValueError("run requires explicit --acknowledge-paid-attempts")
            result = run_campaign(args)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if result.get("campaign_status", "complete") == "complete" else 2
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Catalog campaign error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
