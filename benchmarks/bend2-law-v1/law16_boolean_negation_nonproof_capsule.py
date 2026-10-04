#!/usr/bin/env python3
"""Offline validator for Boolean-negation ordinary/check process receipts."""
import argparse
import hashlib
import importlib.util
import json
import pathlib
import statistics

ROOT = pathlib.Path(__file__).parent
SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-nonproof-process.v1"
CELL_SCHEMA = "semaprax.bend2-law-benchmark.cold-warm-process-cell.v2"
RESULT_SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-nonproof-process-review.v1"
BEND_COMMIT = "947db722640c86247849343657bf2f7ef01cb7f1"
SEMAPRAX_SHA256 = "sha256:cc9dd3ca99a74dd973cbfb904621e27b801d8ddea6065873d24c14de9dee1d89"

SPEC = importlib.util.spec_from_file_location("law16_boolean_negation_pair", ROOT / "law16_boolean_negation_pair.py")
PAIR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PAIR)


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def same_recorded_path(left, right):
    if not isinstance(left, str) or not isinstance(right, str):
        return False
    if left == right:
        return True
    # macOS records /tmp and /private/tmp for the same path. Compare the
    # retained names without requiring the original temporary files to exist.
    return ((left.startswith("/tmp/") and "/private" + left == right)
            or (right.startswith("/tmp/") and "/private" + right == left))


def ref(root, value):
    if not isinstance(value, dict) or set(value) != {"path", "bytes", "sha256"}:
        raise ValueError("raw reference is malformed")
    relative = pathlib.PurePosixPath(value["path"])
    if relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts):
        raise ValueError("raw reference path is unsafe")
    path = root / relative
    if not path.is_file() or path.is_symlink() or path.stat().st_size != value["bytes"] or digest(path) != value["sha256"]:
        raise ValueError("raw reference identity drifted")
    return path


def read_json(path):
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ValueError("JSON document is not an object")
    return value


def review(root):
    root = root.resolve(strict=True)
    plan = PAIR.review(ROOT / "fixtures/boolean-negation-pair-v1.json")
    identity = read_json(root / "identity.json")
    if identity.get("schema") != SCHEMA or identity.get("status") != "completed_local_process_provisioning":
        raise ValueError("identity status drifted")
    if identity.get("sources") != {"bend": plan["sources"]["bend_candidate"]["sha256"], "semaprax": plan["sources"]["semaprax_candidate"]["sha256"]}:
        raise ValueError("identity sources differ from canonical pair")
    tools = identity.get("tools", {})
    if tools.get("bend_commit") != BEND_COMMIT or tools.get("semaprax", {}).get("sha256") != SEMAPRAX_SHA256:
        raise ValueError("pinned tool identity drifted")

    expected = {"bend-ordinary": "bend", "semaprax-check": "semaprax"}
    routes = {}
    for label, lane in expected.items():
        receipt = read_json(root / f"{label}.json")
        if receipt.get("schema") != CELL_SCHEMA or receipt.get("cold_state", {}).get("status") != "unavailable":
            raise ValueError(f"{label} receipt schema or cold state drifted")
        artifacts = receipt.get("artifacts")
        if not isinstance(artifacts, list) or len(artifacts) < 2:
            raise ValueError(f"{label} has no input artifacts")
        wanted = plan["sources"][f"{lane}_candidate"]["sha256"]
        for input_ref in artifacts[:2]:
            if input_ref.get("sha256") != wanted or not isinstance(input_ref.get("path"), str):
                raise ValueError(f"{label} input differs from canonical candidate")
        summaries = {}
        for state, input_ref in zip(("fresh_process", "repeat_process"), artifacts[:2]):
            cell = receipt.get("cells", {}).get(state)
            samples = cell.get("samples") if isinstance(cell, dict) else None
            if cell.get("status") != "completed" or not isinstance(samples, list) or len(samples) != 30:
                raise ValueError(f"{label} {state} lacks thirty successful samples")
            elapsed = []
            for sample in samples:
                argv = sample.get("argv")
                if (sample.get("exit_code") != 0 or not isinstance(argv, list) or not argv
                        or not same_recorded_path(argv[-1], input_ref["path"])):
                    raise ValueError(f"{label} command or exit code drifted")
                if label == "bend-ordinary" and (len(argv) != 3 or "--verdict" in argv):
                    raise ValueError("ordinary Bend receipt is not an ordinary three-argument invocation")
                if label == "semaprax-check" and (len(argv) != 3 or argv[1] != "check"):
                    raise ValueError("SEMAPRAX receipt is not the nonproof check command")
                if label == "bend-ordinary" and not (same_recorded_path(argv[0], tools["bun"]["path"])
                                                             and same_recorded_path(argv[1], tools["bend_main"]["path"])):
                    raise ValueError("ordinary Bend tool path drifted")
                if label == "semaprax-check" and not same_recorded_path(argv[0], tools["semaprax"]["path"]):
                    raise ValueError("SEMAPRAX tool path drifted")
                ref(root / f"{label}-raw", sample.get("stdout"))
                ref(root / f"{label}-raw", sample.get("stderr"))
                if not isinstance(sample.get("elapsed_ns"), int):
                    raise ValueError(f"{label} sample elapsed time is malformed")
                elapsed.append(sample["elapsed_ns"])
            summary = {"count": 30, "p50_ns": statistics.median(elapsed), "p95_ns": sorted(elapsed)[28]}
            if cell.get("summary") != summary:
                raise ValueError(f"{label} {state} summary drifted")
            summaries[state] = summary
        routes[label] = summaries
    return {"schema": RESULT_SCHEMA, "status": "local_matched_nonproof_routes_authenticated", "routes": routes, "nonclaims": identity["nonclaims"]}


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
