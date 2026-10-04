#!/usr/bin/env python3
"""Authenticate why the checked-u32 incremental control has no retained matched route.

This static admission review validates the fixture and its stale-cache negative
control, then binds the parser non-admission receipt and the public
``project-proof-check`` exact-obligation surface. It does not execute a checker
or claim that the repository lacks every possible incremental implementation.
"""
import argparse
import hashlib
import importlib.util
import json
import pathlib

ROOT = pathlib.Path(__file__).parent
REPO = ROOT.parent.parent
FIXTURE = ROOT / "fixtures/project-incremental-edit-v1.json"
MANIFEST = ROOT / "manifest.json"
U32_RECEIPT = ROOT / "evidence/law16-checked-u32-nonadmission-v1/review.json"
CLI_HELP = REPO / "src/cli/help.rs"
PROJECT_PROOF = REPO / "src/cli/project_proof.rs"
SCHEMA = "semaprax.bend2-law-benchmark.project-incremental-admission.v1"

SPEC = importlib.util.spec_from_file_location("equal_spec_controls", ROOT / "equal_spec_controls.py")
CONTROLS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CONTROLS)


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def source_ref(path):
    return {"path": str(path.relative_to(REPO)), "sha256": digest(path), "bytes": path.stat().st_size}


def one_cell(manifest):
    cells = [cell for cell in manifest["cells"] if cell.get("id") == "project-incremental-edit-v1"]
    if len(cells) != 1:
        raise ValueError("benchmark manifest must contain exactly one incremental cell")
    return cells[0]


def require_surface(text, label, required, prohibited):
    if not all(value in text for value in required):
        raise ValueError(f"{label} no longer has the reviewed exact-obligation surface")
    if any(value in text for value in prohibited):
        raise ValueError(f"{label} gained an incremental-accounting surface; refresh this non-admission review")


def review():
    manifest = json.loads(MANIFEST.read_text())
    fixture = json.loads(FIXTURE.read_text())
    cell = one_cell(manifest)
    CONTROLS.validate(cell, fixture)
    if fixture.get("numeric_domain") != "u32 checked":
        raise ValueError("incremental control must retain checked-u32 semantics")

    u32 = json.loads(U32_RECEIPT.read_text())
    if u32.get("status") != "authenticated_unsupported_by_pinned_parser" or u32.get("numeric_domain") != "u32 checked":
        raise ValueError("checked-u32 parser non-admission receipt drifted")

    help_source = CLI_HELP.read_text()
    proof_source = PROJECT_PROOF.read_text()
    require_surface(
        help_source, "CLI help",
        ["canonical: \"project-proof-check\"", "--source <project-path> --declaration <stable-id> --ensures <index>"],
        ["--changed-module", "--rechecked-module", "--reused-module"],
    )
    require_surface(
        proof_source, "project proof command",
        ["let proof = prove_postcondition", "\"project_assurance\"", "\"--source\"", "\"--ensures\""],
        ["\"rechecked\"", "\"reused\"", "\"changed_module\""],
    )

    success = fixture["success"][0]
    attack = fixture["attacks"]["stale-cache-reuse"][0]
    return {
        "schema": SCHEMA,
        "status": "unsupported",
        "cell": cell["id"],
        "fixture": {"path": "fixtures/project-incremental-edit-v1.json", "sha256": digest(FIXTURE)},
        "semantic_contract": {
            "numeric_domain": "u32 checked", "laws": cell["laws"],
            "success_witness": success,
            "negative_control": {"id": "stale-cache-reuse", "witness": attack},
        },
        "admission_gates": [
            {"id": "checked-u32-parser", "status": "unsupported", "receipt": {"path": "evidence/law16-checked-u32-nonadmission-v1/review.json", "sha256": digest(U32_RECEIPT)}, "reason": "the pinned SEMAPRAX parser rejects checked-u32 input before a same-domain proof route can run"},
            {"id": "incremental-accounting-output", "status": "unsupported", "reviewed_sources": [source_ref(CLI_HELP), source_ref(PROJECT_PROOF)], "reason": "the retained public project-proof-check source route selects one source postcondition and emits project assurance; its reviewed option/output surface has no changed-module, rechecked-module, or reused-module accounting"},
        ],
        "nonclaims": [
            "no checker was executed by this static review",
            "this does not claim the repository has no incremental implementation outside the retained matched CLI route",
            "no i32, i64, u8, or usize substitute",
            "no matched execution, proof, timing, cache-work, or winner result",
        ],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.output.exists() or not args.output.parent.is_dir():
        parser.error("output must be new below an existing directory")
    try:
        document = review()
    except (OSError, ValueError, json.JSONDecodeError) as error:
        parser.error(str(error))
    args.output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
