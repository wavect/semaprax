#!/usr/bin/env python3
"""Offline validator for the equal-semantics Boolean-negation process capsule."""
import argparse
import hashlib
import importlib.util
import json
import pathlib
import stat
import statistics

ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("law16_boolean_negation_pair", ROOT / "law16_boolean_negation_pair.py")
PAIR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PAIR)
SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-process-capsule.v1"
RESULT_SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-process-capsule-review.v1"
MAX_STREAM = 1024 * 1024


def digest(data): return "sha256:" + hashlib.sha256(data).hexdigest()


def read_json(path, label):
    try: value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error: raise ValueError(f"cannot read {label}") from error
    if not isinstance(value, dict): raise ValueError(f"{label} is not an object")
    return value


def relative_file(root, reference, maximum=MAX_STREAM):
    if not isinstance(reference, dict) or set(reference) != {"path", "bytes", "sha256"}: raise ValueError("malformed file reference")
    name, count, expected = reference["path"], reference["bytes"], reference["sha256"]
    relative = pathlib.PurePosixPath(name)
    if not isinstance(name, str) or relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts) or not isinstance(count, int) or not 0 <= count <= maximum or not isinstance(expected, str): raise ValueError("unsafe file reference")
    path = root
    try:
        for part in relative.parts:
            path /= part
            mode = path.lstat().st_mode
            if stat.S_ISLNK(mode): raise ValueError("file reference crosses symlink")
        if not stat.S_ISREG(mode) or path.stat().st_size != count: raise ValueError("file reference byte count drifted")
        if digest(path.read_bytes()) != expected: raise ValueError("file reference digest drifted")
    except OSError as error: raise ValueError("file reference unavailable") from error
    return path


def raw(root, reference): return relative_file(root / "raw", reference)


def review(capsule):
    capsule = capsule.resolve(strict=True)
    plan = PAIR.review(ROOT / "fixtures/boolean-negation-pair-v1.json")
    manifest = read_json(capsule / "manifest.json", "manifest")
    if manifest.get("schema") != SCHEMA or manifest.get("status") != "completed" or manifest.get("raw_stream_count") != 480: raise ValueError("capsule manifest identity or status drifted")
    relative_file(capsule, manifest.get("identity"), MAX_STREAM)
    expected = {(lane, kind) for lane in ("bend", "semaprax") for kind in ("candidate", "attack")}
    references = manifest.get("receipts")
    if not isinstance(references, list) or len(references) != 4: raise ValueError("capsule lacks four lane receipts")
    actual, summaries = set(), {}
    plan_sources = plan["sources"]
    expected_source = {("bend", "candidate"): plan_sources["bend_candidate"], ("bend", "attack"): plan_sources["bend_attack"], ("semaprax", "candidate"): plan_sources["semaprax_candidate"], ("semaprax", "attack"): plan_sources["semaprax_attack"]}
    for reference in references:
        receipt_path = relative_file(capsule, reference)
        receipt = read_json(receipt_path, "lane receipt")
        lane, kind = receipt.get("lane"), receipt.get("kind")
        actual.add((lane, kind))
        if receipt.get("schema") != SCHEMA or receipt.get("status") != "completed_expected_observation" or receipt.get("samples_per_state") != 30 or receipt.get("timeout_seconds") != 15: raise ValueError("lane receipt status or budget drifted")
        source = receipt.get("sources", {})
        source_refs = (source.get("fresh_input"), source.get("repeat_input")) if lane == "bend" else (source.get("fresh_project_app"), source.get("repeat_project_app"))
        for observed in source_refs:
            if not isinstance(observed, dict) or observed.get("bytes") != expected_source[(lane,kind)]["bytes"] or observed.get("sha256") != expected_source[(lane,kind)]["sha256"]: raise ValueError("lane source differs from matched plan")
            relative_file(capsule, observed)
        if lane == "semaprax":
            for manifest_ref in (source.get("fresh_manifest"), source.get("repeat_manifest")):
                relative_file(capsule, manifest_ref)
        for state in ("fresh_process", "repeat_process"):
            cell = receipt.get("cells", {}).get(state)
            if not isinstance(cell, dict) or cell.get("expected_observation_count") != 30: raise ValueError("route did not observe all expected outcomes")
            samples = cell.get("samples")
            if not isinstance(samples, list) or len(samples) != 30: raise ValueError("route sample count drifted")
            elapsed=[]
            for sample in samples:
                if not isinstance(sample,dict) or sample.get("timed_out") or not isinstance(sample.get("elapsed_ns"),int): raise ValueError("timed route contains timeout or malformed sample")
                raw(capsule / lane / kind, sample.get("stdout")); raw(capsule / lane / kind, sample.get("stderr")); elapsed.append(sample["elapsed_ns"])
                exit_code=sample.get("exit_code")
                if kind == "candidate" and exit_code != 0: raise ValueError("candidate route did not exit successfully")
                if kind == "attack" and exit_code == 0: raise ValueError("seeded attack unexpectedly exited successfully")
            ordered=sorted(elapsed)
            summary={"count":30,"p50_ns":statistics.median(elapsed),"p95_ns":ordered[(95*30+99)//100-1]}
            if cell.get("summary") != summary: raise ValueError("route timing summary differs from raw samples")
            summaries[f"{lane}_{kind}_{state}"]=summary
    if actual != expected: raise ValueError("capsule lane coverage drifted")
    return {"schema":RESULT_SCHEMA,"status":"matched_semantics_local_routes_authenticated","semantic_contract":plan["semantic_contract"],"observations":{"bend_verdict_candidate_acceptances":60,"bend_verdict_seeded_attack_rejections":60,"semaprax_z3_candidate_postcondition_discharges":60,"semaprax_z3_seeded_attack_rejections":60,"raw_streams":480},"process_states":summaries,"comparability":{"semantic_contract":"accepted: both lanes bind exact total Boolean negation and the two-value truth table","negative_controls":"accepted: both assertion-retaining attacks were rejected in all 60 child processes","cross_route_timing":"not_reported: separate trusted computing bases and process-provisioning observations; no ratio or winner","cold_cache":"unavailable: fresh/repeat paths do not isolate OS, executable, solver, or tool caches"},"nonclaims":["Bend verdict is not an independently replayed proof system","installed-Z3 source proof does not prove lowering or execution","no checked-u32, general theorem, timing ratio, or winner result","local pinned evidence is not current-head evidence"]}


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument("--capsule",required=True,type=pathlib.Path); parser.add_argument("--output",required=True,type=pathlib.Path); args=parser.parse_args(argv)
    if args.output.exists() or not args.output.parent.is_dir(): parser.error("output must be new beneath an existing directory")
    try: result=review(args.capsule)
    except (OSError,ValueError,json.JSONDecodeError) as error: parser.error(str(error))
    args.output.write_text(json.dumps(result,indent=2,sort_keys=True)+"\n")

if __name__ == "__main__": main()
