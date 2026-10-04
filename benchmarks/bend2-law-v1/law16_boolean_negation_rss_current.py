#!/usr/bin/env python3
"""Capture a current-checkout, 30-sample-per-route LAW-16 RSS capsule."""
import argparse
import hashlib
import importlib.util
import json
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).parent
SAMPLES = 30
SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-peak-rss-current.v2"

PROVENANCE_SPEC = importlib.util.spec_from_file_location("law16_boolean_negation_provenance", ROOT / "law16_boolean_negation_provenance.py")
PROVENANCE = importlib.util.module_from_spec(PROVENANCE_SPEC)
PROVENANCE_SPEC.loader.exec_module(PROVENANCE)


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def reference(path, root):
    return {"path": str(path.relative_to(root)), "bytes": path.stat().st_size, "sha256": digest(path)}


def command(argv, output):
    completed = subprocess.run(
        [sys.executable, str(ROOT / "law16_peak_rss.py"), "--command", json.dumps([str(part) for part in argv]), "--runs", str(SAMPLES), "--output", str(output)],
        capture_output=True,
    )
    if completed.returncode:
        raise RuntimeError(f"RSS capture failed: {completed.stderr.decode('utf-8', errors='replace')}")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bend-root", required=True, type=pathlib.Path)
    parser.add_argument("--bun", required=True, type=pathlib.Path)
    parser.add_argument("--semaprax", required=True, type=pathlib.Path)
    parser.add_argument("--z3", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.output.exists() or not args.output.parent.is_dir(): parser.error("--output must be new beneath an existing directory")
    args.output = args.output.resolve()
    bend_main = args.bend_root / "bend2/main.ts"
    if not all(path.is_file() for path in (args.bun, args.semaprax, args.z3, bend_main)): parser.error("required tools are unavailable")
    args.output.mkdir()
    bend_input = args.output / "bend-input.bend"
    project = args.output / "semaprax-project"
    shutil.copyfile(ROOT / "fixtures/bend-boolean-negation-v1.bend", bend_input)
    shutil.copytree(ROOT / "fixtures/boolean-negation-project-v1/candidate", project)
    bend_argv = [args.bun, bend_main, bend_input, "--verdict"]
    semaprax_argv = [args.semaprax, "project-proof-check", project / "semaprax.toml", "--tool", "z3", "--executable", args.z3, "--version-line", "Z3 version 4.12.5 - 64 bit", "--host-profile", "trusted-local", "--source", "src/app.spx", "--declaration", "app.negate", "--ensures", "0"]
    try:
        command(bend_argv, args.output / "bend-verdict.json")
        command(semaprax_argv, args.output / "semaprax-z3.json")
    except RuntimeError as error:
        parser.error(str(error))
    tools = {
        "bun": {"identity": PROVENANCE.file_reference(args.bun), "version": PROVENANCE.observe([str(args.bun), "--version"])},
        "bend_main": {"identity": PROVENANCE.file_reference(bend_main)},
        "semaprax": {"identity": PROVENANCE.file_reference(args.semaprax), "version": PROVENANCE.observe([str(args.semaprax), "--version"])},
        "z3": {"identity": PROVENANCE.file_reference(args.z3), "version": PROVENANCE.observe([str(args.z3), "--version"])},
        "bend_commit": PROVENANCE.observe(["/usr/bin/git", "-C", str(args.bend_root / "bend2"), "rev-parse", "HEAD"]),
    }
    provenance = {"schema": SCHEMA, "status": "observed_current_checkout", "host": PROVENANCE.host_observation(), "tools": tools, "flags": {"BEND_NO_TELEMETRY": "1", "wrapper": ["/usr/bin/time", "-l"]}, "commands": {"bend_verdict": [str(part) for part in bend_argv], "semaprax_z3": [str(part) for part in semaprax_argv]}, "cold_cache": {"status": "unavailable", "reason": "RSS runs do not clear or verify host caches"}}
    (args.output / "provenance.json").write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n")
    manifest = {"schema": SCHEMA, "status": "completed", "samples_per_route": SAMPLES, "bend_input": reference(bend_input, args.output), "semaprax_source": reference(project / "src/app.spx", args.output), "semaprax_manifest": reference(project / "semaprax.toml", args.output), "bend_receipt": reference(args.output / "bend-verdict.json", args.output), "semaprax_receipt": reference(args.output / "semaprax-z3.json", args.output), "provenance": reference(args.output / "provenance.json", args.output), "nonclaims": ["v2 is a current-checkout observation and does not rebind historical RSS evidence", "no RSS ratio, winner, or cold-cache claim"]}
    (args.output / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
