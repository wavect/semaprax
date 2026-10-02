"""Frozen, additive two-model laboratory pilot; no execution authority."""
from __future__ import annotations
import hashlib
import json
import pathlib
import re
import sys
import subprocess

SUITE = pathlib.Path(__file__).resolve().parents[1]
if str(SUITE) not in sys.path:
    sys.path.insert(0, str(SUITE))
import runnable_v3_provenance as provenance
import runnable_v3_corrections as corrections
import supported_scope

SCHEMA = "benchmark.cross_language.live_pilot.v1"
TASK = "structured-input-error-handling-v1"
CANDIDATE = "validate.ts"
PROFILE = "darwin-arm64-official-typescript-pilot.v1"
SYSTEM = ('Return only one JSON object mapping "validate.ts" to its complete TypeScript source string. '
          'Do not use tools, prose, or additional files. Implement the supplied equivalence contract. '
          'Export the numeric validate function. It runs with numeric arguments in an isolated context '
          'without Node globals, imports, timers, or generated code; return a safe integer.')


def canonical(value):
    return (json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True, allow_nan=False) + "\n").encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def strict_json(data):
    def pairs(rows):
        result = {}
        for key, value in rows:
            if key in result:
                raise ValueError("duplicate_json_key")
            result[key] = value
        return result
    def bad(_):
        raise ValueError("nonfinite_json_number")
    return json.loads(data, object_pairs_hook=pairs, parse_constant=bad)


def exact(value, keys, reason):
    if not isinstance(value, dict) or set(value) != set(keys):
        raise ValueError(reason)


def integer(value, maximum, reason):
    if type(value) is not int or not 0 < value <= maximum:
        raise ValueError(reason)


def source_inputs():
    manifest, baseline = provenance.source_snapshot()
    effective = corrections.admit(manifest, baseline)
    sources = effective["sources"]
    task = next(row for row in json.loads(sources["benchmarks/cross-language-v1/tasks.json"])["tasks"] if row["id"] == TASK)
    paths = task["languages"]["typescript"]
    public = {name[len(paths["public"]) + 1:]: data for name, data in sources.items() if name.startswith(paths["public"] + "/")}
    hidden = {name[len(paths["hidden"]) + 1:]: data for name, data in sources.items() if name.startswith(paths["hidden"] + "/")}
    admit_paths(public, hidden, (CANDIDATE,))
    contract = sources["benchmarks/cross-language-v1/" + task["equivalence"]].decode()
    prompt = "# " + TASK + " (typescript)\n\n" + contract + "\n## Fixed public scaffold\n"
    for name, data in sorted(public.items()):
        if name != CANDIDATE:
            prompt += "\n### " + name + "\n" + data.decode()
    prompt += "\n## Produce exactly this file\nvalidate.ts\n"
    return manifest, sources, task, public, hidden, prompt


def admit_paths(public, hidden, candidates):
    if not candidates or len(set(candidates)) != len(candidates):
        raise ValueError("candidate_paths_refused")
    for name in candidates:
        path = pathlib.PurePosixPath(name)
        if (not isinstance(name, str) or not name or path.is_absolute() or str(path) != name
                or any(part in (".", "..") or ":" in part for part in path.parts)
                or "\\" in name or "\x00" in name or name not in public):
            raise ValueError("candidate_paths_refused")
        if name in hidden:
            raise ValueError("candidate_hidden_overlay_collision")
    if not hidden:
        raise ValueError("hidden_overlay_empty")


def implementation_identity():
    names = ["agent/pilot_protocol.py", "agent/claude_subscription.py", "agent/pilot_score.py", "agent/pilot_run.py",
             "runnable_adapter.py", "runnable_adapter_v2.py", "runnable_adapter_v3.py", "supported_scope.py"]
    names += [path.name for path in sorted(SUITE.glob("runnable_v3_*.py"))]
    return {name: digest(provenance.read_regular(SUITE / name, 256 * 1024)) for name in names}


def runner_revision():
    result = subprocess.run(["/usr/bin/git", "-C", str(provenance.ROOT), "rev-parse", "HEAD"],
                            env={"PATH": "/usr/bin:/bin"}, capture_output=True, timeout=5, check=True)
    revision = result.stdout.decode().strip()
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("runner_revision_refused")
    return revision


