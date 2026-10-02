"""Freeze, run one one-use model cell, and account for the full pilot inventory."""
from __future__ import annotations
import argparse
import json
import os
import pathlib
import sys
import uuid
from . import pilot_protocol as p
from .claude_subscription import ClaudeSubscription, Failure
from .pilot_score import CandidateSession


def read_json(path):
    return p.strict_json(p.provenance.read_regular(pathlib.Path(path), 2 * 1024 * 1024))


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def write_new(path, value):
    """Private create-new evidence; no overwrite or automatic publication."""
    path = pathlib.Path(path)
    if not path.is_absolute() or path.parent != path.parent.resolve():
        raise ValueError("evidence_path_must_be_canonical_absolute")
    data = p.canonical(value)
    if len(data) > 16 * 1024 * 1024:
        raise ValueError("evidence_size_bound")
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    sync_directory(path.parent)


def run_cell(plan, model_id, host_id, directory, provenance_directory, executable, home, login):
    p.admit(plan, p.digest(p.canonical(plan)))
    config = plan["configuration"]
    if host_id not in config["host_ids"]:
        raise ValueError("host_not_in_frozen_plan")
    model = next((row for row in config["models"] if row["id"] == model_id), None)
    if model is None:
        raise ValueError("model_not_in_frozen_plan")
    directory = pathlib.Path(directory)
    if not directory.is_absolute() or directory.parent != directory.parent.resolve():
        raise ValueError("trial_directory_must_be_canonical_absolute")
    directory.mkdir(mode=0o700)  # One attempt, never overwrite/retry a consumed cell.
    sync_directory(directory.parent)
    receipt = {"schema": "benchmark.cross_language.live_pilot_trial.v1", "plan_sha256": p.digest(p.canonical(plan)),
               "invocation_id": uuid.uuid4().hex, "host_id": host_id, "model_id": model_id,
               "task_id": p.TASK, "adapter_id": "typescript", "status": "started", "model_dispatches": 0,
               "score": None, "response": None, "reason": None}
    write_new(directory / "intent.json", receipt)
    session = None
    try:
        # Unsupported host/provenance/candidate admission refuses BEFORE spend.
        with CandidateSession(provenance_directory) as session:
            session.admit_pilot(plan)
            transport = ClaudeSubscription(executable, home, login, directory)
            def started():
                receipt["model_dispatches"] = 1
                write_new(directory / "transport-started.json", {"invocation_id": receipt["invocation_id"], "kind": "claude_cli_process_spawned"})
            receipt["response"] = transport.complete(plan, model, on_started=started)
            receipt["score"] = session.score_candidate(plan, receipt["response"]["candidate_files"])
            receipt["status"] = receipt["score"]["status"]
            write_new(directory / "scoring.json", session.evidence())
    except Failure as error:
        receipt.update(status="failed", reason=str(error), model_dispatches=1, transport_failure=error.receipt)
    except (ValueError, OSError, KeyError, TypeError) as error:
        receipt.update(status="failed", reason=str(error))
    write_new(directory / "result.json", receipt)
    return receipt


def account(plan, host_id, trial_directories):
    """Untrusted receipt inventory, never grants execution or host admission."""
    p.admit(plan, p.digest(p.canonical(plan)))
    if host_id not in plan["configuration"]["host_ids"]:
        raise ValueError("host_not_in_frozen_plan")
    rows = [dict(row) for row in plan["trials_per_host"]]
    observed = {}
    for directory in trial_directories:
        directory = pathlib.Path(directory)
        intent = read_json(directory / "intent.json")
        if intent.get("plan_sha256") != p.digest(p.canonical(plan)) or intent.get("host_id") != host_id:
            raise ValueError("trial_plan_or_host_mismatch")
        key = (intent.get("model_id"), intent.get("task_id"), intent.get("adapter_id"))
        if key not in {(row["model_id"], row["task_id"], row["adapter_id"]) for row in rows if row["disposition"] == "planned"} or key in observed:
            raise ValueError("duplicate_or_unplanned_trial")
        result_path = directory / "result.json"
        if not result_path.exists():
            observed[key] = {"status": "interrupted", "reason": "intent_without_terminal_receipt"}
            continue
        result = read_json(result_path)
        for field in ("plan_sha256", "host_id", "model_id", "task_id", "adapter_id", "invocation_id"):
            if result.get(field) != intent.get(field):
                raise ValueError("terminal_receipt_binding_refused")
        if result.get("status") not in ("ok", "failed"):
            raise ValueError("terminal_receipt_status_refused")
        observed[key] = {"status": result["status"], "reason": result.get("reason"),
                         "result_sha256": p.digest(p.canonical(result)), "model_dispatches": result.get("model_dispatches")}
    for row in rows:
        key = (row["model_id"], row["task_id"], row["adapter_id"])
        row["observation"] = observed.get(key, {"status": "not_attempted" if row["disposition"] == "planned" else "unavailable"})
    return {"schema": "benchmark.cross_language.live_pilot_accounting.v1", "plan_sha256": p.digest(p.canonical(plan)),
            "host_id": host_id, "comparison_inventory": plan["comparison_inventory"], "trials": rows,
            "claim": "local_receipt_accounting_only_not_independent_host_or_review_attestation",
            "all_planned_trials_have_receipts": len(observed) == 2}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    freeze = commands.add_parser("freeze")
    freeze.add_argument("--configuration", required=True)
    freeze.add_argument("--output", required=True)
    for name in ("run", "account"):
        child = commands.add_parser(name)
        child.add_argument("--plan", required=True)
        child.add_argument("--plan-sha256", required=True)
        child.add_argument("--host-id", required=True)
        if name == "run":
            for arg in ("model-id", "directory", "provenance-directory", "executable", "home", "login"):
                child.add_argument("--" + arg, required=True)
        else:
            child.add_argument("--trial-directory", action="append", default=[])
            child.add_argument("--output", required=True)
    args = parser.parse_args(argv)
    try:
        if args.command == "freeze":
            plan = p.freeze(read_json(args.configuration))
            write_new(args.output, plan)
            print(p.digest(p.canonical(plan)))
            return 0
        plan = p.admit(read_json(args.plan), args.plan_sha256)
        if args.command == "account":
            write_new(args.output, account(plan, args.host_id, args.trial_directory))
            return 0
        result = run_cell(plan, args.model_id, args.host_id, args.directory, args.provenance_directory,
                          args.executable, args.home, args.login)
        print(json.dumps({"status": result["status"], "reason": result["reason"], "model_dispatches": result["model_dispatches"]}))
        return 0 if result["status"] == "ok" else 3
    except (ValueError, OSError, KeyError, TypeError) as error:
        print("pilot refused: " + str(error), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
