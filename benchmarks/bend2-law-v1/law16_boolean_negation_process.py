#!/usr/bin/env python3
"""Capture the exact Boolean-negation plan's candidate and mutant process receipts.

This runner only accepts the committed prepared plan.  It copies source/project
fixtures into two paths per lane, runs 30 fresh and 30 repeat child processes,
and preserves stdout/stderr for every child.  It treats attack failure as an
expected observation, never as a failed capture.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import shutil
import statistics
import subprocess
import time

ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("law16_boolean_negation_pair", ROOT / "law16_boolean_negation_pair.py")
PAIR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PAIR)
SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-process-capsule.v1"
SAMPLES = 30
TIMEOUT_SECONDS = 15


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def ref(path, root):
    data = path.read_bytes()
    return {"path": str(path.relative_to(root)), "bytes": len(data), "sha256": digest(data)}


def tool_ref(path):
    data = path.read_bytes()
    return {"path": str(path.resolve()), "bytes": len(data), "sha256": digest(data)}


def copy_file(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)


def copy_project(source, target):
    shutil.copytree(source, target)


def invoke(command, raw, label, ordinal):
    started = time.monotonic_ns()
    try:
        completed = subprocess.run(command, capture_output=True, env=dict(os.environ, BEND_NO_TELEMETRY="1"), timeout=TIMEOUT_SECONDS)
        code, stdout, stderr, timeout = completed.returncode, completed.stdout, completed.stderr, False
    except subprocess.TimeoutExpired as error:
        code, stdout, stderr, timeout = None, error.stdout or b"", error.stderr or b"", True
    elapsed = time.monotonic_ns() - started
    stdout_path, stderr_path = raw / f"{label}-{ordinal:03d}.stdout", raw / f"{label}-{ordinal:03d}.stderr"
    stdout_path.write_bytes(stdout)
    stderr_path.write_bytes(stderr)
    return {"argv": command, "command_sha256": digest(json.dumps(command, separators=(",", ":")).encode()), "elapsed_ns": elapsed, "exit_code": code, "timed_out": timeout, "stdout": ref(stdout_path, raw), "stderr": ref(stderr_path, raw)}


def expectation(sample, lane, kind, raw):
    stdout = (raw / sample["stdout"]["path"]).read_text(errors="replace")
    stderr = (raw / sample["stderr"]["path"]).read_text(errors="replace")
    if sample["timed_out"]:
        return False
    if lane == "bend":
        return sample["exit_code"] == 0 and "ALL PROOFS CHECK" in stdout if kind == "candidate" else sample["exit_code"] != 0 and "SOME PROOFS FAIL" in stderr
    if kind == "candidate":
        if sample["exit_code"] != 0 or stderr:
            return False
        try:
            result = json.loads(stdout)
            obligations = result["project_assurance"]["payload"]["obligations"]
            return any(o.get("declaration_id") == "app.negate" and o.get("kind") == "postcondition" and o.get("classification") == "smt_proved" and any(m.get("tool") == "z3" and m.get("class") == "smt_proved" for m in o.get("methods", [])) for o in obligations)
        except (json.JSONDecodeError, KeyError, TypeError):
            return False
    return sample["exit_code"] != 0 and "SPX-LW140" in stderr


def capture_lane(root, lane, kind, fresh, repeat, command_for, sources):
    raw = root / lane / kind / "raw"
    raw.mkdir(parents=True)
    rows = {}
    for state, input_path in (("fresh_process", fresh), ("repeat_process", repeat)):
        samples = [invoke(command_for(input_path), raw, f"{state}-{kind}", ordinal) for ordinal in range(1, SAMPLES + 1)]
        accepted = [expectation(sample, lane, kind, raw) for sample in samples]
        elapsed = [sample["elapsed_ns"] for sample in samples]
        ordered = sorted(elapsed)
        rows[state] = {"samples": samples, "expected_observation_count": sum(accepted), "summary": {"count": SAMPLES, "p50_ns": statistics.median(elapsed), "p95_ns": ordered[(95 * SAMPLES + 99) // 100 - 1]}}
    receipt = {"schema": SCHEMA, "lane": lane, "kind": kind, "expected": "candidate_acceptance" if kind == "candidate" else "seeded_attack_rejection", "samples_per_state": SAMPLES, "timeout_seconds": TIMEOUT_SECONDS, "sources": sources, "cells": rows, "status": "completed_expected_observation" if all(cell["expected_observation_count"] == SAMPLES for cell in rows.values()) else "failed_expected_observation", "cold_state": {"status":"unavailable", "reason":"fresh paths and child processes do not isolate macOS page, executable, solver, or tool caches"}}
    (root / lane / kind / "receipt.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    return receipt


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bend-root", required=True, type=pathlib.Path)
    parser.add_argument("--bun", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax", required=True, type=pathlib.Path)
    parser.add_argument("--z3", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.output.exists() or not args.output.parent.is_dir(): parser.error("--output must be new beneath an existing directory")
    try: plan_review = PAIR.review(ROOT / "fixtures/boolean-negation-pair-v1.json")
    except (OSError, ValueError, json.JSONDecodeError) as error: parser.error(str(error))
    bend_main = args.bend_root / "bend2/main.ts"
    if not all(path.is_file() for path in (args.bun, args.semaprax, args.z3, bend_main)): parser.error("one or more pinned tools are unavailable")
    args.output.mkdir()
    fixtures = ROOT / "fixtures"
    for lane, kind, source in (("bend", "candidate", fixtures / "bend-boolean-negation-v1.bend"), ("bend", "attack", fixtures / "bend-boolean-negation-law-gaming-v1.bend")):
        fresh, repeat = args.output / lane / kind / "fresh-input.bend", args.output / lane / kind / "repeat-input.bend"
        copy_file(source, fresh); copy_file(source, repeat)
        sources={"fresh_input":ref(fresh,args.output),"repeat_input":ref(repeat,args.output),"bun":tool_ref(args.bun),"bend_main":tool_ref(bend_main)}
        capture_lane(args.output, lane, kind, fresh, repeat, lambda input_path:[str(args.bun),str(bend_main),str(input_path),"--verdict"], sources)
    for kind in ("candidate", "attack"):
        source = fixtures / "boolean-negation-project-v1" / kind
        fresh, repeat = args.output / "semaprax" / kind / "fresh-project", args.output / "semaprax" / kind / "repeat-project"
        copy_project(source, fresh); copy_project(source, repeat)
        sources={"fresh_project_app":ref(fresh/"src/app.spx",args.output),"repeat_project_app":ref(repeat/"src/app.spx",args.output),"fresh_manifest":ref(fresh/"semaprax.toml",args.output),"repeat_manifest":ref(repeat/"semaprax.toml",args.output),"semaprax":tool_ref(args.semaprax),"z3":tool_ref(args.z3)}
        capture_lane(args.output, "semaprax", kind, fresh, repeat, lambda project:[str(args.semaprax),"project-proof-check",str(project/"semaprax.toml"),"--tool","z3","--executable",str(args.z3),"--version-line","Z3 version 4.12.5 - 64 bit","--host-profile","trusted-local","--source","src/app.spx","--declaration","app.negate","--ensures","0"], sources)
    identity={"schema":SCHEMA,"plan_review":plan_review,"tools":{"bend_commit":subprocess.check_output(["git","-C",str(args.bend_root/"bend2"),"rev-parse","HEAD"],text=True).strip(),"bend_main":tool_ref(bend_main),"bun":tool_ref(args.bun),"semaprax":tool_ref(args.semaprax),"z3":tool_ref(args.z3),"z3_version":"Z3 version 4.12.5 - 64 bit"}}
    (args.output / "identity.json").write_text(json.dumps(identity,indent=2,sort_keys=True)+"\n")
    receipts=[json.loads(path.read_text()) for path in sorted(args.output.glob("*/*/receipt.json"))]
    manifest={"schema":SCHEMA,"status":"completed" if all(r["status"]=="completed_expected_observation" for r in receipts) else "failed","receipts":[ref(path,args.output) for path in sorted(args.output.glob("*/*/receipt.json"))],"identity":ref(args.output/"identity.json",args.output),"raw_stream_count":sum(2*SAMPLES*2 for _ in receipts),"nonclaims":["no cross-route timing ratio or winner","Bend verdict and installed-Z3 have distinct trusted computing bases","fresh/repeat is not OS-cache cold","source proof does not prove lowering or execution"]}
    (args.output / "manifest.json").write_text(json.dumps(manifest,indent=2,sort_keys=True)+"\n")
    return 0 if manifest["status"] == "completed" else 1

if __name__ == "__main__": raise SystemExit(main())
