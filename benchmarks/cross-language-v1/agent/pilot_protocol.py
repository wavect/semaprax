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
PROFILE = "typescript-live-pilot.v1"
DARWIN_PROFILE = "darwin-arm64-official-typescript-pilot.v1"
LINUX_PROFILE = "apple-container-linux-arm64-typescript-pilot.v1"
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
    names += ["agent/" + path.name for path in sorted((SUITE / "agent").glob("pilot_linux_*")) if path.suffix in (".py", ".c")]
    return {name: digest(provenance.read_regular(SUITE / name, 256 * 1024)) for name in names}


def runner_revision():
    result = subprocess.run(["/usr/bin/git", "-C", str(provenance.ROOT), "rev-parse", "HEAD"],
                            env={"PATH": "/usr/bin:/bin"}, capture_output=True, timeout=60, check=True)
    revision = result.stdout.decode().strip()
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("runner_revision_refused")
    return revision


def freeze(configuration):
    return _freeze(configuration, runner_revision())


def _freeze(configuration, revision):
    exact(configuration, ("models", "limits", "approval", "hosts", "cli_version", "execution_profiles", "controller_ledger"), "pilot_configuration_shape")
    ledger = configuration["controller_ledger"]
    if (not isinstance(ledger, str) or not ledger.startswith("/") or "\x00" in ledger
            or str(pathlib.PurePosixPath(ledger)) != ledger or ".." in pathlib.PurePosixPath(ledger).parts):
        raise ValueError("canonical_controller_ledger_required")
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
    hosts = configuration["hosts"]
    if (not isinstance(hosts, dict) or len(hosts) != 2
            or any(not isinstance(x, str) or not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", x) for x in hosts)):
        raise ValueError("two_host_ids_required")
    for host in hosts.values():
        exact(host, ("native_platform", "kernel_release", "boot_id", "executable", "claude_sha256", "home", "login"), "host_authority_shape")
        if host["native_platform"] not in ("darwin-arm64", "linux-arm64"):
            raise ValueError("native_platform_refused")
        if not isinstance(host["kernel_release"], str) or not 1 <= len(host["kernel_release"]) <= 256:
            raise ValueError("kernel_release_required")
        if host["native_platform"] == "linux-arm64":
            if not isinstance(host["boot_id"], str) or not re.fullmatch(r"[0-9a-f-]{36}", host["boot_id"]):
                raise ValueError("guest_boot_identity_required")
        elif host["boot_id"] is not None:
            raise ValueError("darwin_boot_identity_must_be_null")
        for name in ("executable", "home"):
            value = host[name]
            if (not isinstance(value, str) or not value.startswith("/") or "\x00" in value
                    or str(pathlib.PurePosixPath(value)) != value or ".." in pathlib.PurePosixPath(value).parts):
                raise ValueError("canonical_host_authority_path_required")
        if not re.fullmatch(r"[0-9a-f]{64}", host["claude_sha256"]):
            raise ValueError("claude_executable_pin_required")
        if not re.fullmatch(r"[A-Za-z0-9_.-]{1,256}", host["login"]):
            raise ValueError("host_login_refused")
    if {host["native_platform"] for host in hosts.values()} != {"darwin-arm64", "linux-arm64"}:
        raise ValueError("distinct_native_hosts_required")
    if configuration["cli_version"] != "2.1.286":
        raise ValueError("unreviewed_cli_version")
    profiles = configuration["execution_profiles"]
    exact(profiles, ("darwin-arm64", "linux-arm64"), "execution_profiles_shape")
    for platform, profile in profiles.items():
        exact(profile, ("profile", "provision_sha256"), "execution_profile_shape")
        if profile["profile"] != (DARWIN_PROFILE if platform == "darwin-arm64" else LINUX_PROFILE):
            raise ValueError("unreviewed_scoring_profile")
        if platform == "darwin-arm64":
            if profile["provision_sha256"] is not None:
                raise ValueError("darwin_uses_fixed_v3_provenance")
        elif not isinstance(profile["provision_sha256"], str) or not re.fullmatch(r"[0-9a-f]{64}", profile["provision_sha256"]):
            raise ValueError("linux_provision_pin_required")
    if not isinstance(configuration["approval"], str) or not 1 <= len(configuration["approval"]) <= 4096:
        raise ValueError("explicit_approval_reference_required")
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
            "candidate_paths": [CANDIDATE], "profile": PROFILE, "runner_revision": revision, "source_manifest_sha256": provenance.SOURCE_HASH,
            "source_correction_sha256": corrections.HASH, "implementation": implementation_identity(),
            "execution_profiles": configuration["execution_profiles"],
            "comparison_inventory": inventory, "trials_per_host": trials, "repetitions": 1,
            "prompt": prompt, "prompt_sha256": digest(prompt.encode()), "system_prompt": SYSTEM,
            "sampling": {"temperature": None, "top_p": None, "seed": None, "control": "provider_default_not_exposed"},
            "egress": "public_contract_and_fixed_public_scaffold_only; subscription_provider_transport",
            "billing": "provider_estimated_api_equivalent_usd; subscription_invoice_cost_unknown"}


def admit(plan, expected_digest):
    if digest(canonical(plan)) != expected_digest or plan != freeze(plan["configuration"]):
        raise ValueError("frozen_pilot_binding_refused")
    return plan


def admit_guest(plan, expected_digest):
    """Authenticate a controller snapshot; never claim a guest Git observation."""
    if (not isinstance(expected_digest, str) or not re.fullmatch(r"[0-9a-f]{64}", expected_digest)
            or digest(canonical(plan)) != expected_digest):
        raise ValueError("guest_plan_digest_refused")
    revision = plan.get("runner_revision")
    if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("guest_controller_revision_refused")
    # Everything except the externally pinned controller revision is derived
    # again from this machine's exact implementation and frozen source bytes.
    if plan != _freeze(plan["configuration"], revision):
        raise ValueError("guest_source_snapshot_refused")
    return plan


def source_admission(plan, kind):
    if kind not in ("local_git_checkout", "controller_frozen_snapshot"):
        raise ValueError("source_admission_kind_refused")
    return {"kind": kind, "plan_sha256": digest(canonical(plan)),
            "controller_runner_revision": plan["runner_revision"],
            "implementation_sha256": digest(canonical(plan["implementation"])),
            "source_manifest_sha256": plan["source_manifest_sha256"],
            "source_correction_sha256": plan["source_correction_sha256"],
            "prompt_sha256": plan["prompt_sha256"]}
