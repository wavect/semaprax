#!/usr/bin/env python3
"""Run and bind the source-authenticated all-i64 list-sort Lean proof for LAW-16.

This capsule selects the existing LAW-15 declaration identities deliberately.
They are the only identities admitted by the collection proof exporter.  Since
every guarded U32 value is an i64, the checked theorem is stronger than the
full U32 element domain, but it does not certify the separately named
``law16.*`` fixture or any runtime lowering.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).parent.parent.parent
BENCHMARK = ROOT / "benchmarks/bend2-law-v1"
SOURCE = ROOT / "examples/law-packs/collection/sort.spx"
EMPTY = ROOT / "examples/law-packs/collection/mutants/empty-output.spx"
DUPLICATE = ROOT / "examples/law-packs/collection/mutants/duplicate-element.spx"
PROOFS = ROOT / "examples/law-packs/collection/lemmas.json"
HARNESS = ROOT / "tests/language/collection_law_pack.rs"
PIN = (ROOT / "proofs/kernel0-lean/lean-toolchain").read_text().strip()
VERSION = "Lean (version 4.34.0, arm64-apple-darwin24.6.0, commit 293d5d0c0c3f3dded4688b3ccd6a33939ac5102b, Release)"
SCHEMA = "semaprax.bend2-law-benchmark.law16-i64-list-proof-capsule.v1"
TEST = "collection_law_pack::collection_law_pack_pinned_lean_rejects_empty_and_duplicate_outputs_and_repairs"


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def reference(path: pathlib.Path, root: pathlib.Path) -> dict:
    data = path.read_bytes()
    try:
        display = str(path.relative_to(root))
    except ValueError:
        display = str(path.resolve())
    return {"path": display, "bytes": len(data), "sha256": digest(data)}


def command(test_binary: pathlib.Path) -> list[str]:
    return [str(test_binary), TEST, "--ignored", "--exact", "--nocapture"]


def accepted(returncode: int, output: bytes) -> bool:
    text = output.decode("utf-8", "replace")
    return (returncode == 0 and f"{TEST} ... ok" in text
            and "empty-output: count differs for value 2 on input [1, 2]" in text
            and "duplicate-element: count differs for value 2 on input [1, 2]" in text)


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", type=pathlib.Path, required=True)
    parser.add_argument("--lean", type=pathlib.Path, required=True)
    parser.add_argument("--raw-artifact-dir", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args(argv)
    if args.output.exists() or args.raw_artifact_dir.exists() or not args.output.parent.is_dir():
        parser.error("output and raw artifact directory must be new under an existing directory")
    if not all(path.is_file() for path in [args.test_binary, args.lean, SOURCE, EMPTY, DUPLICATE, PROOFS, HARNESS]):
        parser.error("test binary, Lean, or a bound proof input is unavailable")
    probe = subprocess.run([str(args.lean), "--version"], capture_output=True, text=True, env=dict(os.environ, ELAN_TOOLCHAIN=PIN))
    if probe.returncode or probe.stdout.strip() != VERSION:
        parser.error("Lean does not match the pinned law-pack version")
    args.raw_artifact_dir.mkdir(parents=True)
    argv = command(args.test_binary)
    environment = dict(os.environ, ELAN_TOOLCHAIN=PIN, SEMAPRAX_LAW_LEAN=str(args.lean),
                       SEMAPRAX_LAW_LEAN_VERSION=VERSION)
    completed = subprocess.run(argv, cwd=ROOT, env=environment, capture_output=True, check=False)
    stdout, stderr = args.raw_artifact_dir / "kernel-test.stdout", args.raw_artifact_dir / "kernel-test.stderr"
    stdout.write_bytes(completed.stdout)
    stderr.write_bytes(completed.stderr)
    raw = completed.stdout + completed.stderr
    result = {
        "schema": SCHEMA,
        "status": "supplemental_i64_list_profile_proved" if accepted(completed.returncode, raw) else "incomplete",
        "profile": "semaprax.collection-sort-i64.v1",
        "kernel": {"tool": reference(args.lean, ROOT), "pin": PIN, "version": VERSION},
        "test_binary": reference(args.test_binary, ROOT),
        "command": argv,
        "command_sha256": digest(json.dumps(argv, separators=(",", ":")).encode()),
        "source": reference(SOURCE, ROOT),
        "proof_module": reference(PROOFS, ROOT),
        "harness": reference(HARNESS, ROOT),
        "attacks": {"empty_sort": reference(EMPTY, ROOT), "duplicate_multiplicity": reference(DUPLICATE, ROOT)},
        "raw": {"stdout": reference(stdout, args.raw_artifact_dir), "stderr": reference(stderr, args.raw_artifact_dir)},
        "coverage": {"declarations": ["law15.collection.insert", "law15.collection.sort"],
                     "laws": ["sort_sorted", "sort_permutation", "sort_multiplicity"],
                     "domain": "all finite List<i64>; guarded U32 values are a subset"},
        "original_law16_cell": "unchanged; its law16.* identities are not covered by this certificate",
        "nonclaims": ["test-binary source association is local rather than a build attestation", "no certificate for law16.* declarations", "no Bend source proof", "no runtime resource or lowering proof", "no public list ABI proof"],
    }
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    return 0 if result["status"] == "supplemental_i64_list_profile_proved" else 1


if __name__ == "__main__":
    raise SystemExit(main())
