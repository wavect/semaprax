#!/usr/bin/env python3
"""
Cross-language toolchain-conformance harness (v1).

This module scores a fixed, human-written source tree against each
language's official toolchain; it has no model, provider, sampling, or
budget concept, and no code path here invokes a model. `agent/run_agent.py`
(a sibling module, not this one) is where an Agent-driven candidate — today
only through a deterministic offline replay transport — is scored through
the same build/test/leak-check/provenance machinery this module owns; see
`agent/README.md`.

Usage:
  python3 benchmarks/cross-language-v1/run.py --dry-run --output /tmp/plan.json
  python3 benchmarks/cross-language-v1/run.py --semaprax <bin> --output /tmp/result.json
  python3 benchmarks/cross-language-v1/run.py --semaprax <bin> --only sequence-digest-v1 \
      --language rust --output /tmp/result.json
  python3 benchmarks/cross-language-v1/run.py --semaprax <bin> --output /tmp/r.json \
      --compare benchmarks/cross-language-v1/results/prior.json

Paths are resolved from the script location, never from the working
directory: task and adapter inventories against this suite directory, task
source trees against the repository root. Only `--output` and `--compare`
follow the caller's cwd.

What this schema version measures: build/compile success, declared-outcome
success, and hidden-test success, for one task/language pair, using each
language's own officially documented toolchain invocation (see
`adapters.json`). It never measures wall-clock time. This host runs many
concurrent build lanes, so any timing captured here would measure contention,
not the subject; issues #85, #130 and #131 own adding a timing metric once an
exclusive quiet host is available, and this schema has no field to receive
one by accident. See `docs/METHODOLOGY.md` for the full equivalence contract.

Hidden-test isolation: a task's `hidden` directory is never copied into the
tree an adapter's public build step sees. It is only ever overlaid onto a
second, separate scratch copy for the hidden-test phase, and the harness
records a leak check confirming the public scratch directory never gained a
hidden-only path.
"""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import selectors
import signal
import shutil
import stat
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from typing import Optional
from datetime import datetime, timezone

SUITE = pathlib.Path(__file__).resolve().parent
ROOT = SUITE.parent.parent
TASKS = SUITE / "tasks.json"
ADAPTERS = SUITE / "adapters.json"
TASKS_SCHEMA = "benchmark.cross_language.tasks.v1"
ADAPTERS_SCHEMA = "benchmark.cross_language.adapters.v1"
SCHEMA = "benchmark.cross_language.v1"
PLAN_SCHEMA = "benchmark.cross_language.plan.v1"
TIMEOUT_SECONDS = 120

# Result statuses. `ok` is the only comparable one; `blocked` is a declared
# adapter with no wired toolchain and is never counted as a pass or a fail.
OK = "ok"
FAILED = "failed"
BLOCKED = "blocked"
DRIFTED = "drifted"


@dataclass(frozen=True)
class HardenedExecution:
    """Optional POSIX-only process policy for an already-snapshotted subject.

    The ordinary benchmark path intentionally keeps its established behavior.
    Runnable-adapter v1 opts into this profile only after creating private
    regular-file inputs and a closed environment.
    """

    environment: dict[str, str]
    deadline: float
    output_limit: int
    group_id: int


def sha256_bytes(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def digest_tree(directory: pathlib.Path) -> str:
    """One digest over every file's relative path and bytes, order-independent.

    Sorting by relative path before hashing means moving the same bytes to
    the same destination in a different traversal order never changes the
    identity, and adding, removing or editing one file always does.
    """
    if not directory.is_dir():
        return sha256_bytes(b"")
    rows = []
    for path in sorted(directory.rglob("*")):
        if path.is_file():
            relative = path.relative_to(directory).as_posix()
            rows.append(f"{relative}\0{hashlib.sha256(path.read_bytes()).hexdigest()}")
    return sha256_bytes("\n".join(rows).encode())


def tool_version(command: list, execution: Optional[HardenedExecution] = None) -> str:
    try:
        code, stdout, stderr = run_command(command, pathlib.Path.cwd(), execution)
        text = (stdout or stderr or "").strip()
        return text.splitlines()[0] if text else "unknown"
    except Exception:
        return "unknown"


def host_facts() -> dict:
    system = platform.system().lower()
    machine = platform.machine().lower()
    return {
        "platform": f"{system}-{machine}",
        "system": platform.system(),
        "release": platform.release(),
        "cpu_count": os.cpu_count(),
        "python": platform.python_version(),
    }


def git_revision(root: pathlib.Path) -> dict:
    revision = {"commit": "unknown", "dirty": None}
    try:
        revision["commit"] = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=root, text=True
        ).strip()
        status = subprocess.check_output(
            ["git", "status", "--porcelain"], cwd=root, text=True
        )
        revision["dirty"] = bool(status.strip())
    except Exception:
        pass
    return revision


