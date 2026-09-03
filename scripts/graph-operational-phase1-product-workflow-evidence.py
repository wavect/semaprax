#!/usr/bin/env python3
"""Execute the closed Phase 1 product workflow at one exact clean local HEAD."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parent.parent
GIT = shutil.which("git")
SCHEMA = "semaprax.graph-operational-phase1-product-workflow-execution-evidence.v1"
DOMAIN = b"semaprax.graph-operational-phase1-product-workflow-execution-evidence.bundle.v1\0"
OBSERVATION_SCHEMA = "semaprax.graph-operational-phase1-product-workflow-observation.v1"
HOSTILE_SCHEMA = "semaprax.graph-operational-phase1-product-workflow-hostile-observation.v1"
HANDOFF_SCHEMA = "semaprax.graph-operational-phase1-product-workflow-handoff.v1"
WORKFLOW = "function_signature_review_publish_v1"
EVIDENCE_ENV = "SEMAPRAX_PRODUCT_WORKFLOW_EVIDENCE_DIR"
MAX_LOG = 16 * 1024 * 1024
MAX_ARTIFACT = 64 * 1024 * 1024
MAX_INPUT = 16 * 1024 * 1024
MAX_TOOL = 256 * 1024 * 1024
MAX_TEST_BINARY = 512 * 1024 * 1024
MAX_TYPESCRIPT_PACKAGE = 512 * 1024 * 1024
TIMEOUT = 30 * 60
FAIL_SENSITIVE_ENVIRONMENT = (
    "AR",
    "CARGO_BUILD_RUSTFLAGS",
    "CARGO_BUILD_TARGET",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_TARGET_DIR",
    "CC",
    "CFLAGS",
    "CPPFLAGS",
    "CXX",
    "CXXFLAGS",
    "LDFLAGS",
    "MACOSX_DEPLOYMENT_TARGET",
    "NODE_OPTIONS",
    "NODE_PATH",
    "RANLIB",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTFLAGS",
    "RUSTDOCFLAGS",
)
HEX_OBJECT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
HEX_SHA256 = re.compile(r"[0-9a-f]{64}")
SHA256 = re.compile(r"sha256:[0-9a-f]{64}")
RESULT = re.compile(
    r"^test ([A-Za-z0-9_:]+) \.\.\. (ok|FAILED|ignored)(?:, [^\r\n]+)?$",
    re.MULTILINE,
)
SUMMARY = re.compile(
    r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; "
    r"(\d+) measured; (\d+) filtered out; finished in [^\r\n]+$",
    re.MULTILINE,
)
PYTHON = "generated_python_reference_review_export_and_real_git_commit"
RUST = "generated_rust_reference_review_export_and_real_git_commit"
HOSTILE = "hostile_workflow_transitions_fail_closed"
TYPESCRIPT = "provisioned_typescript_reference_review_export_and_real_git_commit"
LANGUAGES = ("python", "rust", "typescript")
REVIEW_METHODS = (
    "workspace/open",
    "image/function-reference-export",
    "image/function-reference-resolve",
    "image/analysis-coverage",
    "candidate/open",
    "candidate/apply-intent",
    "candidate/validate",
    "candidate/semantic-delta",
    "candidate/test-plan",
    "candidate/test",
    "candidate/source-review",
    "candidate/analysis-coverage",
    "candidate/recovery-export",
)
PUBLISH_METHODS = (
    "workspace/open",
    "image/function-reference-resolve",
    "candidate/recovery-restore",
    "candidate/validate",
    "candidate/source-review",
    "source-commit/status",
    "candidate/commit",
    "source-commit/status",
    "candidate/commit-report",
)
BINDING_KEYS = {
    "image_revision",
    "project_revision",
    "candidate_revision",
    "review_policy_sha256",
    "publish_policy_sha256",
    "compact_reference_sha256",
    "intention_sha256",
    "validation_sha256",
    "semantic_delta_sha256",
    "test_plan_sha256",
    "test_report_sha256",
    "source_review_sha256",
    "base_analysis_coverage_sha256",
    "candidate_analysis_coverage_sha256",
    "recovery_capsule_sha256",
    "approval_revision",
    "commit_report_revision",
}
BLIND_SPOTS = {
    "analysis_completeness": "partial",
    "deployment_configuration": "not_inspected",
    "generated_file_provenance": "not_inspected",
    "generated_artifacts": "not_inspected",
    "external_api_behavior": "not_inspected",
    "runtime_environment": "partial_bounded_reference_interpreter",
    "external_consumers": "not_inspected",
}
TEST_POLICY = {
    "max_steps": 100_000,
    "max_execution_bytes": 65_536,
    "max_report_bytes": 262_144,
    "engine": "project_interpreter",
    "request_overrides": False,
}
HOSTILE_ROWS = (
    ("stale_reference", "stale_subject", False, "unchanged"),
    ("source_drift", "stale_subject", False, "unchanged"),
    ("failed_test", "review_rejected", False, "unchanged"),
    ("tampered_recovery", "publish_precondition_rejected", False, "unchanged"),
    ("wrong_approval", "publish_precondition_rejected", False, "unchanged"),
    ("definite_pre_pivot_failure", "publish_failed_pre_pivot", True, "unchanged"),
    ("post_ref_result_loss", "publication_uncertain", True, "updated_to_prepared_commit"),
    ("malformed_response_python", "transport_uncertain_no_publish_claim", False, "unchanged"),
    ("malformed_response_rust", "transport_uncertain_no_publish_claim", False, "unchanged"),
    ("malformed_response_typescript", "transport_uncertain_no_publish_claim", False, "unchanged"),
)


class Failure(Exception):
    """A condition that prevents an honest evidence archive."""


def canonical(value):
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def sha256(body):
    return "sha256:" + hashlib.sha256(body).hexdigest()


def run(arguments, *, env=None, timeout=60, combine=False):
    try:
        return subprocess.run(
            arguments,
            cwd=ROOT,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT if combine else subprocess.PIPE,
            check=False,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        raise Failure(
            f"command exceeded its {timeout}-second bound: {arguments[0]}"
        ) from error


def command(arguments, label, *, env=None):
    completed = run(arguments, env=env)
    output = completed.stdout
    error = completed.stderr or b""
    if len(output) > MAX_LOG or len(error) > MAX_LOG:
        raise Failure(f"{label} output exceeds {MAX_LOG} bytes")
    if completed.returncode:
        detail = (error or output).decode("utf-8", "replace").strip()
        raise Failure(f"cannot determine {label}: {detail[-8192:]}")
    value = output.decode("utf-8", "strict").strip()
    if not value:
        raise Failure(f"{label} is empty")
    return value


def git(*arguments):
    if GIT is None:
        raise Failure("Git is unavailable")
    return command((os.path.abspath(GIT),) + arguments, "Git state")


def regular(path, maximum, label, *, reject_links=True):
    try:
        status = path.lstat()
    except FileNotFoundError as error:
        raise Failure(f"missing {label}: {path}") from error
    if path.is_symlink() or not path.is_file():
        raise Failure(f"{label} is not a regular file: {path}")
    if reject_links and status.st_nlink != 1:
        raise Failure(f"{label} is not single-link: {path}")
    if status.st_size > maximum:
        raise Failure(f"{label} exceeds {maximum} bytes: {path}")
    return path.read_bytes()


def selected_tool(value, name):
    selected = Path(value).expanduser()
    if not selected.is_absolute() or not os.access(selected, os.X_OK):
        raise Failure(f"{name} must be an absolute executable")
    resolved = selected.resolve(strict=True)
    body = regular(resolved, MAX_TOOL, f"{name} executable", reject_links=False)
    return selected, resolved, body


def tool(value, name, version_arguments, *, launcher=None, env=None):
    selected, resolved, body = selected_tool(value, name)
    invocation = (str(selected),) if launcher is None else (str(launcher), str(selected))
    row = {
        "selected_path": str(selected),
        "resolved_path": str(resolved),
        "bytes": len(body),
        "sha256": sha256(body),
        "version": command(invocation + version_arguments, f"{name} version", env=env),
    }
    return row, (selected, resolved, body)


def typescript_payload(package_root, tsc_resolved):
    root = package_root.expanduser()
    if not root.is_absolute():
        raise Failure("TypeScript package root must be absolute")
    root = root.resolve(strict=True)
    if not root.is_dir():
        raise Failure("TypeScript package root is not a directory")
    if tsc_resolved != (root / "bin" / "tsc").resolve(strict=True):
        raise Failure("--tsc must select the bound TypeScript package bin/tsc")
    rows = []
    bindings = []
    aggregate_bytes = 0
    for path in sorted(root.rglob("*"), key=lambda value: value.relative_to(root).as_posix()):
        relative = path.relative_to(root).as_posix()
        status = path.lstat()
        if stat.S_ISLNK(status.st_mode):
            raise Failure(f"TypeScript payload contains a symbolic link: {relative}")
        if stat.S_ISDIR(status.st_mode):
            continue
        if not stat.S_ISREG(status.st_mode):
            raise Failure(f"TypeScript payload contains a non-regular entry: {relative}")
        body = regular(path, MAX_TOOL, f"TypeScript payload {relative}", reject_links=False)
        aggregate_bytes += len(body)
        if aggregate_bytes > MAX_TYPESCRIPT_PACKAGE:
            raise Failure(
                f"TypeScript package exceeds {MAX_TYPESCRIPT_PACKAGE} aggregate bytes"
            )
        rows.append({"path": relative, "bytes": len(body), "sha256": sha256(body)})
        bindings.append((relative, path.resolve(strict=True), body))
    bodies = {relative: body for relative, _path, body in bindings}
    required = {"package.json", "bin/tsc", "lib/tsc.js", "lib/_tsc.js"}
    if not required <= set(bodies):
        raise Failure("TypeScript package is missing a required compiler payload file")
    package = json.loads(bodies["package.json"])
    if not isinstance(package, dict) or package.get("name") != "typescript":
        raise Failure("TypeScript package.json does not identify the typescript package")
    if package.get("version") != "5.8.3":
        raise Failure("TypeScript package payload must be exactly 5.8.3")
    if bodies["bin/tsc"] != b"#!/usr/bin/env node\nrequire('../lib/tsc.js')\n":
        raise Failure("TypeScript bin/tsc is not the expected Node entry point")
    if b'require("./_tsc.js")' not in bodies["lib/tsc.js"]:
        raise Failure("TypeScript lib/tsc.js does not load the bound compiler payload")
    return {
        "package_root": str(root),
        "file_count": len(rows),
        "aggregate_bytes": aggregate_bytes,
        "files": rows,
    }, bindings


def verify_typescript_payload(row, bindings):
    current, current_bindings = typescript_payload(
        Path(row["package_root"]), Path(row["package_root"]) / "bin" / "tsc"
    )
    if current != row or current_bindings != bindings:
        raise Failure("TypeScript compiler payload changed during execution")


def inputs():
    bodies = {}
    rows = []
    for name in ("Cargo.toml", "Cargo.lock"):
        body = regular(ROOT / name, MAX_INPUT, "repository input")
        bodies[name] = body
        rows.append({"path": name, "bytes": len(body), "sha256": sha256(body)})
    return bodies, rows


def clean():
    completed = run(
        (
            os.path.abspath(GIT),
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
        )
    )
    if completed.returncode:
        detail = (completed.stderr or completed.stdout).decode("utf-8", "replace").strip()
        raise Failure(f"cannot determine Git worktree status: {detail[-8192:]}")
    if len(completed.stdout) > MAX_LOG or len(completed.stderr or b"") > MAX_LOG:
        raise Failure("Git worktree status output exceeds its bound")
    if completed.stdout:
        raise Failure("repository worktree is not clean")


def verify_repository(commit, tree, bodies):
    clean()
    if git("rev-parse", "--verify", "HEAD^{commit}") != commit:
        raise Failure("repository HEAD changed during execution")
    if git("rev-parse", "--verify", "HEAD^{tree}") != tree:
        raise Failure("repository tree changed during execution")
    current, _rows = inputs()
    if current != bodies:
        raise Failure("repository inputs changed during execution")


def verify_tools(tools, bindings, version_arguments, environment):
    for name, row in tools.items():
        launcher = tools["node"]["selected_path"] if name == "tsc" else None
        current, binding = tool(
            row["selected_path"],
            name,
            version_arguments[name],
            launcher=launcher,
            env=environment,
        )
        if current != row or binding[1:] != bindings[name][1:]:
            raise Failure(f"{name} identity changed during execution")


def parse_log(body, expected_rows, expected_summary, label):
    if len(body) > MAX_LOG:
        raise Failure(f"{label} exceeds {MAX_LOG} bytes")
    text = body.decode("utf-8", "strict")
    rows = sorted(RESULT.findall(text))
    if rows != sorted(expected_rows):
        raise Failure(f"{label} selected unexpected test rows: {rows!r}")
    summaries = [tuple(int(value) for value in row) for row in SUMMARY.findall(text)]
    if summaries != [expected_summary]:
        raise Failure(f"{label} has unexpected test summaries: {summaries!r}")


def execute(arguments, environment, expected_rows, expected_summary, label):
    completed = run(arguments, env=environment, timeout=TIMEOUT, combine=True)
    if len(completed.stdout) > MAX_LOG:
        raise Failure(f"{label} exceeds {MAX_LOG} bytes")
    if completed.returncode:
        detail = completed.stdout.decode("utf-8", "replace")[-8192:]
        raise Failure(f"{label} failed with exit {completed.returncode}: {detail}")
    parse_log(completed.stdout, expected_rows, expected_summary, label)
    return completed.stdout


def artifact_row(path, relative):
    body = regular(path, MAX_ARTIFACT, "workflow evidence artifact")
    return {"path": relative, "bytes": len(body), "sha256": sha256(body)}, body


def require_keys(value, expected, label):
    if not isinstance(value, dict) or set(value) != set(expected):
        raise Failure(f"{label} fields do not match the closed schema")


def read_canonical_json(path, label):
    body = regular(path, MAX_ARTIFACT, label)
    value = json.loads(body)
    if canonical(value) != body:
        raise Failure(f"{label} is not canonical JSON")
    return value, body


def read_ndjson(path, label):
    body = regular(path, MAX_ARTIFACT, label)
    if not body or not body.endswith(b"\n"):
        raise Failure(f"{label} must be nonempty LF-terminated NDJSON")
    rows = []
    for line in body.splitlines():
        value = json.loads(line)
        if not isinstance(value, dict) or canonical(value) != line:
            raise Failure(f"{label} contains a noncanonical object")
        rows.append(value)
    return rows, body


def validate_line_binding(row, label, *, hostile=False):
    keys = (
        "case",
        "sequence",
        "method",
        "request_line",
        "response_line",
        "request_sha256",
        "response_sha256",
    ) if hostile else (
        "phase",
        "sequence",
        "method",
        "request_line",
        "response_line",
        "request_sha256",
        "response_sha256",
    )
    require_keys(row, keys, label)
    if not isinstance(row["sequence"], int) or isinstance(row["sequence"], bool) or row["sequence"] <= 0:
        raise Failure(f"{label} has an invalid sequence")
    if not isinstance(row["method"], str) or not row["method"]:
        raise Failure(f"{label} has an invalid method")
    for field in ("request_line", "response_line"):
        if not isinstance(row[field], str):
            raise Failure(f"{label} {field} is not UTF-8 text")
        digest = sha256(row[field].encode("utf-8"))
        if row[field.removesuffix("_line") + "_sha256"] != digest:
            raise Failure(f"{label} {field} digest mismatch")


def validate_success_transcript(path, language):
    label = f"{language} workflow transcript"
    rows, body = read_ndjson(path, label)
    for index, row in enumerate(rows, 1):
        validate_line_binding(row, f"{label} row {index}")
        if row["sequence"] != index:
            raise Failure(f"{label} sequence is not contiguous")
    phase_rows = {"review": [], "publish": []}
    phases = []
    for row in rows:
        phase = row["phase"]
        if phase not in phase_rows:
            raise Failure(f"{label} has an unknown phase")
        phase_rows[phase].append(row["method"])
        phases.append(phase)
    if phases != sorted(phases, key={"review": 0, "publish": 1}.get):
        raise Failure(f"{label} phases are out of order")
    repeatable = {
        "candidate/semantic-delta",
        "candidate/source-review",
        "candidate/recovery-export",
        "candidate/commit-report",
    }
    for phase, expected in (("review", REVIEW_METHODS), ("publish", PUBLISH_METHODS)):
        compressed = []
        for method in phase_rows[phase]:
            if compressed and compressed[-1] == method:
                if method not in repeatable:
                    raise Failure(f"{label} unexpectedly repeats {method}")
                continue
            compressed.append(method)
        if compressed != list(expected):
            raise Failure(f"{label} {phase} method sequence mismatch")
    return body


def validate_hostile_transcript(path, hostile_rows=HOSTILE_ROWS):
    label = "hostile workflow transcript"
    rows, body = read_ndjson(path, label)
    expected_cases = [row[0] for row in hostile_rows]
    observed = []
    sequences = {}
    for index, row in enumerate(rows, 1):
        validate_line_binding(row, f"{label} row {index}", hostile=True)
        case = row["case"]
        if case not in expected_cases:
            raise Failure(f"{label} has an unknown case")
        if not observed or observed[-1] != case:
            observed.append(case)
        sequences[case] = sequences.get(case, 0) + 1
        if row["sequence"] != sequences[case]:
            raise Failure(f"{label} {case} sequence is not contiguous")
    if observed != expected_cases:
        raise Failure(f"{label} case order or inventory mismatch")
    return body


def validate_binding(binding, expected_path, body, label):
    require_keys(binding, ("path", "bytes", "sha256"), label)
    expected = {
        "path": expected_path,
        "bytes": len(body),
        "sha256": sha256(body),
    }
    if binding != expected:
        raise Failure(f"{label} does not bind the archived bytes")


def validate_generated_client(body, language):
    prefix = b"review-bytes:"
    if not body.startswith(prefix):
        raise Failure(f"{language} generated-client envelope lacks the review header")
    newline = body.find(b"\n", len(prefix))
    if newline < 0:
        raise Failure(f"{language} generated-client review header is incomplete")
    review_length_text = body[len(prefix):newline]
    if not review_length_text.isdigit() or review_length_text.startswith(b"0"):
        raise Failure(f"{language} generated-client review length is not canonical")
    review_length = int(review_length_text)
    review_start = newline + 1
    review_end = review_start + review_length
    publish_prefix = b"publish-bytes:"
    if body[review_end:review_end + len(publish_prefix)] != publish_prefix:
        raise Failure(f"{language} generated-client envelope boundary mismatch")
    newline = body.find(b"\n", review_end + len(publish_prefix))
    if newline < 0:
        raise Failure(f"{language} generated-client publish header is incomplete")
    publish_length_text = body[review_end + len(publish_prefix):newline]
    if not publish_length_text.isdigit() or publish_length_text.startswith(b"0"):
        raise Failure(f"{language} generated-client publish length is not canonical")
    publish_length = int(publish_length_text)
    publish_start = newline + 1
    if publish_start + publish_length != len(body):
        raise Failure(f"{language} generated-client publish length mismatch")
    for label, source in (("review", body[review_start:review_end]), ("publish", body[publish_start:])):
        if not source or not source.decode("utf-8", "strict"):
            raise Failure(f"{language} generated-client {label} source is empty")


def validate_success_artifacts(root, language):
    observation_name = f"workflow-{language}.observation.json"
    transcript_name = f"workflow-{language}.transcript.ndjson"
    handoff_name = f"workflow-{language}.handoff.json"
    generated_name = f"workflow-{language}.generated-client.txt"
    observation, observation_body = read_canonical_json(
        root / observation_name, f"{language} workflow observation"
    )
    transcript_body = validate_success_transcript(root / transcript_name, language)
    handoff, handoff_body = read_canonical_json(
        root / handoff_name, f"{language} workflow handoff"
    )
    generated_body = regular(
        root / generated_name, MAX_ARTIFACT, f"{language} generated client"
    )
    validate_generated_client(generated_body, language)
    require_keys(
        handoff,
        (
            "schema",
            "workflow",
            "language",
            "candidate_revision",
            "compact_reference",
            "typed_intention",
            "validation",
            "semantic_delta",
            "test_plan",
            "test_report",
            "source_review_sha256",
            "base_analysis_coverage_sha256",
            "candidate_analysis_coverage_sha256",
            "recovery_capsule",
        ),
        f"{language} workflow handoff",
    )
    if (
        handoff["schema"] != HANDOFF_SCHEMA
        or handoff["workflow"] != WORKFLOW
        or handoff["language"] != language
    ):
        raise Failure(f"{language} workflow handoff identity mismatch")
    require_keys(
        observation,
        (
            "schema", "workflow", "language", "terminal_outcome", "generated_client",
            "methods", "policies", "bindings", "blind_spots", "source", "git", "receipt", "artifacts",
        ),
        f"{language} workflow observation",
    )
    if (
        observation["schema"] != OBSERVATION_SCHEMA
        or observation["workflow"] != WORKFLOW
        or observation["language"] != language
        or observation["terminal_outcome"] != "published"
    ):
        raise Failure(f"{language} workflow outcome mismatch")
    generated = observation["generated_client"]
    validate_binding(generated, generated_name, generated_body, f"{language} generated-client binding")
    if observation["methods"] != {"review": list(REVIEW_METHODS), "publish": list(PUBLISH_METHODS)}:
        raise Failure(f"{language} workflow method sequence mismatch")
    bindings = observation["bindings"]
    require_keys(bindings, BINDING_KEYS, f"{language} workflow bindings")
    for name, value in bindings.items():
        if name.endswith("_sha256"):
            valid = isinstance(value, str) and SHA256.fullmatch(value) is not None
        else:
            valid = isinstance(value, str) and bool(value)
        if not valid:
            raise Failure(f"{language} workflow binding is malformed: {name}")
    for field, binding in (
        ("compact_reference", "compact_reference_sha256"),
        ("typed_intention", "intention_sha256"),
        ("recovery_capsule", "recovery_capsule_sha256"),
        ("semantic_delta", "semantic_delta_sha256"),
    ):
        value = handoff[field]
        if not isinstance(value, str) or not value or sha256(value.encode("utf-8")) != bindings[binding]:
            raise Failure(f"{language} handoff {field} binding mismatch")
    if handoff["candidate_revision"] != bindings["candidate_revision"]:
        raise Failure(f"{language} handoff candidate revision mismatch")
    for field, binding in (
        ("validation", "validation_sha256"),
        ("test_plan", "test_plan_sha256"),
        ("test_report", "test_report_sha256"),
    ):
        value = handoff[field]
        if not isinstance(value, dict) or sha256(canonical(value)) != bindings[binding]:
            raise Failure(f"{language} handoff {field} binding mismatch")
    for field in (
        "source_review_sha256",
        "base_analysis_coverage_sha256",
        "candidate_analysis_coverage_sha256",
    ):
        if handoff[field] != bindings[field]:
            raise Failure(f"{language} handoff {field} binding mismatch")
    if observation["blind_spots"] != BLIND_SPOTS:
        raise Failure(f"{language} blind-spot ledger mismatch")
    source = observation["source"]
    require_keys(source, ("before_sha256", "after_sha256", "unchanged"), f"{language} source binding")
    if source["unchanged"] is not True or source["before_sha256"] != source["after_sha256"] or SHA256.fullmatch(source["before_sha256"]) is None:
        raise Failure(f"{language} raw source preservation mismatch")
    git_row = observation["git"]
    require_keys(
        git_row,
        ("object_format", "ref", "old", "new", "parent", "tree", "source_objects", "independently_inspected"),
        f"{language} Git binding",
    )
    if git_row["object_format"] != "sha256" or git_row["independently_inspected"] is not True:
        raise Failure(f"{language} Git inspection mismatch")
    if not isinstance(git_row["ref"], str) or not git_row["ref"].startswith("refs/"):
        raise Failure(f"{language} Git ref mismatch")
    for name in ("old", "new", "parent", "tree"):
        if not isinstance(git_row[name], str) or HEX_SHA256.fullmatch(git_row[name]) is None:
            raise Failure(f"{language} Git {name} is not a SHA-256 object ID")
    if git_row["parent"] != git_row["old"]:
        raise Failure(f"{language} Git parent does not match the expected base")
    review_policy = {"candidate_prepare": True, "source_commit": False, "test_policy": TEST_POLICY}
    policies = observation["policies"]
    if not isinstance(policies, dict) or set(policies) != {"review", "publish"}:
        raise Failure(f"{language} selected host-policy inventory mismatch")
    publish = policies["publish"]
    require_keys(
        publish,
        ("candidate_prepare", "source_commit", "test_policy", "repository", "approval"),
        f"{language} publish policy",
    )
    repository = publish["repository"]
    require_keys(
        repository,
        ("object_format", "identity", "ref", "expected_old"),
        f"{language} publish repository policy",
    )
    if not isinstance(repository["identity"], str) or not repository["identity"]:
        raise Failure(f"{language} publish repository identity is empty")
    publish_policy = {
        "candidate_prepare": True,
        "source_commit": True,
        "test_policy": TEST_POLICY,
        "repository": {
            "object_format": "sha256",
            "identity": repository["identity"],
            "ref": git_row["ref"],
            "expected_old": git_row["old"],
        },
        "approval": {
            "candidate_revision": bindings["candidate_revision"],
            "approval_revision": bindings["approval_revision"],
        },
    }
    if policies != {"review": review_policy, "publish": publish_policy}:
        raise Failure(f"{language} selected host-policy binding mismatch")
    if bindings["review_policy_sha256"] != sha256(canonical(review_policy)):
        raise Failure(f"{language} review-policy digest mismatch")
    if bindings["publish_policy_sha256"] != sha256(canonical(publish_policy)):
        raise Failure(f"{language} publish-policy digest mismatch")
    objects = git_row["source_objects"]
    if not isinstance(objects, list) or not objects:
        raise Failure(f"{language} Git source-object inventory is empty")
    paths = []
    for item in objects:
        require_keys(item, ("path", "object"), f"{language} Git source object")
        if not isinstance(item["path"], str) or not item["path"] or HEX_SHA256.fullmatch(item["object"]) is None:
            raise Failure(f"{language} Git source-object binding mismatch")
        paths.append(item["path"])
    expected_source_paths = ["semaprax.toml", "src/app.spx", "src/core.spx", "src/tests.spx"]
    if paths != expected_source_paths:
        raise Failure(f"{language} Git Project-object inventory mismatch")
    receipt = observation["receipt"]
    require_keys(receipt, ("bytes", "sha256", "commit", "complete"), f"{language} receipt")
    if (
        not isinstance(receipt["bytes"], int)
        or receipt["bytes"] <= 0
        or SHA256.fullmatch(receipt["sha256"]) is None
        or receipt["commit"] != git_row["new"]
        or receipt["complete"] is not True
    ):
        raise Failure(f"{language} receipt binding mismatch")
    artifacts = observation["artifacts"]
    if not isinstance(artifacts, list) or len(artifacts) != 3:
        raise Failure(f"{language} observation artifact inventory mismatch")
    by_path = {row.get("path"): row for row in artifacts if isinstance(row, dict)}
    if set(by_path) != {transcript_name, handoff_name, generated_name}:
        raise Failure(f"{language} observation artifact paths mismatch")
    validate_binding(by_path[transcript_name], transcript_name, transcript_body, f"{language} transcript binding")
    validate_binding(by_path[handoff_name], handoff_name, handoff_body, f"{language} handoff binding")
    validate_binding(by_path[generated_name], generated_name, generated_body, f"{language} generated-client artifact binding")
    if by_path[generated_name] != generated:
        raise Failure(f"{language} generated-client bindings disagree")
    return {
        observation_name: observation_body,
        transcript_name: transcript_body,
        handoff_name: handoff_body,
        generated_name: generated_body,
    }


def validate_hostile_artifacts(root, hostile_rows=HOSTILE_ROWS):
    observation_name = "hostile-workflow.observation.json"
    transcript_name = "hostile-workflow.transcript.ndjson"
    observation, observation_body = read_canonical_json(root / observation_name, "hostile workflow observation")
    transcript_body = validate_hostile_transcript(root / transcript_name, hostile_rows)
    require_keys(observation, ("schema", "workflow", "cases", "artifacts"), "hostile workflow observation")
    if observation["schema"] != HOSTILE_SCHEMA or observation["workflow"] != WORKFLOW:
        raise Failure("hostile workflow identity mismatch")
    expected = [
        {
            "case": case,
            "terminal_outcome": outcome,
            "commit_invoked": invoked,
            "blind_retry_allowed": False,
            "git_ref_outcome": git_outcome,
            **({"basis": "distinct_insufficient_fuel_host_test_policy_nonpassing"} if case == "failed_test" else {}),
        }
        for case, outcome, invoked, git_outcome in hostile_rows
    ]
    if observation["cases"] != expected:
        raise Failure("hostile workflow case inventory or outcome mismatch")
    artifacts = observation["artifacts"]
    if not isinstance(artifacts, list) or len(artifacts) != 1:
        raise Failure("hostile observation artifact inventory mismatch")
    validate_binding(artifacts[0], transcript_name, transcript_body, "hostile transcript binding")
    return {observation_name: observation_body, transcript_name: transcript_body}


def collect_workflow_artifacts(root):
    if not root.is_dir() or root.is_symlink():
        raise Failure("workflow evidence export is not a directory")
    bodies = {}
    for language in LANGUAGES:
        bodies.update(validate_success_artifacts(root, language))
    bodies.update(validate_hostile_artifacts(root))
    if {path.name for path in root.iterdir()} != set(bodies):
        raise Failure("workflow evidence export contains unexpected or missing artifacts")
    return bodies


def test_binary_identity(target, stem):
    dependencies = target / "debug" / "deps"
    candidates = []
    if dependencies.is_dir():
        for path in dependencies.glob(f"{stem}-*"):
            if path.is_file() and not path.is_symlink() and os.access(path, os.X_OK):
                candidates.append(path)
    if len(candidates) != 1:
        raise Failure(
            f"expected one {stem} libtest executable, found "
            f"{[path.name for path in candidates]!r}"
        )
    path = candidates[0]
    body = regular(path, MAX_TEST_BINARY, "generated product-workflow libtest executable")
    return {
        "selection": "unique_executable_under_fresh_run_scoped_cargo_target_dir",
        "relative_path": f"debug/deps/{path.name}",
        "bytes": len(body),
        "sha256": sha256(body),
    }, path, body


def bundle_id(artifacts):
    return hashlib.sha256(DOMAIN + b"".join(canonical(row) for row in artifacts)).hexdigest()


def verify_bundle(destination, evidence):
    envelope = regular(destination / "evidence.json", MAX_ARTIFACT, "evidence envelope")
    if envelope != canonical(evidence) or json.loads(envelope) != evidence:
        raise Failure("evidence envelope does not replay")
    artifacts = evidence["artifacts"]
    paths = [row.get("path") for row in artifacts]
    if any(not isinstance(path, str) or Path(path).name != path for path in paths):
        raise Failure("evidence artifact paths are unsafe")
    if paths != sorted(set(paths)):
        raise Failure("evidence artifact paths are not unique canonical order")
    if {path.name for path in destination.iterdir()} != {"evidence.json", *paths}:
        raise Failure("evidence inventory does not replay")
    for row in artifacts:
        body = regular(destination / row["path"], MAX_ARTIFACT, "evidence artifact")
        if row != {"path": row["path"], "bytes": len(body), "sha256": sha256(body)}:
            raise Failure(f"evidence artifact does not replay: {row['path']}")
    if evidence["bundle_id"] != bundle_id(artifacts):
        raise Failure("evidence bundle ID does not replay")


def gate(identifier, selection, command_line, tests, counts, log):
    return {
        "id": identifier,
        "selection": selection,
        "command": list(command_line),
        "outcome": "passed",
        "exit_code": 0,
        "counts": counts,
        "tests": tests,
        "log": log,
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python", required=True, type=Path, help="absolute Python executable")
    parser.add_argument("--node", required=True, type=Path, help="absolute Node.js >=22 executable")
    parser.add_argument(
        "--tsc",
        required=True,
        type=Path,
        help="absolute bin/tsc from the selected TypeScript 5.8.3 package",
    )
    parser.add_argument(
        "--typescript-package-root",
        required=True,
        type=Path,
        help="absolute root of the selected TypeScript 5.8.3 package",
    )
    parser.add_argument("--output", type=Path, help="exact new bundle directory")
    args = parser.parse_args(argv)
    top = Path(git("rev-parse", "--show-toplevel")).resolve()
    if top != ROOT.resolve():
        raise Failure("script root disagrees with Git")
    clean()
    commit = git("rev-parse", "--verify", "HEAD^{commit}")
    tree = git("rev-parse", "--verify", "HEAD^{tree}")
    if HEX_OBJECT.fullmatch(commit) is None or HEX_OBJECT.fullmatch(tree) is None:
        raise Failure("subject is not an exact lowercase Git commit and tree")
    input_bodies, input_rows = inputs()
    discovered = {name: shutil.which(name) for name in ("cargo", "rustc", "git")}
    if any(value is None for value in discovered.values()):
        raise Failure("Cargo, rustc, and Git must be available")
    paths = {name: os.path.abspath(value) for name, value in discovered.items()}
    if Path(paths["git"]).resolve(strict=True) != Path(os.path.abspath(GIT)).resolve(strict=True):
        raise Failure("Git discovery changed during runner startup")
    paths.update({"python": str(args.python), "node": str(args.node), "tsc": str(args.tsc)})
    version_arguments = {
        "cargo": ("--version", "--verbose"),
        "rustc": ("--version", "--verbose"),
        "git": ("--version",),
        "python": ("--version",),
        "node": ("--version",),
        "tsc": ("--version",),
    }
    base_environment = os.environ.copy()
    for name in FAIL_SENSITIVE_ENVIRONMENT:
        base_environment.pop(name, None)
    tools = {}
    bindings = {}
    for name in ("cargo", "rustc", "git", "python", "node"):
        tools[name], bindings[name] = tool(
            paths[name], name, version_arguments[name], env=base_environment
        )
    tools["tsc"], bindings["tsc"] = tool(
        paths["tsc"],
        "tsc",
        version_arguments["tsc"],
        launcher=tools["node"]["selected_path"],
        env=base_environment,
    )
    if Path(sys.executable).resolve(strict=True) != bindings["python"][1]:
        raise Failure("runner interpreter does not match the selected Python executable")
    typescript, typescript_bindings = typescript_payload(
        args.typescript_package_root, bindings["tsc"][1]
    )
    if tools["tsc"]["version"] != "Version 5.8.3":
        raise Failure("TypeScript must be exactly 5.8.3")
    node = re.fullmatch(r"v(\d+)\.\d+\.\d+(?:[-+].*)?", tools["node"]["version"])
    if node is None or int(node.group(1)) < 22:
        raise Failure("Node must be version 22 or newer")
    verify_tools(tools, bindings, version_arguments, base_environment)
    verify_typescript_payload(typescript, typescript_bindings)
    verify_repository(commit, tree, input_bodies)
    cargo = tools["cargo"]["selected_path"]
    ordinary = (
        cargo, "test", "--locked", "--offline", "-p", "semaprax", "--test",
        "image_generated_product_workflow_v5", "--", "--test-threads=1", "--nocapture",
    )
    hostile = (
        cargo, "test", "--locked", "--offline", "-p", "semaprax", "--test",
        "image_generated_product_workflow_hostile_v5", "--", "--test-threads=1", "--nocapture",
    )
    provisioned = (
        cargo, "test", "--locked", "--offline", "-p", "semaprax", "--test",
        "image_generated_product_workflow_v5", TYPESCRIPT, "--", "--exact", "--ignored",
        "--test-threads=1", "--nocapture",
    )
    environment = base_environment.copy()
    environment.update(
        {
            "CARGO_INCREMENTAL": "0",
            "CARGO_NET_OFFLINE": "true",
            "CARGO_TERM_COLOR": "never",
            "RUSTC": tools["rustc"]["selected_path"],
            "SEMAPRAX_TEST_CARGO": cargo,
            "SEMAPRAX_TEST_RUSTC": tools["rustc"]["selected_path"],
            "SEMAPRAX_TEST_GIT": tools["git"]["selected_path"],
            "SEMAPRAX_TEST_PYTHON": tools["python"]["selected_path"],
            "SEMAPRAX_TEST_NODE": tools["node"]["selected_path"],
            "SEMAPRAX_TEST_TSC": tools["tsc"]["selected_path"],
        }
    )
    local_temporary_root = ROOT / "target" / "phase1-product-workflow-evidence"
    local_temporary_root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="run-", dir=local_temporary_root
    ) as temporary:
        export = Path(temporary) / "export"
        target = Path(temporary) / "target"
        tool_bin = Path(temporary) / "tool-bin"
        export.mkdir()
        tool_bin.mkdir()
        selected_node = tool_bin / "node"
        selected_node.symlink_to(bindings["node"][1])
        environment["PATH"] = str(tool_bin) + os.pathsep + environment.get("PATH", "")
        discovered_node = shutil.which("node", path=environment["PATH"])
        if discovered_node != str(selected_node) or selected_node.resolve(strict=True) != bindings["node"][1]:
            raise Failure("TypeScript compiler does not resolve the selected Node executable")
        environment[EVIDENCE_ENV] = str(export)
        environment["CARGO_TARGET_DIR"] = str(target)
        logs = {
            "generated-product-workflow-cargo.log": execute(
                ordinary,
                environment,
                [(PYTHON, "ok"), (RUST, "ok"), (TYPESCRIPT, "ignored")],
                (2, 0, 1, 0, 0),
                "default Python/Rust workflow",
            )
        }
        verify_repository(commit, tree, input_bodies)
        verify_tools(tools, bindings, version_arguments, base_environment)
        verify_typescript_payload(typescript, typescript_bindings)
        logs["generated-product-workflow-hostile-cargo.log"] = execute(
            hostile,
            environment,
            [(HOSTILE, "ok")],
            (1, 0, 0, 0, 0),
            "default hostile workflow transitions",
        )
        intermediate_hostile = validate_hostile_artifacts(export, HOSTILE_ROWS[:9])
        intermediate_hostile = {
            "hostile-workflow.default.snapshot.json": canonical({
                "schema": "semaprax.graph-operational-phase1-product-workflow-hostile-default-snapshot.v1",
                "observation_json": intermediate_hostile["hostile-workflow.observation.json"].decode("utf-8", "strict"),
                "transcript_ndjson": intermediate_hostile["hostile-workflow.transcript.ndjson"].decode("utf-8", "strict"),
            })
        }
        verify_repository(commit, tree, input_bodies)
        verify_tools(tools, bindings, version_arguments, base_environment)
        verify_typescript_payload(typescript, typescript_bindings)
        logs["generated-product-workflow-typescript-cargo.log"] = execute(
            provisioned,
            environment,
            [(TYPESCRIPT, "ok")],
            (1, 0, 0, 0, 2),
            "explicitly provisioned TypeScript workflow",
        )
        verify_repository(commit, tree, input_bodies)
        verify_tools(tools, bindings, version_arguments, base_environment)
        verify_typescript_payload(typescript, typescript_bindings)
        exported = collect_workflow_artifacts(export)
        test_binaries = []
        test_binary_bindings = []
        for stem in (
            "image_generated_product_workflow_v5",
            "image_generated_product_workflow_hostile_v5",
        ):
            row, path, body = test_binary_identity(target, stem)
            test_binaries.append(row)
            test_binary_bindings.append((stem, path, body))
        bodies = {**logs, **intermediate_hostile, **exported}
        artifacts = [
            {"path": name, "bytes": len(body), "sha256": sha256(body)}
            for name, body in sorted(bodies.items())
        ]
        bundle = bundle_id(artifacts)
        evidence = {
            "schema": SCHEMA,
            "bundle_id": bundle,
            "repository": {
                "commit": commit,
                "tree": tree,
                "subject_kind": "exact_local_commit",
                "head_relation_at_capture": "HEAD",
                "current_head_at_capture": True,
                "inputs": input_rows,
                "clean_before": True,
                "clean_after": True,
                "head_unchanged": True,
            },
            "exact_tag": {"selection": "not_required", "claim": "not_claimed"},
            "runner": {
                "scope": "local_exact_commit",
                "network": "cargo_offline_network_isolation_not_claimed",
                "cargo_incremental": False,
                "host": {
                    "system": platform.system(),
                    "release": platform.release(),
                    "machine": platform.machine(),
                },
                "tools": tools,
                "typescript_compiler_payload": typescript,
                "environment_policy": {
                    "cleared_variables": list(FAIL_SENSITIVE_ENVIRONMENT),
                    "node_resolution": "selected_node_prepended_to_inherited_path",
                },
                "generated_workflow_libtest_executables": test_binaries,
                "evidence_export_environment": EVIDENCE_ENV,
            },
            "executions": [
                gate(
                    "generated_product_workflow_python_rust_v1",
                    "default",
                    ordinary,
                    [
                        {"name": PYTHON, "outcome": "passed"},
                        {"name": RUST, "outcome": "passed"},
                        {"name": TYPESCRIPT, "outcome": "ignored"},
                    ],
                    {"selected": 3, "passed": 2, "failed": 0, "ignored": 1, "measured": 0, "filtered_out": 0},
                    "generated-product-workflow-cargo.log",
                ),
                gate(
                    "generated_product_workflow_hostile_v1",
                    "default",
                    hostile,
                    [{
                        "name": HOSTILE,
                        "outcome": "passed",
                        "hostile_cases_observed": 9,
                        "typescript_malformed_response": "not_selected_without_provisioned_runtime",
                    }],
                    {"selected": 1, "passed": 1, "failed": 0, "ignored": 0, "measured": 0, "filtered_out": 0},
                    "generated-product-workflow-hostile-cargo.log",
                ),
                gate(
                    "generated_product_workflow_typescript_v1",
                    "explicit_ignored",
                    provisioned,
                    [{
                        "name": TYPESCRIPT,
                        "outcome": "passed_provisioned_local",
                        "successful_workflows": 1,
                        "hostile_cases_added": ["malformed_response_typescript"],
                    }],
                    {"selected": 1, "passed": 1, "failed": 0, "ignored": 0, "measured": 0, "filtered_out": 2},
                    "generated-product-workflow-typescript-cargo.log",
                ),
            ],
            "artifacts": artifacts,
            "observations": {
                "workflow": WORKFLOW,
                "generated_python": "passed_local_exact_subject",
                "generated_rust": "passed_local_exact_subject",
                "generated_typescript": "passed_explicitly_provisioned_local_exact_subject",
                "closed_success_transcripts": 3,
                "closed_handoff_artifacts": 3,
                "closed_generated_client_artifacts": 3,
                "hostile_transition_cases": len(HOSTILE_ROWS),
                "publication_fixture": "isolated_local_unix_bare_sha256_git",
                "raw_source_preservation": "passed_per_language",
            },
            "nonclaims": [
                "general_signature_or_owned_resource_migration",
                "dynamic_or_external_callers_or_behavioral_compatibility",
                "deployment_configuration_or_generated_provenance",
                "provider_or_external_api_or_installed_consumer_validity",
                "native_or_wasm_runtime_equivalence",
                "filesystem_or_checkout_or_remote_git_publication",
                "physical_crash_power_loss_durability_or_multi_writer_atomicity",
                "cancellation_deduplication_retry_exactly_once_or_session_durability",
                "approval_or_authority_transfer_through_handoff",
                "packaged_sdk_editor_ui_or_mcp_certification",
                "network_isolation_hosted_cross_platform_or_exact_release_tag",
                "comparative_economics_full_quality_completion_matrix_or_programme_completion",
            ],
        }
        destination = (
            args.output.expanduser().resolve()
            if args.output
            else ROOT / ".semaprax/evidence/graph-operational-phase1-product-workflow" / commit / bundle
        )
        git_directory = Path(git("rev-parse", "--absolute-git-dir")).resolve()
        if destination == git_directory or git_directory in destination.parents:
            raise Failure("evidence output must not be inside the Git administrative directory")
        if destination.exists() or destination.is_symlink():
            raise Failure(f"destination exists: {destination}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        stage = Path(tempfile.mkdtemp(prefix=".phase1-product-workflow-", dir=destination.parent))
        try:
            for name, body in bodies.items():
                (stage / name).write_bytes(body)
            (stage / "evidence.json").write_bytes(canonical(evidence))
            stage.replace(destination)
            verify_repository(commit, tree, input_bodies)
            verify_tools(tools, bindings, version_arguments, base_environment)
            verify_typescript_payload(typescript, typescript_bindings)
            verify_bundle(destination, evidence)
            for row, (stem, path, body) in zip(test_binaries, test_binary_bindings):
                if regular(path, MAX_TEST_BINARY, f"{stem} libtest executable") != body:
                    raise Failure(f"{stem} libtest executable changed")
                if test_binary_identity(target, stem)[0] != row:
                    raise Failure(f"{stem} libtest executable identity changed")
            verify_repository(commit, tree, input_bodies)
            verify_tools(tools, bindings, version_arguments, base_environment)
            verify_typescript_payload(typescript, typescript_bindings)
        except BaseException:
            shutil.rmtree(stage, ignore_errors=True)
            shutil.rmtree(destination, ignore_errors=True)
            raise
    print(destination)
    print(bundle)


if __name__ == "__main__":
    try:
        main()
    except (Failure, OSError, UnicodeError, ValueError, KeyError, TypeError) as error:
        print(f"Phase 1 product-workflow evidence failed: {error}", file=sys.stderr)
        raise SystemExit(1)
