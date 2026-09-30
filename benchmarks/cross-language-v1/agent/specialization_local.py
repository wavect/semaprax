#!/usr/bin/env python3
"""Prepare or execute the original 81 control slots using reviewed local inputs.

Preparation contacts only explicit loopback metadata endpoints; it performs no
inference. Run requires a separately reviewed, byte-pinned authorization. The
frozen v1 protocol builder remains proposal-only and is never made executable.
Model results, daemon failures, unavailable oracle cells and absent review are
separate states. This tool never closes an issue or creates reviewer approval.
"""
from __future__ import annotations
import argparse
import contextlib
import json
import os
import pathlib
import random
import re
import sys
import time

SUITE = pathlib.Path(__file__).resolve().parent.parent
if str(SUITE) not in sys.path:
    sys.path.insert(0, str(SUITE))
import runnable_adapter as bounds
import runnable_v3_provenance as provenance
from agent import specialization_accounting as accounting
from agent import specialization_protocol as protocol
from agent.contracts import Budget, ModelIdentity, PricingRates, SamplingParams, SolverRequest
from agent.local_ollama import CONTEXT_TOKENS, Client, LocalOllamaTransport, LocalTransportError, ModelPin, canonical, digest, raw_prompt, strict_json
from agent.specialization_inputs import CANDIDATES, GUIDANCE, UNAVAILABLE, PREFIX, model_prompt, projection_identity, public_tree
from agent.specialization_native import NativeScorer

PLAN_SCHEMA = "benchmark.cross_language.agent.local_specialization_inputs.v1"
REVIEW_SCHEMA = "benchmark.cross_language.agent.local_specialization_review.v1"
RUN_SCHEMA = "benchmark.cross_language.agent.local_specialization_results.v1"
PRECHECKS = ("model_origin_and_local_daemon", "zero_spend_and_no_egress",
             "source_oracle_and_excluded_discount", "prompt_and_split_leakage",
             "compiler_source_and_sandbox_negative_controls", "independent_review_and_custody")
RUNTIME_FILES = ("local_ollama.py", "specialization_inputs.py", "specialization_native.py",
                 "specialization_local.py", "specialization_accounting.py",
                 "specialization_protocol.py", "contracts.py", "budget.py", "transport.py")


def read_json(path, limit=bounds.MAX_RESULT_BYTES):
    raw = provenance.read_regular(pathlib.Path(path), limit)
    value = strict_json(raw)
    if not isinstance(value, dict):
        raise LocalTransportError("object_document_required")
    return value, raw


@contextlib.contextmanager
def held_directory(path):
    """Open every directory component without following a replaced ancestor."""
    path = pathlib.Path(path)
    if not path.is_absolute() or ".." in path.parts:
        raise LocalTransportError("absolute_nofollow_directory_required")
    fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        for component in path.parts[1:]:
            nxt = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=fd)
            os.close(fd)
            fd = nxt
        yield fd
    finally:
        os.close(fd)