def load_json(path: pathlib.Path, expected_schema: str, label: str) -> dict:
    document = json.loads(path.read_text())
    if document.get("schema") != expected_schema:
        raise ValueError(
            f"{label} {path} declares schema {document.get('schema')!r}, "
            f"expected {expected_schema!r}"
        )
    return document


def resolve_adapters(path: pathlib.Path) -> dict:
    document = load_json(path, ADAPTERS_SCHEMA, "adapter inventory")
    return {row["id"]: row for row in document["adapters"]}


def command_for(adapter: dict, key: str, semaprax_binary: str) -> list:
    template = adapter.get(key)
    if template is None:
        return []
    line = list(template)
    if line and line[0] == "{semaprax}":
        line[0] = semaprax_binary
    return line


def _kill_group(group_id: int) -> None:
    try:
        os.killpg(group_id, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        # A prior kill can race process-group teardown on Darwin.
        pass


def _run_hardened(command: list, cwd: pathlib.Path, execution: HardenedExecution) -> tuple:
    """Run one bounded child process without retaining unbounded output.

    This profile is intentionally POSIX-scoped.  A Windows job-object
    implementation would be a separate reviewed contract; falling back to a
    parent-only kill would weaken the containment claim.
    """
    if os.name != "posix":
        return None, "", "hardened execution is unavailable on this host"
    if os.getpid() != execution.group_id or os.getpgrp() != execution.group_id:
        return None, "", "hardened execution lost its containment group"
    if time.monotonic() >= execution.deadline:
        return None, "", "shared execution deadline expired"
    process = subprocess.Popen(
        command,
        cwd=cwd,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=False,
        env=execution.environment,
        # The admitted runner is the sole group leader.  Adapter children join
        # it so the outer snapshot executor can terminate every descendant.
        start_new_session=False,
    )
    selector = selectors.DefaultSelector()
    streams = {process.stdout, process.stderr}
    for stream in streams:
        assert stream is not None
        os.set_blocking(stream.fileno(), False)
        selector.register(stream, selectors.EVENT_READ)
    output = {process.stdout: bytearray(), process.stderr: bytearray()}
    overflow = False
    timed_out = False
    while selector.get_map():
        remaining = execution.deadline - time.monotonic()
        if remaining <= 0:
            timed_out = True
            _kill_group(execution.group_id)
            break
        events = selector.select(min(remaining, 0.1))
        for key, _ in events:
            chunk = os.read(key.fileobj.fileno(), min(8192, execution.output_limit + 1))
            if not chunk:
                selector.unregister(key.fileobj)
                continue
            output[key.fileobj].extend(chunk)
            if len(output[key.fileobj]) > execution.output_limit:
                overflow = True
                _kill_group(execution.group_id)
                break
        if overflow:
            break
    selector.close()
    if overflow or timed_out:
        _kill_group(execution.group_id)
    try:
        process.wait(timeout=max(0.0, execution.deadline - time.monotonic()))
    except subprocess.TimeoutExpired:
        _kill_group(execution.group_id)
        process.wait()
        timed_out = True
    stdout = bytes(output[process.stdout]).decode("utf-8", "replace")
    stderr = bytes(output[process.stderr]).decode("utf-8", "replace")
    assert process.stdout is not None and process.stderr is not None
    process.stdout.close()
    process.stderr.close()
    if overflow:
        return None, stdout, "adapter output exceeded byte bound"
    if timed_out:
        return None, stdout, "shared execution deadline expired"
    return process.returncode, stdout, stderr


def run_command(command: list, cwd: pathlib.Path, execution: Optional[HardenedExecution] = None) -> tuple:
    """Execute one adapter step. Returns (returncode, stdout, stderr) or a
    timeout sentinel (`None`, "", "timeout after N seconds")."""
    if execution is not None:
        try:
            return _run_hardened(command, cwd, execution)
        except FileNotFoundError as error:
            return None, "", f"tool not found: {error}"
    try:
        completed = subprocess.run(
            command,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=TIMEOUT_SECONDS,
        )
        return completed.returncode, completed.stdout, completed.stderr
    except subprocess.TimeoutExpired:
        return None, "", f"timeout after {TIMEOUT_SECONDS} seconds"
    except FileNotFoundError as error:
        return None, "", f"tool not found: {error}"


def evaluate_success(adapter: dict, returncode, stdout: str) -> tuple:
    """Apply one adapter's declared success predicate. Returns (passed, why)."""
    predicate = adapter.get("success", {"kind": "exit_code_zero"})
    kind = predicate.get("kind")
    if returncode is None:
        return False, "did not complete"
    if kind == "exit_code_zero":
        return (returncode == 0), f"exit {returncode}"
    if kind == "stdout_equals":
        if returncode != 0:
            return False, f"exit {returncode}"
        observed = stdout.strip()
        expected = predicate["value"]
        if observed == expected:
            return True, f"stdout {observed!r}"
        return False, f"stdout {observed!r}, expected {expected!r}"
    raise ValueError(f"unknown success predicate kind: {kind}")


def stage(scratch: pathlib.Path, adapter: dict, semaprax_binary: str,
          execution: Optional[HardenedExecution] = None) -> dict:
    """Build (if declared) then run one adapter phase inside `scratch`.

    Returns a dict with `passed`, `phase` ("build" or the run phase), the
    tail of stderr on failure, and the raw stdout of the run step (the
    `stdout_equals` predicate needs it).
    """
    build_command = command_for(adapter, "build_command", semaprax_binary)
    if build_command:
        code, _, err = run_command(build_command, scratch, execution)
        if code != 0:
            return {
                "passed": False,
                "phase": "build",
                "detail": (err or "").strip().splitlines()[-5:] or [f"exit {code}"],
            }
    run_line = command_for(adapter, "run_command", semaprax_binary)
    code, out, err = run_command(run_line, scratch, execution)
    passed, why = evaluate_success(adapter, code, out)
    result = {"passed": passed, "phase": "run", "detail": [why]}
    if not passed and err:
        result["detail"] += err.strip().splitlines()[-5:]
    return result


def copy_tree(source: pathlib.Path, destination: pathlib.Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    for path in source.rglob("*"):
        target = destination / path.relative_to(source)
        if path.is_dir():
            target.mkdir(parents=True, exist_ok=True)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)


def relative_files(directory: pathlib.Path) -> set:
    if not directory.is_dir():
        return set()
    return {p.relative_to(directory).as_posix() for p in directory.rglob("*") if p.is_file()}


def scratch_dir(label: str) -> pathlib.Path:
    directory = pathlib.Path(tempfile.mkdtemp(prefix=f"spx-cross-lang-{label}-"))
    # macOS's platform temp directory is itself a symlink; several project
    # loading paths in this repository reject a symlinked ancestor.
    return directory.resolve()


def hidden_overlay_problem(public_dir: pathlib.Path, hidden_dir: pathlib.Path) -> str | None:
    """Require additional fixture bytes, not proof of additional coverage.

    Callers admit the public directory first. Fixture trees must remain stable
    during evaluation; this read-only check is not filesystem confinement.
    """
    def inspection_failed(error: OSError) -> None:
        raise error

    try:
        if not hidden_dir.is_dir():
            return f"missing hidden directory: {hidden_dir}"
        has_files = False
        changes_public = False
        for directory, directories, filenames in os.walk(hidden_dir, onerror=inspection_failed):
            directories.sort()
            for filename in sorted(filenames):
                hidden_file = pathlib.Path(directory) / filename
                if not stat.S_ISREG(hidden_file.stat().st_mode):
                    continue
                has_files = True
                hidden_bytes = hidden_file.read_bytes()
                public_file = public_dir / hidden_file.relative_to(hidden_dir)
                if not public_file.is_file() or hidden_bytes != public_file.read_bytes():
                    changes_public = True
        if not has_files:
            return f"empty hidden overlay: {hidden_dir}"
        if not changes_public:
            return f"hidden overlay adds no changes: {hidden_dir}"
    except OSError:
        return f"cannot inspect hidden overlay: {hidden_dir}"
    return None


def evaluate_pair(root: pathlib.Path, task: dict, language: str, adapter: dict,
                   semaprax_binary: str, execution: Optional[HardenedExecution] = None) -> dict:
    record = {
        "id": f"{task['id']}::{language}",
        "task": task["id"],
        "language": language,
        "category": task["category"],
    }
    if not adapter.get("implemented", False):
        record.update(status=BLOCKED, reason=adapter.get("blocked_reason", "not implemented"))
        return record

    languages = task.get("languages", {})
    paths = languages.get(language)
    if paths is None:
        record.update(status=BLOCKED, reason=f"task declares no {language} implementation")
        return record

    public_dir = root / paths["public"]
    hidden_dir = root / paths["hidden"]
    if not public_dir.is_dir():
        record.update(status=FAILED, reason=f"missing public directory: {public_dir}")
        return record
    problem = hidden_overlay_problem(public_dir, hidden_dir)
    if problem is not None:
        record.update(status=FAILED, reason=problem)
        return record

    public_digest = digest_tree(public_dir)
    hidden_digest = digest_tree(hidden_dir)
    combined = sha256_bytes(f"{public_digest}\n{hidden_digest}".encode())
    record["provenance"] = {
        "adapter_version": tool_version(command_for(adapter, "version_command", semaprax_binary), execution),
        "public_digest": public_digest,
        "hidden_digest": hidden_digest,
        "digest": combined,
    }
    expected = paths.get("expected_digest")
    if expected is not None and expected != combined:
        record.update(
            status=DRIFTED,
            reason=f"subject digest {combined} does not match the expected {expected}",
        )
        return record

    public_scratch = scratch_dir(f"public-{language}")
    hidden_scratch = scratch_dir(f"hidden-{language}")
    try:
        copy_tree(public_dir, public_scratch)
        public_outcome = stage(public_scratch, adapter, semaprax_binary, execution)

        # Leak check: the public scratch tree must never contain a
        # hidden-only path, regardless of whether the public phase passed.
        leaked = relative_files(hidden_dir) & relative_files(public_scratch)
        # Files an overlay would also touch (same relative path as a public
        # file) are not a leak; only a hidden-only addition is.
        hidden_only = relative_files(hidden_dir) - relative_files(public_dir)
        leaked_only = leaked & hidden_only
        record["leak_check"] = "ok" if not leaked_only else sorted(leaked_only)

        record["public"] = {"passed": public_outcome["passed"], "detail": public_outcome["detail"]}
        if not public_outcome["passed"]:
            record.update(status=FAILED, reason=f"public {public_outcome['phase']}: "
                          f"{'; '.join(public_outcome['detail'])}")
            return record
        if leaked_only:
            record.update(status=FAILED, reason=f"hidden path leaked into the public "
                          f"build tree: {sorted(leaked_only)}")
            return record

        copy_tree(public_dir, hidden_scratch)
        copy_tree(hidden_dir, hidden_scratch)  # overlay: same-path files replace
        hidden_outcome = stage(hidden_scratch, adapter, semaprax_binary, execution)
        record["hidden"] = {"passed": hidden_outcome["passed"], "detail": hidden_outcome["detail"]}
        if not hidden_outcome["passed"]:
            record.update(status=FAILED, reason=f"hidden {hidden_outcome['phase']}: "
                          f"{'; '.join(hidden_outcome['detail'])}")
            return record

        record["status"] = OK
        return record
    finally:
        shutil.rmtree(public_scratch, ignore_errors=True)
        shutil.rmtree(hidden_scratch, ignore_errors=True)


def summarize(records: list) -> dict:
    summary = {OK: 0, FAILED: 0, BLOCKED: 0, DRIFTED: 0}
    for record in records:
        summary[record["status"]] = summary.get(record["status"], 0) + 1
    return summary


def compare_rows(local: dict, baseline: dict) -> list:
    """Pass/fail regression only. There is no timing field to compare in v1."""
    base_map = {row["id"]: row for row in baseline.get("results", [])}
    rows = []
    for record in local.get("results", []):
        base = base_map.get(record["id"])
        if base is None:
            rows.append({"id": record["id"], "verdict": "no baseline"})
            continue
        if record["status"] != OK or base["status"] != OK:
            rows.append({
                "id": record["id"],
                "verdict": "incomparable",
                "reason": f"baseline={base['status']} local={record['status']}",
            })
            continue
        rows.append({"id": record["id"], "verdict": "unchanged (both ok)"})
    return rows


def selected_pairs(tasks: list, adapters: dict, args) -> list:
    pairs = []
    for task in tasks:
        if args.only and task["id"] not in args.only:
            continue
        languages = sorted(set(task.get("languages", {})) | set(adapters))
        for language in languages:
            if args.language and language not in args.language:
                continue
            pairs.append((task, language))
    return pairs


def write_json(path: pathlib.Path, document: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, sort_keys=True, indent=2) + "\n")


