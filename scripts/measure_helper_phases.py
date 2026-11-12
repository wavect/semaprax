#!/usr/bin/env python3
"""Run the qualified, private OPT738 paired compiler-phase harness; no cost claims."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile

SELECTOR = "cleanup_plan::phase_measurement::generated_codec_construction_and_replay_measurement"
PHASES = ("construction", "replay", "validation")
def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()
def command(args: list[str], root: Path) -> str:
    return subprocess.check_output(args, cwd=root, text=True).strip()
def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path, help="new directory for all raw logs and records")
    parser.add_argument("--warmup", type=int, default=2)
    parser.add_argument("--samples", type=int, default=10)
    args = parser.parse_args()
    if not 1 <= args.warmup <= 100 or not 1 <= args.samples <= 1000:
        parser.error("warmup must be 1..100 and samples 1..1000")
    root = args.root.resolve()
    if command(["git", "status", "--porcelain"], root):
        parser.error("measurement requires a clean committed source checkout")
    head = command(["git", "rev-parse", "HEAD"], root)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    target = root / "target" / f"opt738-phase-measurement-{head}"
    target.mkdir(parents=True, exist_ok=True)
    if not target.resolve().is_relative_to(root / "target"):
        parser.error("private target must stay inside this worktree")
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target)
    env["CARGO_BUILD_JOBS"] = "1"
    for name in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"):
        env.pop(name, None)
    build = ["cargo", "test", "--release", "--lib", "--all-features", "--locked", "--offline", "--no-run", "--message-format=json"]
    context = {"schema": "semaprax.opt738.phase-driver.v1", "source": head,
               "cargo_lock_sha256": sha(root / "Cargo.lock"), "rustc": command(["rustc", "-Vv"], root),
               "cargo": command(["cargo", "-V"], root), "build_command": build,
               "private_target": str(target), "warmup": args.warmup, "samples": args.samples,
               "status": "pending"}
    def save() -> None:
        (output / "context.json").write_text(json.dumps(context, indent=2) + "\n")
    save()
    with (output / "build.jsonl").open("w") as stdout, (output / "build.stderr").open("w") as stderr:
        result = subprocess.run(build, cwd=root, env=env, stdout=stdout, stderr=stderr)
    if result.returncode:
        context["status"] = "build_failed"; save(); return result.returncode
    binaries = []
    for line in (output / "build.jsonl").read_text().splitlines():
        item = json.loads(line)
        if item.get("reason") == "compiler-artifact" and item.get("executable") and item.get("profile", {}).get("test") and item.get("target", {}).get("name") == "semaprax":
            binaries.append(Path(item["executable"]))
    if len(binaries) != 1:
        raise RuntimeError(f"expected exactly one owning library test binary, found {binaries}")
    binary = binaries[0]
    context["binary_sha256"] = sha(binary)
    context["binary"] = str(binary)
    context["run_command"] = [str(binary), SELECTOR, "--exact", "--ignored", "--nocapture", "--test-threads=1"]
    with tempfile.TemporaryDirectory(prefix="opt738-generated-") as temporary:
        fixture = Path(temporary)
        (fixture / "src").mkdir()
        (fixture / "semaprax.toml").write_bytes((root / "tests/native/string_settlement/bulk_utf8/semaprax.toml").read_bytes())
        (fixture / "src/schema.spx").write_bytes((root / "src/cleanup_plan/phase_measurement/schema.spx").read_bytes())
        for module, identity in (("app", "consumer.main"), ("tests", "consumer.tests")):
            (fixture / "src" / f"{module}.spx").write_text(
                f'module consumer.{module};\n\n'
                f'@id("{identity}")\n'
                'fn main() -> i64\n{\n    0\n}\n'
            )
        env.update(SEMAPRAX_OPT738_PROJECT=str(fixture), SEMAPRAX_OPT738_WARMUP=str(args.warmup), SEMAPRAX_OPT738_SAMPLES=str(args.samples))
        with (output / "run.stdout").open("w") as stdout, (output / "run.stderr").open("w") as stderr:
            result = subprocess.run(context["run_command"], cwd=root, env=env, stdout=stdout, stderr=stderr)
        for name in ("schema", "app"):
            (output / f"actual-{name}.spx").write_bytes((fixture / f"src/{name}.spx").read_bytes())
    records = []
    for line in (output / "run.stdout").read_text().splitlines():
        if "OPT738_JSON=" in line:
            records.append(json.loads(line.split("OPT738_JSON=", 1)[1]))
    (output / "raw-samples.jsonl").write_text("".join(json.dumps(item, sort_keys=True) + "\n" for item in records))
    context["status"] = "execution_failed" if result.returncode else "incomplete"
    context["exit_code"] = result.returncode
    save()
    if result.returncode:
        return result.returncode
    headers = [item for item in records if item["record"] == "context"]
    samples = [item for item in records if item["record"] == "sample"]
    expected = {
        (phase, indexed, iteration)
        for phase in PHASES
        for indexed in (False, True)
        for iteration in range(args.warmup + args.samples)
    }
    actual = [(item["phase"], item["indexed"], item["iteration"]) for item in samples]
    if len(headers) != 1 or len(actual) != len(expected) or set(actual) != expected or any(item["status"] != "success" for item in samples):
        raise RuntimeError("missing, duplicate, or failed raw paired samples")
    if headers[0]["generated_source_sha256"] != sha(output / "actual-schema.spx") or headers[0]["app_source_sha256"] != sha(output / "actual-app.spx"):
        raise RuntimeError("actual generated workload binding differs")
    if command(["git", "rev-parse", "HEAD"], root) != head or command(["git", "status", "--porcelain"], root) or sha(binary) != context["binary_sha256"]:
        raise RuntimeError("source or binary drift during measurement")
    summary = []
    for phase in PHASES:
        for indexed in (False, True):
            group = [item for item in samples if item["phase"] == phase and item["indexed"] == indexed and not item["warmup"]]
            if len(group) != args.samples:
                raise RuntimeError("warmup/sample classification differs")
            summary.append({
                "phase": phase,
                "indexed": indexed,
                "samples": len(group),
                "median_elapsed_ns": statistics.median(item["elapsed_ns"] for item in group),
                "reservation_ledger_bytes": [item["reservation_ledger_bytes"] for item in group],
                "replay_work_units": [item["replay_work_units"] for item in group],
                "skeleton_work_units": [item.get("skeleton_work_units") for item in group],
            })
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    context["status"] = "measured_paired_success"; save()
    return 0
if __name__ == "__main__":
    raise SystemExit(main())
