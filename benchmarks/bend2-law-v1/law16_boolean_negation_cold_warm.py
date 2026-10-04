#!/usr/bin/env python3
"""Capture separately bound fresh/repeat timing rows for matched Boolean negation.

This wrapper is deliberately prepared only: it validates the committed pair,
fixed tool identities, and two byte-identical input copies before delegating
30 fresh-path and 30 repeat-path child processes to ``law16_cold_warm_cell``.
A fresh path and process cannot flush macOS caches, so output is never called a
cold-cache result.
"""
import argparse
import hashlib
import importlib.util
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).parent
BEND_FIXTURE = ROOT / "fixtures/bend-boolean-negation-v1.bend"
SEMAPRAX_FIXTURE = ROOT / "fixtures/semaprax-boolean-negation-v1.spx"
BEND_COMMIT = "947db722640c86247849343657bf2f7ef01cb7f1"
SEMAPRAX_SHA256 = "sha256:cc9dd3ca99a74dd973cbfb904621e27b801d8ddea6065873d24c14de9dee1d89"
Z3_VERSION = "Z3 version 4.12.5 - 64 bit"
SAMPLES = 30

SPEC = importlib.util.spec_from_file_location(
    "law16_boolean_negation_pair", ROOT / "law16_boolean_negation_pair.py"
)
PAIR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PAIR)


def sha256(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def require_exact_copy(path, fixture, label):
    if not path.is_file() or path.read_bytes() != fixture.read_bytes():
        raise ValueError(f"{label} must be byte-identical to the canonical matched fixture")


def json_command(parts):
    return json.dumps([str(part) for part in parts], separators=(",", ":"))


def bend_command(bun, bend_main, source):
    return [bun, bend_main, source, "--verdict"]


def semaprax_command(semaprax, z3, project):
    return [
        semaprax,
        "project-proof-check",
        project / "semaprax.toml",
        "--tool",
        "z3",
        "--executable",
        z3,
        "--version-line",
        Z3_VERSION,
        "--host-profile",
        "trusted-local",
        "--source",
        "src/app.spx",
        "--declaration",
        "app.negate",
        "--ensures",
        "0",
    ]


def check_identities(args):
    bend_main = args.bend_root / "bend2/main.ts"
    if not all(path.is_file() for path in (args.bun, args.semaprax, args.z3, bend_main)):
        raise ValueError("one or more required pinned tools are unavailable")
    commit = subprocess.check_output(
        ["git", "-C", str(args.bend_root / "bend2"), "rev-parse", "HEAD"], text=True
    ).strip()
    if commit != BEND_COMMIT:
        raise ValueError("Bend root is not the pinned commit")
    if sha256(args.semaprax) != SEMAPRAX_SHA256:
        raise ValueError("SEMAPRAX executable digest is not the pinned proof executable")
    version = subprocess.check_output([str(args.z3), "--version"], text=True).strip()
    if version != Z3_VERSION:
        raise ValueError("Z3 version is not the pinned proof solver")
    return bend_main


def capture_cell(runner, output_root, label, fresh_command, repeat_command, fresh_input, repeat_input, artifacts):
    argv = [
        "python3",
        str(runner),
        "--fresh-command",
        json_command(fresh_command),
        "--repeat-command",
        json_command(repeat_command),
        "--fresh-input",
        str(fresh_input),
        "--repeat-input",
        str(repeat_input),
        "--samples",
        str(SAMPLES),
        "--timeout-seconds",
        "120",
        "--raw-artifact-dir",
        str(output_root / f"{label}-raw"),
        "--output",
        str(output_root / f"{label}.json"),
    ]
    for artifact in artifacts:
        argv.extend(("--artifact", str(artifact)))
    completed = subprocess.run(argv, capture_output=True)
    (output_root / f"{label}-runner.stdout").write_bytes(completed.stdout)
    (output_root / f"{label}-runner.stderr").write_bytes(completed.stderr)
    if completed.returncode:
        raise RuntimeError(f"{label} capture failed; see {label}-runner.stderr")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bend-root", required=True, type=pathlib.Path)
    parser.add_argument("--bun", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax", required=True, type=pathlib.Path)
    parser.add_argument("--z3", required=True, type=pathlib.Path)
    parser.add_argument("--bend-fresh", required=True, type=pathlib.Path)
    parser.add_argument("--bend-repeat", required=True, type=pathlib.Path)
    parser.add_argument("--sem-fresh-project", required=True, type=pathlib.Path)
    parser.add_argument("--sem-repeat-project", required=True, type=pathlib.Path)
    parser.add_argument("--output-root", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.output_root.exists() or not args.output_root.parent.is_dir():
        parser.error("output root must be new below an existing directory")
    try:
        PAIR.review(ROOT / "fixtures/boolean-negation-pair-v1.json")
        bend_main = check_identities(args)
        require_exact_copy(args.bend_fresh, BEND_FIXTURE, "Bend fresh input")
        require_exact_copy(args.bend_repeat, BEND_FIXTURE, "Bend repeat input")
        for project, label in (
            (args.sem_fresh_project, "SEMAPRAX fresh project"),
            (args.sem_repeat_project, "SEMAPRAX repeat project"),
        ):
            require_exact_copy(project / "src/app.spx", SEMAPRAX_FIXTURE, label)
            if not (project / "semaprax.toml").is_file():
                raise ValueError(f"{label} must include semaprax.toml")
    except (OSError, subprocess.CalledProcessError, ValueError, json.JSONDecodeError) as error:
        parser.error(str(error))

    args.output_root.mkdir()
    runner = ROOT / "law16_cold_warm_cell.py"
    capture_cell(
        runner,
        args.output_root,
        "bend-verdict",
        bend_command(args.bun, bend_main, args.bend_fresh),
        bend_command(args.bun, bend_main, args.bend_repeat),
        args.bend_fresh,
        args.bend_repeat,
        [args.bun, bend_main],
    )
    capture_cell(
        runner,
        args.output_root,
        "semaprax-z3",
        semaprax_command(args.semaprax, args.z3, args.sem_fresh_project),
        semaprax_command(args.semaprax, args.z3, args.sem_repeat_project),
        args.sem_fresh_project / "src/app.spx",
        args.sem_repeat_project / "src/app.spx",
        [args.semaprax, args.z3, args.sem_fresh_project / "semaprax.toml", args.sem_repeat_project / "semaprax.toml"],
    )
    (args.output_root / "plan.json").write_text(json.dumps({
        "schema": "semaprax.bend2-law-benchmark.boolean-negation-cold-warm.v1",
        "status": "completed_local_process_provisioning",
        "samples_per_state": SAMPLES,
        "sources": {"bend": sha256(BEND_FIXTURE), "semaprax": sha256(SEMAPRAX_FIXTURE)},
        "cold_cache": {"status": "unavailable", "reason": "fresh paths cannot isolate OS, executable, solver, or tool caches"},
        "nonclaims": ["no cross-route timing ratio or winner", "no OS-cache-cold observation"],
    }, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
