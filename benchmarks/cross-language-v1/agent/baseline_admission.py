"""Offline, fail-closed provenance admission for future external baselines.

This module deliberately does *not* provision a toolchain, read a port from
disk, invoke an agent, or score a candidate.  It only verifies that a caller
has supplied a complete, canonical description of the immutable inputs a
future independent execution would need.  A successful schema check still
returns ``status: unavailable``: it is not an execution observation.
"""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any


ADMISSION_SCHEMA = "benchmark.cross_language.agent.baseline_admission.v1"
DECISION_SCHEMA = "benchmark.cross_language.agent.baseline_admission_decision.v1"
OWNER_TASKS_SCHEMA = "benchmark.cross_language.tasks.v1"

# SHA-256 of benchmarks/cross-language-v1/tasks.json's canonical owner bytes.
# Updating this pin is a reviewed corpus change, not caller-supplied metadata.
OWNER_TASK_INVENTORY_SHA256 = "sha256:cd6cef4d71810f1f1c9187b5750fa2dc8a0c9a1fe4dee32a9a70b51f19e3700f"

_DIGEST = re.compile(r"sha256:[0-9a-f]{64}\Z")
_IDENTIFIER = re.compile(r"[a-z0-9][a-z0-9-]*\Z")
_MUTABLE = {"head", "latest", "main", "master", "stable", "trunk"}

# The only external baseline identity the owning benchmark has pinned.  This
# records the reserved subject from agent-task-comparison-v1; it does not make
# that subject admissible for execution or change its external_unrun state.
_PINNED_BASELINE_SYSTEMS = {
    "zero": {
        "official_source": "https://github.com/vercel-labs/zerolang",
        "revision": "eb2ed6c22fe3f6e3152efa0c0d05ffcf1ff4a2c7",
    },
}

_OWNER_TASK_KEYS = {
    "id", "category", "issue_211_category", "split", "summary", "equivalence", "languages",
}


def _unavailable(reason: str) -> dict[str, Any]:
    return {
        "schema": DECISION_SCHEMA,
        "status": "unavailable",
        "reason": reason,
    }


def _canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii")


def _is_digest(value: Any) -> bool:
    return isinstance(value, str) and bool(_DIGEST.fullmatch(value))


def _is_immutable_text(value: Any) -> bool:
    if not isinstance(value, str) or value != value.strip():
        return False
    normalized = value.lower()
    return bool(normalized) and normalized not in _MUTABLE and not normalized.startswith("@")


def _is_immutable_revision(value: Any) -> bool:
    if not _is_immutable_text(value):
        return False
    normalized = value.strip().lower()
    return not normalized.startswith(("refs/heads/", "branch:"))


def _has_exact_keys(value: Any, keys: set[str]) -> bool:
    return isinstance(value, dict) and set(value) == keys


def _is_relative_path(value: Any) -> bool:
    if (not isinstance(value, str) or not value or value.startswith("/")
            or "\\" in value or ":" in value):
        return False
    parts = value.split("/")
    return all(part not in {"", ".", ".."} for part in parts)