def write_json(path, value):
    data = canonical(value)
    if len(data) > bounds.MAX_RESULT_BYTES:
        raise LocalTransportError("record_byte_bound")
    path = pathlib.Path(path)
    with held_directory(path.parent) as fd:
        leaf = os.open(path.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=fd)
        # Retain partial files on storage failure: no silent overwrite/retry.
        with os.fdopen(leaf, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
    return digest(data)


def save_scoring(output, label, check):
    """Bound each process receipt separately instead of raising result limits."""
    result = {k: v for k, v in check.items() if k != "commands"}
    result["command_receipts"] = []
    for index, command in enumerate(check.get("commands", [])):
        name = f"{label}.command-{index:02d}.json"
        result["command_receipts"].append({"path": name, "sha256": write_json(output / name, command)})
    return result


def inputs_identity():
    rows = []
    for relative in ["agent/" + name for name in RUNTIME_FILES] + [
            "runnable_adapter.py", "runnable_v3_provenance.py"]:
        data = provenance.read_regular(SUITE / relative, bounds.MAX_SOURCE_FILE_BYTES)
        rows.append({"path": PREFIX + relative, "sha256": digest(data)})
    return rows


def snapshot():
    accounting.report()  # Reuse the original no-follow/pinned 81-cell owner.
    manifest, sources = provenance.source_snapshot()
    frozen_bytes = accounting._snapshot()
    frozen_input = strict_json(frozen_bytes["frozen_protocol"])
    frozen = protocol.build_plan(frozen_input, frozen_bytes["frozen_tasks"])
    return manifest, sources, frozen_bytes, frozen_input, frozen


def prepare(compiler, address, model, operator):
    if not operator or operator != operator.strip():
        raise LocalTransportError("named_operator_required")
    manifest, sources, frozen_bytes, original, frozen = snapshot()
    compiler = pathlib.Path(compiler)
    compiler_digest = digest(provenance.read_regular(compiler, bounds.MAX_TOOL_BYTES))
    if compiler != compiler.resolve() or not os.access(compiler, os.X_OK):
        raise LocalTransportError("explicit_executable_compiler_required")
    client = Client(address)
    pin = client.inspect(model)
    proposal = json.loads(json.dumps(original))
    proposal["id"] = "semaprax-specialization-local-control-v1"
    proposal["base_model"] = {"provider": "ollama-local", "model": pin.name, "revision": pin.sha256}
    proposal["acceptance_oracle"] = {"runner": PREFIX + "run.py", "revision": provenance.SOURCE_COMMIT,
                                     "sha256": digest(sources[PREFIX + "run.py"])}
    action_digest = digest(canonical({"operation": "write_candidate_files", "candidate_paths": CANDIDATES}))
    proposal["tool_configuration"] = {"runner": PREFIX + "agent/specialization_local.py",
                                        "revision": "local-control-v1", "action_api_sha256": action_digest}
    for variant in proposal["variants"]:
        variant["model"] = proposal["base_model"].copy()
        if variant["id"] != "base":
            variant["guidance"]["sha256"] = digest(GUIDANCE.encode())
        if variant["id"] == "constrained":
            variant["action_constraint"]["sha256"] = action_digest
    local_plan = protocol.build_plan(proposal, frozen_bytes["frozen_tasks"])
    # All original cell keys and their declaration order are preserved.
    key = accounting._cell_key
    if [key(x) for x in local_plan["rows"]] != [key(x) for x in frozen["rows"]]:
        raise LocalTransportError("frozen_schedule_changed")
    prompts = []
    seen_prompts = set()
    for row in frozen["rows"]:
        if row["task"] not in UNAVAILABLE and (row["variant"], row["task"]) not in seen_prompts:
            seen_prompts.add((row["variant"], row["task"]))
            prompt, projection = model_prompt(row["task"], row["variant"], sources)
            prompts.append({"variant": row["variant"], "task": row["task"], "sha256": digest(prompt.encode()), "projection": projection})
    return {"schema": PLAN_SCHEMA, "status": "pending_independent_preflight_review",
            "operator": operator, "implementation_author": "implementation-assistant",
            "execution": "not_attempted", "issue_closable": False,
            "endpoint": address, "model_pin": pin.to_dict(), "model_observation": client.last_inspection,
            "model_trust": "operator-provisioned local daemon; manifest digest is not a hash of every weight byte",
            "compiler": {"path": str(compiler), "sha256": compiler_digest},
            "source_manifest_sha256": "sha256:" + provenance.SOURCE_HASH,
            "runtime_files": inputs_identity(), "python_version": sys.version,
            "projection_sha256": projection_identity(), "proposal": proposal, "plan": local_plan,
            "original_plan_sha256": accounting.FROZEN_PLAN_SHA256,
            "oracle_change": {"original_proposal": original["acceptance_oracle"],
                              "local_candidate": proposal["acceptance_oracle"],
                              "requires_independent_review": True},
            "sampling": {"temperature": 0.2, "top_p": 0.95, "max_output_tokens": 2048,
                         "seed": "repeat number (1, 2, 3), identical across controls"},
            "excluded_tasks": UNAVAILABLE, "prompts": prompts,
            "adaptation": "none; no weight training, tuning, tools or additional adapted arm",
            "cost_scope": "zero incremental provider charges; electricity/hardware/operator costs are not measured",
            "host_scope": "existing Darwin arm64 / macOS 26.5.1 build 25F80; no fallback",
            "runtime_limits": {"http_timeout_seconds": 120, "scorer_timeout_seconds": 120,
                               "context_tokens": CONTEXT_TOKENS, "automatic_retries": 0, "parallel_model_calls": 1}}


def review_template(plan_digest, operator):
    return {"schema": REVIEW_SCHEMA, "phase": "preflight", "subject_sha256": plan_digest,
            "decision": "pending", "reviewer": None, "operator": operator,
            "data_custodian": None, "reviewed_at": None, "independence_statement": None,
            "checks": {name: {"passed": False, "evidence": None} for name in PRECHECKS},
            "notes": "Complete only after actual independent review. A generated template is not an approval."}


def validate_review(review, plan_hash, operator):
    expected = set(review_template(plan_hash, operator))
    if (set(review) != expected or review["schema"] != REVIEW_SCHEMA or
            review["phase"] != "preflight" or review["decision"] != "approved" or
            review["subject_sha256"] != plan_hash or review["operator"] != operator):
        raise LocalTransportError("independent_preflight_approval_required")
    for field in ("reviewer", "data_custodian", "reviewed_at", "independence_statement", "notes"):
        if not isinstance(review[field], str) or not review[field].strip():
            raise LocalTransportError("incomplete_review:" + field)
    if review["reviewer"].strip().casefold() in {operator.strip().casefold(), "implementation-assistant"}:
        raise LocalTransportError("reviewer_not_independent_of_operator_or_author")
    if not isinstance(review["checks"], dict) or set(review["checks"]) != set(PRECHECKS):
        raise LocalTransportError("incomplete_preflight_checks")
    for check in review["checks"].values():
        if (not isinstance(check, dict) or set(check) != {"passed", "evidence"} or
                check["passed"] is not True or not isinstance(check["evidence"], str) or
                not check["evidence"].strip()):
            raise LocalTransportError("unsubstantiated_preflight_check")
    # The operator provides an independently obtained expected review digest
    # on the command line. These are human records, not cryptographic proof of
    # identity or automatically manufactured independence.


def cell_identity(row):
    # The frozen plan's execution=not_attempted is historical planning state,
    # not an observed result. Only stable cell identifiers enter result rows.
    return {name: row[name] for name in ("variant", "task", "language", "split", "trial")}


def not_run(row, reason):
    return {"cell": cell_identity(row), "status": "unexecuted", "reason": reason,
            "generation_dispatched": False, "actual_model_response": False,
            "metrics": {name: None for name in protocol.REQUIRED_METRICS},
            "trial_wall_ms": None}


def compare(rows):
    if len(rows) != 81 or len({accounting._cell_key(r["cell"]) for r in rows}) != 81:
        raise LocalTransportError("comparison_requires_all_81_unique_cells")
    by_key = {(x["cell"]["variant"], x["cell"]["task"], x["cell"]["trial"]): x for x in rows}
    summaries, comparisons = [], []
    for variant in protocol.REQUIRED_VARIANTS:
        group = [r for r in rows if r["cell"]["variant"] == variant]
        known = [r for r in group if type(r["metrics"]["accepted_outcome"]) is bool]
        success = sum(r["metrics"]["accepted_outcome"] for r in known)
        summaries.append({"variant": variant, "planned_cells": 27, "known_correctness": len(known),
                          "missing_correctness": 27 - len(known), "accepted": success,
                          "observed_acceptance_rate": success / len(known) if known else None,
                          "planned_denominator_bounds": [success / 27, (success + 27 - len(known)) / 27]})
    for left, right in (("guided", "base"), ("constrained", "base"), ("constrained", "guided")):
        diffs, included, missing = [], [], []
        # Repeats are clustered by task, not falsely treated as 27 independent
        # sampled tasks. The interval is descriptive for this fixed small set.
        for task in accounting.FROZEN_TASK_IDS:
            pairs = []
            for repeat in range(1, 4):
                a = by_key[(left, task, repeat)]["metrics"]["accepted_outcome"]
                b = by_key[(right, task, repeat)]["metrics"]["accepted_outcome"]
                if type(a) is bool and type(b) is bool:
                    pairs.append(int(a) - int(b))
            if len(pairs) == 3:
                included.append(task)
                diffs.append(sum(pairs) / 3)
            else:
                missing.append(task)
        interval = None
        if len(diffs) > 1:
            rng = random.Random(326)
            values = sorted(sum(rng.choice(diffs) for _ in diffs) / len(diffs) for _ in range(2000))
            interval = [values[49], values[1949]]
        comparisons.append({"left": left, "right": right, "complete_task_clusters": included,
                            "excluded_incomplete_task_clusters": missing,
                            "mean_paired_acceptance_difference": sum(diffs) / len(diffs) if diffs else None,
                            "descriptive_cluster_bootstrap_95_interval": interval,
                            "uncertainty_note": "2000 task-cluster resamples, seed 326; small fixed corpus, not a population-performance or equivalence claim."})
    return {"controls": summaries, "comparisons": comparisons}


def verify_plan(plan):
    if plan.get("schema") != PLAN_SCHEMA or plan.get("status") != "pending_independent_preflight_review":
        raise LocalTransportError("invalid_prepared_plan")
    manifest, sources, frozen_bytes, original, frozen = snapshot()
    if (plan.get("runtime_files") != inputs_identity() or plan.get("python_version") != sys.version or
            plan.get("projection_sha256") != projection_identity() or
            plan.get("source_manifest_sha256") != "sha256:" + provenance.SOURCE_HASH or
            plan.get("original_plan_sha256") != accounting.FROZEN_PLAN_SHA256 or
            plan.get("excluded_tasks") != UNAVAILABLE):
        raise LocalTransportError("prepared_input_identity_drift")
    built = protocol.build_plan(plan["proposal"], frozen_bytes["frozen_tasks"])
    if built != plan["plan"] or [accounting._cell_key(r) for r in built["rows"]] != [accounting._cell_key(r) for r in frozen["rows"]]:
        raise LocalTransportError("prepared_schedule_or_proposal_drift")
    # Check the whole generated prepare document, not only selected fields.
    # This performs metadata probes only, never a generation request.
    current = prepare(plan["compiler"]["path"], plan["endpoint"], plan["model_pin"]["name"], plan["operator"])
    if current != plan:
        raise LocalTransportError("prepared_plan_or_runtime_drift")
    return sources, built


def measured_metrics(response, transport):
    return {"accepted_outcome": None,
            "model_input_tokens": response.usage.prompt_tokens,
            "model_output_tokens": response.usage.completion_tokens,
            "presented_context_bytes": len(transport.observation["request"]["prompt"].encode()),
            "tool_calls": 0, "tool_request_bytes": 0, "tool_response_bytes": 0,
            "failed_attempts": None, "stale_failures": 0, "stale_recovery_actions": 0,
            "validation_wall_ms": None, "review_wall_ms": None, "human_interventions": 0,
            "total_cost_usd": 0.0}


def execute(plan_path, expected_plan, review_path, expected_review, output):
    plan, raw = read_json(plan_path)
    if digest(raw) != expected_plan or raw != canonical(plan):
        raise LocalTransportError("plan_digest_or_encoding_mismatch")
    review, review_raw = read_json(review_path)
    if digest(review_raw) != expected_review:
        raise LocalTransportError("independent_review_digest_mismatch")
    validate_review(review, expected_plan, plan["operator"])
    sources, built = verify_plan(plan)
    output = pathlib.Path(output)
    if (not output.is_absolute() or output.parent != output.parent.resolve() or
            not output.parent.is_dir()):
        raise LocalTransportError("explicit_private_output_parent_required")
    with held_directory(output.parent) as parent_fd:
        os.mkdir(output.name, mode=0o700, dir_fd=parent_fd)  # No overwrite or silent resume.
    write_json(output / "plan.json", plan)
    write_json(output / "preflight-review.json", review)
    rows, receipt_index = [], []
    halted = None
    client = Client(plan["endpoint"])
    pin = ModelPin(**plan["model_pin"])
    scorer = None
    reference_checks = []
    try:
        scorer = NativeScorer(sources, pathlib.Path(plan["compiler"]["path"]), plan["compiler"]["sha256"])
        scorer.__enter__()
        # Genuine reference/scorer preflight, not model trials. Every admitted
        # task must pass public AND hidden before any generation is attempted.
        for task in accounting.FROZEN_TASK_IDS:
            if task in UNAVAILABLE:
                continue
            public = public_tree(sources, task)
            check = scorer.score(task, {name: public[name].decode() for name in CANDIDATES[task]})
            retained = save_scoring(output, "reference-" + task, check)
            name = "reference-" + task + ".json"
            reference_checks.append({"task": task, "path": name,
                                     "sha256": write_json(output / name, {"is_model_result": False, **retained})})
            if check["result"].get("status") != "ok":
                raise LocalTransportError("reference_scorer_preflight_failed:" + task)
        write_json(output / "reference-checks.json", {"is_model_result": False, "checks": reference_checks})
    except (OSError, ValueError, KeyboardInterrupt) as error:
        halted = "preflight_unavailable:" + (str(error) or type(error).__name__)
    try:
        for index, row in enumerate(built["rows"]):
            if row["task"] in UNAVAILABLE or halted:
                result = not_run(row, UNAVAILABLE.get(row["task"], halted))
            else:
                start = time.monotonic()
                prompt, _ = model_prompt(row["task"], row["variant"], sources)
                request = SolverRequest(row["task"], "semaprax-project", prompt,
                    ModelIdentity("ollama-local", pin.name, pin.sha256),
                    SamplingParams(0.2, 0.95, row["trial"], 2048),
                    Budget(**built["resource_policy"]), PricingRates(0, 0))
                transport = LocalOllamaTransport(client, pin, CANDIDATES[row["task"]], row["variant"] == "constrained")
                write_json(output / f"cell-{index:03d}.intent.json", {"cell": cell_identity(row),
                           "kind": "generation_intent_not_execution_receipt", "prompt_sha256": digest(prompt.encode())})
                response = None
                evidence_refs = []
                try:
                    response = transport.complete(request)
                    # Persist actual prompt/reply before scoring. A scoring
                    # exception cannot erase real model output or token usage.
                    for suffix, value in (("request", transport.observation["request"]),
                                          ("response", transport.observation["response"])):
                        name = f"cell-{index:03d}.{suffix}.json"
                        evidence_refs.append({"path": name, "sha256": write_json(output / name, value)})
                    correctness = False
                    scoring = None
                    validation_ms = 0.0
                    if response.candidate_files:
                        began = time.monotonic()
                        scoring = save_scoring(output, f"cell-{index:03d}",
                                               scorer.score(row["task"], response.candidate_files))
                        validation_ms = (time.monotonic() - began) * 1000
                        correctness = scoring["result"].get("status") == "ok"
                    metrics = measured_metrics(response, transport)
                    metrics.update(accepted_outcome=correctness, failed_attempts=int(not correctness),
                                   validation_wall_ms=validation_ms)
                    result = {"cell": cell_identity(row), "status": "accepted" if correctness else "failed",
                              "generation_dispatched": True, "actual_model_response": True,
                              "reason": None if response.candidate_files else "invalid_or_truncated_model_candidate",
                              "metrics": metrics, "trial_wall_ms": (time.monotonic() - start) * 1000,
                              "generation_wall_ms": transport.observation["wall_ms"],
                              "scoring": scoring, "evidence": evidence_refs}
                except (ValueError, OSError, KeyboardInterrupt) as error:
                    # Unknown terminal usage stays null. One ambiguous attempt
                    # stops all subsequent calls; an interrupt never retries.
                    dispatched = bool(getattr(error, "dispatched", False) or
                                      client.last_generation_started or transport.observation)
                    reason = str(error) or type(error).__name__
                    result = not_run(row, reason)
                    result.update(status="attempt_outcome_unavailable" if dispatched else "unexecuted",
                                  generation_dispatched=dispatched, actual_model_response=response is not None,
                                  trial_wall_ms=(time.monotonic() - start) * 1000, evidence=evidence_refs)
                    if response is not None:
                        result["metrics"] = measured_metrics(response, transport)
                    elif transport.observation is not None:
                        # Integrity-failed daemon output remains unverified;
                        # do not turn a model-digest mismatch into a model score.
                        for suffix in ("request", "response"):
                            name = f"cell-{index:03d}.unverified-{suffix}.json"
                            evidence_refs.append({"path": name, "sha256": write_json(
                                output / name, transport.observation[suffix])})
                    halted = "study_stopped_after_ambiguous_or_authority_failure:" + reason
            rows.append(result)
            name = f"cell-{index:03d}.json"
            receipt_index.append({"path": name, "sha256": write_json(output / name, result)})
    finally:
        if scorer is not None:
            scorer.close()
    report = {"schema": RUN_SCHEMA, "plan_sha256": expected_plan, "review_sha256": expected_review,
              "preflight_review": "operator-supplied independent review; not identity-authenticated by this program",
              "issue_closable": False, "post_run_independent_review": "not_recorded",
              "source_manifest_sha256": plan["source_manifest_sha256"],
              "model_pin": plan["model_pin"], "row_files": receipt_index,
              "reference_checks": reference_checks,
              "counts": {"planned_cells": 81, "generation_dispatched": sum(r["generation_dispatched"] for r in rows),
                         "actual_model_responses": sum(r["actual_model_response"] for r in rows),
                         "unexecuted_cells": sum(r["status"] == "unexecuted" for r in rows)},
              "halted": halted, "cost_scope": plan["cost_scope"], "summary": compare(rows)}
    summary_hash = write_json(output / "summary.json", report)
    write_json(output / "post-run-review-template.json", {
        "schema": REVIEW_SCHEMA, "phase": "post_run", "subject_sha256": summary_hash,
        "decision": "pending", "reviewer": None, "reviewed_at": None,
        "leakage_review": None, "control_comparison_review": None,
        "data_custody_evidence": None, "issue_closable": False})
    return report


def audit(directory, expected_summary):
    """Replay receipt integrity, public prompts and denominators without a model.

    The expected summary digest must come from the operator's original run.
    Hashes are not signatures or proof that an independent reviewer approved,
    or that a claimed process really executed. This is input for that review.
    """
    directory = pathlib.Path(directory)
    report, raw = read_json(directory / "summary.json")
    if digest(raw) != expected_summary or report.get("schema") != RUN_SCHEMA:
        raise LocalTransportError("summary_identity_mismatch")
    plan, plan_raw = read_json(directory / "plan.json")
    review, review_raw = read_json(directory / "preflight-review.json")
    if digest(plan_raw) != report["plan_sha256"] or digest(review_raw) != report["review_sha256"]:
        raise LocalTransportError("plan_or_review_receipt_drift")
    validate_review(review, report["plan_sha256"], plan["operator"])
    _, sources, frozen_bytes, _, frozen = snapshot()
    if (plan["runtime_files"] != inputs_identity() or
            plan["source_manifest_sha256"] != "sha256:" + provenance.SOURCE_HASH or
            protocol.build_plan(plan["proposal"], frozen_bytes["frozen_tasks"]) != plan["plan"] or
            [accounting._cell_key(x) for x in plan["plan"]["rows"]] !=
            [accounting._cell_key(x) for x in frozen["rows"]]):
        raise LocalTransportError("audit_source_or_schedule_drift")
    checked = set()
    def receipt(ref, pattern):
        if (set(ref) != {"path", "sha256"} or not isinstance(ref["path"], str) or
                not re.fullmatch(pattern, ref["path"])):
            raise LocalTransportError("unbound_receipt_path")
        value, data = read_json(directory / ref["path"])
        if digest(data) != ref["sha256"]:
            raise LocalTransportError("receipt_digest_mismatch:" + ref["path"])
        checked.add(ref["path"])
        return value
    if len(report["row_files"]) != 81:
        raise LocalTransportError("audit_requires_81_rows")
    rows, request_count = [], 0
    for index, (planned, ref) in enumerate(zip(plan["plan"]["rows"], report["row_files"])):
        label = f"cell-{index:03d}"
        row = receipt(ref, re.escape(label + ".json"))
        if (row["cell"] != cell_identity(planned) or set(row["metrics"]) != set(protocol.REQUIRED_METRICS) or
                type(row["generation_dispatched"]) is not bool or type(row["actual_model_response"]) is not bool):
            raise LocalTransportError("audit_cell_inventory_drift")
        if row["status"] == "unexecuted":
            if row["generation_dispatched"] or row["actual_model_response"] or any(v is not None for v in row["metrics"].values()):
                raise LocalTransportError("unexecuted_cell_contains_model_result")
        elif row["status"] in {"accepted", "failed"}:
            if (not row["generation_dispatched"] or not row["actual_model_response"] or
                    row["metrics"]["accepted_outcome"] is not (row["status"] == "accepted") or
                    [r.get("path") for r in row.get("evidence", [])] !=
                    [label + ".request.json", label + ".response.json"]):
                raise LocalTransportError("model_outcome_without_matching_evidence")
        elif row["status"] != "attempt_outcome_unavailable":
            raise LocalTransportError("unknown_cell_status")
        elif row["metrics"]["accepted_outcome"] is not None:
            raise LocalTransportError("ambiguous_attempt_has_correctness_claim")
        if planned["task"] in UNAVAILABLE and row["status"] != "unexecuted":
            raise LocalTransportError("excluded_oracle_task_was_executed")
        for evidence in row.get("evidence", []):
            value = receipt(evidence, re.escape(label) + r"\.(?:unverified-)?(?:request|response)\.json")
            if evidence["path"].endswith("request.json"):
                request_count += 1
                text, _ = model_prompt(planned["task"], planned["variant"], sources)
                runner = LocalOllamaTransport(None, ModelPin(**plan["model_pin"]),
                                              CANDIDATES[planned["task"]], planned["variant"] == "constrained")
                expected = {"model": plan["model_pin"]["name"], "prompt": raw_prompt(text),
                            "raw": True, "stream": False, "keep_alive": 0,
                            "options": {"temperature": 0.2, "top_p": 0.95, "seed": planned["trial"],
                                        "num_predict": 2048, "num_ctx": CONTEXT_TOKENS}}
                if planned["variant"] == "constrained": expected["format"] = runner.response_schema
                if value != expected:
                    raise LocalTransportError("model_request_or_public_projection_drift")
            elif row["status"] in {"accepted", "failed"}:
                if (type(value.get("prompt_eval_count")) is not int or type(value.get("eval_count")) is not int or
                        value["prompt_eval_count"] != row["metrics"]["model_input_tokens"] or
                        value["eval_count"] != row["metrics"]["model_output_tokens"] or
                        not 0 < value["prompt_eval_count"] <= len(raw_prompt(model_prompt(planned["task"], planned["variant"], sources)[0]).encode()) + 32 or
                        not 0 <= value["eval_count"] <= 2048 or row["metrics"]["total_cost_usd"] != 0 or
                        value.get("model") != plan["model_pin"]["name"] or value.get("done") is not True):
                    raise LocalTransportError("reported_usage_or_model_disagrees_with_reply")
                if row["status"] == "accepted":
                    try:
                        answer = strict_json(value["response"].encode())
                        candidate_valid = (set(answer) == {"files"} and isinstance(answer["files"], dict) and
                                           set(answer["files"]) == set(CANDIDATES[planned["task"]]) and
                                           all(isinstance(v, str) for v in answer["files"].values()))
                    except (KeyError, AttributeError, TypeError, ValueError):
                        candidate_valid = False
                    score = (row.get("scoring") or {}).get("result", {})
                    if (not candidate_valid or value.get("done_reason") != "stop" or
                            score.get("status") != "ok" or score.get("leak_check") != "ok" or
                            score.get("public", {}).get("passed") is not True or
                            score.get("hidden", {}).get("passed") is not True):
                        raise LocalTransportError("accepted_result_without_complete_candidate_and_scorer")
        if row.get("scoring"):
            for ref in row["scoring"]["command_receipts"]:
                receipt(ref, re.escape(label) + r"\.command-[0-9]{2}\.json")
        rows.append(row)
    for ref in report["reference_checks"]:
        if ref["task"] not in accounting.FROZEN_TASK_IDS or ref["task"] in UNAVAILABLE:
            raise LocalTransportError("unbound_reference_task")
        label = "reference-" + ref["task"]
        value = receipt({"path": ref["path"], "sha256": ref["sha256"]}, re.escape(label + ".json"))
        if value.get("is_model_result") is not False:
            raise LocalTransportError("reference_claimed_as_model_result")
        for ref in value["command_receipts"]:
            receipt(ref, re.escape(label) + r"\.command-[0-9]{2}\.json")
    counts = {"planned_cells": 81, "generation_dispatched": sum(r["generation_dispatched"] for r in rows),
              "actual_model_responses": sum(r["actual_model_response"] for r in rows),
              "unexecuted_cells": sum(r["status"] == "unexecuted" for r in rows)}
    if counts != report["counts"] or compare(rows) != report["summary"]:
        raise LocalTransportError("summary_denominator_or_comparison_drift")
    return {"schema": "benchmark.cross_language.agent.local_specialization_receipt_audit.v1",
            "summary_sha256": expected_summary, "receipt_integrity": "checked", "counts": counts,
            "authenticated_receipt_files": len(checked), "public_projection_requests_checked": request_count,
            "model_calls_made_by_audit": 0, "actual_execution_independently_attested": False,
            "independent_leakage_and_control_review": "still_required", "issue_closable": False}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prep = commands.add_parser("prepare", help="freeze metadata and emit an unapproved review template; no inference")
    prep.add_argument("--compiler", required=True, type=pathlib.Path)
    prep.add_argument("--endpoint", default="http://127.0.0.1:11434")
    prep.add_argument("--model", default="qwen2.5-coder:7b")
    prep.add_argument("--operator", required=True)
    prep.add_argument("--output", required=True, type=pathlib.Path)
    run = commands.add_parser("run", help="execute only the independently reviewed exact plan")
    run.add_argument("--plan", required=True, type=pathlib.Path)
    run.add_argument("--plan-sha256", required=True)
    run.add_argument("--review", required=True, type=pathlib.Path)
    run.add_argument("--review-sha256", required=True)
    run.add_argument("--output", required=True, type=pathlib.Path)
    check = commands.add_parser("audit", help="replay receipt hashes, public prompts and summaries offline; not an independent review")
    check.add_argument("--run", required=True, type=pathlib.Path)
    check.add_argument("--summary-sha256", required=True)
    check.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        if args.command == "prepare":
            plan = prepare(args.compiler, args.endpoint, args.model, args.operator)
            hash_value = write_json(args.output, plan)
            review_path = args.output.with_name(args.output.stem + ".review-template.json")
            write_json(review_path, review_template(hash_value, args.operator))
            print(json.dumps({"plan_sha256": hash_value, "review_template": str(review_path),
                              "execution": "not_attempted", "issue_closable": False}))
            return 0
        if args.command == "audit":
            report = audit(args.run, args.summary_sha256)
            write_json(args.output, report)
            print(json.dumps(report))
            return 0
        report = execute(args.plan, args.plan_sha256, args.review, args.review_sha256, args.output)
        print(json.dumps({"output": str(args.output), "summary_sha256": digest(canonical(report)),
                          "counts": report["counts"],
                          "issue_closable": False, "post_run_independent_review": "not_recorded"}))
        return 3 if report["halted"] else 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(json.dumps({"status": "refused", "reason": str(error), "issue_closable": False}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
