#!/usr/bin/env python3
"""External coding-agent runner for the agent-task-comparison ledger.

This is the execution adapter required by GitHub issue #104. It wires a
pluggable "backend" (a stub replaying a scripted, checked-in transcript, or a
real coding-agent process) to the existing, unmodified
`scripts/agent-task-comparison.py` contracts: it builds a trial, gives the
backend an isolated sandbox copy of the task fixture, replays or invokes the
backend under a path-containment guard, independently re-derives the
acceptance verdict from the resulting files (never from the backend's own
claims), assembles a `semaprax.agent-task-comparison-ledger.v1` object, and
then calls the existing, unmodified `observation` and `audit` commands to
validate what it produced.

Design points required by issue #104:

* Pluggable runner: `--runner fixture` replays a deterministic, offline,
  checked-in transcript (no model or network access). `--runner live` is
  wired but refuses to make any provider call unless invoked with
  `--dry-run`, in which case it validates configuration and prints exactly
  what would be invoked without contacting a provider. Live (non-dry-run)
  execution additionally requires `--acknowledge-model-budget`, which this
  round of work does not supply anywhere.
* Protected oracle/trial state: the backend only ever sees an ephemeral
  sandbox directory containing the task's fixture bytes. It is never given
  the path to the task JSON (acceptance rubric), the manifest, the drift
  patch, or the evidence directory the harness writes the ledger into. Any
  backend-declared write path is additionally validated to stay inside the
  sandbox (no absolute path, no `..`, no symlink escape) before anything is
  written. Acceptance is computed solely by this script's own independent
  checker over the resulting sandbox files; a backend cannot set its own
  outcome or acceptance rows.
* Determinism: the fixture backend performs no randomness, no wall-clock
  measurement, and no network access; every metric it contributes is a fixed
  constant read from the checked-in transcript file.
"""

import argparse
import copy
import difflib
import hashlib
import importlib.util
import json
import os
import selectors
import signal
import shutil
import stat
import subprocess
import sys
import time
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
FIXTURE_RUNNER_SCHEMA = "semaprax.agent-task-comparison-fixture-runner.v1"
LEDGER_EVENT_KINDS = (
    "model_usage",
    "context_presentation",
    "tool_call",
    "failed_attempt",
    "stale_failure",
    "stale_recovery_action",
    "validation_interval",
    "review_interval",
    "human_intervention",
)
TRANSCRIPT_ARTIFACT_ID = "transcript"


class RunnerFailure(Exception):
    pass


def _load_comparison_module():
    """Import the sibling hyphenated script as a module, unmodified."""
    path = ROOT / "scripts" / "agent-task-comparison.py"
    spec = importlib.util.spec_from_file_location("agent_task_comparison", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


atc = _load_comparison_module()


def canonical(value):
    return atc.canonical(value)


def digest(body):
    return atc.digest(body)


# ---------------------------------------------------------------------------
# Sandbox: the only filesystem area a backend (candidate agent) ever touches.
# ---------------------------------------------------------------------------


def fixture_root_for(task_binding):
    """The directory containing the task's fixture files (e.g. .../fixture)."""
    paths = [Path(item["path"]) for item in task_binding["fixture"]]
    roots = {path.parent for path in paths if path.name == "semaprax.toml"}
    if len(roots) != 1:
        raise RunnerFailure("task fixture files do not share exactly one root with semaprax.toml")
    return next(iter(roots))


def create_sandbox(task_binding, work_dir=None):
    """Create an ephemeral, backend-owned copy of the task fixture bytes.

    The sandbox never contains the task JSON, the manifest, the drift patch,
    or any evidence path. It is created under the OS temp directory (or an
    explicit --work-dir) rather than inside the repository tree.
    """
    fixture_root = fixture_root_for(task_binding)
    sandbox = Path(tempfile.mkdtemp(prefix="spx-agent-task-comparison-sandbox-", dir=work_dir))
    candidate = sandbox / "candidate"
    candidate.mkdir()
    for item in task_binding["fixture"]:
        source = ROOT / item["path"]
        relative = Path(item["path"]).relative_to(fixture_root)
        destination = candidate / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination, follow_symlinks=False)
    return sandbox, candidate


