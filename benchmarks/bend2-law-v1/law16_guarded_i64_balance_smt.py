#!/usr/bin/env python3
"""Run or authenticate the bounded guarded-i64 balance source-proof route.

This proves five selected source postconditions with pinned local Z3. It does
not authenticate the original full-u32 fixture, prove backend lowering, or
claim a complete LAW16 result.
"""

import argparse
import hashlib
import json
import pathlib
import platform
import shutil
import subprocess
import tempfile


ROOT = pathlib.Path(__file__).resolve().parent
FIXTURE = ROOT / "fixtures/law16-guarded-i64-balance-smt-v1/candidate"
EVIDENCE = ROOT / "evidence/law16-guarded-i64-balance-smt-v1"
SCHEMA = "semaprax.bend2-law-benchmark.guarded-i64-balance-smt.v1"
SOURCE_COMMIT = "5e3720672e441b0202b69b51862058493a1939e9"
SEMAPRAX_SHA256 = "6f6fa6384c8d2bacca4740485bcb6241dc06fc388e3d3dc36a7a38420d720e5a"
Z3_SHA256 = "bbb24b8fed27552f7fb7cfdf0ed9101e6e24fd0cc0b9cfb945233b3f2ed333c5"
Z3_VERSION = "Z3 version 4.12.5 - 64 bit"
MAX_RAW_BYTES = 65536
CASES = (
    ("debit_u32_range", "law16.balance.debit_after", 0),
    ("debit_exact", "law16.balance.debit_after", 1),
    ("debit_positive_transfer", "law16.balance.debit_after", 2),
    ("credit_u32_range", "law16.balance.credit_after", 0),
    ("credit_exact", "law16.balance.credit_after", 1),
    ("credit_conservation", "law16.balance.credit_after", 2),
    ("credit_positive_transfer", "law16.balance.credit_after", 3),
)
NOOP_NAME = "noop_mutant"
NOOP_DIAGNOSTIC = "solver returned false, unknown, malformed or partial evidence"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_digest(path):
    return digest(path.read_bytes())


def raw_ref(path, root):
    data = path.read_bytes()
    if len(data) > MAX_RAW_BYTES:
        raise ValueError(f"raw output exceeds {MAX_RAW_BYTES} bytes")
    return {"path": path.relative_to(root).as_posix(), "bytes": len(data), "sha256": digest(data)}


def verify_raw(root, ref):
    if not isinstance(ref, dict) or set(ref) != {"path", "bytes", "sha256"}:
        raise ValueError("malformed raw-output reference")
    path = (root / ref["path"]).resolve(strict=True)
    if root.resolve() not in path.parents or not path.is_file():
        raise ValueError("raw-output path escapes capsule or is absent")
    data = path.read_bytes()
    if len(data) != ref["bytes"] or digest(data) != ref["sha256"]:
        raise ValueError("raw output size or digest drifted")
    return path


def expected_obligation(document, declaration, index):
    suffix = f"{declaration}:8:ensure:{index}"
    rows = [
        row
        for row in document["project_assurance"]["payload"]["obligations"]
        if row["declaration_id"] == declaration
        and row["kind"] == "postcondition"
        and row["id"].endswith(suffix)
    ]
    if len(rows) != 1:
        raise ValueError("exact selected source postcondition is absent or ambiguous")
    proofs = [method for method in rows[0]["methods"] if method["class"] == "smt_proved"]
    if len(proofs) != 1 or proofs[0]["tool"] != "z3" or proofs[0]["tool_version"] != Z3_VERSION:
        raise ValueError("exact selected postcondition lacks the pinned Z3 discharge")
    return rows[0]


