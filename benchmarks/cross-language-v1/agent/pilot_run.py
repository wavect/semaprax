"""Native generation and one-use controller scoring; receipts carry no authority."""
from __future__ import annotations
import argparse
import base64
import json
import os
import pathlib
import stat
import sys
import uuid
from . import pilot_protocol as p
from .claude_subscription import ClaudeSubscription, Failure, decode, native_identity, model_arguments
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


def binding(plan, model_id, host_id, *, guest_plan_sha256=None):
    if guest_plan_sha256 is None:
        p.admit(plan, p.digest(p.canonical(plan)))
    else:
        p.admit_guest(plan, guest_plan_sha256)
    host = plan["configuration"]["hosts"].get(host_id)
    model = next((row for row in plan["configuration"]["models"] if row["id"] == model_id), None)
    if host is None or model is None:
        raise ValueError("model_or_host_not_in_frozen_plan")
    if guest_plan_sha256 is not None and host["native_platform"] != "linux-arm64":
        raise ValueError("guest_snapshot_requires_linux_host")
    return host, model


def generate_cell(plan, model_id, host_id, directory, *, guest_plan_sha256=None):
    host, model = binding(plan, model_id, host_id, guest_plan_sha256=guest_plan_sha256)
    directory = pathlib.Path(directory)
    if not directory.is_absolute() or directory.parent != directory.parent.resolve():
        raise ValueError("trial_directory_must_be_canonical_absolute")
    directory.mkdir(mode=0o700)
    sync_directory(directory.parent)
    receipt = {"schema": "benchmark.cross_language.live_pilot_trial.v1", "plan_sha256": p.digest(p.canonical(plan)),
               "invocation_id": uuid.uuid4().hex, "host_id": host_id, "model_id": model_id,
               "task_id": p.TASK, "adapter_id": "typescript", "status": "started", "model_dispatches": 0,
               "score": None, "response": None, "reason": None,
               "source_admission": p.source_admission(plan, "controller_frozen_snapshot" if guest_plan_sha256 is not None else "local_git_checkout")}
    write_new(directory / "intent.json", receipt)
    try:
        # Metadata and platform are checked on the actual provider machine;
        # a Mac process can never borrow a Linux host label or its OAuth home.
        transport = ClaudeSubscription(host, directory)
        def started():
            receipt["model_dispatches"] = 1
            write_new(directory / "transport-started.json", {"invocation_id": receipt["invocation_id"], "kind": "native_claude_process_spawned"})
        receipt["response"] = transport.complete(plan, model, on_started=started)
        receipt["model_dispatches"] = 1
        receipt["status"] = "generated"
        receipt["candidate_files_sha256"] = p.digest(p.canonical(receipt["response"]["candidate_files"]))
    except Failure as error:
        receipt.update(status="failed", reason=str(error), model_dispatches=error.receipt.get("dispatches", 0), transport_failure=error.receipt)
    except (ValueError, OSError, KeyError, TypeError) as error:
        receipt.update(status="failed", reason=str(error))
    write_new(directory / "generation.json", receipt)
    if receipt["status"] == "failed":
        write_new(directory / "result.json", receipt)
    return receipt


def admit_generation(plan, generation, expected_digest):
    if p.digest(p.canonical(generation)) != expected_digest:
        raise ValueError("generation_digest_mismatch")
    host, model = binding(plan, generation["model_id"], generation["host_id"])
    if (generation.get("schema") != "benchmark.cross_language.live_pilot_trial.v1"
            or generation.get("plan_sha256") != p.digest(p.canonical(plan))
            or generation.get("status") != "generated" or type(generation.get("model_dispatches")) is not int
            or generation["model_dispatches"] != 1
            or generation.get("task_id") != p.TASK or generation.get("adapter_id") != "typescript"):
        raise ValueError("generation_binding_refused")
    source = generation.get("source_admission", {})
    kind = source.get("kind")
    if (source != p.source_admission(plan, kind)
            or (kind == "controller_frozen_snapshot" and host["native_platform"] != "linux-arm64")):
        raise ValueError("generation_source_admission_refused")
    response = generation["response"]
    transport = response["transport_receipt"]
    expected_host = {key: host[key] for key in ("native_platform", "kernel_release", "boot_id")}
    if (type(transport.get("dispatches")) is not int or transport["dispatches"] != 1
            or transport.get("provider_host") != expected_host
            or transport.get("host_authority_sha256") != p.digest(p.canonical(host))
            or transport.get("cli_sha256") != host["claude_sha256"]
            or transport.get("cli_version") != plan["configuration"]["cli_version"]
            or transport.get("requested_model") != model["requested_model"]
            or transport.get("prompt_sha256") != plan["prompt_sha256"]
            or transport.get("argv") != model_arguments(plan, model)
            or transport.get("failure") is not None or transport.get("exit_code") != 0):
        raise ValueError("provider_host_or_transport_binding_refused")
    version = transport["cli_version_receipt"]
    if (version.get("failure") is not None or version.get("exit_code") != 0
            or base64.b64decode(version["stdout_base64"], validate=True).decode().strip() != plan["configuration"]["cli_version"] + " (Claude Code)"):
        raise ValueError("provider_version_receipt_refused")
    wire = base64.b64decode(transport["stdout_base64"], validate=True)
    if len(wire) > 1024 * 1024:
        raise ValueError("provider_wire_bound")
    decoded = decode(wire, model, plan["configuration"]["limits"])
    if response != dict(decoded, transport_receipt=transport):
        raise ValueError("candidate_or_usage_disagrees_with_provider_bytes")
    if generation["candidate_files_sha256"] != p.digest(p.canonical(decoded["candidate_files"])):
        raise ValueError("candidate_digest_mismatch")
    return host