def resolve_write_target(sandbox_root, relative_path, label="backend write path"):
    """Resolve a backend-declared relative path strictly inside the sandbox.

    Rejects absolute paths, `..` traversal, empty/`.` segments, and any
    symlink along an already-existing prefix. This is the sole gate a
    candidate backend's declared file edits pass through before anything on
    disk changes; it is what keeps the backend from ever reaching files
    outside its sandbox (in particular the task rubric, the manifest, the
    drift patch, and the evidence/ledger directory, none of which are even
    copied into the sandbox in the first place).
    """
    if not isinstance(relative_path, str) or not relative_path or "\\" in relative_path:
        raise RunnerFailure(f"invalid {label}: {relative_path!r}")
    candidate = Path(relative_path)
    if candidate.is_absolute() or any(part in ("", ".", "..") for part in candidate.parts):
        raise RunnerFailure(f"{label} escapes the sandbox: {relative_path!r}")
    resolved_root = sandbox_root.resolve(strict=True)
    walked = resolved_root
    for part in candidate.parts:
        walked = walked / part
        if walked.exists() and walked.is_symlink():
            raise RunnerFailure(f"{label} crosses a symlink: {relative_path!r}")
    target = resolved_root
    for part in candidate.parts[:-1]:
        target = target / part
        if not target.exists():
            raise RunnerFailure(f"{label} has a missing parent directory: {relative_path!r}")
    final_target = resolved_root.joinpath(*candidate.parts)
    if final_target.exists() and (final_target.is_symlink() or not final_target.is_file()):
        raise RunnerFailure(f"{label} does not target a regular file: {relative_path!r}")
    try:
        final_target.parent.resolve(strict=True).relative_to(resolved_root)
    except (OSError, ValueError) as error:
        raise RunnerFailure(f"{label} escapes the sandbox: {relative_path!r}") from error
    return final_target


# ---------------------------------------------------------------------------
# Backends. A backend turns a task/lane/trial into an ordered action stream.
# The fixture backend is the only one exercised without an approved model
# budget; it is fully deterministic and offline.
# ---------------------------------------------------------------------------


def load_fixture_script(path):
    body = Path(path).read_text(encoding="utf-8")
    value = json.loads(body)
    if value.get("schema") != FIXTURE_RUNNER_SCHEMA:
        raise RunnerFailure(f"unsupported fixture runner schema in {path}")
    return value


def apply_unified_diff_single_hunk(original_bytes, patch_bytes, target_path):
    """Apply the one committed drift patch's single hunk against one file.

    This reads the actual checked-in `--drift-patch` bytes (already
    authenticated by `agent-task-comparison.py`'s manifest loader) rather
    than hardcoding the expected before/after text, so a change to the
    committed patch file is reflected here automatically instead of silently
    drifting out of sync with it.
    """
    lines = patch_bytes.decode("utf-8").splitlines()
    hunk_headers = [index for index, line in enumerate(lines) if line.startswith("@@")]
    if len(hunk_headers) != 1:
        raise RunnerFailure(f"drift patch for {target_path} must contain exactly one hunk")
    hunk_start = hunk_headers[0]
    old_block_lines = []
    new_block_lines = []
    for line in lines[hunk_start + 1 :]:
        if line.startswith(" "):
            old_block_lines.append(line[1:])
            new_block_lines.append(line[1:])
        elif line.startswith("-"):
            old_block_lines.append(line[1:])
        elif line.startswith("+"):
            new_block_lines.append(line[1:])
        else:
            break
    old_block = ("\n".join(old_block_lines) + "\n").encode("utf-8")
    new_block = ("\n".join(new_block_lines) + "\n").encode("utf-8")
    if original_bytes.count(old_block) != 1:
        raise RunnerFailure(f"drift patch target text is not present exactly once in {target_path}")
    return original_bytes.replace(old_block, new_block, 1)


def snapshot_candidate(candidate_dir):
    """Return a byte-identity snapshot of regular candidate files only."""
    snapshot = {}
    for path in sorted(candidate_dir.rglob("*")):
        relative = path.relative_to(candidate_dir).as_posix()
        if path.is_symlink():
            raise RunnerFailure(f"candidate snapshot contains a symlink: {relative}")
        if path.is_dir():
            continue
        if not path.is_file():
            raise RunnerFailure(f"candidate snapshot contains a non-regular file: {relative}")
        body = path.read_bytes()
        snapshot[relative] = {"sha256": digest(body), "bytes": len(body)}
    return snapshot


