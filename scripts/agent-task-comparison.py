#!/usr/bin/env python3
"""Validate and summarize evidence-backed agent task comparison observations."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parent.parent
MANIFEST_SCHEMA = "semaprax.agent-task-comparison-manifest.v1"
TASK_SCHEMA = "semaprax.agent-task-comparison-task.v1"
OBSERVATION_SCHEMA = "semaprax.agent-task-comparison-observation.v1"
LEDGER_SCHEMA = "semaprax.agent-task-comparison-ledger.v1"
PLAN_SCHEMA = "semaprax.agent-task-comparison-plan.v1"
TRIAL_SCHEMA = "semaprax.agent-task-comparison-trial.v1"
MATRIX_SCHEMA = "semaprax.agent-task-comparison-matrix.v1"
REPORT_SCHEMA = "semaprax.agent-task-comparison-report.v1"
MAX_JSON = 1024 * 1024
MAX_EVIDENCE = 32 * 1024 * 1024
MAX_EVIDENCE_TOTAL = 64 * 1024 * 1024
MAX_ARTIFACTS = 64
MAX_LEDGER = 8 * 1024 * 1024
MAX_LEDGER_EVENTS = 65536
LEDGER_ARTIFACT_ID = "typed-event-ledger"
METRICS = (
    "model_input_tokens",
    "model_output_tokens",
    "presented_context_bytes",
    "tool_calls",
    "tool_request_bytes",
    "tool_response_bytes",
    "failed_attempts",
    "stale_failures",
    "stale_recovery_actions",
    "validation_wall_ms",
    "review_wall_ms",
    "human_interventions",
)


class Failure(Exception):
    pass


def canonical(value):
    return json.dumps(
        value, ensure_ascii=False, allow_nan=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def digest(body):
    return "sha256:" + hashlib.sha256(body).hexdigest()


def relative_file(base, name, maximum, label):
    if not isinstance(name, str) or not name or "\\" in name:
        raise Failure(f"invalid {label} path")
    relative = Path(name)
    if relative.is_absolute() or any(part in ("", ".", "..") for part in relative.parts):
        raise Failure(f"invalid {label} path: {name}")
    try:
        resolved_base = base.resolve(strict=True)
    except OSError as error:
        raise Failure(f"cannot resolve {label} base") from error
    path = resolved_base
    for part in relative.parts:
        path = path / part
        try:
            component = path.lstat()
        except FileNotFoundError as error:
            raise Failure(f"missing {label}: {name}") from error
        if path.is_symlink():
            raise Failure(f"{label} path crosses a symlink: {name}")
    try:
        status = path.lstat()
    except FileNotFoundError as error:
        raise Failure(f"missing {label}: {name}") from error
    try:
        path.resolve(strict=True).relative_to(resolved_base)
    except (OSError, ValueError) as error:
        raise Failure(f"{label} escapes its base: {name}") from error
    if not path.is_file() or status.st_nlink != 1:
        raise Failure(f"{label} must be a single-link regular file: {name}")
    if status.st_size > maximum:
        raise Failure(f"{label} exceeds {maximum} bytes: {name}")
    return path.read_bytes()


def object_json(body, label):
    try:
        value = json.loads(body)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise Failure(f"invalid {label} JSON") from error
    if not isinstance(value, dict):
        raise Failure(f"{label} must be a JSON object")
    return value


def exact_keys(value, required, optional, label):
    if not isinstance(value, dict):
        raise Failure(f"{label} must be a JSON object")
    keys = set(value)
    missing = set(required) - keys
    unknown = keys - set(required) - set(optional)
    if missing or unknown:
        raise Failure(
            f"{label} keys disagree: missing={sorted(missing)}, unknown={sorted(unknown)}"
        )


def text(value, label):
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > 65536:
        raise Failure(f"invalid {label}")
    return value


def natural(value, label):
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise Failure(f"invalid nonnegative integer {label}")
    return value


def load_manifest(path):
    body = relative_file(ROOT, path, MAX_JSON, "manifest")
    value = object_json(body, "manifest")
    exact_keys(value, ("schema", "id", "repetitions", "tasks", "lanes", "pairing"), (), "manifest")
    if value["schema"] != MANIFEST_SCHEMA:
        raise Failure("unsupported manifest schema")
    text(value["id"], "manifest id")
    if natural(value["repetitions"], "repetitions") < 1:
        raise Failure("repetitions must be positive")
    tasks = value["tasks"]
    lanes = value["lanes"]
    if (
        not isinstance(tasks, list)
        or not tasks
        or any(not isinstance(item, str) for item in tasks)
        or len(tasks) != len(set(tasks))
    ):
        raise Failure("tasks must be a nonempty unique path array")
    if not isinstance(lanes, list) or not lanes:
        raise Failure("lanes must be nonempty")
    lane_ids = []
    for index, lane in enumerate(lanes):
        exact_keys(
            lane,
            ("id", "system", "mode", "subject", "availability", "instructions"),
            (),
            f"lane {index}",
        )
        lane_ids.append(text(lane["id"], "lane id"))
        if lane["availability"] not in ("available", "external_unrun"):
            raise Failure("lane availability must be available or external_unrun")
        for key in ("system", "mode", "subject", "instructions"):
            text(lane[key], f"lane {lane['id']} {key}")
    if len(lane_ids) != len(set(lane_ids)):
        raise Failure("lane ids must be unique")
    pairing = value["pairing"]
    exact_keys(pairing, ("same_model", "same_prompt", "same_task", "same_trial", "state"), (), "pairing")
    if any(pairing[key] is not True for key in ("same_model", "same_prompt", "same_task", "same_trial")):
        raise Failure("v1 requires same model, prompt, task, and trial")
    if pairing["state"] not in ("cold", "warm"):
        raise Failure("pairing state must be cold or warm")
    loaded_tasks = []
    task_ids = set()
    for name in tasks:
        task_body = relative_file(ROOT, name, MAX_JSON, "task")
        task = object_json(task_body, f"task {name}")
        exact_keys(
            task,
            (
                "schema", "id", "prompt", "setup", "fixture_files", "drift_injection",
                "drift_patch", "acceptance", "review_protocol",
            ),
            (),
            f"task {name}",
        )
        if task["schema"] != TASK_SCHEMA:
            raise Failure(f"unsupported task schema: {name}")
        task_id = text(task["id"], "task id")
        if task_id in task_ids:
            raise Failure(f"duplicate task id: {task_id}")
        task_ids.add(task_id)
        for key in ("prompt", "setup", "drift_injection", "review_protocol"):
            text(task[key], f"task {task_id} {key}")
        fixture_files = task["fixture_files"]
        if (
            not isinstance(fixture_files, list)
            or not fixture_files
            or any(not isinstance(item, str) for item in fixture_files)
            or len(fixture_files) != len(set(fixture_files))
        ):
            raise Failure(f"task {task_id} fixture files must be a nonempty unique array")
        fixture = []
        for fixture_name in fixture_files:
            fixture_body = relative_file(ROOT, fixture_name, MAX_JSON, "fixture file")
            fixture.append({"path": fixture_name, "bytes": len(fixture_body), "sha256": digest(fixture_body)})
        drift_patch = task["drift_patch"]
        if drift_patch is None:
            drift = None
        else:
            drift_body = relative_file(ROOT, drift_patch, MAX_JSON, "drift patch")
            drift = {"path": drift_patch, "bytes": len(drift_body), "sha256": digest(drift_body)}
        acceptance = task["acceptance"]
        if not isinstance(acceptance, list) or not acceptance:
            raise Failure(f"task {task_id} needs acceptance criteria")
        criterion_ids = []
        for criterion in acceptance:
            exact_keys(criterion, ("id", "requirement"), (), f"task {task_id} criterion")
            criterion_ids.append(text(criterion["id"], "criterion id"))
            text(criterion["requirement"], "criterion requirement")
        if len(criterion_ids) != len(set(criterion_ids)):
            raise Failure(f"task {task_id} criterion ids must be unique")
        loaded_tasks.append(
            {
                "id": task_id,
                "path": name,
                "sha256": digest(task_body),
                "prompt_sha256": digest(task["prompt"].encode()),
                "fixture": fixture,
                "drift_patch": drift,
                "task": task,
            }
        )
    return value, body, loaded_tasks


def head():
    completed = subprocess.run(
        ("git", "rev-parse", "HEAD"), cwd=ROOT, check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    if completed.returncode:
        raise Failure("cannot resolve repository HEAD")
    value = completed.stdout.decode("ascii", "strict").strip()
    if len(value) not in (40, 64) or any(character not in "0123456789abcdef" for character in value):
        raise Failure("repository HEAD is not a full object-format commit")
    return value


def make_plan_from_loaded(manifest_path, manifest, body, tasks, repository_head):
    lanes = []
    for lane in manifest["lanes"]:
        lanes.append(
            {
                "availability": lane["availability"],
                "id": lane["id"],
                "mode": lane["mode"],
                "observation_status": "not_observed",
                "subject": lane["subject"],
                "system": lane["system"],
            }
        )
    plan = {
        "schema": PLAN_SCHEMA,
        "manifest": {"path": manifest_path, "sha256": digest(body)},
        "repository_head": repository_head,
        "repetitions": manifest["repetitions"],
        "pairing": manifest["pairing"],
        "tasks": [
            {
                "id": task["id"],
                "path": task["path"],
                "sha256": task["sha256"],
                "prompt_sha256": task["prompt_sha256"],
                "fixture": task["fixture"],
                "drift_patch": task["drift_patch"],
            }
            for task in tasks
        ],
        "lanes": lanes,
        "required_metrics": list(METRICS),
        "claims": {
            "comparative_result": "not_observed",
            "productivity": "not_claimed",
            "zero": "external_unrun",
        },
    }
    return plan


def make_plan(manifest_path):
    manifest, body, tasks = load_manifest(manifest_path)
    return make_plan_from_loaded(manifest_path, manifest, body, tasks, head())


def trial_from_loaded(
    manifest_path, manifest, manifest_body, loaded_tasks, plan, task_id, lane_id, trial
):
    task_binding = next((item for item in loaded_tasks if item["id"] == task_id), None)
    lane = next((item for item in manifest["lanes"] if item["id"] == lane_id), None)
    if task_binding is None:
        raise Failure(f"unknown trial task: {task_id}")
    if lane is None:
        raise Failure(f"unknown trial lane: {lane_id}")
    if lane["availability"] != "available":
        raise Failure(f"trial lane is not available: {lane_id}")
    if trial < 1 or trial > manifest["repetitions"]:
        raise Failure("trial number is outside the manifest")
    task = task_binding["task"]
    resolved_subject = (
        plan["repository_head"]
        if lane["subject"] == "repository-head-resolved-by-plan"
        else lane["subject"]
    )
    return {
        "schema": TRIAL_SCHEMA,
        "plan_sha256": digest(canonical(plan)),
        "repository_head": plan["repository_head"],
        "manifest": {
            "id": manifest["id"],
            "path": manifest_path,
            "sha256": digest(manifest_body),
        },
        "task": {
            "id": task_id,
            "path": task_binding["path"],
            "sha256": task_binding["sha256"],
            "prompt": task["prompt"],
            "prompt_sha256": task_binding["prompt_sha256"],
            "setup": task["setup"],
            "fixture": task_binding["fixture"],
            "drift_injection": task["drift_injection"],
            "drift_patch": task_binding["drift_patch"],
            "acceptance": task["acceptance"],
            "review_protocol": task["review_protocol"],
        },
        "lane": {
            "id": lane_id,
            "system": lane["system"],
            "mode": lane["mode"],
            "declared_subject": lane["subject"],
            "resolved_subject": resolved_subject,
            "instructions": lane["instructions"],
        },
        "trial": trial,
        "state": manifest["pairing"]["state"],
        "required_metrics": list(METRICS),
        "required_observation_schema": OBSERVATION_SCHEMA,
        "claims": {
            "execution": "not_performed_by_trial_generation",
            "acceptance": "not_observed",
            "comparative_result": "not_observed",
            "publication_authority": False,
        },
    }


def make_trial(manifest_path, task_id, lane_id, trial):
    manifest, manifest_body, loaded_tasks = load_manifest(manifest_path)
    plan = make_plan_from_loaded(
        manifest_path, manifest, manifest_body, loaded_tasks, head()
    )
    return trial_from_loaded(
        manifest_path,
        manifest,
        manifest_body,
        loaded_tasks,
        plan,
        task_id,
        lane_id,
        trial,
    )


def make_matrix(manifest_path):
    manifest, manifest_body, loaded_tasks = load_manifest(manifest_path)
    plan = make_plan_from_loaded(
        manifest_path, manifest, manifest_body, loaded_tasks, head()
    )
    rows = []
    for task in loaded_tasks:
        for lane in manifest["lanes"]:
            if lane["availability"] != "available":
                continue
            for trial in range(1, manifest["repetitions"] + 1):
                contract = trial_from_loaded(
                    manifest_path,
                    manifest,
                    manifest_body,
                    loaded_tasks,
                    plan,
                    task["id"],
                    lane["id"],
                    trial,
                )
                rows.append(
                    {
                        "task": task["id"],
                        "lane": lane["id"],
                        "trial": trial,
                        "trial_sha256": digest(canonical(contract)),
                        "resolved_subject": contract["lane"]["resolved_subject"],
                    }
                )
    external = [
        {
            "lane": lane["id"],
            "subject": lane["subject"],
            "status": "external_unrun",
        }
        for lane in manifest["lanes"]
        if lane["availability"] == "external_unrun"
    ]
    return {
        "schema": MATRIX_SCHEMA,
        "plan": plan,
        "plan_sha256": digest(canonical(plan)),
        "repository_head": plan["repository_head"],
        "trial_contract_command": (
            "python3 scripts/agent-task-comparison.py trial --manifest <manifest> "
            "--task <task> --lane <lane> --trial <trial> --output <absolute-path>"
        ),
        "rows": rows,
        "row_count": len(rows),
        "external_lanes": external,
        "claims": {
            "execution": "not_performed_by_matrix_generation",
            "observations": "not_present",
            "comparative_result": "not_observed",
            "external_lanes": "not_observed",
            "publication_authority": False,
        },
    }


def make_observation(manifest_path, ledger_path, output):
    if output is None:
        raise Failure("observation derivation requires --output")
    destination = Path(output)
    if not destination.is_absolute():
        raise Failure("output path must be absolute")
    ledger_relative = Path(ledger_path)
    expected_parent = (ROOT / ledger_relative.parent).resolve()
    if destination.parent.resolve() != expected_parent or destination.name == ledger_relative.name:
        raise Failure("derived observation must be a distinct file beside its ledger")

    body = relative_file(ROOT, ledger_path, MAX_LEDGER, "ledger")
    value = object_json(body, "ledger")
    if body != canonical(value) + b"\n":
        raise Failure("ledger must use exact canonical JSON with one terminal LF")
    exact_keys(
        value,
        (
            "schema", "plan_sha256", "task", "lane", "trial", "state", "model",
            "tokenizer", "model_configuration", "harness", "host", "toolchain",
            "prompt_sha256", "artifacts", "streams", "events", "acceptance", "outcome",
        ),
        (),
        "ledger",
    )
    plan = make_plan(manifest_path)
    manifest, _, _ = load_manifest(manifest_path)
    if value["schema"] != LEDGER_SCHEMA or value["plan_sha256"] != digest(canonical(plan)):
        raise Failure("ledger does not bind this plan")
    task = next((item for item in plan["tasks"] if item["id"] == value["task"]), None)
    lane = next((item for item in manifest["lanes"] if item["id"] == value["lane"]), None)
    if task is None or lane is None or lane["availability"] != "available":
        raise Failure("ledger selects an unavailable task or lane")
    if value["state"] != manifest["pairing"]["state"]:
        raise Failure("ledger has the wrong workspace state")
    trial = natural(value["trial"], "trial")
    if trial < 1 or trial > manifest["repetitions"]:
        raise Failure("ledger trial is outside the manifest")
    for key in (
        "model", "tokenizer", "model_configuration", "harness", "host", "toolchain",
        "prompt_sha256", "outcome",
    ):
        text(value[key], f"ledger {key}")
    if value["prompt_sha256"] != task["prompt_sha256"]:
        raise Failure("ledger changes the common task prompt")
    if value["outcome"] not in ("completed", "failed", "aborted"):
        raise Failure("ledger has an invalid outcome")

    artifacts = value["artifacts"]
    if not isinstance(artifacts, list) or not artifacts or len(artifacts) >= MAX_ARTIFACTS:
        raise Failure("ledger must authenticate between 1 and 63 external artifacts")
    known = set()
    evidence_bytes = 0
    ledger_base = ledger_relative.parent
    for artifact in artifacts:
        exact_keys(artifact, ("id", "path", "bytes", "sha256", "kind"), (), "artifact")
        artifact_id = text(artifact["id"], "artifact id")
        if artifact_id == LEDGER_ARTIFACT_ID or artifact_id in known:
            raise Failure(f"duplicate or reserved artifact id: {artifact_id}")
        known.add(artifact_id)
        artifact_path = text(artifact["path"], "artifact path")
        artifact_body = relative_file(
            ROOT, str(ledger_base / artifact_path), MAX_EVIDENCE, "evidence artifact"
        )
        evidence_bytes += len(artifact_body)
        if evidence_bytes > MAX_EVIDENCE_TOTAL:
            raise Failure("ledger evidence exceeds the total byte bound")
        if natural(artifact["bytes"], "artifact bytes") != len(artifact_body):
            raise Failure(f"artifact byte binding disagrees: {artifact_id}")
        if artifact["sha256"] != digest(artifact_body):
            raise Failure(f"artifact digest binding disagrees: {artifact_id}")
        text(artifact["kind"], "artifact kind")

    stream_metrics = {
        "model_usage": ("model_input_tokens", "model_output_tokens"),
        "context_presentation": ("presented_context_bytes",),
        "tool_calls": ("tool_calls", "tool_request_bytes", "tool_response_bytes"),
        "failed_attempts": ("failed_attempts",),
        "stale_recovery": ("stale_failures", "stale_recovery_actions"),
        "validation": ("validation_wall_ms",),
        "review": ("review_wall_ms",),
        "human_interventions": ("human_interventions",),
    }
    streams = value["streams"]
    if not isinstance(streams, dict) or set(streams) != set(stream_metrics):
        raise Failure("ledger stream evidence inventory disagrees")

    def refs(selected, label):
        if (
            not isinstance(selected, list)
            or not selected
            or any(not isinstance(item, str) or item not in known for item in selected)
        ):
            raise Failure(f"{label} lacks authenticated evidence")
        return selected

    totals = {metric: 0 for metric in METRICS}
    metric_evidence = {metric: [LEDGER_ARTIFACT_ID] for metric in METRICS}
    for stream, metrics in stream_metrics.items():
        selected = refs(streams[stream], f"ledger stream {stream}")
        for metric in metrics:
            metric_evidence[metric].extend(selected)

    events = value["events"]
    if not isinstance(events, list) or len(events) > MAX_LEDGER_EVENTS:
        raise Failure("ledger events are absent or exceed their bound")
    event_shapes = {
        "model_usage": ("model_input_tokens", "model_output_tokens"),
        "context_presentation": ("presented_context_bytes",),
        "tool_call": ("tool_request_bytes", "tool_response_bytes"),
        "failed_attempt": (),
        "stale_failure": (),
        "stale_recovery_action": (),
        "validation_interval": ("validation_wall_ms",),
        "review_interval": ("review_wall_ms",),
        "human_intervention": (),
    }
    for index, event in enumerate(events):
        exact_keys(event, ("kind", "values", "evidence"), (), f"ledger event {index}")
        kind = event["kind"]
        if not isinstance(kind, str) or kind not in event_shapes:
            raise Failure(f"ledger event {index} has an unknown kind")
        values = event["values"]
        exact_keys(values, event_shapes[kind], (), f"ledger event {index} values")
        selected = refs(event["evidence"], f"ledger event {index}")
        for metric in event_shapes[kind]:
            amount = natural(values[metric], f"ledger event {index} {metric}")
            totals[metric] += amount
            metric_evidence[metric].extend(selected)
        if kind == "tool_call":
            totals["tool_calls"] += 1
            metric_evidence["tool_calls"].extend(selected)
        elif kind == "failed_attempt":
            totals["failed_attempts"] += 1
            metric_evidence["failed_attempts"].extend(selected)
        elif kind == "stale_failure":
            totals["stale_failures"] += 1
            metric_evidence["stale_failures"].extend(selected)
        elif kind == "stale_recovery_action":
            totals["stale_recovery_actions"] += 1
            metric_evidence["stale_recovery_actions"].extend(selected)
        elif kind == "human_intervention":
            totals["human_interventions"] += 1
            metric_evidence["human_interventions"].extend(selected)

    for metric in METRICS:
        metric_evidence[metric] = list(dict.fromkeys(metric_evidence[metric]))
    methods = {
        "model_input_tokens": "sum_typed_model_usage_events",
        "model_output_tokens": "sum_typed_model_usage_events",
        "presented_context_bytes": "sum_typed_context_presentation_events",
        "tool_calls": "count_typed_tool_call_events",
        "tool_request_bytes": "sum_typed_tool_call_events",
        "tool_response_bytes": "sum_typed_tool_call_events",
        "failed_attempts": "count_typed_failed_attempt_events",
        "stale_failures": "count_typed_stale_failure_events",
        "stale_recovery_actions": "count_typed_stale_recovery_action_events",
        "validation_wall_ms": "sum_typed_validation_interval_events",
        "review_wall_ms": "sum_typed_review_interval_events",
        "human_interventions": "count_typed_human_intervention_events",
    }
    metrics = {
        metric: {
            "status": "observed",
            "value": totals[metric],
            "method": methods[metric],
            "evidence": metric_evidence[metric],
        }
        for metric in METRICS
    }

    task_body = task["task"] if "task" in task else object_json(
        relative_file(ROOT, task["path"], MAX_JSON, "task"), "task"
    )
    expected = [criterion["id"] for criterion in task_body["acceptance"]]
    acceptance = value["acceptance"]
    if not isinstance(acceptance, list) or [row.get("id") for row in acceptance if isinstance(row, dict)] != expected:
        raise Failure("ledger acceptance inventory disagrees")
    derived_acceptance = []
    for row in acceptance:
        exact_keys(row, ("id", "outcome", "evidence"), (), "ledger acceptance row")
        if row["outcome"] not in ("passed", "failed"):
            raise Failure(f"ledger acceptance row {row['id']} has an invalid outcome")
        selected = refs(row["evidence"], f"ledger acceptance row {row['id']}")
        derived_acceptance.append(
            {"id": row["id"], "outcome": row["outcome"], "evidence": [LEDGER_ARTIFACT_ID, *selected]}
        )
    acceptance_passed = all(row["outcome"] == "passed" for row in derived_acceptance)
    if (value["outcome"] == "completed") != acceptance_passed:
        raise Failure("ledger outcome disagrees with acceptance")

    output_artifacts = list(artifacts)
    output_artifacts.append(
        {
            "id": LEDGER_ARTIFACT_ID,
            "path": ledger_relative.name,
            "bytes": len(body),
            "sha256": digest(body),
            "kind": "typed_event_ledger",
        }
    )
    return {
        "schema": OBSERVATION_SCHEMA,
        "plan_sha256": value["plan_sha256"],
        "task": value["task"],
        "lane": value["lane"],
        "trial": trial,
        "state": value["state"],
        "model": value["model"],
        "tokenizer": value["tokenizer"],
        "model_configuration": value["model_configuration"],
        "harness": value["harness"],
        "host": value["host"],
        "toolchain": value["toolchain"],
        "prompt_sha256": value["prompt_sha256"],
        "artifacts": output_artifacts,
        "metrics": metrics,
        "acceptance": derived_acceptance,
        "outcome": value["outcome"],
    }


def load_observation(path, plan, manifest):
    body = relative_file(ROOT, path, MAX_JSON, "observation")
    value = object_json(body, f"observation {path}")
    exact_keys(
        value,
        (
            "schema", "plan_sha256", "task", "lane", "trial", "state", "model", "tokenizer",
            "model_configuration", "harness", "host", "toolchain", "prompt_sha256", "artifacts",
            "metrics", "acceptance", "outcome",
        ),
        (),
        f"observation {path}",
    )
    if value["schema"] != OBSERVATION_SCHEMA or value["plan_sha256"] != digest(canonical(plan)):
        raise Failure(f"observation {path} does not bind this plan")
    task = next((item for item in plan["tasks"] if item["id"] == value["task"]), None)
    lane = next((item for item in manifest["lanes"] if item["id"] == value["lane"]), None)
    if task is None or lane is None or lane["availability"] != "available":
        raise Failure(f"observation {path} selects an unavailable task or lane")
    if value["state"] != manifest["pairing"]["state"]:
        raise Failure(f"observation {path} has the wrong workspace state")
    trial = natural(value["trial"], "trial")
    if trial < 1 or trial > manifest["repetitions"]:
        raise Failure(f"observation {path} trial is outside the manifest")
    for key in (
        "model", "tokenizer", "model_configuration", "harness", "host", "toolchain",
        "prompt_sha256", "outcome",
    ):
        text(value[key], f"observation {key}")
    if value["prompt_sha256"] != task["prompt_sha256"]:
        raise Failure(f"observation {path} changes the common task prompt")
    if value["outcome"] not in ("completed", "failed", "aborted"):
        raise Failure(f"observation {path} has an invalid outcome")
    artifacts = value["artifacts"]
    if not isinstance(artifacts, list) or not artifacts or len(artifacts) > MAX_ARTIFACTS:
        raise Failure(f"observation {path} must authenticate evidence artifacts")
    evidence = set()
    evidence_bytes = 0
    observation_base = Path(path).parent
    for artifact in artifacts:
        exact_keys(artifact, ("id", "path", "bytes", "sha256", "kind"), (), "artifact")
        artifact_id = text(artifact["id"], "artifact id")
        if artifact_id in evidence:
            raise Failure(f"duplicate artifact id: {artifact_id}")
        evidence.add(artifact_id)
        artifact_body = relative_file(ROOT, str(observation_base / artifact["path"]), MAX_EVIDENCE, "evidence artifact")
        evidence_bytes += len(artifact_body)
        if evidence_bytes > MAX_EVIDENCE_TOTAL:
            raise Failure(f"observation {path} evidence exceeds {MAX_EVIDENCE_TOTAL} bytes")
        if natural(artifact["bytes"], "artifact bytes") != len(artifact_body) or artifact["sha256"] != digest(artifact_body):
            raise Failure(f"artifact binding disagrees: {artifact_id}")
        text(artifact["kind"], "artifact kind")
    metrics = value["metrics"]
    if not isinstance(metrics, dict) or set(metrics) != set(METRICS):
        raise Failure(f"observation {path} metric inventory disagrees")
    totals = {}
    for metric in METRICS:
        item = metrics[metric]
        exact_keys(item, ("status", "value", "method", "evidence"), (), f"metric {metric}")
        if item["status"] != "observed":
            raise Failure(f"completed comparison observations must observe {metric}")
        totals[metric] = natural(item["value"], metric)
        text(item["method"], f"metric {metric} method")
        refs = item["evidence"]
        if (
            not isinstance(refs, list)
            or not refs
            or any(not isinstance(ref, str) or ref not in evidence for ref in refs)
        ):
            raise Failure(f"metric {metric} lacks authenticated evidence")
    acceptance = value["acceptance"]
    task_body = object_json(relative_file(ROOT, task["path"], MAX_JSON, "task"), "task")
    expected = [criterion["id"] for criterion in task_body["acceptance"]]
    if not isinstance(acceptance, list) or [row.get("id") for row in acceptance if isinstance(row, dict)] != expected:
        raise Failure(f"observation {path} acceptance inventory disagrees")
    for row in acceptance:
        exact_keys(row, ("id", "outcome", "evidence"), (), "acceptance row")
        if (
            row["outcome"] not in ("passed", "failed")
            or not isinstance(row["evidence"], list)
            or not row["evidence"]
            or any(not isinstance(ref, str) or ref not in evidence for ref in row["evidence"])
        ):
            raise Failure(f"acceptance row {row['id']} lacks an authenticated outcome")
    acceptance_passed = all(row["outcome"] == "passed" for row in acceptance)
    if (value["outcome"] == "completed") != acceptance_passed:
        raise Failure(f"observation {path} outcome disagrees with acceptance")
    return {
        "path": path,
        "sha256": digest(body),
        "task": value["task"],
        "lane": value["lane"],
        "trial": trial,
        "state": value["state"],
        "model": value["model"],
        "tokenizer": value["tokenizer"],
        "model_configuration": value["model_configuration"],
        "harness": value["harness"],
        "host": value["host"],
        "toolchain": value["toolchain"],
        "outcome": value["outcome"],
        "acceptance_passed": acceptance_passed,
        "metrics": totals,
    }


def make_report(manifest_path, observation_paths):
    plan = make_plan(manifest_path)
    manifest, _, _ = load_manifest(manifest_path)
    observations = [load_observation(path, plan, manifest) for path in observation_paths]
    keys = [(item["task"], item["lane"], item["trial"]) for item in observations]
    if len(keys) != len(set(keys)):
        raise Failure("duplicate task/lane/trial observation")
    available = [lane["id"] for lane in manifest["lanes"] if lane["availability"] == "available"]
    expected = {
        (task["id"], lane, trial)
        for task in plan["tasks"]
        for lane in available
        for trial in range(1, manifest["repetitions"] + 1)
    }
    if set(keys) != expected:
        missing = sorted(expected - set(keys))
        extra = sorted(set(keys) - expected)
        raise Failure(f"observation matrix is incomplete: missing={missing}, extra={extra}")
    for task in plan["tasks"]:
        for trial in range(1, manifest["repetitions"] + 1):
            pair = [item for item in observations if item["task"] == task["id"] and item["trial"] == trial]
            if len(
                {
                    (
                        item["model"], item["tokenizer"], item["model_configuration"],
                        item["harness"], item["host"], item["toolchain"], item["state"],
                    )
                    for item in pair
                }
            ) != 1:
                raise Failure(f"pairing disagrees for {task['id']} trial {trial}")
    external = [lane["id"] for lane in manifest["lanes"] if lane["availability"] == "external_unrun"]
    totals = {}
    for lane in available:
        selected = [item for item in observations if item["lane"] == lane]
        totals[lane] = {
            "completed": sum(item["outcome"] == "completed" for item in selected),
            "acceptance_passed": sum(item["acceptance_passed"] for item in selected),
            "metrics": {metric: sum(item["metrics"][metric] for item in selected) for metric in METRICS},
        }
    paired_differences = []
    if len(available) == 2:
        left, right = available
        for task in plan["tasks"]:
            for trial in range(1, manifest["repetitions"] + 1):
                left_row = next(item for item in observations if item["task"] == task["id"] and item["trial"] == trial and item["lane"] == left)
                right_row = next(item for item in observations if item["task"] == task["id"] and item["trial"] == trial and item["lane"] == right)
                paired_differences.append(
                    {
                        "task": task["id"],
                        "trial": trial,
                        "left": left,
                        "right": right,
                        "direction": "left_minus_right",
                        "metrics": {metric: left_row["metrics"][metric] - right_row["metrics"][metric] for metric in METRICS},
                    }
                )
    return {
        "schema": REPORT_SCHEMA,
        "plan_sha256": digest(canonical(plan)),
        "observations": sorted(observations, key=lambda item: (item["task"], item["trial"], item["lane"])),
        "lane_totals": totals,
        "paired_differences": paired_differences,
        "claims": {
            "available_lane_comparison": "observed_descriptive",
            "causal_productivity": "not_claimed",
            "statistical_significance": "not_claimed",
            "external_lanes": {lane: "not_observed" for lane in external},
        },
    }


def write(value, output):
    body = canonical(value) + b"\n"
    if output is None:
        sys.stdout.buffer.write(body)
    else:
        destination = Path(output)
        if not destination.is_absolute():
            raise Failure("output path must be absolute")
        destination.parent.mkdir(parents=True, exist_ok=True)
        temporary = destination.with_name(destination.name + ".tmp")
        temporary.write_bytes(body)
        temporary.replace(destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "matrix", "trial", "observation", "report"))
    parser.add_argument("--manifest", default="benchmarks/agent-task-comparison-v1/manifest.json")
    parser.add_argument("--observation", action="append", default=[])
    parser.add_argument("--task")
    parser.add_argument("--lane")
    parser.add_argument("--trial", type=int)
    parser.add_argument("--ledger")
    parser.add_argument("--output")
    arguments = parser.parse_args()
    if arguments.command == "plan":
        if arguments.observation or arguments.task or arguments.lane or arguments.trial is not None or arguments.ledger:
            raise Failure("plan does not accept observations or trial selectors")
        value = make_plan(arguments.manifest)
    elif arguments.command == "matrix":
        if arguments.observation or arguments.task or arguments.lane or arguments.trial is not None or arguments.ledger:
            raise Failure("matrix does not accept observations or trial selectors")
        value = make_matrix(arguments.manifest)
    elif arguments.command == "trial":
        if arguments.observation or arguments.ledger:
            raise Failure("trial does not accept observations")
        if arguments.task is None or arguments.lane is None or arguments.trial is None:
            raise Failure("trial requires --task, --lane, and --trial")
        value = make_trial(arguments.manifest, arguments.task, arguments.lane, arguments.trial)
    elif arguments.command == "observation":
        if arguments.observation or arguments.task or arguments.lane or arguments.trial is not None:
            raise Failure("observation does not accept observations or trial selectors")
        if arguments.ledger is None:
            raise Failure("observation requires --ledger")
        value = make_observation(arguments.manifest, arguments.ledger, arguments.output)
    else:
        if arguments.task or arguments.lane or arguments.trial is not None or arguments.ledger:
            raise Failure("report does not accept trial selectors")
        if not arguments.observation:
            raise Failure("report requires the complete observation matrix")
        value = make_report(arguments.manifest, arguments.observation)
    write(value, arguments.output)


if __name__ == "__main__":
    try:
        main()
    except Failure as error:
        print(f"agent task comparison rejected: {error}", file=sys.stderr)
        raise SystemExit(2)
