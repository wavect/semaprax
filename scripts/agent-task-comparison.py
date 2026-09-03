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
PLAN_SCHEMA = "semaprax.agent-task-comparison-plan.v1"
REPORT_SCHEMA = "semaprax.agent-task-comparison-report.v1"
MAX_JSON = 1024 * 1024
MAX_EVIDENCE = 32 * 1024 * 1024
MAX_EVIDENCE_TOTAL = 64 * 1024 * 1024
MAX_ARTIFACTS = 64
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


def make_plan(manifest_path):
    manifest, body, tasks = load_manifest(manifest_path)
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
        "repository_head": head(),
        "repetitions": manifest["repetitions"],
        "pairing": manifest["pairing"],
        "tasks": tasks,
        "lanes": lanes,
        "required_metrics": list(METRICS),
        "claims": {
            "comparative_result": "not_observed",
            "productivity": "not_claimed",
            "zero": "external_unrun",
        },
    }
    return plan


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
    parser.add_argument("command", choices=("plan", "report"))
    parser.add_argument("--manifest", default="benchmarks/agent-task-comparison-v1/manifest.json")
    parser.add_argument("--observation", action="append", default=[])
    parser.add_argument("--output")
    arguments = parser.parse_args()
    if arguments.command == "plan":
        if arguments.observation:
            raise Failure("plan does not accept observations")
        value = make_plan(arguments.manifest)
    else:
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