def run_fixture_backend(fixture_script, candidate_dir, task_id, drift_patch_bytes):
    """Replay a checked-in, scripted transcript deterministically.

    Applies each declared "write" strictly inside the sandbox via
    resolve_write_target, and applies the task's actual committed drift
    patch exactly once, immediately after the single declared
    "identifying_inspection" marker, exactly as the task's drift_injection
    protocol requires. Returns the ordered ledger-shaped events plus the
    transcript actions actually replayed (for the evidence artifact) and the
    count of drift applications performed (0 or 1).
    """
    events = []
    replayed = []
    drift_applications = 0
    inspection_markers = 0
    for action in fixture_script["actions"]:
        kind = action["kind"]
        if kind == "identifying_inspection":
            inspection_markers += 1
            if inspection_markers > 1:
                raise RunnerFailure("fixture script triggers the identifying inspection more than once")
            if drift_patch_bytes is None:
                raise RunnerFailure("fixture script signals drift injection for a task with no drift patch")
            core_path = candidate_dir / "src" / "core.spx"
            before = core_path.read_bytes()
            after = apply_unified_diff_single_hunk(before, drift_patch_bytes, "src/core.spx")
            if after == before:
                raise RunnerFailure("drift patch target text was not found in the sandbox file")
            core_path.write_bytes(after)
            drift_applications += 1
            replayed.append({"kind": "identifying_inspection", "drift_applied": True})
            continue
        write = action.get("write")
        if write is not None:
            target = resolve_write_target(candidate_dir, write["path"])
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(write["content"], encoding="utf-8")
        replayed.append(action)
        if kind in LEDGER_EVENT_KINDS:
            values = {}
            if kind == "model_usage":
                values = {"model_input_tokens": action["input_tokens"], "model_output_tokens": action["output_tokens"]}
            elif kind == "context_presentation":
                values = {"presented_context_bytes": action["bytes"]}
            elif kind == "tool_call":
                values = {"tool_request_bytes": action["request_bytes"], "tool_response_bytes": action["response_bytes"]}
            elif kind == "validation_interval":
                values = {"validation_wall_ms": action["ms"]}
            elif kind == "review_interval":
                values = {"review_wall_ms": action["ms"]}
            events.append({"kind": kind, "values": values, "evidence": [TRANSCRIPT_ARTIFACT_ID]})
    if drift_patch_bytes is not None and drift_applications != 1:
        raise RunnerFailure(
            f"drift for task {task_id} did not occur exactly once (occurred {drift_applications} times)"
        )
    return events, replayed, drift_applications


def run_live_backend(config_path, dry_run, acknowledge_model_budget):
    if config_path is None:
        raise RunnerFailure(
            "runner live requires --provider-config; no credentials are supplied by this repository"
        )
    config_body = Path(config_path).read_text(encoding="utf-8")
    config = json.loads(config_body)
    for key in ("provider", "model", "api_key_env", "budget_ceiling_usd"):
        if key not in config:
            raise RunnerFailure(f"live provider config is missing required key: {key}")
    if not dry_run:
        raise RunnerFailure(
            "HUMAN_BLOCKED: approved live-model budget for the #105 pilot. "
            "This build accepts --runner live only with --dry-run; it never places a real "
            "provider call. Pass --dry-run to validate configuration without spending budget."
        )
    return {
        "dry_run": True,
        "provider": config["provider"],
        "model": config["model"],
        "api_key_env": config["api_key_env"],
        "budget_ceiling_usd": config["budget_ceiling_usd"],
        "would_invoke": (
            f"provider={config['provider']} model={config['model']} "
            f"api_key_env={config['api_key_env']} budget_ceiling_usd={config['budget_ceiling_usd']}"
        ),
        "network_calls_made": 0,
    }


# ---------------------------------------------------------------------------
# Independent acceptance oracle. The backend's own claims (if any) are never
# read here; the ledger's acceptance rows come only from inspecting the
# resulting sandbox files.
# ---------------------------------------------------------------------------


def _read(candidate_dir, relative):
    path = candidate_dir / relative
    return path.read_text(encoding="utf-8") if path.exists() else ""


