"""Wires a `SolverTransport`'s output into `run.py`'s existing scoring.

`evaluate_agent_pair` is the agent-driven analogue of `run.py::evaluate_pair`:
same digesting, same two-phase (public then hidden) scratch-tree scoring,
same leak check, same `stage()` build/run step — all imported from `run.py`
via `_harness.run_module()`, never reimplemented. The only thing this module
adds is what sits *before* that scoring: build a prompt, call a transport,
enforce its budget, and (only if a candidate was actually produced within
budget) hand the candidate's files to the exact same scratch-tree machinery
every other adapter already goes through.

The result is also the evidence boundary for that transport: every transcript
entry and candidate artifact is retained in a deterministic order and commits
to its original bytes. Callers can supply an explicit literal-redaction policy
for a publishable projection; its secret literals are not emitted, while the
per-item content digest makes every redaction visible and tamper-evident.

A pair never reaches the build/run step at all when the transport raises
`BudgetExceededError` or `RetriesExhaustedError`: the record is written with
status `budget_exceeded` or `retries_exhausted` and no `public`/`hidden` key,
the same discipline `run.py` already uses for `blocked` and `drifted` pairs
(see `evaluate_pair`'s doc comment and the "Result statuses" note at the top
of `run.py`) — a terminal outcome that took no compute is recorded as one,
not silently upgraded or downgraded into a `failed` build that never ran.
"""
from __future__ import annotations

import shutil
from pathlib import PurePosixPath

from ._harness import run_module
from .budget import BudgetExceededError, RetriesExhaustedError
from .contracts import SolverRequest, redact_content, redaction_policy_digest, sha256_text, transcript_digest
from .prompts import build_prompt
from .transport import CredentialsRequiredError, LiveTransportUnexercisedError, SolverTransport

AGENT_SCHEMA = "benchmark.cross_language.agent.v1"


def build_request(task, language, model, sampling, budget, pricing, equivalence_text, public_dir, candidate_paths):
    prompt = build_prompt(task["id"], language, equivalence_text, public_dir, candidate_paths)
    return SolverRequest(
        task_id=task["id"],
        language=language,
        prompt=prompt,
        model=model,
        sampling=sampling,
        budget=budget,
        pricing=pricing,
    )


def _candidate_path_problem(path) -> str | None:
    """Return a stable refusal reason for an untrusted candidate path.

    Candidate artifacts are the only transport-provided filesystem inputs
    that the scorer writes.  Their names must be portable POSIX-relative
    paths, never traversal, host-absolute, or platform-specific escape
    shapes.  The expected-path equality check below then prevents a solver
    from replacing any public scaffold or test file it was not asked to
    author.
    """
    if not isinstance(path, str) or not path:
        return "not a non-empty string"
    if "\\" in path or "\x00" in path:
        return "contains a platform separator or NUL"
    relative = PurePosixPath(path)
    if relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts):
        return "is not a plain relative path"
    if any(":" in part for part in relative.parts):
        return "has a platform-specific root"
    return None


def _candidate_artifacts(candidate_files: dict, redactions) -> list[dict]:
    artifacts = []
    for relative in sorted(candidate_files):
        content = candidate_files[relative]
        projected, redacted = redact_content(content, redactions)
        artifacts.append(
            {
                "path": relative,
                "content": projected,
                "content_digest": sha256_text(content),
                "redacted": redacted,
            }
        )
    return artifacts


def _write_candidate(scratch, candidate_files: dict) -> None:
    for relative in sorted(candidate_files):
        content = candidate_files[relative]
        target = scratch.joinpath(*PurePosixPath(relative).parts)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content)


def _terminal(record: dict, status: str, error, redactions=()) -> dict:
    transcript = list(getattr(error, "transcript", []))
    record.update(
        status=status,
        reason=str(error),
        usage=error.usage.to_dict(),
        transcript=[entry.to_dict(redactions) for entry in transcript],
        transcript_digest=transcript_digest(transcript) if transcript else None,
        evidence={
            "redaction_policy_digest": redaction_policy_digest(redactions),
            "candidate_artifacts": [],
        },
    )
    return record