def _canonical_owner_task_inventory(owner_task_inventory_bytes: Any) -> tuple[list[str], str] | None:
    """Parse the exact, pinned owner inventory without reading a filesystem.

    The bytes must already be canonical JSON so there is one auditable digest
    for one closed task inventory.  Task IDs are derived in owner declaration
    order; callers cannot choose a subset or provide a detached digest.
    """
    if not isinstance(owner_task_inventory_bytes, bytes):
        return None
    try:
        document = json.loads(owner_task_inventory_bytes.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return None
    canonical = (json.dumps(document, indent=2, ensure_ascii=True) + "\n").encode("utf-8")
    if owner_task_inventory_bytes != canonical:
        return None
    tasks_sha256 = "sha256:" + hashlib.sha256(owner_task_inventory_bytes).hexdigest()
    if tasks_sha256 != OWNER_TASK_INVENTORY_SHA256:
        return None
    if not _has_exact_keys(document, {"schema", "tasks"}) or document["schema"] != OWNER_TASKS_SCHEMA:
        return None
    tasks = document["tasks"]
    if not isinstance(tasks, list) or not tasks:
        return None
    task_ids = []
    for task in tasks:
        if not _has_exact_keys(task, _OWNER_TASK_KEYS):
            return None
        if not all(_is_immutable_text(task[field]) for field in (
            "id", "category", "issue_211_category", "split", "summary", "equivalence",
        )) or not _IDENTIFIER.fullmatch(task["id"]):
            return None
        languages = task["languages"]
        if not isinstance(languages, dict) or not languages:
            return None
        for language, paths in languages.items():
            if (not isinstance(language, str) or not _IDENTIFIER.fullmatch(language)
                    or not _has_exact_keys(paths, {"public", "hidden"})
                    or not _is_relative_path(paths["public"])
                    or not _is_relative_path(paths["hidden"])):
                return None
        task_ids.append(task["id"])
    if len(task_ids) != len(set(task_ids)):
        return None
    return task_ids, tasks_sha256


def admit_baseline_descriptor(document: Any, owner_task_inventory_bytes: Any) -> dict[str, Any]:
    """Return a deterministic, non-execution admission decision.

    ``owner_task_inventory_bytes`` supplies the canonical, digest-pinned owner
    inventory. This function recomputes its SHA-256 and derives task IDs; it
    never discovers a filesystem view. Invalid inputs and every otherwise
    valid descriptor are unavailable; only their reason differs.
    """
    inventory = _canonical_owner_task_inventory(owner_task_inventory_bytes)
    if inventory is None:
        return _unavailable("invalid_required_task_inventory")
    required, tasks_sha256 = inventory
    if not _has_exact_keys(document, {
        "schema", "system", "toolchain", "agent_interface", "model", "ports", "execution",
    }):
        return _unavailable("invalid_document_shape")
    if document["schema"] != ADMISSION_SCHEMA:
        return _unavailable("unsupported_schema")

    system = document["system"]
    if not _has_exact_keys(system, {"id", "display_name"}):
        return _unavailable("invalid_system_identity")
    if (not isinstance(system["id"], str) or not _IDENTIFIER.fullmatch(system["id"])
            or system["id"] == "semaprax" or not _is_immutable_text(system["display_name"])):
        return _unavailable("invalid_system_identity")
    pinned_system = _PINNED_BASELINE_SYSTEMS.get(system["id"])
    if pinned_system is None:
        return _unavailable("baseline_system_not_pinned")

    toolchain = document["toolchain"]
    if not _has_exact_keys(toolchain, {
        "official_source", "revision", "version", "artifact_sha256", "installation_receipt_sha256", "license",
    }):
        return _unavailable("invalid_toolchain_provenance")
    if (not isinstance(toolchain["official_source"], str)
            or not toolchain["official_source"].startswith("https://")
            or not _is_immutable_revision(toolchain["revision"])
            or not _is_immutable_text(toolchain["version"])
            or not _is_digest(toolchain["artifact_sha256"])
            or not _is_digest(toolchain["installation_receipt_sha256"])
            or not _is_immutable_text(toolchain["license"])):
        return _unavailable("invalid_toolchain_provenance")
    if (toolchain["official_source"] != pinned_system["official_source"]
            or toolchain["revision"] != pinned_system["revision"]):
        return _unavailable("toolchain_identity_does_not_match_pinned_baseline")

    interface = document["agent_interface"]
    if not _has_exact_keys(interface, {"version", "guidance_sha256", "invocation_contract_sha256"}):
        return _unavailable("invalid_agent_interface_provenance")
    if (not _is_immutable_text(interface["version"])
            or not _is_digest(interface["guidance_sha256"])
            or not _is_digest(interface["invocation_contract_sha256"])):
        return _unavailable("invalid_agent_interface_provenance")

    model = document["model"]
    if not _has_exact_keys(model, {"provider", "model", "revision"}):
        return _unavailable("invalid_model_identity")
    if not all(_is_immutable_text(model[field]) for field in ("provider", "model", "revision")):
        return _unavailable("invalid_model_identity")

    ports = document["ports"]
    if not isinstance(ports, list) or len(ports) != len(required):
        return _unavailable("ports_do_not_cover_required_tasks")
    port_task_ids: list[str] = []
    for port in ports:
        if not _has_exact_keys(port, {
            "task_id", "port_tree_sha256", "oracle_sha256", "equivalence_review_sha256", "candidate_paths",
        }):
            return _unavailable("invalid_port_provenance")
        paths = port["candidate_paths"]
        if not isinstance(paths, list) or not paths or not all(_is_relative_path(path) for path in paths):
            return _unavailable("invalid_port_provenance")
        if (not isinstance(port["task_id"], str) or not _IDENTIFIER.fullmatch(port["task_id"])
                or not all(_is_digest(port[field]) for field in (
                    "port_tree_sha256", "oracle_sha256", "equivalence_review_sha256",
                )) or paths != sorted(set(paths))):
            return _unavailable("invalid_port_provenance")
        port_task_ids.append(port["task_id"])
    if port_task_ids != required:
        return _unavailable("ports_do_not_cover_required_tasks")

    execution = document["execution"]
    if execution != {"status": "not_executed"}:
        return _unavailable("execution_evidence_not_admissible_here")

    descriptor_sha256 = "sha256:" + hashlib.sha256(_canonical_bytes(document)).hexdigest()
    return {
        "schema": DECISION_SCHEMA,
        "status": "unavailable",
        "reason": "offline_admission_is_not_execution_evidence",
        "provenance": {
            "descriptor_sha256": descriptor_sha256,
            "system_id": system["id"],
            "task_ids": required,
            "tasks_sha256": tasks_sha256,
        },
    }