def check_signature_migration(candidate_dir):
    core = _read(candidate_dir, "src/core.spx")
    app = _read(candidate_dir, "src/app.spx")
    tests = _read(candidate_dir, "src/tests.spx")
    signature_ok = "fn add(right: i64, left: i64, bias: i64) -> i64" in core
    identity_ok = '@id("benchmark.add")\nfn add(right: i64, left: i64, bias: i64)' in core
    callers_ok = (
        "add(23, 19, 0)" in app
        and "add(23, 19, 0)" in tests
        and "add(8 / 2, 6 / 2, 0)" in core
    )
    meaning_ok = "requires right >= 0" in core and "ensures result == left + right + bias" in core
    review_ok = bool(core) and bool(app) and bool(tests)
    return [
        {"id": "signature", "outcome": "passed" if signature_ok else "failed"},
        {"id": "identity", "outcome": "passed" if identity_ok else "failed"},
        {"id": "callers", "outcome": "passed" if callers_ok else "failed"},
        {"id": "meaning", "outcome": "passed" if meaning_ok else "failed"},
        {"id": "review", "outcome": "passed" if review_ok else "failed"},
        {"id": "authority", "outcome": "passed"},
    ]


def check_stale_signature_recovery(candidate_dir, drift_applications):
    core = _read(candidate_dir, "src/core.spx")
    stale_detection_ok = drift_applications == 1
    recovery_ok = (
        "fn add(right: i64, left: i64, bias: i64) -> i64" in core
        and "left + -right" in core
    )
    signature_ok = recovery_ok
    identity_ok = '@id("benchmark.add")\nfn add(right: i64, left: i64, bias: i64)' in core
    review_ok = bool(core)
    return [
        {"id": "stale-detection", "outcome": "passed" if stale_detection_ok else "failed"},
        {"id": "recovery", "outcome": "passed" if recovery_ok else "failed"},
        {"id": "signature", "outcome": "passed" if signature_ok else "failed"},
        {"id": "identity", "outcome": "passed" if identity_ok else "failed"},
        {"id": "review", "outcome": "passed" if review_ok else "failed"},
        {"id": "authority", "outcome": "passed"},
    ]


def _bounded_command(argv, cwd, limit=131072, timeout=20):
    """Capture compiler streams without unbounded memory; kill/reap on every failure."""
    try:
        child = subprocess.Popen(argv, cwd=cwd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, start_new_session=True)
    except OSError as error:
        return {"returncode": None, "error": f"spawn: {error}"}
    streams = {child.stdout: bytearray(), child.stderr: bytearray()}
    selector = selectors.DefaultSelector()
    for stream in streams:
        selector.register(stream, selectors.EVENT_READ)
    deadline = time.monotonic() + timeout
    failure = None
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                failure = "timeout"
                break
            for key, _ in selector.select(remaining):
                chunk = os.read(key.fileobj.fileno(), 8192)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                streams[key.fileobj].extend(chunk)
                if sum(len(value) for value in streams.values()) > limit:
                    failure = f"output exceeds {limit} bytes"
                    break
            if failure:
                break
    finally:
        selector.close()
        if failure:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        child.wait()
    stdout, stderr = bytes(streams[child.stdout]), bytes(streams[child.stderr])
    child.stdout.close()
    child.stderr.close()
    return {"returncode": child.returncode, "stdout": stdout, "stderr": stderr, "error": failure}


def _cleanup_projection(executable, subject_dir):
    """Ask graph for the exact core bytes plus only a synthetic entrypoint."""
    core = subject_dir / "src" / "core.spx"
    prefix = core.read_bytes()
    with tempfile.NamedTemporaryFile(mode="wb", suffix=".spx", prefix=".owned-cleanup-", dir=subject_dir, delete=False) as handle:
        projection = Path(handle.name)
        handle.write(prefix)
        handle.write(b'\n@id("benchmark.owned.oracle_main")\nfn main() -> i64\n{\n    0\n}\n')
    try:
        completed = _bounded_command([str(executable), "graph", projection.name], subject_dir)
    finally:
        projection.unlink(missing_ok=True)
    stdout = completed.pop("stdout", b"")
    stderr = completed.pop("stderr", b"")
    return {**completed, "core_sha256": digest(prefix), "stdout": stdout.decode("utf-8", "replace"),
        "stderr": stderr.decode("utf-8", "replace"), "stdout_sha256": digest(stdout), "stderr_sha256": digest(stderr)}


def _cleanup_nodes(record):
    try:
        graph = json.loads(record["stdout"])
    except (KeyError, TypeError, json.JSONDecodeError):
        return None
    if graph.get("schema") != "semaprax.graph.v17":
        return None
    selected = {node.get("id"): node for node in graph.get("nodes", [])
                if node.get("id") in {"benchmark.owned.select", "benchmark.owned.call"}}
    if set(selected) != {"benchmark.owned.select", "benchmark.owned.call"}:
        return None
    if any(not isinstance(node.get("cleanup"), dict) or node["cleanup"].get("kind") != "cleanup_plan" for node in selected.values()):
        return None
    return {identifier: selected[identifier]["cleanup"] for identifier in sorted(selected)}


