#!/usr/bin/env python3
"""Capture and authenticate one matched LAW16 scalar-Boolean refactor cell.

The candidate changes the implementation shape while retaining total Boolean
negation.  The attack retains the same law declaration or source contract but
changes the branch results.  This narrow cell is separate from the unadmitted
checked-u32 fixtures and records only locally observed tool outcomes.
"""

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess


ROOT = pathlib.Path(__file__).resolve().parent
FIXTURE = ROOT / "fixtures/law16-boolean-refactor-v1"
BASE_BEND = ROOT / "fixtures/bend-boolean-negation-v1.bend"
BASE_SEMAPRAX = ROOT / "fixtures/semaprax-boolean-negation-v1.spx"
U32_SUCCESS = ROOT / "fixtures/semaprax-checked-u32-success-v1.spx"
U32_OVERFLOW = ROOT / "fixtures/semaprax-checked-u32-overflow-v1.spx"
SCHEMA = "semaprax.bend2-law-benchmark.boolean-refactor-cell.v1"
BEND_COMMIT = "947db722640c86247849343657bf2f7ef01cb7f1"
BUN_SHA256 = "abe991b29c5151ab11b5344be65dee3a675b0a4a55b8fc493e3cfbe256e61781"
SEMAPRAX_SHA256 = "cc9dd3ca99a74dd973cbfb904621e27b801d8ddea6065873d24c14de9dee1d89"
Z3_SHA256 = "bbb24b8fed27552f7fb7cfdf0ed9101e6e24fd0cc0b9cfb945233b3f2ed333c5"
Z3_VERSION = "Z3 version 4.12.5 - 64 bit"
MAX_RAW_BYTES = 65536


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def ref(path, root):
    data = path.read_bytes()
    return {"path": path.relative_to(root).as_posix(), "bytes": len(data), "sha256": sha256(data)}


def tool_ref(path):
    data = path.read_bytes()
    return {"path": str(path.resolve()), "bytes": len(data), "sha256": sha256(data)}


def checked_ref(root, value):
    if not isinstance(value, dict) or set(value) != {"path", "bytes", "sha256"}:
        raise ValueError("malformed retained-file reference")
    rel = pathlib.PurePosixPath(value["path"])
    if rel.is_absolute() or any(part in {"", ".", ".."} for part in rel.parts):
        raise ValueError("retained-file reference escapes the capsule")
    path = (root / rel).resolve(strict=True)
    if root.resolve() not in path.parents or not path.is_file():
        raise ValueError("retained-file reference is absent")
    if path.stat().st_size != value["bytes"] or sha256(path.read_bytes()) != value["sha256"]:
        raise ValueError("retained-file identity drifted")
    return path


def copy_inputs(output):
    inputs = output / "inputs"
    inputs.mkdir()
    shutil.copyfile(FIXTURE / "bend-candidate.bend", inputs / "bend-candidate.bend")
    shutil.copyfile(FIXTURE / "bend-attack.bend", inputs / "bend-attack.bend")
    shutil.copytree(FIXTURE / "semaprax-candidate", inputs / "semaprax-candidate")
    shutil.copytree(FIXTURE / "semaprax-attack", inputs / "semaprax-attack")
    return inputs


def run(output, label, argv, env=None):
    try:
        completed = subprocess.run(argv, capture_output=True, timeout=30, check=False, env=env)
    except subprocess.TimeoutExpired as error:
        raise ValueError(f"{label} timed out") from error
    if len(completed.stdout) > MAX_RAW_BYTES or len(completed.stderr) > MAX_RAW_BYTES:
        raise ValueError(f"{label} raw output exceeds {MAX_RAW_BYTES} bytes")
    raw = output / "raw"
    stdout, stderr = raw / f"{label}.stdout", raw / f"{label}.stderr"
    stdout.write_bytes(completed.stdout)
    stderr.write_bytes(completed.stderr)
    return {"argv": [str(item) for item in argv], "exit_code": completed.returncode, "stdout": ref(stdout, output), "stderr": ref(stderr, output)}


def positive_z3(output, row):
    if row["exit_code"] != 0 or checked_ref(output, row["stderr"]).read_bytes():
        return False
    try:
        document = json.loads(checked_ref(output, row["stdout"]).read_text())
        obligations = document["project_assurance"]["payload"]["obligations"]
    except (json.JSONDecodeError, KeyError, TypeError):
        return False
    return any(
        item.get("declaration_id") == "app.negate"
        and item.get("kind") == "postcondition"
        and item.get("classification") == "smt_proved"
        and any(method.get("class") == "smt_proved" and method.get("tool") == "z3" and method.get("tool_version") == Z3_VERSION for method in item.get("methods", []))
        for item in obligations
    )


