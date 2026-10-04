#!/usr/bin/env python3
"""Review the retained Boolean Bend-verdict and SEMAPRAX-Z3 process capsules.

The two local routes are useful execution evidence but their fixed contracts
must be checked before timing values can be placed in one matched corpus.
"""
import argparse
import hashlib
import importlib.util
import json
import pathlib


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location(
    "law16_process_state_capsule", ROOT / "law16_process_state_capsule.py"
)
PROCESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROCESS)

SCHEMA = "semaprax.bend2-law-benchmark.boolean-verdict-aggregate-review.v1"
BEND_COMMIT = "947db722640c86247849343657bf2f7ef01cb7f1"
SEMAPRAX_COMMIT = "9a9db7a8117ac8d292b24ffd5671ec3333272290"


def digest(body):
    return "sha256:" + hashlib.sha256(body).hexdigest()


def source(root, name):
    path = root / name
    return path.read_text(), {"path": name, "bytes": path.stat().st_size, "sha256": digest(path.read_bytes())}


def review(bend_root, semaprax_root):
    bend = PROCESS.review(bend_root)
    semaprax = PROCESS.review(semaprax_root)
    if bend["tool"]["commit"] != BEND_COMMIT or bend["route"] != "Bend --verdict; BEND_NO_TELEMETRY=1":
        raise ValueError("Bend verdict capsule identity drifted")
    if semaprax["tool"]["commit"] != SEMAPRAX_COMMIT or semaprax["route"] != "project-proof-check app.negate ensures[0] with installed Z3":
        raise ValueError("SEMAPRAX Z3 capsule identity drifted")
    bend_source, bend_reference = source(bend_root, "fresh-input.bend")
    semaprax_source, semaprax_reference = source(semaprax_root, "fresh-input.spx")
    if "law boolean_score_matches_base:" not in bend_source or "boolean_score(value) == Bool.to_u32(value)" not in bend_source:
        raise ValueError("Bend source no longer binds the Boolean score law")
    if "fn negate(value: bool) -> bool" not in semaprax_source or "ensures result == !value" not in semaprax_source:
        raise ValueError("SEMAPRAX source no longer binds total Boolean negation")
    return {
        "schema": SCHEMA,
        "status": "not_matched",
        "reason": "the retained routes prove different Boolean contracts, so no matched semantic timing aggregate exists",
        "routes": {
            "bend_verdict": {
                "source": bend_reference,
                "contract": "boolean_score(value) == Bool.to_u32(value)",
                "process_state": bend["states"],
                "raw_streams": bend["raw_streams"],
            },
            "semaprax_z3": {
                "source": semaprax_reference,
                "contract": "ensures result == !value for app.negate",
                "process_state": semaprax["states"],
                "raw_streams": semaprax["raw_streams"],
            },
        },
        "comparability": {
            "semantic_contract": "unavailable: score-to-u32 and Boolean negation differ",
            "proof_route": "unavailable: Bend retained verdict output and SEMAPRAX installed-Z3 source proof have distinct trusted computing bases",
            "cold_cache": "unavailable: neither process corpus isolates OS, executable, solver, or tool caches",
            "current_head": "unavailable: both are local pinned historical executable observations",
        },
        "nonclaims": [
            "no cross-language timing ratio or winner",
            "no shared theorem, lowering proof, execution proof, or checked-u32 result",
            "no conclusion from similar Boolean input cardinality",
        ],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bend-capsule", type=pathlib.Path, default=ROOT / "evidence/law16-process-state-bend-bool-verdict-v1")
    parser.add_argument("--semaprax-capsule", type=pathlib.Path, default=ROOT / "evidence/law16-process-state-semaprax-bool-z3-v3")
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.output.exists() or not args.output.parent.is_dir():
        parser.error("output must be new beneath an existing directory")
    try:
        result = review(args.bend_capsule, args.semaprax_capsule)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