def fail(message: str) -> int:
    print(f"error: {message}", file=sys.stderr)
    return 2


def dry_run(root: pathlib.Path, tasks: list, adapters: dict, args) -> int:
    planned = []
    problems = []
    for task, language in selected_pairs(tasks, adapters, args):
        adapter = adapters.get(language, {"implemented": False, "blocked_reason": "undeclared adapter"})
        paths = task.get("languages", {}).get(language)
        row = {
            "id": f"{task['id']}::{language}",
            "task": task["id"],
            "language": language,
            "implemented": adapter.get("implemented", False),
        }
        if paths is not None:
            public_dir = root / paths["public"]
            hidden_dir = root / paths["hidden"]
            row["public_dir"] = str(public_dir)
            row["hidden_dir"] = str(hidden_dir)
            row["exists"] = public_dir.is_dir() and hidden_dir.is_dir()
            if adapter.get("implemented", False):
                problem = (
                    hidden_overlay_problem(public_dir, hidden_dir)
                    if public_dir.is_dir()
                    else f"missing public directory: {public_dir}"
                )
                if problem is not None:
                    problems.append((row["id"], problem))
        planned.append(row)
    document = {
        "schema": PLAN_SCHEMA,
        "root": str(root),
        "suite": str(SUITE),
        "output": args.output,
        "compare": args.compare,
        "pairs": planned,
    }
    write_json(pathlib.Path(args.output), document)
    print(f"Wrote {args.output} ({len(planned)} task/language pairs planned, measured nothing)")
    for identifier, problem in problems:
        print(f"error: {identifier}: {problem}", file=sys.stderr)
    return 1 if problems else 0