def evaluate_agent_pair(
    root,
    task: dict,
    language: str,
    adapter: dict,
    transport: SolverTransport,
    request: SolverRequest,
    semaprax_binary: str = "semaprax",
    candidate_paths=None,
    redactions=(),
) -> dict:
    """Score one (task, language) pair by asking `transport` for a candidate
    and then running it through `run.py`'s own build/test/leak-check/
    provenance machinery. `candidate_paths` names which relative path(s) in
    the task's public tree the transport's `candidate_files` are expected to
    supply; every other file in the public tree is fixed scaffold, copied
    unchanged (the same fixed test harness a human solver would also see and
    could not alter). The response must supply exactly this declared path
    set, so it cannot replace a scaffold, public test, or path outside the
    scratch tree.
    """
    run = run_module()
    candidate_paths = list(candidate_paths or [])
    try:
        redaction_policy = tuple(redactions or ())
        redaction_policy_digest(redaction_policy)
    except ValueError as error:
        raise ValueError(f"invalid redaction policy: {error}") from error
    record = {
        "schema": AGENT_SCHEMA,
        "id": f"{task['id']}::{language}",
        "task": task["id"],
        "language": language,
        "category": task.get("category"),
        "transport": type(transport).__name__,
        "request": request.to_dict(),
    }

    languages = task.get("languages", {})
    paths = languages.get(language)
    if paths is None:
        record.update(status="blocked", reason=f"task declares no {language} implementation")
        return record

    public_dir = root / paths["public"]
    hidden_dir = root / paths["hidden"]
    if not public_dir.is_dir():
        record.update(status="failed", reason=f"missing public directory: {public_dir}")
        return record
    problem = run.hidden_overlay_problem(public_dir, hidden_dir)
    if problem is not None:
        record.update(status="failed", reason=problem)
        return record

    try:
        response = transport.complete(request)
    except BudgetExceededError as error:
        return _terminal(record, "budget_exceeded", error, redaction_policy)
    except RetriesExhaustedError as error:
        return _terminal(record, "retries_exhausted", error, redaction_policy)
    except (CredentialsRequiredError, LiveTransportUnexercisedError) as error:
        record.update(status="blocked", reason=str(error))
        return record

    record["usage"] = response.usage.to_dict()
    record["transcript"] = [entry.to_dict(redaction_policy) for entry in response.transcript]
    record["transcript_digest"] = response.transcript_digest
    record["evidence"] = {
        "redaction_policy_digest": redaction_policy_digest(redaction_policy),
    }

    if not candidate_paths:
        record.update(status="failed", reason="at least one candidate path must be declared")
        return record
    invalid_expected = sorted(
        f"{path!r}: {_candidate_path_problem(path)}"
        for path in candidate_paths
        if _candidate_path_problem(path) is not None
    )
    if invalid_expected:
        record.update(status="failed", reason=f"invalid declared candidate path(s): {invalid_expected}")
        return record
    if len(set(candidate_paths)) != len(candidate_paths):
        invalid_expected.append("duplicate candidate path")
    if invalid_expected:
        record.update(status="failed", reason=f"invalid declared candidate path(s): {invalid_expected}")
        return record
    if not isinstance(response.candidate_files, dict):
        record.update(status="failed", reason="transport candidate artifacts must be an object")
        return record
    invalid_provided = sorted(
        f"{path!r}: {_candidate_path_problem(path)}"
        for path in response.candidate_files
        if _candidate_path_problem(path) is not None
    )
    invalid_contents = sorted(
        repr(path) for path, content in response.candidate_files.items() if not isinstance(content, str)
    )
    if invalid_provided or invalid_contents:
        detail = []
        if invalid_provided:
            detail.append(f"invalid paths={invalid_provided}")
        if invalid_contents:
            detail.append(f"non-text contents={invalid_contents}")
        record.update(status="failed", reason=f"invalid transport candidate artifacts: {'; '.join(detail)}")
        return record

    missing = sorted(set(candidate_paths) - set(response.candidate_files))
    unexpected = sorted(set(response.candidate_files) - set(candidate_paths))
    if missing or unexpected:
        record.update(
            status="failed",
            reason=f"transport candidate paths mismatch: missing={missing}; unexpected={unexpected}",
        )
        return record

    record["evidence"]["candidate_artifacts"] = _candidate_artifacts(
        response.candidate_files, redaction_policy
    )

    scaffold_digest = run.digest_tree(public_dir)
    hidden_digest = run.digest_tree(hidden_dir)
    candidate_rows = sorted(f"{k}\0{v}" for k, v in response.candidate_files.items())
    candidate_digest = run.sha256_bytes("\n".join(candidate_rows).encode())
    combined = run.sha256_bytes(f"{scaffold_digest}\n{candidate_digest}\n{hidden_digest}".encode())
    record["provenance"] = {
        "adapter_version": run.tool_version(run.command_for(adapter, "version_command", semaprax_binary)),
        "scaffold_digest": scaffold_digest,
        "candidate_digest": candidate_digest,
        "hidden_digest": hidden_digest,
        "digest": combined,
        "prompt_digest": request.prompt_digest,
        "transcript_digest": record["transcript_digest"],
        "model": request.model.to_dict(),
        "seed": request.sampling.seed,
    }

    public_scratch = run.scratch_dir(f"agent-public-{language}")
    hidden_scratch = run.scratch_dir(f"agent-hidden-{language}")
    try:
        run.copy_tree(public_dir, public_scratch)
        _write_candidate(public_scratch, response.candidate_files)
        public_outcome = run.stage(public_scratch, adapter, semaprax_binary)

        # Same leak check `run.py::evaluate_pair` performs, over the same
        # scratch tree shape, reusing the same function rather than a second
        # copy of the comparison.
        leaked = run.relative_files(hidden_dir) & run.relative_files(public_scratch)
        hidden_only = run.relative_files(hidden_dir) - run.relative_files(public_dir)
        leaked_only = leaked & hidden_only
        record["leak_check"] = "ok" if not leaked_only else sorted(leaked_only)

        record["public"] = {"passed": public_outcome["passed"], "detail": public_outcome["detail"]}
        if not public_outcome["passed"]:
            record.update(
                status="failed",
                reason=f"public {public_outcome['phase']}: {'; '.join(public_outcome['detail'])}",
            )
            return record
        if leaked_only:
            record.update(
                status="failed",
                reason=f"hidden path leaked into the public build tree: {sorted(leaked_only)}",
            )
            return record

        run.copy_tree(public_dir, hidden_scratch)
        _write_candidate(hidden_scratch, response.candidate_files)
        run.copy_tree(hidden_dir, hidden_scratch)  # overlay: same-path files replace
        hidden_outcome = run.stage(hidden_scratch, adapter, semaprax_binary)
        record["hidden"] = {"passed": hidden_outcome["passed"], "detail": hidden_outcome["detail"]}
        if not hidden_outcome["passed"]:
            record.update(
                status="failed",
                reason=f"hidden {hidden_outcome['phase']}: {'; '.join(hidden_outcome['detail'])}",
            )
            return record

        record["status"] = "ok"
        return record
    finally:
        shutil.rmtree(public_scratch, ignore_errors=True)
        shutil.rmtree(hidden_scratch, ignore_errors=True)