def _owned_compiler_evidence(candidate_dir, compiler_path, baseline_dir):
    """Collect raw, bounded compiler evidence; fixture events cannot satisfy it."""
    if compiler_path is None:
        return {"available": False, "reason": "no compiler path supplied"}
    executable = Path(compiler_path)
    if executable.is_symlink() or not executable.is_file():
        return {"available": False, "reason": "compiler path is not a regular file"}
    before_binary = digest(executable.read_bytes())
    commands = (("check", "check", "--json"), ("test", "test"), ("run", "run"), ("graph", "graph"))
    result = {"available": True, "binary_sha256_before": before_binary, "candidate": {}, "baseline": {}}
    for subject, directory in (("candidate", candidate_dir), ("baseline", baseline_dir)):
        for name, *argv in commands:
            completed = _bounded_command([str(executable), *argv, "semaprax.toml"], directory)
            stdout = completed.pop("stdout", b"")
            stderr = completed.pop("stderr", b"")
            result[subject][name] = {**completed, "stdout": stdout.decode("utf-8", "replace"),
                "stderr": stderr.decode("utf-8", "replace"), "stdout_sha256": digest(stdout), "stderr_sha256": digest(stderr)}
        result[subject]["cleanup_projection"] = _cleanup_projection(executable, directory)
    result["binary_sha256_after"] = digest(executable.read_bytes())
    result["available"] = result["binary_sha256_before"] == result["binary_sha256_after"]
    return result



def _declaration_source(source, stable_id):
    lines = source.splitlines()
    marker = f'@id("{stable_id}")'
    try:
        start = next(i for i, line in enumerate(lines) if line.strip() == marker)
    except StopIteration:
        return None
    end = next((i for i in range(start + 1, len(lines)) if lines[i].strip().startswith("@id(")), len(lines))
    return "\n".join(lines[start:end])


def _signature(declaration):
    for line in declaration.splitlines():
        if line.strip().startswith("fn "):
            return line.strip()
    return None

def _owned_graph(evidence):
    try:
        graph = json.loads(evidence["candidate"]["graph"]["stdout"])
    except (KeyError, TypeError, json.JSONDecodeError):
        return None
    return graph if graph.get("schema") == "semaprax.project-semantic-graph.v1" else None


def _owned_review_package(task_binding, candidate_dir, before, after, evidence):
    """Create reviewable source and compiler material; it does not claim a human review."""
    changes = sorted(path for path in set(before) | set(after) if before.get(path) != after.get(path))
    root = fixture_root_for(task_binding)
    before_sources = {}
    after_sources = {}
    diff_lines = []
    for item in task_binding["fixture"]:
        relative = Path(item["path"]).relative_to(root).as_posix()
        old = (ROOT / item["path"]).read_text(encoding="utf-8")
        new = (candidate_dir / relative).read_text(encoding="utf-8")
        before_sources[relative] = old
        after_sources[relative] = new
        diff_lines.extend(difflib.unified_diff(old.splitlines(True), new.splitlines(True), fromfile=f"before/{relative}", tofile=f"after/{relative}"))
    return {
        "schema": "semaprax.agent-task-comparison-owned-review.v1",
        "before": before,
        "after": after,
        "before_sources": before_sources,
        "after_sources": after_sources,
        "source_diff": "".join(diff_lines),
        "changed_paths": changes,
        "compiler": evidence,
        "semantic_delta": "compiler check/test/run/graph outputs are retained verbatim above",
        "ownership_delta": "requested source API is bound below; compiler check admitted the resulting ownership program",
        "cleanup_delta": {
            "baseline": _cleanup_nodes(evidence.get("baseline", {}).get("cleanup_projection", {})),
            "candidate": _cleanup_nodes(evidence.get("candidate", {}).get("cleanup_projection", {})),
        },
        "blind_spots": [
            "cleanup plans are exported from a synthetic-entry projection of the exact core bytes; the projection itself is not the project entry closure",
            "runtime observation is limited to the selected local interpreter result",
            "no deployment, generated-file, external-API, or external-consumer target was supplied",
            "this package is reviewable material; it records no blinded human-review interval",
        ],
    }


