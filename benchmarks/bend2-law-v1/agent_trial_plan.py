#!/usr/bin/env python3
"""Pre-register matched LAW-16 agent trials without running an agent.

The plan binds the reviewed cell and fixture bytes to fixed model/tool/budget
metadata. It deliberately refuses a numeric domain that either reviewed
language cannot execute; an unavailable plan is not a trial result.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import pathlib
import re
from datetime import datetime, timezone


SCHEMA = "semaprax.bend2-law-benchmark.agent-trial-plan.v1"
CONFIG_SCHEMA = "semaprax.bend2-law-benchmark.agent-trial-config.v1"
LANGUAGES = ("bend2", "semaprax-scalar-v1")
MIN_TRIALS = 10
SHA256 = re.compile(r"sha256:[0-9a-f]{64}\Z")
MEASUREMENT_PHASES = (
    "proof_synthesis",
    "law_kernel_check",
    "compile_or_runtime",
)


def _runner_module():
    path = pathlib.Path(__file__).with_name("run.py")
    spec = importlib.util.spec_from_file_location("bend2_law_run", path)
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    spec.loader.exec_module(module)
    return module


RUN = _runner_module()


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest(path: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def object_json(path: pathlib.Path, schema: str) -> dict:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {path}") from error
    if not isinstance(value, dict) or value.get("schema") != schema:
        raise ValueError(f"{path} has unsupported schema")
    return value


def require_config(config: dict) -> None:
    if set(config) != {"schema", "model", "tool_access", "fixed_budget", "trials_per_cell"}:
        raise ValueError("agent trial config keys are not exact")
    model = config["model"]
    if not isinstance(model, dict) or set(model) != {"provider", "name", "configuration_sha256"}:
        raise ValueError("agent model pin is incomplete")
    if not all(isinstance(model[key], str) and model[key] for key in ("provider", "name")):
        raise ValueError("agent model provider and name must be nonempty")
    if not isinstance(model["configuration_sha256"], str) or not SHA256.fullmatch(model["configuration_sha256"]):
        raise ValueError("agent model configuration must be a pinned sha256")
    tool_access = config["tool_access"]
    if not isinstance(tool_access, dict) or set(tool_access) != {"network", "filesystem", "shell"}:
        raise ValueError("agent tool access pin is incomplete")
    if not all(isinstance(value, str) and value for value in tool_access.values()):
        raise ValueError("agent tool access values must be nonempty")
    budget = config["fixed_budget"]
    if not isinstance(budget, dict) or set(budget) != {"max_tokens", "max_cost_usd"}:
        raise ValueError("agent fixed budget is incomplete")
    if not isinstance(budget["max_tokens"], int) or budget["max_tokens"] < 1:
        raise ValueError("agent fixed token budget must be positive")
    if not isinstance(budget["max_cost_usd"], str) or not re.fullmatch(r"[0-9]+\.[0-9]{2}", budget["max_cost_usd"]):
        raise ValueError("agent fixed cost budget must be canonical USD")
    if not isinstance(config["trials_per_cell"], int) or config["trials_per_cell"] < MIN_TRIALS:
        raise ValueError(f"agent trials per cell must be at least {MIN_TRIALS}")


def semaprax_supports(domain: str) -> bool:
    # The reviewed scalar profile admits the exact two-value Boolean cell and
    # i32/i64/u8. It does not admit a checked u32 comparison cell.
    return domain in {"bool exact", "i32 checked", "i64 checked", "u8 checked"}


def bend_supports(domain: str) -> bool:
    return domain in {"bool exact", "u32 checked"}


def unavailable_reason(domain: str) -> str | None:
    if not bend_supports(domain):
        return f"Bend 2 reviewed inputs do not admit numeric domain `{domain}`"
    if not semaprax_supports(domain):
        return f"SEMAPRAX reviewed scalar profile does not admit numeric domain `{domain}`"
    return None


def trial_rows(cell: dict, fixture: dict, trials_per_cell: int) -> list[dict]:
    """Return independent, unexecuted trial contracts for one admitted cell."""
    acceptance = {
        "success_witnesses": fixture["success"],
        "rejected_law_gaming_attacks": fixture["attacks"],
        "required_outcome": "each success witness is accepted and every seeded attack is rejected",
    }
    observations = {
        "telemetry": {
            "status": "required_on_execution",
            "events": ["token_usage", "cost_usage"],
            "provenance": "existing agent telemetry export",
        },
        "separate_measurements": {
            phase: "required_on_execution" for phase in MEASUREMENT_PHASES
        },
    }
    return [
        {
            "id": f"{cell['id']}:{language}:{ordinal}",
            "language": language,
            "ordinal": ordinal,
            "status": "not_run",
            "acceptance": acceptance,
            **observations,
        }
        for language in LANGUAGES
        for ordinal in range(1, trials_per_cell + 1)
    ]


def plan(manifest_path: pathlib.Path, config_path: pathlib.Path) -> dict:
    manifest = object_json(manifest_path, RUN.MANIFEST_SCHEMA)
    RUN.require_manifest(manifest, manifest_path.parent)
    config = object_json(config_path, CONFIG_SCHEMA)
    require_config(config)
    cells = []
    for cell in manifest["cells"]:
        fixture = manifest_path.parent / cell["fixture"]
        fixture_value = object_json(fixture, "semaprax.bend2-law-benchmark.fixture.v1")
        reason = unavailable_reason(cell["numeric_domain"])
        row = {
            "id": cell["id"],
            "numeric_domain": cell["numeric_domain"],
            "laws": cell["laws"],
            "attacks": cell["attacks"],
            "fixture": {"path": cell["fixture"], "sha256": digest(fixture)},
            "languages": list(LANGUAGES),
            "trials_per_language": config["trials_per_cell"],
        }
        if reason:
            row.update({"status": "unsupported", "reason": reason, "trials": []})
        else:
            row.update({
                "status": "preregistered",
                "trial_status": "not_run",
                "trials": trial_rows(cell, fixture_value, config["trials_per_cell"]),
            })
        cells.append(row)
    registered = sum(row["status"] == "preregistered" for row in cells)
    status = "preregistered" if registered == len(cells) else (
        "partially_preregistered" if registered else "unavailable"
    )
    return {
        "schema": SCHEMA,
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "status": status,
        "manifest": {"path": str(manifest_path.resolve()), "sha256": digest(manifest_path)},
        "agent_configuration": {"path": str(config_path.resolve()), "sha256": digest(config_path), "value": config},
        "cells": cells,
        "measurement_phases": list(MEASUREMENT_PHASES),
        "nonclaims": [
            "no agent trial executed",
            "no token or cost events observed",
            "no proof synthesis, law-kernel check, or compile/runtime time observed",
            "no comparative or superiority result",
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--manifest", type=pathlib.Path, default=pathlib.Path(__file__).with_name("manifest.json"))
    args = parser.parse_args(argv)
    try:
        document = plan(args.manifest, args.config)
    except ValueError as error:
        parser.error(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(canonical(document))
    return 0 if document["status"] == "preregistered" else 1


if __name__ == "__main__":
    raise SystemExit(main())
