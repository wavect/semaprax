#!/usr/bin/env python3
"""Authenticate the prepared, equal-semantics Boolean-negation evidence plan.

This is deliberately static.  It makes a future 30+30 corpus reject source,
truth-table, contract, and attack drift before an operator invokes either CLI.
It does not turn the plan or a prior Bend-only preflight into a comparison.
"""
import argparse
import hashlib
import json
import pathlib

ROOT = pathlib.Path(__file__).parent
SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-pair-plan.v1"
REVIEW_SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-pair-review.v1"


def sha256(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def load_json(path):
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ValueError("plan must be a JSON object")
    return value


def verified_source(plan, name):
    reference = plan["sources"].get(name)
    if not isinstance(reference, dict):
        raise ValueError(f"missing source reference: {name}")
    relative = reference.get("path")
    if not isinstance(relative, str) or not relative.startswith("fixtures/") or ".." in pathlib.PurePosixPath(relative).parts:
        raise ValueError(f"unsafe fixture reference: {name}")
    path = ROOT / relative
    data = path.read_bytes()
    if reference != {"path": relative, "bytes": len(data), "sha256": sha256(data)}:
        raise ValueError(f"fixture identity drifted: {name}")
    return data.decode(), reference


def require_all(text, fragments, name):
    if any(fragment not in text for fragment in fragments):
        raise ValueError(f"{name} no longer has its admitted contract shape")


def review(plan_path):
    plan = load_json(plan_path)
    if plan.get("schema") != SCHEMA or plan.get("id") != "law16-boolean-negation-pair-v1":
        raise ValueError("unexpected Boolean-negation plan identity")
    if plan.get("status") != "prepared_not_executed":
        raise ValueError("an execution result must not overwrite this static plan")
    contract = plan.get("semantic_contract")
    if contract != {
        "name": "total Boolean negation",
        "domain": "Bool / bool; exactly two values",
        "truth_table": [{"input": False, "output": True}, {"input": True, "output": False}],
        "relation": "result == !value / Bool.not(value)",
    }:
        raise ValueError("Boolean domain or truth table drifted")
    bend, bend_ref = verified_source(plan, "bend_candidate")
    bend_attack, bend_attack_ref = verified_source(plan, "bend_attack")
    sem, sem_ref = verified_source(plan, "semaprax_candidate")
    sem_attack, sem_attack_ref = verified_source(plan, "semaprax_attack")
    candidate_project, candidate_project_ref = verified_source(plan, "semaprax_candidate_project")
    attack_project, attack_project_ref = verified_source(plan, "semaprax_attack_project")
    for variant in ("candidate", "attack"):
        for leaf in ("core_core_spx", "tests_tests_spx"):
            verified_source(plan, f"semaprax_{variant}_{leaf}")
    require_all(bend, ["def negate(value: Bool) -> Bool:", "  Bool.not(value)", "law negate_matches_boolean_not:", "{negate(value) == Bool.not(value) : Bool}", "  {==}"], "Bend candidate")
    require_all(bend_attack, ["def negate(value: Bool) -> Bool:", "  value\n", "law negate_matches_boolean_not:", "{negate(value) == Bool.not(value) : Bool}", "  {==}"], "Bend attack")
    if "Bool.not(value)\n\nlaw" in bend_attack:
        raise ValueError("Bend attack accidentally repairs the body")
    require_all(sem, ["@id(\"app.negate\")", "fn negate(value: bool) -> bool", "ensures result == !value", "    !value"], "SEMAPRAX candidate")
    require_all(sem_attack, ["@id(\"app.negate\")", "fn negate(value: bool) -> bool", "ensures result == !value", "    false"], "SEMAPRAX attack")
    if sem == sem_attack:
        raise ValueError("SEMAPRAX attack must differ from candidate")
    for project, reference, source, name in ((candidate_project, candidate_project_ref, sem_ref, "candidate"), (attack_project, attack_project_ref, sem_attack_ref, "attack")):
        if project != 'schema = "semaprax.project.v1"\nname = "law16-proof-probe"\nentry = "app"\nsources = ["core/core.spx", "src/app.spx", "tests/tests.spx"]\nweb_exports = ["app.main"]\ntests = ["app.tests"]\n':
            raise ValueError(f"SEMAPRAX {name} project manifest drifted")
        project_root = ROOT / pathlib.PurePosixPath(reference["path"]).parent
        app = project_root / "src/app.spx"
        if app.read_bytes() != (ROOT / source["path"]).read_bytes():
            raise ValueError(f"SEMAPRAX {name} project app source differs from pair source")
        for leaf, expected in (("core/core.spx", f"semaprax_{name}_core_core_spx"), ("tests/tests.spx", f"semaprax_{name}_tests_tests_spx")):
            if (project_root / leaf).read_bytes() != (ROOT / plan["sources"][expected]["path"]).read_bytes():
                raise ValueError(f"SEMAPRAX {name} project {leaf} drifted")
    return {
        "schema": REVIEW_SCHEMA,
        "status": "prepared_not_executed",
        "semantic_contract": contract,
        "sources": {key: plan["sources"][key] for key in plan["sources"]},
        "routes": plan["routes"],
        "comparability": plan["comparability_gate"],
        "nonclaims": plan["nonclaims"],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=pathlib.Path, default=ROOT / "fixtures/boolean-negation-pair-v1.json")
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.output.exists() or not args.output.parent.is_dir():
        parser.error("output must be new beneath an existing directory")
    try:
        result = review(args.plan)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        parser.error(str(error))
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
