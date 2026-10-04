#!/usr/bin/env python3
"""Capture ordinary Bend and nonproof SEMAPRAX check rows for Boolean negation.

This wrapper intentionally captures checking/provisioning only.  It requires
canonical equal-semantics negation input in separate fresh/repeat paths and
keeps the ordinary Bend and SEMAPRAX ``check`` routes distinct from the
retained Bend verdict and installed-Z3 proof routes.
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
SAMPLES = 30
SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-nonproof-process.v1"

SPEC = importlib.util.spec_from_file_location("law16_boolean_negation_pair", ROOT / "law16_boolean_negation_pair.py")
PAIR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PAIR)


def sha256(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def reference(path, root):
    return {"path": str(path.resolve()), "bytes": path.stat().st_size, "sha256": sha256(path)}


def require_exact_copy(path, fixture, label):
    if not path.is_file() or path.read_bytes() != fixture.read_bytes():
        raise ValueError(f"{label} must be byte-identical to the canonical matched fixture")


def json_command(parts):
    return json.dumps([str(part) for part in parts], separators=(",", ":"))


def bend_ordinary_command(bun, bend_main, source):
    return [bun, bend_main, source]


def semaprax_check_command(semaprax, source):
    return [semaprax, "check", source]


def capture_cell(runner, output_root, label, fresh_command, repeat_command, fresh_input, repeat_input, artifacts):
    argv = [
        "python3", str(runner),
        "--fresh-command", json_command(fresh_command),
        "--repeat-command", json_command(repeat_command),
        "--fresh-input", str(fresh_input),
        "--repeat-input", str(repeat_input),
        "--samples", str(SAMPLES),
        "--timeout-seconds", "120",
        "--raw-artifact-dir", str(output_root / f"{label}-raw"),
        "--output", str(output_root / f"{label}.json"),
    ]
    for artifact in artifacts:
        argv.extend(("--artifact", str(artifact)))
    completed = subprocess.run(argv, capture_output=True)
    (output_root / f"{label}-runner.stdout").write_bytes(completed.stdout)
    (output_root / f"{label}-runner.stderr").write_bytes(completed.stderr)
    if completed.returncode:
        raise RuntimeError(f"{label} capture failed; see {label}-runner.stderr")


def check_identities(args):
    bend_main = args.bend_root / "bend2/main.ts"
    if not all(path.is_file() for path in (args.bun, args.semaprax, bend_main)):
        raise ValueError("one or more required pinned tools are unavailable")
    commit = subprocess.check_output(["git", "-C", str(args.bend_root / "bend2"), "rev-parse", "HEAD"], text=True).strip()
    if commit != BEND_COMMIT:
        raise ValueError("Bend root is not the pinned commit")
    if sha256(args.semaprax) != SEMAPRAX_SHA256:
        raise ValueError("SEMAPRAX executable digest is not the pinned checker executable")
    return bend_main


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bend-root", required=True, type=pathlib.Path)
    parser.add_argument("--bun", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax", required=True, type=pathlib.Path)
    parser.add_argument("--bend-fresh", required=True, type=pathlib.Path)
    parser.add_argument("--bend-repeat", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax-fresh", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax-repeat", required=True, type=pathlib.Path)
    parser.add_argument("--output-root", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.output_root.exists() or not args.output_root.parent.is_dir():
        parser.error("output root must be new below an existing directory")
    try:
        PAIR.review(ROOT / "fixtures/boolean-negation-pair-v1.json")
        bend_main = check_identities(args)
        require_exact_copy(args.bend_fresh, BEND_FIXTURE, "Bend fresh input")
        require_exact_copy(args.bend_repeat, BEND_FIXTURE, "Bend repeat input")
        require_exact_copy(args.semaprax_fresh, SEMAPRAX_FIXTURE, "SEMAPRAX fresh input")
        require_exact_copy(args.semaprax_repeat, SEMAPRAX_FIXTURE, "SEMAPRAX repeat input")
    except (OSError, subprocess.CalledProcessError, ValueError, json.JSONDecodeError) as error:
        parser.error(str(error))

    args.output_root.mkdir()
    identity = {
        "schema": SCHEMA,
        "status": "completed_local_process_provisioning",
        "sources": {"bend": sha256(BEND_FIXTURE), "semaprax": sha256(SEMAPRAX_FIXTURE)},
        "tools": {
            "bend_commit": BEND_COMMIT,
            "bun": reference(args.bun, args.output_root.parent),
            "bend_main": reference(bend_main, args.output_root.parent),
            "semaprax": reference(args.semaprax, args.output_root.parent),
        },
        "routes": {
            "bend_ordinary": "BEND_NO_TELEMETRY=1 bun bend2/main.ts SOURCE",
            "semaprax_check": "semaprax check SOURCE",
        },
        "nonclaims": [
            "ordinary Bend checking is not Bend --verdict",
            "SEMAPRAX check is not external-Z3 proof checking",
            "successful checker execution does not establish law repair",
            "no cross-route timing ratio, winner, or cache-isolation claim",
        ],
    }
    (args.output_root / "identity.json").write_text(json.dumps(identity, indent=2, sort_keys=True) + "\n")
    runner = ROOT / "law16_cold_warm_cell.py"
    capture_cell(runner, args.output_root, "bend-ordinary", bend_ordinary_command(args.bun, bend_main, args.bend_fresh), bend_ordinary_command(args.bun, bend_main, args.bend_repeat), args.bend_fresh, args.bend_repeat, [args.bun, bend_main, args.output_root / "identity.json"])
    capture_cell(runner, args.output_root, "semaprax-check", semaprax_check_command(args.semaprax, args.semaprax_fresh), semaprax_check_command(args.semaprax, args.semaprax_repeat), args.semaprax_fresh, args.semaprax_repeat, [args.semaprax, args.output_root / "identity.json"])


if __name__ == "__main__":
    main()