def check_positive(stdout_path, stderr_path, exit_code, declaration, index, expected_source_digest=None):
    if exit_code != 0 or stderr_path.stat().st_size:
        raise ValueError("positive proof command did not exit cleanly")
    document = json.loads(stdout_path.read_text())
    if document.get("schema") != "semaprax.installed-project-proof-check.v1":
        raise ValueError("positive proof output has wrong schema")
    if document.get("application_executed") is not False or document.get("publication_authority") is not False:
        raise ValueError("proof output overclaims execution or publication authority")
    payload = document["project_assurance"]["payload"]
    source = next((row for row in payload["sources"] if row["path"] == "src/app.spx"), None)
    if source is None or (expected_source_digest and source["source_digest"] != expected_source_digest):
        raise ValueError("proof output source digest differs from the pinned selected source")
    obligation = expected_obligation(document, declaration, index)
    proof = next(method for method in obligation["methods"] if method["class"] == "smt_proved")
    if source["source_digest"] not in proof["inputs"]:
        raise ValueError("proof digest inputs do not include exact selected source bytes")
    return document


def run_case(cli, manifest, z3, output_root, case):
    name, declaration, index = case
    stdout = output_root / f"{name}.stdout.json"
    stderr = output_root / f"{name}.stderr.txt"
    command = [
        str(cli), "project-proof-check", str(manifest), "--tool", "z3",
        "--executable", str(z3), "--version-line", Z3_VERSION,
        "--host-profile", "trusted-local", "--source", "src/app.spx",
        "--declaration", declaration, "--ensures", str(index),
    ]
    process = subprocess.run(command, capture_output=True, check=False, timeout=20)
    if len(process.stdout) > MAX_RAW_BYTES or len(process.stderr) > MAX_RAW_BYTES:
        raise ValueError("proof command output exceeds capsule byte bound")
    stdout.write_bytes(process.stdout)
    stderr.write_bytes(process.stderr)
    document = check_positive(stdout, stderr, process.returncode, declaration, index)
    return {
        "name": name,
        "declaration_id": declaration,
        "ensures_index": index,
        "exit_code": process.returncode,
        "obligation_id": next(
            row["id"] for row in document["project_assurance"]["payload"]["obligations"]
            if row["declaration_id"] == declaration and row["kind"] == "postcondition"
            and row["id"].endswith(f"{declaration}:8:ensure:{index}")
        ),
        "source_digest": next(
            row["source_digest"] for row in document["project_assurance"]["payload"]["sources"]
            if row["path"] == "src/app.spx"
        ),
        "status": "smt_proved",
        "stdout": raw_ref(stdout, output_root),
        "stderr": raw_ref(stderr, output_root),
    }


def run_noop(cli, candidate, z3, output_root):
    app = candidate / "src/app.spx"
    source = app.read_text()
    original = "    if amount > debit { debit } else { if credit > 4294967295 - amount { debit } else { debit - amount } }\n}\n\n@id(\"law16.balance.credit_after\")"
    if source.count(original) != 1:
        raise ValueError("no-op mutation anchor drifted")
    app.write_text(source.replace(original, "    debit\n}\n\n@id(\"law16.balance.credit_after\")", 1))
    mutant = output_root / "noop_mutant.app.spx"
    mutant.write_bytes(app.read_bytes())
    command = [
        str(cli), "project-proof-check", str(candidate / "semaprax.toml"),
        "--tool", "z3", "--executable", str(z3), "--version-line", Z3_VERSION,
        "--host-profile", "trusted-local", "--source", "src/app.spx",
        "--declaration", "law16.balance.debit_after", "--ensures", "2",
    ]
    process = subprocess.run(command, capture_output=True, check=False, timeout=20)
    if len(process.stdout) > MAX_RAW_BYTES or len(process.stderr) > MAX_RAW_BYTES:
        raise ValueError("negative-control output exceeds capsule byte bound")
    stdout = output_root / f"{NOOP_NAME}.stdout.json"
    stderr = output_root / f"{NOOP_NAME}.stderr.txt"
    stdout.write_bytes(process.stdout)
    stderr.write_bytes(process.stderr)
    diagnostic = process.stderr.decode("utf-8", errors="replace").strip()
    if process.returncode == 0 or process.stdout or not diagnostic.startswith("SPX-LW140:") or NOOP_DIAGNOSTIC not in diagnostic:
        raise ValueError("no-op control was not refused by the installed proof route")
    return {
        "name": NOOP_NAME,
        "declaration_id": "law16.balance.debit_after",
        "ensures_index": 2,
        "exit_code": process.returncode,
        "status": "proof_tool_refused_no_solver_status_claimed",
        "diagnostic": diagnostic,
        "mutant_source": raw_ref(mutant, output_root),
        "stdout": raw_ref(stdout, output_root),
        "stderr": raw_ref(stderr, output_root),
    }


