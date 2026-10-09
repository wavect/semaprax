"""Catalog-only identity, qualification and runtime checks; no model dispatch."""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from typing import Any

BENCHMARK = Path(__file__).resolve().parent
REPO = BENCHMARK.parents[1]
sys.path.insert(0, str(REPO / "benchmarks"))
import live_campaign_common as common
import cli_typescript_bootstrap as ts_bootstrap

# Reuse only pure closed-source inventory/phase primitives. Do not rewrite any
# existing cohort's globals, corpus, prompt or qualification route.
_shared_spec = importlib.util.spec_from_file_location(
    "_catalog_closed_inventory", REPO / "benchmarks/event-sim-tokens-v1/campaign.py")
if _shared_spec is None or _shared_spec.loader is None:
    raise RuntimeError("cannot load closed inventory helpers")
sys.path.insert(0, str(REPO / "benchmarks/event-sim-tokens-v1"))
shared = importlib.util.module_from_spec(_shared_spec)
_shared_spec.loader.exec_module(shared)
closed_authored_inventory = shared.closed_authored_inventory
copy_qualification_artifacts = shared.copy_qualification_artifacts
resolve_commit = shared.resolve_commit
blob_at_commit = shared.blob_at_commit
sha_text = shared.sha_text
sha_bytes = shared.sha_bytes
_bound_file = shared._bound_file

MODEL, EFFORT = "gpt-6.1-sol", "medium"
ARMS, MIN_TRIALS_PER_ARM = ("semaprax", "typescript"), 5
AUTHORING_PROFILE_V30 = "semaprax-project-v30-owned-data-v1"
PINNED_AUTHORING_PROFILES = (AUTHORING_PROFILE_V30,)
SPEC_RELATIVE = "benchmarks/catalog-tokens-v1/SPEC.md"
SEED_FILES = ("/" + SPEC_RELATIVE,)
CANDIDATE_RELATIVE = "benchmarks/catalog-tokens-v1/candidate"
CALIBRATION_PROMPT = shared.CALIBRATION_PROMPT
QUALIFICATION_SCHEMA = "semaprax.catalog-qualification.v1"
BUILD_RECEIPT_SCHEMA = "semaprax.catalog-qualification-build.v1"
REPORT_SCHEMA = "semaprax.catalog-acceptance.v1"
ROUTE = shared.AUTHORING_PROFILES[shared.AUTHORING_PROFILE_V30]["route"]
AUTHORING_PROFILES = {AUTHORING_PROFILE_V30: {
    "route": ROUTE, "codex_campaign_schema": "semaprax.catalog-codex-campaign.v1"}}
FROZEN_INPUTS = {'benchmarks/catalog-tokens-v1/SPEC.md': 'aa8073239954deb802bbd0278412a76fa8bb367f93d48ce91ecd09a1222893ad', 'benchmarks/catalog-tokens-v1/acceptance/corpus.json': 'd8469e9f91d832de41084ce12c97390c682f5b4aef30df075ec3b7f24db2ed73', 'benchmarks/catalog-tokens-v1/oracle.py': 'ed7532a00869333a06e6e5a242d1799f7a106db6fb47efd698ec822bed15590b'}


def regular(path: Path) -> Path:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"expected a regular file: {path}")
    return path.resolve(strict=True)


def frozen_inputs(repo: Path, commit: str) -> dict[str, str]:
    hashes = {path: sha_bytes(blob_at_commit(repo, commit, path)) for path in FROZEN_INPUTS}
    if hashes != FROZEN_INPUTS or any(common.digest(REPO / path) != digest for path, digest in hashes.items()):
        raise ValueError("catalog cohort requires unchanged original SPEC,23 corpus and independent oracle")
    return hashes


def candidate_authoring_admission(candidate: Path, arm: str, authoring_profile: str) -> dict[str, Any]:
    if authoring_profile != AUTHORING_PROFILE_V30:
        return {"status": "failed", "error": "catalog cohort requires explicit v30 owned-data profile"}
    if arm == "typescript":
        if (candidate / "semaprax.toml").exists() or (candidate / "semaprax.toml").is_symlink():
            return {"status": "failed", "error": "TypeScript arm must not contain a SEMAPRAX manifest"}
        return {"status": "passed", "route": "pinned Node; authored TypeScript; retained dist/catalog.mjs closure"}
    if arm != "semaprax":
        return {"status": "failed", "error": "unknown catalog arm"}
    return shared.candidate_authoring_admission(candidate, arm, authoring_profile)


