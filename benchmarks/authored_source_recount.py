#!/usr/bin/env python3
"""Audit an existing final-source token proxy by hash-bound file provenance.

This never mutates a candidate or its original campaign metrics. It separates
legacy final-inventory proxy tokens into authored source, proven generated
outputs, dependency locks, and unresolved files. It is not provider-output or
cumulative-authorship accounting.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
from pathlib import Path
from typing import Any

import live_campaign_common as common

SCHEMA = "semaprax.authored-source-recount.v1"
CLASSIFICATION_SCHEMA = "semaprax.authored-source-classification.v1"
CLASSES = {"authored_source", "generated_output", "dependency_lock", "unresolved"}


def digest(path: Path) -> str:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"expected regular file: {path}")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def json_digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode()).hexdigest()


def load(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"expected regular JSON: {path}")
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ValueError("JSON root must be an object")
    return value


def source_inventory(candidate: Path) -> dict[str, dict[str, Any]]:
    if candidate.is_symlink() or not candidate.is_dir():
        raise ValueError("candidate must be a real directory")
    files: dict[str, dict[str, Any]] = {}
    for root, names, filenames in os.walk(candidate, topdown=True, followlinks=False):
        directory = Path(root)
        retained = []
        for name in sorted(names):
            path = directory / name
            mode = path.stat(follow_symlinks=False).st_mode
            relative = path.relative_to(candidate)
            if path.is_symlink() or not stat.S_ISDIR(mode):
                raise ValueError(f"candidate directory is unsafe: {relative}")
            if name not in common.AUTHORED_EXCLUDED_DIRS:
                retained.append(name)
        names[:] = retained
        for name in sorted(filenames):
            path = directory / name
            relative = path.relative_to(candidate)
            mode = path.stat(follow_symlinks=False).st_mode
            if path.is_symlink() or not stat.S_ISREG(mode):
                raise ValueError(f"candidate file is unsafe: {relative}")
            if path.suffix.lower() not in common.AUTHORED_SUFFIXES and path.name not in common.AUTHORED_SPECIAL_NAMES:
                continue
            if path.suffix.lower() in {".c", ".h"}:
                continue
            if path.suffix.lower() in {".js", ".mjs", ".cjs"} and any(
                path.with_suffix(suffix).is_file() for suffix in (".ts", ".tsx")
            ):
                continue
            key = relative.as_posix()
            files[key] = {"path": key, "sha256": digest(path), "bytes": path.stat().st_size}
    return files


def metrics_inventory(candidate: Path, metrics: dict[str, Any]) -> dict[str, dict[str, Any]]:
    if metrics.get("status") != "measured_proxy" or not isinstance(metrics.get("tokenizer"), dict):
        raise ValueError("metrics is not a measured tokenizer-bound proxy")
    total = metrics.get("total_tokens")
    rows = metrics.get("files")
    if isinstance(total, bool) or not isinstance(total, int) or total < 0 or not isinstance(rows, list):
        raise ValueError("metrics lacks a valid total or per-file inventory")
    actual = source_inventory(candidate)
    inventory: dict[str, dict[str, Any]] = {}
    for row in rows:
        if (not isinstance(row, dict) or not isinstance(row.get("path"), str)
                or not isinstance(row.get("sha256"), str) or isinstance(row.get("tokens"), bool)
                or not isinstance(row.get("tokens"), int) or row["tokens"] < 0):
            raise ValueError("metrics has malformed per-file row")
        relative = Path(row["path"])
        if relative.is_absolute() or ".." in relative.parts or row["path"] in inventory:
            raise ValueError("metrics path is unsafe or duplicate")
        expected = actual.get(row["path"])
        if expected is None or row["sha256"] != expected["sha256"]:
            raise ValueError(f"candidate inventory drifted: {row['path']}")
        inventory[row["path"]] = row
    if set(inventory) != set(actual):
        raise ValueError("metrics omitted or added measured source")
    if sum(row["tokens"] for row in inventory.values()) != total:
        raise ValueError("metrics total differs from per-file token counts")
    return inventory


def classify(inventory: dict[str, dict[str, Any]], candidate: Path, sidecar: dict[str, Any]) -> list[dict[str, Any]]:
    if sidecar.get("schema") != CLASSIFICATION_SCHEMA or not isinstance(sidecar.get("origin"), dict):
        raise ValueError("classification sidecar schema or origin differs")
    rows = sidecar.get("files")
    if not isinstance(rows, list):
        raise ValueError("classification sidecar lacks files")
    classified: dict[str, dict[str, Any]] = {}
    for row in rows:
        if (not isinstance(row, dict) or row.get("classification") not in CLASSES
                or not isinstance(row.get("path"), str) or not isinstance(row.get("sha256"), str)):
            raise ValueError("classification row is malformed")
        path = row["path"]
        if path in classified or path not in inventory or row["sha256"] != inventory[path]["sha256"]:
            raise ValueError("classification inventory/hash differs")
        classified[path] = row
    if set(classified) != set(inventory):
        raise ValueError("classification is incomplete")
    def prove_generated(path: str, chain: set[str]) -> None:
        if path in chain:
            raise ValueError("generated output provenance has a cycle")
        row = classified[path]
        if row["classification"] == "authored_source":
            return
        if row["classification"] != "generated_output":
            raise ValueError("generated output provenance does not end in authored source")
        next_chain = {*chain, path}
        for path_key, hash_key in (("recipe_path", "recipe_sha256"), ("entrypoint_path", "entrypoint_sha256")):
            evidence_path, evidence_hash = row.get(path_key), row.get(hash_key)
            if not isinstance(evidence_path, str) or not isinstance(evidence_hash, str):
                raise ValueError("generated output lacks exact provenance")
            relative = Path(evidence_path)
            if relative.is_absolute() or ".." in relative.parts or evidence_path == path:
                raise ValueError("generated output provenance is unsafe")
            evidence = inventory.get(evidence_path)
            if (evidence is None or evidence["sha256"] != evidence_hash
                    or digest(candidate / relative) != evidence_hash):
                raise ValueError("generated output provenance differs from candidate inventory")
            prove_generated(evidence_path, next_chain)

    for path, row in classified.items():
        if row["classification"] == "generated_output":
            prove_generated(path, set())
    return [classified[path] for path in sorted(classified)]


def recount(candidate: Path, metrics: dict[str, Any], sidecar: dict[str, Any],
            metrics_sha256: str | None = None) -> dict[str, Any]:
    if candidate.is_symlink() or not candidate.is_dir():
        raise ValueError("candidate must be a real directory")
    inventory = metrics_inventory(candidate, metrics)
    rows = classify(inventory, candidate, sidecar)
    totals = {kind: 0 for kind in CLASSES}
    output = []
    for row in rows:
        metric, kind = inventory[row["path"]], row["classification"]
        totals[kind] += metric["tokens"]
        output.append({"path": row["path"], "sha256": metric["sha256"], "tokens": metric["tokens"],
                       "classification": kind, **({key: row[key] for key in
                       ("recipe_path", "recipe_sha256", "entrypoint_path", "entrypoint_sha256")}
                       if kind == "generated_output" else {})})
    return {
        "schema": SCHEMA, "candidate": str(candidate.resolve()),
        "legacy_final_inventory_proxy_tokens": metrics["total_tokens"],
        "metrics_binding": {"status": metrics["status"], "scope": metrics.get("scope"),
                            "tokenizer": metrics["tokenizer"],
                            "document_sha256": metrics_sha256 or json_digest(metrics),
                            "source_inventory_sha256": json_digest([
                                {"path": key, "sha256": row["sha256"]}
                                for key, row in sorted(inventory.items())])},
        "components": totals, "files": output, "classification_origin": sidecar["origin"],
        "classification_complete": True, "authorship_verified": False, "ratio_eligible": False,
        "limits": "final-file proxy only; not cumulative authorship or provider output",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--metrics", required=True)
    parser.add_argument("--classification", required=True)
    parser.add_argument("--output", required=True)
    try:
        args = parser.parse_args()
        raw_candidate, metrics_path = Path(args.candidate), Path(args.metrics)
        if raw_candidate.is_symlink():
            raise ValueError("candidate must not be a symlink")
        result = recount(raw_candidate.resolve(strict=True), load(metrics_path), load(Path(args.classification)),
                         digest(metrics_path))
        output = Path(args.output)
        if output.exists() or output.is_symlink():
            raise ValueError("output must be absent")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
        return 0
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"authored recount error: {error}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
