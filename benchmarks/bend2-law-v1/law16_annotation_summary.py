#!/usr/bin/env python3
"""Summarize explicit annotation and proof-body bytes in retained Boolean trials."""
import argparse
import hashlib
import json
import pathlib
import re
import stat

ROOT = pathlib.Path(__file__).parent
PLAN = ROOT / "evidence/law16-boolean-negation-agent-plan-v1.json"
PAIR_PLAN = ROOT / "fixtures/boolean-negation-pair-v1.json"
PILOT = ROOT / "evidence/law16-boolean-negation-agent-pilot-v1"
CAMPAIGN = ROOT / "evidence/law16-boolean-negation-agent-campaign-v1"
SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-annotation-summary.v1"


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def safe_read(root, relative):
    """Read a regular, non-symlink artifact beneath root using a POSIX path."""
    rel = pathlib.PurePosixPath(relative)
    if rel.is_absolute() or not rel.parts or any(part in ("", ".", "..") for part in rel.parts):
        raise ValueError(f"unsafe artifact path: {relative!r}")
    root = root.resolve(strict=True)
    path = root
    for part in rel.parts:
        path = path / part
        if stat.S_ISLNK(path.lstat().st_mode):
            raise ValueError(f"artifact path contains symlink: {relative!r}")
    if not stat.S_ISREG(path.stat().st_mode):
        raise ValueError(f"artifact is not a regular file: {relative!r}")
    return path.read_bytes()


def line_metrics(data, patterns):
    lines = data.splitlines(keepends=True)
    selected = {name: [line for line in lines if pattern.search(line)] for name, pattern in patterns.items()}
    return {
        name: {"count": len(rows), "bytes": sum(map(len, rows))}
        for name, rows in selected.items()
    }


def analyze(language, data):
    if language == "bend2":
        # A Bend law declaration is an explicit universal-law annotation;
        # the exact `{==}` line is the proof term body. Function bodies are
        # implementation code and are deliberately excluded.
        return line_metrics(data, {
            "law_declarations": re.compile(rb"^law [^:\n]+:\s*(?:\r?\n)?$"),
            "proof_terms": re.compile(rb"^\s*\{==\}\s*(?:\r?\n)?$"),
        })
    if language == "semaprax-scalar-v1":
        # Count only explicit identity and postcondition annotation lines.
        # An ordinary function implementation block is not a proof body.
        return line_metrics(data, {
            "stable_ids": re.compile(rb"^\s*@id\("),
            "requires": re.compile(rb"^\s*requires\b"),
            "ensures": re.compile(rb"^\s*ensures\b"),
            "explicit_proof_bodies": re.compile(rb"^\s*proof\s*\{"),
        })
    raise ValueError(f"unknown language: {language}")


def changed_bytes(before, after):
    return sum(left != right for left, right in zip(before, after)) + abs(len(before) - len(after))


def reference(path, data):
    return {"path": path, "bytes": len(data), "sha256": digest(data)}


def summarize():
    plan = json.loads(PLAN.read_text())
    if plan.get("schema") != "semaprax.bend2-law-benchmark.agent-trial-plan.v1":
        raise ValueError("fixed Boolean pair plan schema mismatch")
    pair_plan_data = PAIR_PLAN.read_bytes()
    pair_plan_ref = plan.get("pair_plan", {})
    if pair_plan_ref.get("path") != "fixtures/boolean-negation-pair-v1.json" or pair_plan_ref.get("sha256") != digest(pair_plan_data):
        raise ValueError("agent plan does not bind the fixed Boolean pair plan")
    pair_plan = json.loads(pair_plan_data)
    sources = pair_plan["sources"]
    seeds = {
        "bend2": sources["bend_candidate"],
        "semaprax-scalar-v1": sources["semaprax_candidate"],
    }
    rows = []
    for ordinal in range(1, 11):
        for language, subdir, suffix in (
            ("bend2", "bend", "bend"),
            ("semaprax-scalar-v1", "semaprax", "spx"),
        ):
            if ordinal == 1:
                evidence_root = PILOT
                rel = f"{subdir}-ordinal-1/final-source.{suffix}"
            else:
                evidence_root = CAMPAIGN
                rel = f"ordinal-{ordinal}/{subdir}/final-source.{suffix}"
            final = safe_read(evidence_root, rel)
            seed_ref = seeds[language]
            seed_repo_root = ROOT
            seed_rel = pathlib.PurePosixPath(seed_ref["path"])
            if seed_rel.is_absolute() or any(p in ("", ".", "..") for p in seed_rel.parts):
                raise ValueError("unsafe fixed-seed path in pair plan")
            seed = safe_read(seed_repo_root, seed_ref["path"])
            if len(seed) != seed_ref["bytes"] or digest(seed) != seed_ref["sha256"]:
                raise ValueError(f"fixed seed differs from preregistered pair plan: {seed_ref['path']}")
            rows.append({
                "ordinal": ordinal,
                "language": language,
                "final_source": reference(f"{evidence_root.name}/{rel}", final),
                "fixed_seed": reference(seed_ref["path"], seed),
                "changed_bytes_vs_seed": changed_bytes(seed, final),
                "explicit_annotation_and_proof_counts": analyze(language, final),
            })
    return {
        "schema": SCHEMA,
        "status": "ten_matched_pairs_summarized_from_retained_source",
        "plan": reference(PLAN.relative_to(ROOT.parent.parent).as_posix(), PLAN.read_bytes()),
        "fixed_pair_plan": reference(PAIR_PLAN.relative_to(ROOT.parent).as_posix(), pair_plan_data),
        "matched_pairs": 10,
        "rows": rows,
        "definitions": {
            "bend2": {
                "law_declarations": "full source lines beginning `law <name>:`; count and UTF-8 source bytes including line ending",
                "proof_terms": "full source line consisting of the explicit `{==}` proof term; count and bytes including line ending",
                "excluded": "function signatures and implementation bodies are not counted as proof effort",
            },
            "semaprax-scalar-v1": {
                "annotations": "full source lines beginning @id(, requires, or ensures; counts and bytes include line endings",
                "explicit_proof_bodies": "full source lines beginning `proof {`; ordinary function bodies are excluded",
            },
            "changed_bytes_vs_seed": "bytewise positional differences plus any length difference; this is a textual distance, not semantic effort",
        },
        "nonclaims": [
            "byte counts do not measure reasoning, difficulty, or semantic proof effort",
            "the two languages use different annotation and proof syntax, so their raw byte totals are not directly comparable",
            "the summary does not validate or strengthen the retained candidate proof verdicts",
        ],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=pathlib.Path, default=ROOT / "evidence/law16-boolean-negation-annotation-summary-v1.json")
    args = parser.parse_args(argv)
    value = summarize()
    encoded = json.dumps(value, indent=2, sort_keys=True) + "\n"
    if args.output.resolve() == (ROOT / "LAW16-REPORT.md").resolve():
        parser.error("output may not overwrite the LAW16 report")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(encoded)


if __name__ == "__main__":
    main()