def verify_capsule(capsule):
    capsule = capsule.resolve(strict=True)
    result = json.loads((capsule / "result.json").read_text())
    if result.get("schema") != SCHEMA or result.get("status") != "bounded_source_postconditions_proved_full_u32_route_incomplete":
        raise ValueError("capsule identity or status drifted")
    if result.get("semaprax", {}).get("source_commit") != SOURCE_COMMIT:
        raise ValueError("compiler source commit pin drifted")
    if result.get("semaprax", {}).get("sha256") != SEMAPRAX_SHA256 or result.get("z3", {}).get("sha256") != Z3_SHA256:
        raise ValueError("executable hash pin drifted")
    build = result.get("build", {})
    if build.get("platform") != "Darwin arm64" or build.get("target_profile") != "dev; debug=0; incremental=0; build_jobs=1":
        raise ValueError("build profile or platform pin drifted")
    for field, path in (
        ("manifest_sha256", FIXTURE / "semaprax.toml"),
        ("app_sha256", FIXTURE / "src/app.spx"),
        ("core_sha256", FIXTURE / "core/core.spx"),
        ("tests_sha256", FIXTURE / "tests/tests.spx"),
    ):
        if result.get("fixture", {}).get(field) != file_digest(path):
            raise ValueError(f"checked-in fixture drifted: {field}")
    if result["fixture"].get("app_bytes") != (FIXTURE / "src/app.spx").stat().st_size:
        raise ValueError("checked-in source byte count drifted")
    raw_root = capsule / "raw"
    if len(result.get("positive_cases", [])) != len(CASES):
        raise ValueError("positive proof coverage is incomplete")
    for expected, recorded in zip(CASES, result["positive_cases"], strict=True):
        name, declaration, index = expected
        if (recorded.get("name"), recorded.get("declaration_id"), recorded.get("ensures_index"), recorded.get("status")) != (name, declaration, index, "smt_proved"):
            raise ValueError("positive proof case identity drifted")
        out, err = verify_raw(raw_root, recorded["stdout"]), verify_raw(raw_root, recorded["stderr"])
        if recorded.get("source_digest") != result["fixture"].get("project_source_digest"):
            raise ValueError("source digest differs across exact selected proof cases")
        check_positive(out, err, recorded["exit_code"], declaration, index, recorded["source_digest"])
    negative = result.get("negative_control", {})
    if negative.get("status") != "proof_tool_refused_no_solver_status_claimed" or negative.get("exit_code") == 0:
        raise ValueError("no-op negative classification drifted")
    out, err = verify_raw(raw_root, negative["stdout"]), verify_raw(raw_root, negative["stderr"])
    mutant = verify_raw(raw_root, negative["mutant_source"])
    original = (FIXTURE / "src/app.spx").read_text()
    anchor = "    if amount > debit { debit } else { if credit > 4294967295 - amount { debit } else { debit - amount } }\n}\n\n@id(\"law16.balance.credit_after\")"
    expected_mutant = original.replace(anchor, "    debit\n}\n\n@id(\"law16.balance.credit_after\")", 1)
    if mutant.read_text() != expected_mutant:
        raise ValueError("retained no-op mutant source differs from the declared attack")
    if out.stat().st_size or NOOP_DIAGNOSTIC not in err.read_text():
        raise ValueError("no-op refusal raw evidence drifted")
    return {
        "schema": SCHEMA + ".review.v1",
        "status": result["status"],
        "positive_smt_discharges": len(CASES),
        "no_op_negative": negative["status"],
        "full_u32_original": "unsupported_by_this_source_profile",
        "overall_law16": "incomplete",
        "raw_streams": 2 * (len(CASES) + 1),
    }