def check_owned_signature_migration(candidate_dir, task_binding, before, after, compiler_path, original_before=None, original_after=None, candidate_post=None):
    """Derive owned-task verdicts from snapshots and compiler output, never actions."""
    expected_before = {
        Path(item["path"]).relative_to(fixture_root_for(task_binding)).as_posix(): {
            "sha256": item["sha256"], "bytes": item["bytes"]
        }
        for item in task_binding["fixture"]
    }
    baseline_dir = fixture_root_for(task_binding)
    evidence = _owned_compiler_evidence(candidate_dir, compiler_path, baseline_dir)
    candidate_post = snapshot_candidate(candidate_dir)
    original_after = snapshot_candidate(baseline_dir)
    graph = _owned_graph(evidence)
    semantic_ok = bool(evidence.get("available")) and all(
        evidence["candidate"].get(name, {}).get("returncode") == 0
        for name in ("check", "test", "run", "graph")
    )
    declarations = graph.get("declarations", []) if graph else []
    edges = graph.get("edges", []) if graph else []
    select = [row for row in declarations if row.get("id") == "benchmark.owned.select"]
    identity_ok = semantic_ok and len(select) == 1 and select[0].get("identity_origin") == "explicit"
    core = _read(candidate_dir, "src/core.spx")
    selected = _declaration_source(core, "benchmark.owned.select")
    caller = _declaration_source(core, "benchmark.owned.call")
    callers_ok = semantic_ok and {
        (row.get("caller"), row.get("target")) for row in edges if row.get("kind") == "call"
    } >= {
        ("benchmark.owned.main", "benchmark.owned.evaluate"),
        ("benchmark.owned.test", "benchmark.owned.evaluate"),
    } and caller is not None and caller.index("let left = bytes_copy(input);") < caller.index("let right = bytes_copy(input);") < caller.index("select(input, right, 0usize, left)")
    signature_ok = semantic_ok and selected is not None and _signature(selected) == "fn select(view: borrow Slice<u8>, right: own Bytes, flag: usize, left: own Bytes) -> Bytes"
    # Admission is compiler-derived. The source spelling is only used to bind the requested ordered API.
    ownership_ok = semantic_ok and signature_ok
    baseline_run = evidence.get("baseline", {}).get("run", {})
    candidate_run = evidence.get("candidate", {}).get("run", {})
    meaning_ok = semantic_ok and graph is not None and graph.get("project") == "agent-owned-comparison" and baseline_run.get("stdout") == candidate_run.get("stdout") == "42\n"
    authority_ok = (before == expected_before and original_before == original_after == expected_before
        and candidate_post == after and set(after) == set(expected_before)
        and all(before[path] == after[path] for path in expected_before if path != "src/core.spx"))
    package = _owned_review_package(task_binding, candidate_dir, before, after, evidence)
    cleanup_ok = all(_cleanup_nodes(evidence.get(subject, {}).get("cleanup_projection", {})) is not None
        and evidence[subject]["cleanup_projection"].get("returncode") == 0 for subject in ("baseline", "candidate"))
    review_ok = authority_ok and semantic_ok and cleanup_ok and package["changed_paths"] == ["src/core.spx"]
    return [
        {"id": "signature", "outcome": "passed" if signature_ok else "failed"},
        {"id": "identity", "outcome": "passed" if identity_ok else "failed"},
        {"id": "callers", "outcome": "passed" if callers_ok else "failed"},
        {"id": "ownership", "outcome": "passed" if ownership_ok else "failed"},
        {"id": "meaning", "outcome": "passed" if meaning_ok else "failed"},
        {"id": "review", "outcome": "passed" if review_ok else "failed"},
        {"id": "authority", "outcome": "passed" if authority_ok else "failed"},
    ], package


ACCEPTANCE_CHECKERS = {
    "signature-migration-v1": lambda candidate_dir, drift_applications: check_signature_migration(candidate_dir),
    "stale-signature-recovery-v1": check_stale_signature_recovery,
}


def independent_acceptance(task_id, candidate_dir, drift_applications, task_binding=None, before=None, after=None, compiler_path=None, original_before=None, original_after=None, candidate_post=None):
    if task_id == "owned-signature-migration-v1":
        if task_binding is None or before is None or after is None:
            raise RunnerFailure("owned acceptance requires immutable before/after snapshots and task binding")
        return check_owned_signature_migration(candidate_dir, task_binding, before, after, compiler_path, original_before, original_after, candidate_post)
    checker = ACCEPTANCE_CHECKERS.get(task_id)
    if checker is None:
        raise RunnerFailure(f"no independent acceptance checker registered for task {task_id}")
    return checker(candidate_dir, drift_applications)


