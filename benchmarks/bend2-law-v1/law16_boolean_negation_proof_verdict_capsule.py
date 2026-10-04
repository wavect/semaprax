#!/usr/bin/env python3
"""Validate retained bounded Boolean-negation Bend-verdict/Z3 process evidence."""
import argparse
import hashlib
import importlib.util
import json
import pathlib
import statistics

ROOT = pathlib.Path(__file__).parent
SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-proof-verdict-capsule.v1"
CELL_SCHEMA = "semaprax.bend2-law-benchmark.cold-warm-process-cell.v2"
RESULT_SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-proof-verdict-review.v1"
BEND_COMMIT = "947db722640c86247849343657bf2f7ef01cb7f1"
SEMAPRAX_SHA256 = "sha256:cc9dd3ca99a74dd973cbfb904621e27b801d8ddea6065873d24c14de9dee1d89"
PATH_ERROR = "SPX-J102: Project v1 ancestor /tmp must be a real directory\n"

SPEC = importlib.util.spec_from_file_location("law16_boolean_negation_pair", ROOT / "law16_boolean_negation_pair.py")
PAIR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PAIR)


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path):
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ValueError("JSON document is not an object")
    return value


def reference(root, value):
    if not isinstance(value, dict) or set(value) != {"path", "bytes", "sha256"}:
        raise ValueError("file reference is malformed")
    relative = pathlib.PurePosixPath(value["path"])
    if relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts):
        raise ValueError("file reference path is unsafe")
    path = root / relative
    if not path.is_file() or path.is_symlink() or path.stat().st_size != value["bytes"] or digest(path) != value["sha256"]:
        raise ValueError("file reference identity drifted")
    return path


def raw(root, value):
    return reference(root, value)


def summary(samples):
    elapsed = [sample["elapsed_ns"] for sample in samples]
    return {"count": 30, "p50_ns": statistics.median(elapsed), "p95_ns": sorted(elapsed)[28]}


def expected_source(plan, lane):
    return plan["sources"][f"{lane}_candidate"]["sha256"]


def successful_route(root, label, lane, plan):
    receipt = read_json(root / f"{label}.json")
    if receipt.get("schema") != CELL_SCHEMA or receipt.get("configuration", {}).get("timeout_seconds") != 120:
        raise ValueError(f"{label} receipt schema or timeout drifted")
    artifacts = receipt.get("artifacts")
    if not isinstance(artifacts, list) or len(artifacts) < 2:
        raise ValueError(f"{label} lacks source artifacts")
    for source in artifacts[:2]:
        if source.get("sha256") != expected_source(plan, lane):
            raise ValueError(f"{label} source differs from canonical candidate")
    routes = {}
    for state in ("fresh_process", "repeat_process"):
        cell = receipt.get("cells", {}).get(state)
        samples = cell.get("samples") if isinstance(cell, dict) else None
        if cell.get("status") != "completed" or not isinstance(samples, list) or len(samples) != 30:
            raise ValueError(f"{label} {state} is not a completed 30-sample cell")
        for item in samples:
            argv = item.get("argv")
            if item.get("outcome") != "completed" or item.get("exit_code") != 0 or item.get("timeout_seconds") != 120 or not isinstance(argv, list):
                raise ValueError(f"{label} {state} sample did not complete within the fixed limit")
            if label == "bend-verdict" and "--verdict" not in argv:
                raise ValueError("Bend route omitted --verdict")
            if label == "semaprax-z3" and not (len(argv) > 2 and argv[1] == "project-proof-check" and "z3" in argv):
                raise ValueError("SEMAPRAX route is not the retained Z3 proof command")
            raw(root / f"{label}-raw", item.get("stdout"))
            raw(root / f"{label}-raw", item.get("stderr"))
        expected = summary(samples)
        if cell.get("summary") != expected:
            raise ValueError(f"{label} {state} summary differs from raw samples")
        routes[state] = expected
    return routes


def nonresult(root, plan):
    receipt = read_json(root / "semaprax-z3.json")
    if receipt.get("schema") != CELL_SCHEMA or receipt.get("configuration", {}).get("timeout_seconds") != 120:
        raise ValueError("path nonresult receipt drifted")
    if any(source.get("sha256") != expected_source(plan, "semaprax") for source in receipt.get("artifacts", [])[:2]):
        raise ValueError("path nonresult source drifted")
    count = 0
    for state in ("fresh_process", "repeat_process"):
        cell = receipt.get("cells", {}).get(state)
        samples = cell.get("samples") if isinstance(cell, dict) else None
        if cell.get("status") != "unavailable" or not isinstance(samples, list) or len(samples) != 30:
            raise ValueError("path nonresult lacks both failed 30-sample cells")
        for item in samples:
            if item.get("outcome") != "completed" or item.get("exit_code") != 1 or item.get("timeout_seconds") != 120:
                raise ValueError("path nonresult is not the retained exit-one admission failure")
            if raw(root / "semaprax-z3-raw", item.get("stderr")).read_bytes() != PATH_ERROR.encode():
                raise ValueError("path nonresult diagnostic drifted")
            count += 1
    return {"samples": count, "error": PATH_ERROR.rstrip()}


def review(capsule):
    capsule = capsule.resolve(strict=True)
    plan = PAIR.review(ROOT / "fixtures/boolean-negation-pair-v1.json")
    identity = read_json(capsule / "identity.json")
    if identity.get("schema") != SCHEMA or identity.get("bend_commit") != BEND_COMMIT or identity.get("semaprax_sha256") != SEMAPRAX_SHA256:
        raise ValueError("capsule identity drifted")
    for value in identity.get("successful", {}).values():
        reference(capsule, value)
    setup = reference(capsule, identity.get("setup_nonresult"))
    if read_json(setup).get("status") != "infrastructure_nonresult":
        raise ValueError("setup nonresult drifted")
    failed = identity.get("path_admission_nonresult", {})
    reference(capsule, failed.get("receipt"))
    if failed.get("exact_error") != PATH_ERROR.rstrip():
        raise ValueError("path nonresult identity drifted")
    routes = {
        "bend_verdict": successful_route(capsule / "successful", "bend-verdict", "bend", plan),
        "semaprax_z3": successful_route(capsule / "successful", "semaprax-z3", "semaprax", plan),
    }
    return {
        "schema": RESULT_SCHEMA,
        "status": "local_matched_proof_verdict_routes_authenticated",
        "routes": routes,
        "path_admission_nonresult": nonresult(capsule / "path-admission-nonresult", plan),
        "nonclaims": identity["nonclaims"],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capsule", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.output.exists() or not args.output.parent.is_dir():
        parser.error("output must be new below an existing directory")
    try:
        document = review(args.capsule)
    except (OSError, ValueError, json.JSONDecodeError, TypeError) as error:
        parser.error(str(error))
    args.output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