def verify_report(data: bytes, expected_command: list[str] | None = None) -> dict[str, Any]:
    report = json.loads(data)
    corpus = json.loads((BENCHMARK / "acceptance/corpus.json").read_bytes())
    cases = corpus["cases"]
    if (not isinstance(report, dict) or report.get("schema") != REPORT_SCHEMA or report.get("accepted") is not True
            or report.get("required_cases") != 23 or len(cases) != 23
            or report.get("corpus_sha256") != common.digest(BENCHMARK / "acceptance/corpus.json")
            or not isinstance(report.get("cases"), list) or len(report["cases"]) != 23):
        raise ValueError("catalog qualification requires all original23 cases")
    if expected_command is not None and report.get("command") != expected_command:
        raise ValueError("acceptance report does not bind the actual qualified command")
    for actual, case in zip(report["cases"], cases):
        raw = bytes.fromhex(case["input_hex"])
        if (not isinstance(actual, dict) or actual.get("name") != case["name"] or actual.get("accepted") is not True
                or type(actual.get("status")) is not int or actual.get("status") != case["status"]
                or actual.get("stdout_hex") != case["stdout_hex"]
                or actual.get("stderr_hex") != case["stderr_hex"]
                or actual.get("input_bytes") != len(raw) or actual.get("input_sha256") != sha_bytes(raw)
                or actual.get("timeout") is True):
            raise ValueError(f"catalog acceptance differs from frozen case: {case['name']}")
    return report


def validate_qualification_evidence(path: Path, repo: Path, commit: str, compiler_hash: str) -> dict[str, Any]:
    regular(path)
    frozen_inputs(repo, commit)
    evidence = json.loads(path.read_bytes())
    if (not isinstance(evidence, dict) or evidence.get("schema") != QUALIFICATION_SCHEMA or evidence.get("native_project_route") != ROUTE
            or evidence.get("compiler_source_commit") != commit
            or evidence.get("compiler_binary_sha256") != compiler_hash
            or evidence.get("benchmark_inputs_sha256") != FROZEN_INPUTS):
        raise ValueError("catalog qualification compiler/source/profile or frozen-input binding differs")
    if not isinstance(evidence.get("candidate_source"), dict):
        raise ValueError("catalog qualification requires closed source inventory and manifest")
    binding = {}
    for reference, label, path_key, hash_key in (
        (evidence.get("acceptance_report"), "acceptance report", "acceptance_report_path", "acceptance_report_sha256"),
        (evidence.get("candidate_source", {}).get("inventory"), "source inventory", "candidate_source_inventory_path", "candidate_source_inventory_sha256"),
        (evidence.get("candidate_source", {}).get("manifest"), "manifest", "candidate_manifest_path", "candidate_manifest_sha256"),
        (evidence.get("qualified_native_binary"), "native binary", "qualified_native_binary_path", "qualified_native_binary_sha256"),
        (evidence.get("qualification_build_receipt"), "build receipt", "qualification_build_receipt_path", "qualification_build_receipt_sha256"),
    ):
        file, data, digest = _bound_file(reference, path, label)
        binding[path_key], binding[hash_key] = str(file), digest
    inventory = json.loads(Path(binding["candidate_source_inventory_path"]).read_bytes())
    rows = shared._validate_closed_inventory_document(inventory)
    manifest = Path(binding["candidate_manifest_path"]).read_bytes()
    shared._manifest_route(manifest, AUTHORING_PROFILE_V30)
    if [row for row in rows if row["path"] == "semaprax.toml"] != [{
            "path": "semaprax.toml", "bytes": len(manifest), "sha256": sha_bytes(manifest)}]:
        raise ValueError("closed catalog inventory does not bind exact manifest")
    subject = {"compiler_source_commit": commit, "compiler_binary_sha256": compiler_hash,
        "closed_authored_inventory_sha256": inventory["sha256"],
        "candidate_manifest_sha256": sha_bytes(manifest),
        "native_binary_sha256": binding["qualified_native_binary_sha256"]}
    receipt = json.loads(Path(binding["qualification_build_receipt_path"]).read_bytes())
    if (not isinstance(receipt, dict) or evidence.get("qualification_subject") != subject or receipt.get("schema") != BUILD_RECEIPT_SCHEMA
            or receipt.get("qualification_subject") != subject
            or receipt.get("acceptance_report_sha256") != binding["acceptance_report_sha256"]):
        raise ValueError("catalog build receipt does not bind the accepted source/native subject")
    report = verify_report(Path(binding["acceptance_report_path"]).read_bytes(),
                           [binding["qualified_native_binary_path"]])
    return {"status": "evidence_gate_passed", "scored_trials_allowed": True,
        "evidence_path": str(path.resolve()), "evidence_sha256": common.digest(path),
        "compiler_source_commit": commit, "compiler_binary_sha256": compiler_hash,
        "native_project_route": ROUTE, "qualification_subject": subject,
        "closed_authored_inventory_sha256": inventory["sha256"], "acceptance_cases_passed": len(report["cases"]),
        **binding}