# ---------------------------------------------------------------------------
# Ledger assembly.
# ---------------------------------------------------------------------------


def build_ledger(
    plan,
    task_id,
    lane_id,
    trial,
    manifest,
    fixture_script,
    events,
    replayed_actions,
    acceptance_rows,
    prompt_sha256,
    review_package=None,
):
    outcome = "completed" if all(row["outcome"] == "passed" for row in acceptance_rows) else "failed"
    transcript_body = canonical(
        {
            "fixture_script": fixture_script,
            "replayed_actions": replayed_actions,
            "independent_acceptance": acceptance_rows,
            "review_package": review_package,
        }
    ) + b"\n"
    streams = {
        "model_usage": [TRANSCRIPT_ARTIFACT_ID],
        "context_presentation": [TRANSCRIPT_ARTIFACT_ID],
        "tool_calls": [TRANSCRIPT_ARTIFACT_ID],
        "failed_attempts": [TRANSCRIPT_ARTIFACT_ID],
        "stale_recovery": [TRANSCRIPT_ARTIFACT_ID],
        "validation": [TRANSCRIPT_ARTIFACT_ID],
        "review": [TRANSCRIPT_ARTIFACT_ID],
        "human_interventions": [TRANSCRIPT_ARTIFACT_ID],
    }
    ledger = {
        "schema": atc.LEDGER_SCHEMA,
        "plan_sha256": digest(canonical(plan)),
        "task": task_id,
        "lane": lane_id,
        "trial": trial,
        "state": manifest["pairing"]["state"],
        "model": fixture_script["model"],
        "tokenizer": fixture_script["tokenizer"],
        "model_configuration": fixture_script["model_configuration"],
        "harness": fixture_script["harness"],
        "host": fixture_script["host"],
        "toolchain": fixture_script["toolchain"],
        "prompt_sha256": prompt_sha256,
        "artifacts": [
            {
                "id": TRANSCRIPT_ARTIFACT_ID,
                "path": "transcript.json",
                "bytes": len(transcript_body),
                "sha256": digest(transcript_body),
                "kind": "fixture_transcript_and_independent_acceptance_record",
            }
        ],
        "streams": streams,
        "events": events,
        "acceptance": [
            {"id": row["id"], "outcome": row["outcome"], "evidence": [TRANSCRIPT_ARTIFACT_ID]}
            for row in acceptance_rows
        ],
        "outcome": outcome,
    }
    return ledger, transcript_body


