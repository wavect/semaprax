#!/usr/bin/env python3
"""Capture/review bounded Boolean check, build, and native-run phases."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parent
SCHEMA = "semaprax.bend2-law-benchmark.boolean-native-phases.v1"
BEND_COMMIT = "947db722640c86247849343657bf2f7ef01cb7f1"
EXPECTED = {"bend_run": b"True\nFalse\n", "semaprax_run": b"0\n"}
PHASES = ("bend_check", "bend_emit_c", "clang_compile", "bend_run", "semaprax_check", "semaprax_build", "semaprax_run")
NONCLAIMS = [
    "SEMAPRAX build combines internal checking, code generation, and native compilation; those internals are not timed separately",
    "Bend emit-C and Clang compilation are distinct local phases but do not measure Bend --verdict",
    "process startup and local caches are not isolated; this is not a cold/warm or cross-route comparison",
    "native execution is a two-input witness, not a source-proof or lowering theorem",
    "no performance winner or LAW16 issue closure",
]


def digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def ref(path: Path, root: Path) -> dict:
    return {"path": path.relative_to(root).as_posix(), "bytes": path.stat().st_size, "sha256": digest(path)}


def checked(root: Path, item: dict) -> Path:
    if not isinstance(item, dict) or set(item) != {"path", "bytes", "sha256"}:
        raise ValueError("artifact reference malformed")
    rel = Path(item["path"])
    if rel.is_absolute() or ".." in rel.parts or not rel.parts:
        raise ValueError("artifact path escapes capsule")
    path = root / rel
    if path.is_symlink() or not path.is_file() or ref(path, root) != item:
        raise ValueError("artifact identity drifted")
    return path


def call(argv: list[str], raw: Path, label: str, env: dict[str, str]) -> dict:
    start = time.monotonic_ns()
    try:
        run = subprocess.run(argv, capture_output=True, env=env, timeout=60)
        elapsed = time.monotonic_ns() - start
        code, stdout, stderr, timeout = run.returncode, run.stdout, run.stderr, False
    except subprocess.TimeoutExpired as error:
        elapsed = time.monotonic_ns() - start
        code, stdout, stderr, timeout = None, error.stdout or b"", error.stderr or b"", True
    if len(stdout) > 65536 or len(stderr) > 65536:
        raise ValueError("phase output exceeded 64 KiB")
    stdout_path, stderr_path = raw / (label + ".stdout"), raw / (label + ".stderr")
    stdout_path.write_bytes(stdout)
    stderr_path.write_bytes(stderr)
    return {"argv": argv, "exit_code": code, "timed_out": timeout, "elapsed_ns": elapsed,
            "stdout": ref(stdout_path, raw), "stderr": ref(stderr_path, raw)}


def tools(sem: Path, bun: Path, bend: Path, clang: Path) -> dict:
    if subprocess.check_output(["git", "-C", str(bend), "rev-parse", "HEAD"], text=True).strip() != BEND_COMMIT:
        raise ValueError("Bend source commit differs")
    if subprocess.run(["git", "-C", str(bend), "diff", "--quiet", "HEAD", "--", "bend2"]).returncode:
        raise ValueError("Bend source differs from pin")
    return {"semaprax": {"path": str(sem), "sha256": digest(sem)},
            "bun": {"path": str(bun), "sha256": digest(bun)},
            "bend_main": {"path": str(bend / "bend2/main.ts"), "sha256": digest(bend / "bend2/main.ts"), "commit": BEND_COMMIT},
            "clang": {"path": str(clang), "sha256": digest(clang)}}


def commands(pins: dict, capsule: Path, ordinal: int) -> dict[str, list[str]]:
    bend_src = str(ROOT / "fixtures/bend-boolean-negation-v1.bend")
    sem_src = str(ROOT / "fixtures/semaprax-boolean-negation-v1.spx")
    out = capsule / "build" / f"{ordinal:02d}"
    c_file, bend_exe, sem_exe = out / "bend.c", out / "bend", out / "semaprax"
    bend = [pins["bun"]["path"], pins["bend_main"]["path"], bend_src]
    sem = pins["semaprax"]["path"]
    return {
        "bend_check": bend + ["--check-only"],
        "bend_emit_c": bend + ["-o", str(c_file)],
        "clang_compile": [pins["clang"]["path"], "-O2", str(c_file), "-o", str(bend_exe)],
        "bend_run": [str(bend_exe)],
        "semaprax_check": [sem, "check", sem_src],
        "semaprax_build": [sem, "build", sem_src, "--target", "native", "-o", str(sem_exe)],
        "semaprax_run": [str(sem_exe)],
    }


def capture(output: Path, sem: Path, bun: Path, bend: Path, clang: Path, repetitions: int) -> dict:
    if output.exists() or not output.parent.is_dir() or repetitions not in (1, 30):
        raise ValueError("output must be new; repetitions must be pilot 1 or campaign 30")
    pins = tools(sem.resolve(strict=True), bun.resolve(strict=True), bend.resolve(strict=True), clang.resolve(strict=True))
    output.mkdir()
    raw, build = output / "raw", output / "build"
    raw.mkdir(); build.mkdir()
    env = dict(os.environ, BEND_NO_TELEMETRY="1", DO_NOT_TRACK="1")
    samples = []
    for ordinal in range(1, repetitions + 1):
            (build / f"{ordinal:02d}").mkdir()
            row = {"ordinal": ordinal, "phases": {}, "generated": {}}
            for phase, argv in commands(pins, output, ordinal).items():
                record = call(argv, raw, f"{ordinal:02d}-{phase}", env)
                row["phases"][phase] = record
                if record["exit_code"] != 0 or record["timed_out"] or checked(raw, record["stderr"]).read_bytes():
                    raise ValueError(f"{ordinal} {phase} failed")
                stdout = checked(raw, record["stdout"]).read_bytes()
                if phase in EXPECTED and stdout != EXPECTED[phase]:
                    raise ValueError(f"{ordinal} {phase} output differs")
            paths = {"bend_c": build / f"{ordinal:02d}/bend.c", "bend_native": build / f"{ordinal:02d}/bend",
                     "semaprax_native": build / f"{ordinal:02d}/semaprax"}
            row["generated"] = {name: ref(path, output) for name, path in paths.items()}
            samples.append(row)
    receipt = {"schema": SCHEMA, "status": "pilot" if repetitions == 1 else "thirty_repetitions",
               "repetitions": repetitions, "capture_root": str(output), "phase_order": list(PHASES), "pins": pins,
               "inputs": {name: ref(ROOT / "fixtures" / name, ROOT) for name in ("bend-boolean-negation-v1.bend", "semaprax-boolean-negation-v1.spx")},
               "environment": {"BEND_NO_TELEMETRY": "1", "DO_NOT_TRACK": "1"}, "samples": samples,
               "nonclaims": NONCLAIMS}
    (output / "receipt.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    return review(output)


def review(output: Path) -> dict:
    receipt = json.loads((output / "receipt.json").read_text())
    count = receipt.get("repetitions")
    if receipt.get("schema") != SCHEMA or count not in (1, 30) or receipt.get("status") != ("pilot" if count == 1 else "thirty_repetitions"):
        raise ValueError("native phase receipt schema or sample count differs")
    if receipt.get("nonclaims") != NONCLAIMS or receipt.get("phase_order") != list(PHASES) or receipt.get("environment") != {"BEND_NO_TELEMETRY": "1", "DO_NOT_TRACK": "1"}:
        raise ValueError("native phase boundary differs")
    for name, item in receipt["inputs"].items():
        if checked(ROOT, item) != ROOT / "fixtures" / name:
            raise ValueError("native phase input differs")
    if len(receipt["samples"]) != count:
        raise ValueError("native phase inventory differs")
    elapsed = {phase: [] for phase in PHASES}
    for ordinal, row in enumerate(receipt["samples"], 1):
        if row["ordinal"] != ordinal or set(row["phases"]) != set(PHASES):
            raise ValueError("native phase ordering differs")
        expected = commands(receipt["pins"], Path(receipt["capture_root"]), ordinal)
        for phase, command in expected.items():
            sample = row["phases"][phase]
            stdout, stderr = checked(output / "raw", sample["stdout"]).read_bytes(), checked(output / "raw", sample["stderr"]).read_bytes()
            if sample["argv"] != command or sample["exit_code"] != 0 or sample["timed_out"] or sample["elapsed_ns"] <= 0 or stderr:
                raise ValueError("native phase command or result differs")
            if phase in EXPECTED and stdout != EXPECTED[phase]:
                raise ValueError("native execution output differs")
            elapsed[phase].append(sample["elapsed_ns"])
        for item in row["generated"].values():
            checked(output, item)
    return {"status": "pilot_authenticated" if count == 1 else "thirty_repetitions_authenticated",
            "summary": {phase: {"count": count, "p50_ns": statistics.median(values),
                                "p95_ns": sorted(values)[(95 * count + 99) // 100 - 1]} for phase, values in elapsed.items()},
            "nonclaims": NONCLAIMS}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--review", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--semaprax", type=Path)
    parser.add_argument("--bun", type=Path)
    parser.add_argument("--bend", type=Path)
    parser.add_argument("--clang", type=Path, default=Path("/usr/bin/clang"))
    parser.add_argument("--repetitions", type=int, choices=(1, 30), default=1)
    args = parser.parse_args()
    try:
        result = review(args.review.resolve()) if args.review else capture(args.output.resolve(), args.semaprax, args.bun, args.bend, args.clang, args.repetitions)
        print(json.dumps(result, indent=2, sort_keys=True))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