def require_authoring_eligibility(settings: dict[str, Any], compiler: Path) -> None:
    common.require_compiler_binding(settings, compiler)
    if (settings.get("cohort") != "catalog-owned-data-v1"
            or settings.get("authoring_profile") != AUTHORING_PROFILE_V30
            or settings.get("native_project_route") != ROUTE):
        raise ValueError("catalog dispatch requires its explicit independently qualified cohort/profile")
    qualified = settings.get("qualification", {})
    if (not isinstance(qualified, dict) or qualified.get("scored_trials_allowed") is not True
            or not isinstance(qualified.get("evidence_path"), str)
            or not isinstance(settings.get("qualification_repository"), str)
            or not isinstance(settings.get("compiler_source_commit"), str)):
        raise ValueError("catalog dispatch requires fresh closed source/compiler/profile all23 qualification")
    fresh = validate_qualification_evidence(Path(qualified.get("evidence_path", "")),
        Path(settings["qualification_repository"]), settings["compiler_source_commit"], common.digest(compiler))
    artifact_keys = {"evidence_artifact": "evidence_sha256", "acceptance_report_artifact": "acceptance_report_sha256",
        "candidate_source_inventory_artifact": "candidate_source_inventory_sha256", "candidate_manifest_artifact": "candidate_manifest_sha256",
        "qualification_build_receipt_artifact": "qualification_build_receipt_sha256", "qualified_native_binary_artifact": "qualified_native_binary_sha256"}
    if fresh != {key: value for key, value in qualified.items() if key not in artifact_keys}:
        raise ValueError("catalog qualification changed after planning")
    for key, digest_key in artifact_keys.items():
        if key in qualified and common.digest(regular(Path(qualified[key]))) != fresh[digest_key]:
            raise ValueError("catalog retained qualification copy changed")
    if not settings.get("typescript_bootstrap"):
        raise ValueError("catalog requires the strong pinned Node/TypeScript bootstrap")
    ts_bootstrap.verify_plan(settings["typescript_bootstrap"])


def trial_environment(compiler: Path) -> dict[str, str]:
    return shared.trial_environment(compiler)


def retain_fixed_harness_context(row: dict[str, Any], settings: dict[str, Any], prompt: str) -> None:
    row["fixed_harness_context"] = {"prompt_sha256": sha_text(prompt),
        "prompt_utf8_bytes": len(prompt.encode()), "tokens": None, "actual_billed_usd": None,
        "scope": "full retained task/harness prompt including paths and TS setup note; separate from authored/generated source"}