def main():
    parser = argparse.ArgumentParser(description="cross-language toolchain-conformance harness")
    parser.add_argument("--output", required=True, help="output JSON path")
    parser.add_argument("--compare", help="baseline JSON to compare against")
    parser.add_argument("--dry-run", action="store_true", help="resolve the inventory, run nothing")
    parser.add_argument("--only", action="append", metavar="TASK_ID", help="restrict to this task id (repeatable)")
    parser.add_argument("--language", action="append", metavar="ID", help="restrict to this language id (repeatable)")
    parser.add_argument("--tasks", help=f"task inventory to read (default: {TASKS})")
    parser.add_argument("--adapters", help=f"adapter inventory to read (default: {ADAPTERS})")
    parser.add_argument("--root", help=f"repository root task paths resolve against (default: {ROOT})")
    parser.add_argument("--semaprax", help="path to the semaprax binary the `semaprax` adapter invokes")
    parser.add_argument("--hardened-posix", action="store_true",
                        help="use the optional bounded POSIX execution profile")
    parser.add_argument("--execution-deadline-monotonic", type=float,
                        help="shared monotonic deadline required with --hardened-posix")
    parser.add_argument("--execution-output-bytes", type=int,
                        help="per-stream cap required with --hardened-posix")
    args = parser.parse_args()

    root = pathlib.Path(args.root).resolve() if args.root else ROOT
    tasks_path = pathlib.Path(args.tasks).resolve() if args.tasks else TASKS
    adapters_path = pathlib.Path(args.adapters).resolve() if args.adapters else ADAPTERS

    execution = None
    if args.hardened_posix:
        if os.name != "posix":
            return fail("hardened execution is unavailable on this host")
        if os.getpgrp() != os.getpid():
            return fail("hardened execution requires an admitted group leader")
        if (args.execution_deadline_monotonic is None
                or type(args.execution_output_bytes) is not int
                or not 1 <= args.execution_output_bytes <= 1024 * 1024):
            return fail("hardened execution requires bounded deadline and output")
        execution = HardenedExecution(
            environment=dict(os.environ),
            deadline=args.execution_deadline_monotonic,
            output_limit=args.execution_output_bytes,
            group_id=os.getpgrp(),
        )

    try:
        tasks_document = load_json(tasks_path, TASKS_SCHEMA, "task inventory")
    except FileNotFoundError:
        return fail(f"task inventory not found: {tasks_path}")
    except (json.JSONDecodeError, ValueError) as error:
        return fail(f"task inventory is invalid: {tasks_path}: {error}")
    try:
        adapters = resolve_adapters(adapters_path)
    except FileNotFoundError:
        return fail(f"adapter inventory not found: {adapters_path}")
    except (json.JSONDecodeError, ValueError) as error:
        return fail(f"adapter inventory is invalid: {adapters_path}: {error}")

    tasks = sorted(tasks_document["tasks"], key=lambda item: item["id"])
    unknown = set(args.only or []) - {task["id"] for task in tasks}
    if unknown:
        return fail(f"unknown task id(s): {', '.join(sorted(unknown))}")

    if args.dry_run:
        return dry_run(root, tasks, adapters, args)

    pairs = selected_pairs(tasks, adapters, args)
    if not pairs:
        return fail("no task/language pair selected")

    semaprax_binary = args.semaprax or "semaprax"
    results = []
    for task, language in pairs:
        adapter = adapters.get(language)
        if adapter is None:
            record = {
                "id": f"{task['id']}::{language}",
                "task": task["id"],
                "language": language,
                "category": task["category"],
                "status": BLOCKED,
                "reason": "no declared adapter",
            }
        else:
            print(f"[{task['id']}::{language}] ...", flush=True)
            record = evaluate_pair(root, task, language, adapter, semaprax_binary, execution)
        print(f"  -> status={record['status']} {record.get('reason', '')}".rstrip(), flush=True)
        results.append(record)

    summary = summarize(results)
    document = {
        "schema": SCHEMA,
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "host": host_facts(),
        "revision": git_revision(ROOT),
        "timing": {
            "collected": False,
            "reason": "this host runs many concurrent build lanes; any wall-clock "
                      "measurement here would record contention, not the subject. "
                      "See issues #85, #130, #131 and docs/METHODOLOGY.md.",
        },
        "summary": summary,
        "results": results,
    }
    out_path = pathlib.Path(args.output)
    write_json(out_path, document)
    print(f"Wrote {out_path} ({len(results)} task/language pairs)")

    status = 0
    if summary[FAILED] or summary[DRIFTED]:
        print(f"error: {summary[FAILED]} failed and {summary[DRIFTED]} drifted pair(s)", file=sys.stderr)
        status = 1

    if args.compare:
        compare_path = pathlib.Path(args.compare)
        try:
            baseline = json.loads(compare_path.read_text())
        except FileNotFoundError:
            return fail(f"baseline not found: {compare_path}")
        except json.JSONDecodeError as error:
            return fail(f"baseline is not valid JSON: {compare_path}: {error}")
        if not baseline.get("results"):
            return fail(
                f"baseline holds no recorded measurement: {compare_path}"
                f" ({baseline.get('reason', 'no results')})"
            )
        print("\nComparison (local vs baseline):")
        for row in compare_rows(document, baseline):
            reason = row.get("reason", "")
            print(f"  {row['id']}: {row['verdict']} {reason}".rstrip())

    return status


if __name__ == "__main__":
    sys.exit(main())