def run(cli, z3, output):
    cli, z3, output = cli.resolve(strict=True), z3.resolve(strict=True), output.resolve()
    if not cli.is_absolute() or not z3.is_absolute() or not output.is_absolute():
        raise ValueError("all executable and output paths must be absolute")
    if file_digest(cli) != SEMAPRAX_SHA256 or file_digest(z3) != Z3_SHA256:
        raise ValueError("executable digest does not match the pinned run")
    version = subprocess.run([str(z3), "--version"], capture_output=True, check=True, timeout=5)
    if version.stdout.decode().strip() != Z3_VERSION or version.stderr:
        raise ValueError("installed Z3 version line drifted")
    repo = ROOT.parent.parent
    subprocess.run(["git", "-C", str(repo), "diff", "--quiet", SOURCE_COMMIT, "--", "src", "crates"], check=True)
    checked_out_head = subprocess.run(["git", "-C", str(repo), "rev-parse", "HEAD"], capture_output=True, check=True, text=True).stdout.strip()
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise ValueError("this evidence pin requires macOS arm64")
    if output.exists() or not output.parent.is_dir():
        raise ValueError("output must be a new directory with an existing parent")
    output.mkdir()
    raw = output / "raw"
    raw.mkdir()
    private_tmp = pathlib.Path("/private/tmp")
    if not private_tmp.is_dir():
        raise ValueError("this pinned capsule requires canonical /private/tmp")
    with tempfile.TemporaryDirectory(prefix="law16-guarded-i64-", dir=private_tmp) as temporary:
        candidate = pathlib.Path(temporary) / "candidate"
        shutil.copytree(FIXTURE, candidate)
        manifest = candidate / "semaprax.toml"
        positive = [run_case(cli, manifest, z3, raw, case) for case in CASES]
        negative = run_noop(cli, candidate, z3, raw)
    source = FIXTURE / "src/app.spx"
    project_source_digest = positive[0]["source_digest"]
    result = {
        "schema": SCHEMA,
        "status": "bounded_source_postconditions_proved_full_u32_route_incomplete",
        "semantic_scope": "guarded i64 projections for full-u32 values, scalar pre/postconditions only",
        "nonclaims": [
            "does not close the original structured full-u32 balance fixture",
            "does not prove source lowering or execute the application",
            "no-op refusal does not identify a solver counterexample versus another refusal status",
            "overall LAW16 remains incomplete",
        ],
        "semaprax": {"source_commit": SOURCE_COMMIT, "sha256": file_digest(cli), "path_at_run": str(cli)},
        "z3": {"sha256": file_digest(z3), "version": Z3_VERSION, "path_at_run": str(z3)},
        "build": {
            "checked_out_head": checked_out_head,
            "target_profile": "dev; debug=0; incremental=0; build_jobs=1",
            "platform": f"{platform.system()} {platform.machine()}",
            "command": "cargo build --locked -p semaprax --bin semaprax",
        },
        "fixture": {
            "manifest_sha256": file_digest(FIXTURE / "semaprax.toml"),
            "app_sha256": file_digest(source),
            "app_bytes": source.stat().st_size,
            "project_source_digest": project_source_digest,
            "core_sha256": file_digest(FIXTURE / "core/core.spx"),
            "tests_sha256": file_digest(FIXTURE / "tests/tests.spx"),
        },
        "positive_cases": positive,
        "negative_control": negative,
        "raw_stream_count": 2 * (len(positive) + 1),
    }
    (output / "result.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    return verify_capsule(output)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--semaprax", type=pathlib.Path)
    parser.add_argument("--z3", type=pathlib.Path)
    parser.add_argument("--output", type=pathlib.Path)
    parser.add_argument("--review", type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        if args.review:
            result = verify_capsule(args.review)
        elif args.semaprax and args.z3 and args.output:
            result = run(args.semaprax, args.z3, args.output)
        else:
            parser.error("provide --review CAPSULE or all of --semaprax, --z3, --output")
    except (OSError, ValueError, json.JSONDecodeError, subprocess.SubprocessError) as error:
        parser.error(str(error))
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