def prompt_for(arm: str, candidate: Path, compiler: Path, authoring_profile: str) -> str:
    if arm not in ARMS or authoring_profile != AUTHORING_PROFILE_V30:
        raise ValueError("unsupported catalog authoring arm/profile")
    language = "SEMAPRAX native Project" if arm == "semaprax" else "idiomatic TypeScript on Node.js"
    base = f"""Implement the complete Catalog restock command in {language} according to
`{SPEC_RELATIVE}`. Write all application source, build.sh, run.sh and test.sh
under `{candidate}` only. The public SPEC is the only application input;
the independent acceptance corpus, oracle, reference applications and original
repository history are absent. Preserve every requirement, including repeated
keys, decoded identifiers, maximum cardinalities and unlimited raw whitespace.
Use an offline build.sh and automated test.sh that fail nonzero on errors.
run.sh must read stdin and publish exactly the contracted bytes/status.
No application source, generated bodies or reference solution is supplied.
Use built-in language/runtime help and the SPEC; do not read other materials.
"""
    if arm == "semaprax":
        return base + f"""Use the compiler at `{compiler}` (also $SEMAPRAX_BIN), Project v30 profile
`language-command-io.owned-data.v1`, input `argv-utf8+stdin-stream.v1`, a single
external fn() -> i64 command/export, and exactly the sorted grants
process.args.read, process.stderr.write, process.stdin.read, process.stdout.write.
Private admitted owned records/collections remain subject to ordinary checks.
Read `$SEMAPRAX_BIN help language author:owned-data` for exact shapes and
unchanged transitive Bytes allocation/deep-clone loop restrictions. Derivation
is optional; discover checked commands from built-in help rather than assumed APIs.
Build a native command and have run.sh execute it; ordinary `semaprax run`
is not the selected command process adapter. Invalid application input is
status 2 with the specified stderr, not a failed contract/runtime failure.
"""
    return base + """Use the supplied pinned TypeScript compiler and Node from PATH. Author all
package.json, tsconfig.json and application files. build.sh must produce
`dist/catalog.mjs` as the command entry (other local compiled modules may live
under dist). run.sh must execute Node on that entry. Use idiomatic native
JavaScript/TypeScript data structures and JSON processing with exact validation;
no SEMAPRAX representation restriction is imposed on the TypeScript arm.
Do not install packages or fetch network resources. The harness will retain the
complete local dist closure and execute it with the pinned Node binary.
"""


def runtime_inventory(root: Path) -> dict[str, Any]:
    if root.is_symlink() or not root.is_dir():
        raise ValueError("compiled TypeScript runtime must be a regular directory")
    files, total = [], 0
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise ValueError("compiled TypeScript runtime cannot contain symlinks")
        if path.is_dir():
            continue
        regular(path)
        total += path.stat().st_size
        if total > 16 * 1024 * 1024 or len(files) >= 4096:
            raise ValueError("compiled runtime artifact retention limit exceeded; not an input-byte limit")
        files.append({"path": path.relative_to(root).as_posix(), "bytes": path.stat().st_size,
                      "sha256": common.digest(path)})
    return {"root": str(root.resolve()), "files": files}


def _phase_source_and_binary_guard(candidate, inventory, native, native_hash, compiler, compiler_hash,
                                   *, exclude_verified_node_modules=False, expected_runtime=None):
    passed, result = shared._phase_source_and_binary_guard(candidate, inventory, native, native_hash,
        compiler, compiler_hash, exclude_verified_node_modules=exclude_verified_node_modules)
    if expected_runtime is not None:
        try:
            node = regular(Path(expected_runtime["node_binary"]))
            observed = runtime_inventory(Path(expected_runtime["root"]))
            intact = (observed["files"] == expected_runtime["files"]
                      and common.digest(node) == expected_runtime["node_binary_sha256"]
                      and common.digest(regular(Path(expected_runtime["tsc_path"]))) == expected_runtime["tsc_sha256"])
            result["typescript_runtime"] = {"status": "passed" if intact else "failed", "inventory": observed}
            passed = passed and intact
        except (OSError, ValueError) as error:
            result["typescript_runtime"] = {"status": "failed", "error": str(error)}
            passed = False
    return passed, result