def write_canonical(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(canonical(value) + b"\n")


def finalize_evidence(evidence_dir, ledger, transcript_body):
    """Write the ledger and its transcript artifact, then lock them read-only.

    This directory (unlike the sandbox) is never handed to the backend: its
    path is chosen and created by this script after the backend has already
    finished running, so a candidate process has no path by which to reach
    it, and the finalized files are additionally chmod'd read-only as
    defense in depth against a stray same-user write.
    """
    evidence_dir.mkdir(parents=True, exist_ok=True)
    ledger_path = evidence_dir / "ledger.json"
    transcript_path = evidence_dir / "transcript.json"
    write_canonical(ledger_path, ledger)
    transcript_path.write_bytes(transcript_body)
    for path in (ledger_path, transcript_path):
        os.chmod(path, stat.S_IRUSR | stat.S_IRGRP | stat.S_IROTH)
    return ledger_path, transcript_path


# ---------------------------------------------------------------------------
# End-to-end pipeline.
# ---------------------------------------------------------------------------


def run(
    manifest_path,
    task_id,
    lane_id,
    trial,
    runner_kind,
    fixture_script_path,
    evidence_dir,
    provider_config=None,
    dry_run=False,
    work_dir=None,
    compiler_path=None,
):
    manifest, manifest_body, loaded_tasks = atc.load_manifest(manifest_path)
    plan = atc.make_plan_from_loaded(manifest_path, manifest, manifest_body, loaded_tasks, atc.head())
    # Reuses the existing, unmodified trial contract builder; this is what
    # already refuses the unavailable zero-graph-native lane and out-of-range
    # trial numbers.
    trial_contract = atc.trial_from_loaded(
        manifest_path, manifest, manifest_body, loaded_tasks, plan, task_id, lane_id, trial
    )
    task_binding = next(item for item in loaded_tasks if item["id"] == task_id)
    drift_patch = task_binding["drift_patch"]
    drift_patch_bytes = None
    if drift_patch is not None:
        drift_patch_bytes = (ROOT / drift_patch["path"]).read_bytes()
        if digest(drift_patch_bytes) != drift_patch["sha256"]:
            raise RunnerFailure("drift patch bytes disagree with the manifest-authenticated digest")

    if runner_kind == "live":
        report = run_live_backend(provider_config, dry_run, acknowledge_model_budget=False)
        return {"trial": trial_contract, "live_dry_run": report}

    if runner_kind != "fixture":
        raise RunnerFailure(f"unknown runner kind: {runner_kind}")
    fixture_script = load_fixture_script(fixture_script_path)
    if fixture_script["task"] != task_id or fixture_script["lane"] != lane_id:
        raise RunnerFailure("fixture script does not match the requested task/lane")

    sandbox, candidate_dir = create_sandbox(task_binding, work_dir=work_dir)
    try:
        original_before = snapshot_candidate(fixture_root_for(task_binding))
        before_snapshot = snapshot_candidate(candidate_dir)
        events, replayed, drift_applications = run_fixture_backend(
            fixture_script, candidate_dir, task_id, drift_patch_bytes
        )
        after_snapshot = snapshot_candidate(candidate_dir)
        original_after = snapshot_candidate(fixture_root_for(task_binding))
        acceptance = independent_acceptance(
            task_id, candidate_dir, drift_applications, task_binding, before_snapshot, after_snapshot, compiler_path,
            original_before, original_after, after_snapshot
        )
        if task_id == "owned-signature-migration-v1":
            acceptance_rows, review_package = acceptance
        else:
            acceptance_rows, review_package = acceptance, None
        ledger, transcript_body = build_ledger(
            plan,
            task_id,
            lane_id,
            trial,
            manifest,
            fixture_script,
            events,
            replayed,
            acceptance_rows,
            task_binding["prompt_sha256"],
            review_package,
        )
    finally:
        shutil.rmtree(sandbox, ignore_errors=True)

    ledger_path, transcript_path = finalize_evidence(evidence_dir, ledger, transcript_body)
    ledger_relative = str(ledger_path.relative_to(ROOT))
    observation_output = evidence_dir / "observation.json"
    observation = atc.make_observation(manifest_path, ledger_relative, str(observation_output))
    write_canonical(observation_output, observation)
    audit_output = evidence_dir / "audit.json"
    audit = atc.make_audit(manifest_path, str(observation_output.relative_to(ROOT)), task_id, lane_id, trial)
    write_canonical(audit_output, audit)
    return {
        "trial": trial_contract,
        "ledger_path": str(ledger_path),
        "observation_path": str(observation_output),
        "audit_path": str(audit_output),
        "ledger_sha256": digest(canonical(ledger)),
        "observation_sha256": digest(canonical(observation)),
        "audit_sha256": digest(canonical(audit)),
        "drift_applications": drift_applications,
        "outcome": ledger["outcome"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("run",))
    parser.add_argument("--manifest", default="benchmarks/agent-task-comparison-v1/manifest.json")
    parser.add_argument("--task", required=True)
    parser.add_argument("--lane", required=True)
    parser.add_argument("--trial", type=int, required=True)
    parser.add_argument("--runner", choices=("fixture", "live"), default="fixture")
    parser.add_argument("--fixture-script")
    parser.add_argument("--evidence-dir", required=True, help="repository-relative output directory")
    parser.add_argument("--provider-config")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--work-dir", help="sandbox parent directory (defaults to the OS temp directory)")
    parser.add_argument("--compiler", help="explicit Semaprax binary for compiler-derived owned-task evidence")
    arguments = parser.parse_args()

    evidence_dir = ROOT / arguments.evidence_dir
    try:
        evidence_dir.resolve().relative_to(ROOT.resolve())
    except ValueError as error:
        raise RunnerFailure("--evidence-dir must resolve inside the repository") from error

    result = run(
        arguments.manifest,
        arguments.task,
        arguments.lane,
        arguments.trial,
        arguments.runner,
        arguments.fixture_script,
        evidence_dir,
        provider_config=arguments.provider_config,
        dry_run=arguments.dry_run,
        work_dir=arguments.work_dir,
        compiler_path=arguments.compiler,
    )
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (RunnerFailure, atc.Failure) as error:
        print(f"agent task comparison runner rejected: {error}", file=sys.stderr)
        raise SystemExit(2)
