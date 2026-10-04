#!/usr/bin/env python3
"""Verify pinned Bend 2 fixture inputs without invoking either implementation."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
RUN_SPEC = importlib.util.spec_from_file_location("bend2_run", ROOT / "run.py")
RUN = importlib.util.module_from_spec(RUN_SPEC)
RUN_SPEC.loader.exec_module(RUN)
SCHEMA = "semaprax.bend2-law-benchmark.fixture-receipt.v1"


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def receipt(manifest_path):
    manifest = RUN.load(manifest_path, RUN.MANIFEST_SCHEMA)
    RUN.require_manifest(manifest, manifest_path.parent)
    cells = []
    for cell in manifest["cells"]:
        fixture = manifest_path.parent / cell["fixture"]
        cells.append({
            "id": cell["id"],
            "fixture": cell["fixture"],
            "fixture_sha256": digest(fixture),
            "numeric_domain": cell["numeric_domain"],
            "laws": cell["laws"],
            "attacks": cell["attacks"],
        })
    return {"schema": SCHEMA, "manifest_sha256": digest(manifest_path), "cells": cells}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=ROOT / "manifest.json")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(receipt(args.manifest), sort_keys=True, indent=2) + "\n")


if __name__ == "__main__":
    main()