def check_program(candidate, timeout, env, qualification_mode="evidence_gated_scored",
                  authoring_profile=AUTHORING_PROFILE_V30, harness_output=None,
                  compiler_binary_sha256=None, expected_inventory=None, arm=None,
                  *, exclude_verified_node_modules=False):
    result = {"accepted": False, "qualification_mode": qualification_mode}
    admission = candidate_authoring_admission(candidate, arm, authoring_profile)
    result["authoring_admission"] = admission
    if admission["status"] != "passed":
        return result
    initial = closed_authored_inventory(candidate, exclude_verified_node_modules=exclude_verified_node_modules)
    if expected_inventory is not None and initial != expected_inventory:
        result["source_consistency"] = {"status": "failed"}
        return result
    result["closed_authored_inventory"] = initial
    regular(candidate / "run.sh")
    if harness_output is None:
        raise ValueError("catalog acceptance needs a fresh external runtime output path")
    harness_output.parent.mkdir(parents=True, exist_ok=False)
    compiler = regular(Path(env["SEMAPRAX_BIN"])) if arm == "semaprax" else None
    if compiler is not None and common.digest(compiler) != compiler_binary_sha256:
        raise ValueError("catalog compiler differs from qualification")
    runtime = None
    locked_native, locked_native_hash = None, None
    commands = []
    if compiler is not None:
        commands += [("pinned_compiler_check", [str(compiler), "check", "--manifest-path", str(candidate / "semaprax.toml")]),
            ("pinned_native_build", [str(compiler), "build", "--manifest-path", str(candidate / "semaprax.toml"),
                                    "--target", "native", "--output", str(harness_output)])]
    commands += [("build", ["/bin/sh", str(regular(candidate / "build.sh"))]),
                 ("candidate_tests", ["/bin/sh", str(regular(candidate / "test.sh"))])]
    for key, command in commands:
        try:
            completed = subprocess.run(command, cwd=candidate, capture_output=True, check=False,
                                       timeout=timeout, env=env)
            result[key] = {"status": "passed" if completed.returncode == 0 else "failed",
                "exit_code": completed.returncode, "stdout": common.bounded_text(completed.stdout),
                "stderr": common.bounded_text(completed.stderr)}
        except subprocess.TimeoutExpired as error:
            result[key] = {"status": "timeout", "stdout": common.bounded_text(error.stdout or b""),
                           "stderr": common.bounded_text(error.stderr or b"")}
        if key == "pinned_native_build" and result[key]["status"] == "passed":
            locked_native = regular(harness_output)
            locked_native_hash = common.digest(locked_native)
        intact, guard = _phase_source_and_binary_guard(candidate, initial, locked_native, locked_native_hash, compiler,
            compiler_binary_sha256, exclude_verified_node_modules=exclude_verified_node_modules)
        result[key + "_source_consistency"] = guard
        if result[key]["status"] != "passed" or not intact:
            return result
    if arm == "semaprax":
        binary = regular(harness_output)
        command = [str(binary)]
    else:
        if not any(Path(row["path"]).suffix in (".ts", ".tsx") for row in initial["files"]):
            raise ValueError("strong TypeScript arm requires actual authored TypeScript source")
        node = regular(Path(env["CATALOG_NODE_BINARY"]))
        if common.digest(node) != env["CATALOG_NODE_SHA256"]:
            raise ValueError("pinned Node runtime changed before acceptance")
        runtime_root = harness_output.parent / "typescript-runtime"
        tsc = regular(Path(env["CATALOG_TSC_JS"]))
        config = regular(candidate / "tsconfig.json")
        tsc_command = [str(node), str(tsc), "--project", str(config), "--outDir", str(runtime_root),
            "--noEmitOnError", "--noEmit", "false", "--allowJs", "false", "--incremental", "false",
            "--composite", "false", "--declaration", "false", "--emitDeclarationOnly", "false"]
        try:
            compiled = subprocess.run(tsc_command, cwd=candidate, capture_output=True, check=False,
                                      timeout=timeout, env=env)
            result["pinned_typescript_build"] = {"status": "passed" if compiled.returncode == 0 else "failed",
                "command": tsc_command, "exit_code": compiled.returncode,
                "stdout": common.bounded_text(compiled.stdout), "stderr": common.bounded_text(compiled.stderr)}
        except subprocess.TimeoutExpired as error:
            result["pinned_typescript_build"] = {"status": "timeout",
                "stdout": common.bounded_text(error.stdout or b""), "stderr": common.bounded_text(error.stderr or b"")}
        intact, guard = _phase_source_and_binary_guard(candidate, initial, None, None, None, None,
            exclude_verified_node_modules=exclude_verified_node_modules)
        result["pinned_typescript_build_source_consistency"] = guard
        if not intact or result["pinned_typescript_build"]["status"] != "passed":
            return result
        after = runtime_inventory(runtime_root)
        binary = regular(runtime_root / "catalog.mjs")
        runtime = {**after, "node_binary": str(node), "node_binary_sha256": common.digest(node),
                   "tsc_path": str(tsc), "tsc_sha256": common.digest(tsc)}
        result["runtime_artifacts"] = runtime
        command = [str(node), str(binary)]
    binary_hash = common.digest(binary)
    result["native_binary"] = {"path": str(binary), "sha256": binary_hash,
                              "kind": "native" if arm == "semaprax" else "retained-typescript-entry"}
    report_path = harness_output.parent / "acceptance-report.json"
    try:
        accepted = subprocess.run([sys.executable, str(BENCHMARK / "acceptance/run.py"),
            "--command-json", json.dumps(command), "--report-json", str(report_path), "--timeout", str(timeout)],
            cwd=candidate, capture_output=True, check=False, timeout=timeout, env=env)
        result["independent_acceptance"] = {"status": "passed" if accepted.returncode == 0 else "failed",
            "exit_code": accepted.returncode, "stdout": common.bounded_text(accepted.stdout),
            "stderr": common.bounded_text(accepted.stderr)}
        if accepted.returncode == 0:
            verify_report(regular(report_path).read_bytes(), command)
            result["acceptance_report"] = {"path": str(report_path), "sha256": common.digest(report_path)}
    except subprocess.TimeoutExpired as error:
        result["independent_acceptance"] = {"status": "timeout",
            "stdout": common.bounded_text(error.stdout or b""), "stderr": common.bounded_text(error.stderr or b"")}
    intact, guard = _phase_source_and_binary_guard(candidate, initial, binary, binary_hash, compiler,
        compiler_binary_sha256, exclude_verified_node_modules=exclude_verified_node_modules, expected_runtime=runtime)
    result["source_consistency"] = guard
    if compiler is not None:
        result["pinned_compiler"] = {"path": str(compiler), "sha256": compiler_binary_sha256,
                                     "unchanged_after_acceptance": intact}
    result["accepted"] = intact and result["independent_acceptance"]["status"] == "passed"
    return result