def expected(output, routes):
    text = lambda name, stream: checked_ref(output, routes[name][stream]).read_text(errors="replace")
    return {
        "bend_candidate_ordinary": routes["bend_candidate_ordinary"]["exit_code"] == 0 and text("bend_candidate_ordinary", "stdout") == "True\nFalse\n",
        "bend_candidate_verdict": routes["bend_candidate_verdict"]["exit_code"] == 0 and "ALL PROOFS CHECK" in text("bend_candidate_verdict", "stdout"),
        "bend_attack_ordinary": routes["bend_attack_ordinary"]["exit_code"] != 0 and "SOME PROOFS FAIL" in text("bend_attack_ordinary", "stderr"),
        "bend_attack_verdict": routes["bend_attack_verdict"]["exit_code"] != 0 and "SOME PROOFS FAIL" in text("bend_attack_verdict", "stderr"),
        "semaprax_candidate_check": routes["semaprax_candidate_check"]["exit_code"] == 0 and not text("semaprax_candidate_check", "stderr"),
        "semaprax_attack_check": routes["semaprax_attack_check"]["exit_code"] == 0 and not text("semaprax_attack_check", "stderr"),
        "semaprax_candidate_z3": positive_z3(output, routes["semaprax_candidate_z3"]),
        "semaprax_attack_z3": routes["semaprax_attack_z3"]["exit_code"] != 0 and text("semaprax_attack_z3", "stderr").startswith("SPX-LW140:"),
    }


def inventory(output, inputs):
    paths = [BASE_BEND, BASE_SEMAPRAX, U32_SUCCESS, U32_OVERFLOW]
    return {
        "schema": SCHEMA + ".law-inventory.v1",
        "semantic_contract": {
            "name": "total Boolean negation",
            "domain": "Bool / bool; exactly two values",
            "truth_table": [{"input": False, "output": True}, {"input": True, "output": False}],
            "relation": "result == Bool.not(value) / !value",
        },
        "candidate": {"bend": ref(inputs / "bend-candidate.bend", output), "semaprax_app": ref(inputs / "semaprax-candidate/src/app.spx", output)},
        "attack": {"bend": ref(inputs / "bend-attack.bend", output), "semaprax_app": ref(inputs / "semaprax-attack/src/app.spx", output)},
        "baseline_direct_implementation": [{"path": str(path.relative_to(ROOT)), "bytes": path.stat().st_size, "sha256": sha256(path.read_bytes())} for path in paths[:2]],
        "checked_u32_nonadmission_control": [{"path": str(path.relative_to(ROOT)), "bytes": path.stat().st_size, "sha256": sha256(path.read_bytes())} for path in paths[2:]],
        "nonclaims": [
            "the checked-u32 fixtures are retained controls and are not admitted by this Boolean cell",
            "SEMAPRAX check accepts the source-contract attack and is not the Z3 discharge route",
            "Bend verdict and installed Z3 use distinct trusted computing bases",
            "source proof and verdict do not prove lowering or backend execution equivalence",
        ],
    }


def check_tools(bend_root, bun, semaprax, z3):
    bend_main = bend_root / "bend2/main.ts"
    if not all(path.is_file() for path in (bend_main, bun, semaprax, z3)):
        raise ValueError("a required pinned tool is unavailable")
    if subprocess.check_output(["git", "-C", str(bend_root / "bend2"), "rev-parse", "HEAD"], text=True).strip() != BEND_COMMIT:
        raise ValueError("Bend source commit differs from the required pin")
    expected_hashes = ((bun, BUN_SHA256), (semaprax, SEMAPRAX_SHA256), (z3, Z3_SHA256))
    if any(sha256(path.read_bytes()) != expected for path, expected in expected_hashes):
        raise ValueError("one or more executable hashes differ from the required pins")
    if subprocess.check_output([str(z3), "--version"], text=True).strip() != Z3_VERSION:
        raise ValueError("Z3 version differs from the required pin")
    return bend_main