def freeze(configuration):
    exact(configuration, ("models", "limits", "claude_sha256", "approval", "host_ids"), "pilot_configuration_shape")
    models = configuration["models"]
    if not isinstance(models, list) or len(models) != 2:
        raise ValueError("exactly_two_models_required")
    for model in models:
        exact(model, ("id", "requested_model", "reported_model"), "model_shape")
        if not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", model["id"]):
            raise ValueError("model_id_refused")
        # Dated snapshots plus the explicitly reviewed Sonnet 5.5 version ID.
        # No family alias or guessed undated version is admitted. The receipt preserves the provider's
        # possibly canonicalized key separately, never calls it a snapshot.
        if (model["requested_model"] != "claude-sonnet-5-5"
                and not re.fullmatch(r"claude-[a-z0-9-]+-[0-9]{8}", model["requested_model"])):
            raise ValueError("exact_claude_snapshot_required")
        if not re.fullmatch(r"claude-[a-z0-9-]+", model["reported_model"]):
            raise ValueError("reported_model_refused")
    if (len({row["id"] for row in models}) != 2 or len({row["requested_model"] for row in models}) != 2
            or len({row["reported_model"] for row in models}) != 2):
        raise ValueError("distinct_models_required")
    hosts = configuration["host_ids"]
    if (not isinstance(hosts, list) or len(hosts) != 2 or len(set(hosts)) != 2
            or any(not isinstance(x, str) or not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", x) for x in hosts)):
        raise ValueError("two_host_ids_required")
    if not isinstance(configuration["approval"], str) or not 1 <= len(configuration["approval"]) <= 4096:
        raise ValueError("explicit_approval_reference_required")
    if not re.fullmatch(r"[0-9a-f]{64}", configuration["claude_sha256"]):
        raise ValueError("claude_executable_pin_required")
    limits = configuration["limits"]
    exact(limits, ("deadline_seconds", "max_request_bytes", "max_result_bytes", "max_reported_tokens", "max_estimated_usd"), "limits_shape")
    for key, cap in (("deadline_seconds", 90), ("max_request_bytes", 65536), ("max_result_bytes", 65536), ("max_reported_tokens", 65536)):
        integer(limits[key], cap, "invalid_" + key)
    cost = limits["max_estimated_usd"]
    if type(cost) not in (int, float) or not 0 < cost <= 10:
        raise ValueError("invalid_max_estimated_usd")
    manifest, _, _, _, _, prompt = source_inputs()
    if len(prompt.encode()) + len(SYSTEM.encode()) > limits["max_request_bytes"]:
        raise ValueError("request_exceeds_frozen_budget")
    scope = supported_scope.report()
    inventory = scope["comparison_inventory"]
    trials = []
    for model in models:
        for row in inventory:
            if row["task_id"] == TASK:
                trials.append({"model_id": model["id"], "task_id": TASK, "adapter_id": row["adapter_id"],
                               "disposition": "planned" if row["adapter_id"] == "typescript" else "unavailable",
                               "reason": None if row["adapter_id"] == "typescript" else row["support_reason"]})
    return {"schema": SCHEMA, "configuration": configuration, "task_id": TASK, "split": "validation",
            "candidate_paths": [CANDIDATE], "profile": PROFILE, "runner_revision": runner_revision(), "source_manifest_sha256": provenance.SOURCE_HASH,
            "source_correction_sha256": corrections.HASH, "implementation": implementation_identity(),
            "comparison_inventory": inventory, "trials_per_host": trials, "repetitions": 1,
            "prompt": prompt, "prompt_sha256": digest(prompt.encode()), "system_prompt": SYSTEM,
            "sampling": {"temperature": None, "top_p": None, "seed": None, "control": "provider_default_not_exposed"},
            "egress": "public_contract_and_fixed_public_scaffold_only; subscription_provider_transport",
            "billing": "provider_estimated_api_equivalent_usd; subscription_invoice_cost_unknown"}


def admit(plan, expected_digest):
    if digest(canonical(plan)) != expected_digest or plan != freeze(plan["configuration"]):
        raise ValueError("frozen_pilot_binding_refused")
    return plan
