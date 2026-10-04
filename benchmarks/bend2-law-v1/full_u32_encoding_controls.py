#!/usr/bin/env python3
"""Execute supplemental full-range u32 encoding controls; never close LAW-16."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import platform
import re
import shutil
import subprocess

ROOT = pathlib.Path(__file__).parent
FIXTURES = ROOT / "fixtures/full-u32-encoding-v1"
BEND_COMMIT = "947db722640c86247849343657bf2f7ef01cb7f1"
SCHEMA = "semaprax.bend2-law-benchmark.full-u32-encoding-controls.v1"
MUTATIONS = {
    "balance.spx": (
        "Decision { debit: before.debit - amount, credit: before.credit + amount, code: 0 }",
        "Decision { debit: before.debit, credit: before.credit, code: 0 }",
    ),
    "balance.bend": (
        "Decision{U32.sub(debit, amount), U32.add(credit, amount), 0}",
        "Decision{debit, credit, 0}",
    ),
    "sort.spx": (
        "ListStep::Cons { head, tail } => insert(head, sort(tail))",
        "ListStep::Cons { head, tail } => list_nil()",
    ),
    "sort.bend": ("      insert(head, sort(tail))", "      Nil{}"),
}


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def reference(path: pathlib.Path, root: pathlib.Path | None = None) -> dict:
    return {"path": str(path.relative_to(root) if root else path.resolve()),
            "sha256": digest(path.read_bytes()), "bytes": path.stat().st_size}


def mutation(name: str, source: str) -> str:
    old, new = MUTATIONS[name]
    if source.count(old) != 1:
        raise ValueError(f"{name}: attack requires one exact implementation body")
    return source.replace(old, new)


def accepted(route: str, attack: bool, code: int | None, stdout: bytes, stderr: bytes) -> bool:
    if route.startswith("bend_"):
        if attack:
            return code == 1 and b"SOME PROOFS FAIL" in stdout + stderr and b"Location: case_0" in stdout + stderr
        expected = (b"ALL PROOFS CHECK\nUse --verdict for mathematical validity.\n"
                    if route == "bend_normal" else b"ALL PROOFS CHECK\n")
        return code == 0 and stdout == expected and stderr == b""
    if attack:
        return code == 70 and b"SEMAPRAX contract failure" in stderr and b"contract: ensures" in stderr
    return code == 0 and stdout == b"0\n" and stderr == b""


def invoke(root: pathlib.Path, label: str, argv: list[str], timeout: int, env: dict) -> tuple[dict, bytes, bytes]:
    code, status = None, "exited"
    try:
        done = subprocess.run(argv, capture_output=True, env=env, timeout=timeout, check=False)
        code, out, err = done.returncode, done.stdout, done.stderr
    except subprocess.TimeoutExpired as error:
        status, out, err = "timed_out", error.stdout or b"", error.stderr or b""
    except OSError as error:
        status, out, err = "unavailable", b"", str(error).encode()
    for suffix, data in (("stdout", out), ("stderr", err)):
        (root / f"{label}.{suffix}").write_bytes(data)
    return {"argv": argv, "exit_code": code, "status": status,
            "stdout": reference(root / f"{label}.stdout", root),
            "stderr": reference(root / f"{label}.stderr", root)}, out, err


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bend-root", type=pathlib.Path, required=True)
    parser.add_argument("--bun", type=pathlib.Path, required=True)
    parser.add_argument("--semaprax", type=pathlib.Path, required=True)
    parser.add_argument("--z3", type=pathlib.Path, required=True)
    parser.add_argument("--semaprax-sha256", required=True)
    parser.add_argument("--semaprax-build-commit", required=True,
                        help="Caller-declared build source identity; not a build attestation")
    parser.add_argument("--artifacts", type=pathlib.Path, required=True)
    parser.add_argument("--timeout", type=int, default=60)
    args = parser.parse_args(argv)
    if not 1 <= args.timeout <= 120:
        parser.error("timeout must be 1..120 seconds")
    if args.artifacts.exists():
        parser.error("artifact directory must be new")
    if not re.fullmatch(r"[0-9a-f]{40}", args.semaprax_build_commit):
        parser.error("build commit must be a full SHA")
    for key in ("bend_root", "bun", "semaprax", "z3"):
        setattr(args, key, getattr(args, key).resolve())
    try:
        head = subprocess.check_output(["git", "-C", str(args.bend_root), "rev-parse", "HEAD"], text=True).strip()
        dirty = subprocess.run(["git", "-C", str(args.bend_root), "diff", "--quiet", "HEAD", "--", "bend2"], check=False).returncode
        if head != BEND_COMMIT or dirty:
            raise ValueError("Bend source is not the clean pinned bend2 tree")
        if digest(args.semaprax.read_bytes()) != args.semaprax_sha256:
            raise ValueError("SEMAPRAX executable differs from its explicit SHA256 pin")
        clang = shutil.which("clang")
        if not clang:
            raise ValueError("native control requires installed clang")
        sources = {name: (FIXTURES / name).read_text() for name in MUTATIONS}
        attacks = {name: mutation(name, source) for name, source in sources.items()}
        tool_refs = {"bun": reference(args.bun), "semaprax": reference(args.semaprax),
                     "clang": reference(pathlib.Path(clang)), "z3": reference(args.z3)}
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.error(str(error))
    root = args.artifacts.resolve()
    root.mkdir(parents=True)
    env = dict(os.environ, BEND_NO_TELEMETRY="1")
    report = {"schema": SCHEMA, "status": "incomplete", "original_manifest_unchanged": True,
              "representation_profile": "semaprax.checked-u32-value-encoding.v1",
              "numeric_domain": "all integers 0..4294967295; explicit transfer failure preserves state",
              "bend_commit": head, "semaprax_build_commit": args.semaprax_build_commit,
              "build_commit_association": "caller_declared_not_attested",
              "tools": tool_refs, "host": {"system": platform.system(), "release": platform.release(),
                                         "machine": platform.machine(), "processor": platform.processor()},
              "environment": {"BEND_NO_TELEMETRY": "1"}, "timeout_seconds": args.timeout,
              "versions": {}, "cases": [], "domain_controls": [],
              "nonclaims": ["supplemental representation controls; original checked-u32 cells remain unadmitted",
                            "concrete law witnesses are not universal list or transfer proofs",
                            "Bend ordinary checking, Bend verdict, and SEMAPRAX native runtime are distinct assurance routes",
                            "native compile and run are combined; no timings, agent trial, cost, comparison or issue closure",
                            "no builtin SEMAPRAX u32, lowering proof, full-list runtime totality or current-head build attestation"]}
    for name, command in (("bun", [str(args.bun), "--version"]),
                          ("semaprax", [str(args.semaprax), "--version"]),
                          ("clang", [clang, "--version"]), ("z3", [str(args.z3), "--version"])):
        report["versions"][name] = invoke(root, f"version-{name}", command, args.timeout, env)[0]
    bridge_source = root / "representation.smt2"
    bridge_source.write_bytes((FIXTURES / "representation.smt2").read_bytes())
    bridge, out, err = invoke(root, "representation-z3", [str(args.z3), "-smt2", str(bridge_source)], args.timeout, env)
    bridge.update(source=reference(bridge_source, root),
                  expected_outcome_observed=bridge["exit_code"] == 0 and out == b"unsat\nsat\nsat\n" and err == b"",
                  claim="full-domain mathematical bitvector encoding only; no source translation/lowering certificate")
    report["representation_bridge"] = bridge
    for task in ("balance", "sort"):
        for attack in (False, True):
            kind = "attack" if attack else "candidate"
            paths = {}
            for language, suffix in (("bend", "bend"), ("semaprax", "spx")):
                name = f"{task}.{suffix}"
                path = root / f"{task}-{kind}.{suffix}"
                path.write_text((attacks if attack else sources)[name])
                paths[language] = path
            bend = [str(args.bun), str(args.bend_root / "bend2/main.ts"), str(paths["bend"])]
            commands = {"bend_normal": bend + ["--check-only"], "bend_verdict": bend + ["--verdict"],
                        "semaprax_native": [str(args.semaprax), "run", str(paths["semaprax"]), "--native"]}
            for route, command in commands.items():
                row, out, err = invoke(root, f"{task}-{kind}-{route}", command, args.timeout, env)
                row.update(task=task, kind=kind, route=route,
                           source=reference(paths["bend" if route.startswith("bend_") else "semaprax"], root),
                           expected_outcome_observed=accepted(route, attack, row["exit_code"], out, err))
                report["cases"].append(row)
    for task, needle, replacement in (
        ("balance", "Balances { debit: 9, credit: 4 }", "Balances { debit: VALUE, credit: 4 }"),
        ("sort", "list_cons(3, list_cons(1,", "list_cons(VALUE, list_cons(1,"),
    ):
        for label, value in (("below-zero", "-1"), ("above-max", "4294967296")):
            source = sources[f"{task}.spx"]
            if source.count(needle) != 1:
                raise ValueError("domain-control witness changed")
            path = root / f"{task}-{label}.spx"
            path.write_text(source.replace(needle, replacement.replace("VALUE", value)))
            command = [str(args.semaprax), "run", str(path), "--native"]
            row, out, err = invoke(root, f"{task}-{label}", command, args.timeout, env)
            guard = (b"contract: requires before.debit >= 0" if task == "balance"
                     else b"contract: requires domain(input)")
            row.update(task=task, source=reference(path, root),
                       expected_outcome_observed=row["exit_code"] == 70
                       and b"SEMAPRAX contract failure" in err and guard in err)
            report["domain_controls"].append(row)
    report["status"] = ("supplemental_controls_pass" if all(row["expected_outcome_observed"] for row in report["cases"] + report["domain_controls"])
                        and bridge["expected_outcome_observed"]
                        and all(row["exit_code"] == 0 for row in report["versions"].values()) else "incomplete")
    (root / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"status": report["status"], "report": str(root / "report.json")}, sort_keys=True))
    return 0 if report["status"] == "supplemental_controls_pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