def capture(bend_root, bun, semaprax, z3, output):
    if output.exists() or not output.parent.is_dir():
        raise ValueError("output must be new beneath an existing directory")
    output = output.resolve()
    bend_main = check_tools(bend_root, bun, semaprax, z3)
    output.mkdir()
    (output / "raw").mkdir()
    inputs = copy_inputs(output)
    bend_env = dict(os.environ, BEND_NO_TELEMETRY="1")
    bend = lambda source, verdict=False: [str(bun), str(bend_main), str(source)] + (["--verdict"] if verdict else [])
    proof = lambda project: [str(semaprax), "project-proof-check", str(project / "semaprax.toml"), "--tool", "z3", "--executable", str(z3), "--version-line", Z3_VERSION, "--host-profile", "trusted-local", "--source", "src/app.spx", "--declaration", "app.negate", "--ensures", "0"]
    routes = {
        "bend_candidate_ordinary": run(output, "bend-candidate-ordinary", bend(inputs / "bend-candidate.bend"), bend_env),
        "bend_candidate_verdict": run(output, "bend-candidate-verdict", bend(inputs / "bend-candidate.bend", True), bend_env),
        "bend_attack_ordinary": run(output, "bend-attack-ordinary", bend(inputs / "bend-attack.bend"), bend_env),
        "bend_attack_verdict": run(output, "bend-attack-verdict", bend(inputs / "bend-attack.bend", True), bend_env),
        "semaprax_candidate_check": run(output, "semaprax-candidate-check", [str(semaprax), "check", str(inputs / "semaprax-candidate/src/app.spx")]),
        "semaprax_attack_check": run(output, "semaprax-attack-check", [str(semaprax), "check", str(inputs / "semaprax-attack/src/app.spx")]),
        "semaprax_candidate_z3": run(output, "semaprax-candidate-z3", proof(inputs / "semaprax-candidate")),
        "semaprax_attack_z3": run(output, "semaprax-attack-z3", proof(inputs / "semaprax-attack")),
    }
    law_inventory = inventory(output, inputs)
    (output / "law-inventory.json").write_text(json.dumps(law_inventory, indent=2, sort_keys=True) + "\n")
    observations = expected(output, routes)
    result = {
        "schema": SCHEMA,
        "status": "completed_local_matched_boolean_refactor" if all(observations.values()) else "unexpected_observation",
        "tools": {"bend_commit": BEND_COMMIT, "bend_main": tool_ref(bend_main), "bun": tool_ref(bun), "semaprax": tool_ref(semaprax), "z3": tool_ref(z3), "z3_version": Z3_VERSION},
        "law_inventory": ref(output / "law-inventory.json", output),
        "routes": routes,
        "observations": observations,
        "raw_streams": 16,
        "nonclaims": law_inventory["nonclaims"],
    }
    (output / "result.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    if result["status"] != "completed_local_matched_boolean_refactor":
        raise ValueError("one or more expected observations did not occur")


def verify(capsule):
    capsule = capsule.resolve(strict=True)
    result = json.loads((capsule / "result.json").read_text())
    if result.get("schema") != SCHEMA or result.get("status") != "completed_local_matched_boolean_refactor":
        raise ValueError("capsule identity or status drifted")
    tools = result.get("tools", {})
    if tools.get("bend_commit") != BEND_COMMIT or tools.get("bun", {}).get("sha256") != BUN_SHA256 or tools.get("semaprax", {}).get("sha256") != SEMAPRAX_SHA256 or tools.get("z3", {}).get("sha256") != Z3_SHA256 or tools.get("z3_version") != Z3_VERSION:
        raise ValueError("tool pins drifted")
    inventory_path = checked_ref(capsule, result.get("law_inventory"))
    law_inventory = json.loads(inventory_path.read_text())
    if law_inventory.get("schema") != SCHEMA + ".law-inventory.v1":
        raise ValueError("law inventory identity drifted")
    for lane in ("candidate", "attack"):
        for item in law_inventory.get(lane, {}).values():
            checked_ref(capsule, item)
    for item in law_inventory.get("baseline_direct_implementation", []) + law_inventory.get("checked_u32_nonadmission_control", []):
        path = ROOT / item["path"]
        if not path.is_file() or path.stat().st_size != item["bytes"] or sha256(path.read_bytes()) != item["sha256"]:
            raise ValueError("fixture control identity drifted")
    routes = result.get("routes")
    if not isinstance(routes, dict) or set(routes) != {"bend_candidate_ordinary", "bend_candidate_verdict", "bend_attack_ordinary", "bend_attack_verdict", "semaprax_candidate_check", "semaprax_attack_check", "semaprax_candidate_z3", "semaprax_attack_z3"}:
        raise ValueError("route inventory drifted")
    for row in routes.values():
        checked_ref(capsule, row.get("stdout")); checked_ref(capsule, row.get("stderr"))
    observations = expected(capsule, routes)
    if observations != result.get("observations") or not all(observations.values()) or result.get("raw_streams") != 16:
        raise ValueError("route observations or raw inventory drifted")
    return {"schema": SCHEMA + ".review.v1", "status": result["status"], "routes": len(routes), "raw_streams": result["raw_streams"], "checked_u32": "not_admitted_by_this_boolean_cell"}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(required=True, dest="cmd")
    capture_parser = sub.add_parser("capture")
    capture_parser.add_argument("--bend-root", required=True, type=pathlib.Path)
    capture_parser.add_argument("--bun", required=True, type=pathlib.Path)
    capture_parser.add_argument("--semaprax", required=True, type=pathlib.Path)
    capture_parser.add_argument("--z3", required=True, type=pathlib.Path)
    capture_parser.add_argument("--output", required=True, type=pathlib.Path)
    review_parser = sub.add_parser("review")
    review_parser.add_argument("--capsule", required=True, type=pathlib.Path)
    review_parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        if args.cmd == "capture":
            capture(args.bend_root, args.bun, args.semaprax, args.z3, args.output)
        else:
            if args.output.exists() or not args.output.parent.is_dir():
                raise ValueError("output must be new beneath an existing directory")
            args.output.write_text(json.dumps(verify(args.capsule), indent=2, sort_keys=True) + "\n")
    except (OSError, ValueError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