def score_generation(plan, generation_path, expected_digest, provenance_directory):
    path = pathlib.Path(generation_path)
    raw = p.provenance.read_regular(path, 2 * 1024 * 1024)
    generation = p.strict_json(raw)
    if raw != p.canonical(generation):
        raise ValueError("noncanonical_generation_receipt")
    host = admit_generation(plan, generation, expected_digest)
    controller = next(row for row in plan["configuration"]["hosts"].values() if row["native_platform"] == "darwin-arm64")
    if native_identity() != {key: controller[key] for key in ("native_platform", "kernel_release", "boot_id")}:
        raise ValueError("scoring_controller_host_mismatch")
    ledger = pathlib.Path(plan["configuration"]["controller_ledger"])
    facts = ledger.lstat()
    if ledger != ledger.resolve() or not stat.S_ISDIR(facts.st_mode) or facts.st_uid != os.getuid() or facts.st_mode & 0o077:
        raise ValueError("private_controller_ledger_required")
    # Frozen location + create-new digest key makes copied/reused receipts fail
    # before scorer construction. Failure or interruption never refunds this.
    cell = p.digest(p.canonical({"plan": generation["plan_sha256"], "host": generation["host_id"], "model": generation["model_id"]}))
    write_new(ledger / ("cell-" + cell + ".json"), {"generation_sha256": expected_digest,
              "plan_sha256": p.digest(p.canonical(plan)), "invocation_id": generation["invocation_id"]})
    result = dict(generation, status="scoring_started", generation_sha256=expected_digest)
    write_new(path.parent / "scoring-intent.json", result)
    try:
        if host["native_platform"] == "darwin-arm64":
            session = CandidateSession(provenance_directory)
        else:
            from .pilot_linux_host import LinuxCandidateSession
            session = LinuxCandidateSession(provenance_directory,
                      expected_provision_sha256=plan["execution_profiles"]["linux-arm64"]["provision_sha256"])
        with session:
            session.admit_pilot(plan)
            result["score"] = session.score_candidate(plan, generation["response"]["candidate_files"])
            result["status"] = result["score"]["status"]
            write_new(path.parent / "scoring.json", session.evidence())
    except (ValueError, OSError, KeyError, TypeError, ImportError) as error:
        result.update(status="failed", reason=str(error))
    write_new(path.parent / "result.json", result)
    return result


def account(plan, host_id, trial_directories):
    p.admit(plan, p.digest(p.canonical(plan)))
    if host_id not in plan["configuration"]["hosts"]:
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
            observed[key] = {"status": "interrupted", "reason": "intent_without_terminal_scoring_receipt"}
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
    for name in ("generate", "generate-guest", "score", "account"):
        child = commands.add_parser(name)
        child.add_argument("--plan", required=True)
        child.add_argument("--plan-sha256", required=True)
        if name in ("generate", "generate-guest"):
            for arg in ("host-id", "model-id", "directory"):
                child.add_argument("--" + arg, required=True)
        elif name == "score":
            for arg in ("generation", "generation-sha256", "provenance-directory"):
                child.add_argument("--" + arg, required=True)
        else:
            child.add_argument("--host-id", required=True)
            child.add_argument("--trial-directory", action="append", default=[])
            child.add_argument("--output", required=True)
    args = parser.parse_args(argv)
    try:
        if args.command == "freeze":
            plan = p.freeze(read_json(args.configuration))
            write_new(args.output, plan)
            print(p.digest(p.canonical(plan)))
            return 0
        guest = args.command == "generate-guest"
        plan = (p.admit_guest if guest else p.admit)(read_json(args.plan), args.plan_sha256)
        if args.command == "account":
            write_new(args.output, account(plan, args.host_id, args.trial_directory))
            return 0
        if args.command in ("generate", "generate-guest"):
            result = generate_cell(plan, args.model_id, args.host_id, args.directory,
                                   guest_plan_sha256=args.plan_sha256 if guest else None)
        else:
            result = score_generation(plan, args.generation, args.generation_sha256, args.provenance_directory)
        print(json.dumps({"status": result["status"], "reason": result["reason"], "model_dispatches": result["model_dispatches"],
                          "receipt_sha256": p.digest(p.canonical(result))}))
        return 0 if result["status"] in ("ok", "generated") else 3
    except (ValueError, OSError, KeyError, TypeError) as error:
        print("pilot refused: " + str(error), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