def qualify(candidate: Path, compiler: Path, repo: Path, commit: str, output: Path, timeout: int):
    """Unpaid real build/all23 producer; failure retains diagnostics, never eligibility."""
    compiler = regular(compiler)
    candidate, output = candidate.resolve(strict=True), output.resolve()
    if timeout <= 0 or output.exists() or output.is_relative_to(candidate) or output.is_relative_to(repo.resolve()):
        raise ValueError("qualification requires positive timeout and a new external output directory")
    hashes = frozen_inputs(repo, commit)
    output.mkdir(parents=True)
    original = closed_authored_inventory(candidate)
    inventory_path, manifest_path = output / "source-inventory.json", output / "semaprax.toml"
    inventory_path.write_text(json.dumps(original, indent=2, sort_keys=True) + "\n")
    shutil.copyfile(regular(candidate / "semaprax.toml"), manifest_path)
    compiler_hash = common.digest(compiler)
    result = {"status": "failed", "compiler_source_commit": commit, "compiler_binary_sha256": compiler_hash}
    try:
        checked = check_program(candidate, timeout, trial_environment(compiler), authoring_profile=AUTHORING_PROFILE_V30,
            harness_output=output / "runtime/catalog", compiler_binary_sha256=compiler_hash,
            expected_inventory=original, arm="semaprax")
        result["checks"] = checked
        if not checked["accepted"]:
            raise ValueError("catalog qualification failed; inspect retained qualification-result.json")
        native = checked["native_binary"]
        report = checked["acceptance_report"]
        subject = {"compiler_source_commit": commit, "compiler_binary_sha256": compiler_hash,
            "closed_authored_inventory_sha256": original["sha256"],
            "candidate_manifest_sha256": common.digest(manifest_path), "native_binary_sha256": native["sha256"]}
        receipt = output / "build-receipt.json"
        receipt.write_text(json.dumps({"schema": BUILD_RECEIPT_SCHEMA, "qualification_subject": subject,
            "acceptance_report_sha256": report["sha256"]}, indent=2, sort_keys=True) + "\n")
        evidence = {"schema": QUALIFICATION_SCHEMA, "benchmark_inputs_sha256": hashes,
            "compiler_source_commit": commit, "compiler_binary_sha256": compiler_hash,
            "native_project_route": ROUTE, "qualification_subject": subject, "acceptance_report": report,
            "candidate_source": {"inventory": {"path": str(inventory_path), "sha256": common.digest(inventory_path)},
                                 "manifest": {"path": str(manifest_path), "sha256": common.digest(manifest_path)}},
            "qualified_native_binary": {"path": native["path"], "sha256": native["sha256"]},
            "qualification_build_receipt": {"path": str(receipt), "sha256": common.digest(receipt)}}
        evidence_path = output / "qualification-evidence.json"
        evidence_path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
        validate_qualification_evidence(evidence_path, repo, commit, compiler_hash)
        result.update({"status": "qualified", "evidence": str(evidence_path), "qualification_subject": subject})
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        result["error"] = str(error)
        raise
    finally:
        with (output / "qualification-result.json").open("x") as handle:
            json.dump(result, handle, indent=2, sort_keys=True)
            handle.write("\n")
    return result
