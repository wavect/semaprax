#!/usr/bin/env python3
"""Capture and review a matched, multi-module Boolean provider edit in Linux.

This supplemental cell preserves the original checked-u32 cell as unsupported.
It measures ordinary checker process time, not incremental cache reuse or a
cross-language performance winner.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parent
FIXTURE = ROOT / "fixtures/law16-boolean-project-edit-v1"
SCHEMA = "semaprax.bend2-law-benchmark.boolean-project-edit.v1"
BEND_COMMIT = "947db722640c86247849343657bf2f7ef01cb7f1"
TOOL_SHA256 = {
    "bun": "c356f5fb6f75a0a83d918ee2391bc4ec1e6061baa08b0ee1c3a5cfb4aaf3d567",
    "semaprax": "5d2ecf0a63ce86c967e3f2f1a2f509c71cdfbb4aacddc32e856a17bb1e8d7b97",
    "bendtt": "72e11a86f44563e9decb26fe1c586ed7d5465e98ca282ada5ae8753b1156cad5",
    "bend_main": "c48a0274e661a3e529933550752578768e8f029219d814073fbf86537271d475",
}
GUEST_TOOL_PATHS = {
    "bun": "/bun-dir/bun",
    "semaprax": "/sem-dir/semaprax",
    "bendtt": "/kernel/bendtt",
    "bend_main": "/bend-root/bend2/main.ts",
}
STATES = ("before", "after")
LANES = ("bend", "semaprax")
REPETITIONS = 30
NONCLAIMS = [
    "the original checked-u32 project-incremental cell remains unsupported and is not replaced by this Boolean profile",
    "this is a four-source SEMAPRAX Project and a three-module Bend import closure, not a large-project claim",
    "SEMAPRAX ordinary check does not discharge its source postcondition; Bend ordinary check is distinct from the separately recorded verdict-kernel route",
    "process times include launch and guest-local cache state is uncontrolled; no cold/warm, incremental reuse, cross-language ratio, or winner is claimed",
    "the SEMAPRAX executable SHA binds bytes but its source-build association is historical local provenance, not a reproducible-build attestation",
]


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def reference(path: Path, root: Path) -> dict:
    return {"path": path.relative_to(root).as_posix(), "bytes": path.stat().st_size, "sha256": sha(path)}


def checked(root: Path, item: dict) -> Path:
    if not isinstance(item, dict) or set(item) != {"path", "bytes", "sha256"}:
        raise ValueError("malformed raw reference")
    rel = Path(item["path"])
    if rel.is_absolute() or not rel.parts or ".." in rel.parts:
        raise ValueError("raw reference escapes capsule")
    path = root.joinpath(*rel.parts)
    if path.is_symlink() or not path.is_file() or reference(path, root) != item:
        raise ValueError("raw reference differs")
    return path


def fixture_inventory(root: Path) -> dict[str, dict]:
    return {p.relative_to(root).as_posix(): reference(p, root)
            for p in sorted(root.rglob("*")) if p.is_file() and not p.is_symlink()}


def assert_edit_scope(root: Path) -> None:
    for lane, core in (("bend", "core.bend"), ("semaprax", "core/core.spx")):
        baseline = {name.removeprefix(f"{lane}-before/"): row["sha256"]
                    for name, row in fixture_inventory(root).items() if name.startswith(f"{lane}-before/")}
        if not baseline or core not in baseline:
            raise ValueError("project edit baseline is incomplete")
        for state in ("after", "attack"):
            candidate = {name.removeprefix(f"{lane}-{state}/"): row["sha256"]
                         for name, row in fixture_inventory(root).items() if name.startswith(f"{lane}-{state}/")}
            if set(candidate) != set(baseline) or candidate[core] == baseline[core] or any(
                candidate[name] != baseline[name] for name in baseline if name != core
            ):
                raise ValueError(f"{lane} {state} must change only the provider body/signature file")


def command(lane: str, state: str, inputs: Path, bun: str, semaprax: str, bend_main: str, verdict=False) -> list[str]:
    if lane == "bend":
        return [bun, bend_main, str(inputs / f"bend-{state}/main.bend"), "--verdict" if verdict else "--check-only"]
    return [semaprax, "check", str(inputs / f"semaprax-{state}/semaprax.toml")]


def run(argv: list[str], raw: Path, label: str, env: dict[str, str]) -> dict:
    start = time.monotonic_ns()
    try:
        child = subprocess.run(argv, capture_output=True, env=env, timeout=30, check=False)
        code, stdout, stderr, timed_out = child.returncode, child.stdout, child.stderr, False
    except subprocess.TimeoutExpired as error:
        code, stdout, stderr, timed_out = None, error.stdout or b"", error.stderr or b"", True
    elapsed = time.monotonic_ns() - start
    if len(stdout) > 65536 or len(stderr) > 65536:
        raise ValueError("checker stream exceeds 64 KiB")
    out, err = raw / f"{label}.stdout", raw / f"{label}.stderr"
    out.write_bytes(stdout)
    err.write_bytes(stderr)
    return {"argv": argv, "exit_code": code, "timed_out": timed_out, "elapsed_ns": elapsed,
            "stdout": reference(out, raw), "stderr": reference(err, raw)}


def accepted(lane: str, state: str, row: dict, raw: Path, verdict=False) -> bool:
    stdout = checked(raw, row["stdout"]).read_bytes()
    stderr = checked(raw, row["stderr"]).read_bytes()
    if row["timed_out"] or row["elapsed_ns"] <= 0:
        return False
    if state == "attack":
        marker = b"SOME PROOFS FAIL" if lane == "bend" else b"error[SPX-T204]"
        return row["exit_code"] != 0 and marker in stderr
    if row["exit_code"] != 0 or stderr:
        return False
    if lane == "bend":
        return b"ALL PROOFS CHECK" in stdout
    return stdout.startswith(b"verified project law16-boolean-project-edit")


def quantiles(values: list[int]) -> dict:
    ordered = sorted(values)
    return {"count": len(values), "p50_ns": statistics.median(values),
            "p95_ns": ordered[(95 * len(ordered) + 99) // 100 - 1]}


def capture(output: Path, fixture: Path, bun: Path, semaprax: Path, bend_root: Path, bendtt: Path) -> dict:
    if output.exists() or not output.parent.is_dir():
        raise ValueError("capture output must be new beneath an existing directory")
    paths = {"bun": bun, "semaprax": semaprax, "bendtt": bendtt, "bend_main": bend_root / "bend2/main.ts"}
    if {name: str(path) for name, path in paths.items()} != GUEST_TOOL_PATHS:
        raise ValueError("guest tool paths differ from the fixed capture profile")
    if any(not path.is_file() or sha(path) != TOOL_SHA256[name] for name, path in paths.items()):
        raise ValueError("one or more tool bytes differ from the fixed pin")
    head = subprocess.check_output(["git", "-C", str(bend_root), "rev-parse", "HEAD"], text=True).strip()
    dirty = subprocess.check_output(["git", "-C", str(bend_root), "status", "--porcelain", "--untracked-files=all", "--", "bend2"], text=True)
    if head != BEND_COMMIT or dirty:
        raise ValueError("Bend source differs from the clean pinned revision")
    assert_edit_scope(fixture)
    output.mkdir()
    inputs, raw = output / "inputs", output / "raw"
    shutil.copytree(fixture, inputs)
    raw.mkdir()
    inventory = fixture_inventory(inputs)
    if inventory != fixture_inventory(fixture):
        raise ValueError("fixture bytes changed while staging")
    env = dict(os.environ, BEND_NO_TELEMETRY="1", DO_NOT_TRACK="1", BENDTT=str(bendtt))
    args = GUEST_TOOL_PATHS
    observations = {}
    for lane in LANES:
        for state in STATES:
            argv = command(lane, state, inputs, args["bun"], args["semaprax"], args["bend_main"])
            row = run(argv, raw, f"preflight-{lane}-{state}", env)
            if not accepted(lane, state, row, raw):
                raise ValueError(f"{lane} {state} preflight failed")
            observations[f"{lane}_{state}"] = row
        attack = run(command(lane, "attack", inputs, args["bun"], args["semaprax"], args["bend_main"]), raw, f"attack-{lane}", env)
        if not accepted(lane, "attack", attack, raw):
            raise ValueError(f"{lane} signature attack was not rejected")
        observations[f"{lane}_attack"] = attack
    for state in STATES:
        verdict = run(command("bend", state, inputs, args["bun"], args["semaprax"], args["bend_main"], True), raw, f"verdict-bend-{state}", env)
        if not accepted("bend", state, verdict, raw, verdict=True):
            raise ValueError(f"Bend {state} verdict kernel failed")
        observations[f"bend_{state}_verdict"] = verdict
    samples = []
    for ordinal in range(1, REPETITIONS + 1):
        row = {"ordinal": ordinal, "checks": {}}
        for lane in LANES:
            for state in STATES:
                label = f"{ordinal:02d}-{lane}-{state}"
                item = run(command(lane, state, inputs, args["bun"], args["semaprax"], args["bend_main"]), raw, label, env)
                if not accepted(lane, state, item, raw):
                    raise ValueError(f"{label} failed")
                row["checks"][f"{lane}_{state}"] = item
        samples.append(row)
    result = {"schema": SCHEMA, "status": "matched_boolean_project_edit_observed", "repetitions": REPETITIONS,
              "guest_capture_root": str(output), "bend_commit": head,
              "tool_pins": {name: reference(path, path.parent) for name, path in paths.items()},
              "guest": {"uname": platform.uname()._asdict(), "cpu_count": os.cpu_count(),
                        "cpuinfo_sha256": sha(Path("/proc/cpuinfo")), "os_release_sha256": sha(Path("/etc/os-release"))},
              "environment": {"BEND_NO_TELEMETRY": "1", "DO_NOT_TRACK": "1"},
              "inputs": inventory, "observations": observations, "samples": samples, "nonclaims": NONCLAIMS}
    (output / "result.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    return review(output)


def review(output: Path) -> dict:
    result = json.loads((output / "result.json").read_text())
    if result.get("schema") != SCHEMA or result.get("status") != "matched_boolean_project_edit_observed" or result.get("repetitions") != REPETITIONS:
        raise ValueError("project edit capture identity differs")
    if result.get("nonclaims") != NONCLAIMS or result.get("environment") != {"BEND_NO_TELEMETRY": "1", "DO_NOT_TRACK": "1"}:
        raise ValueError("project edit capture scope differs")
    if result.get("bend_commit") != BEND_COMMIT or {name: item.get("sha256") for name, item in result.get("tool_pins", {}).items()} != TOOL_SHA256:
        raise ValueError("project edit tool/source pins differ")
    inputs, raw = output / "inputs", output / "raw"
    if result.get("inputs") != fixture_inventory(inputs) or fixture_inventory(inputs) != fixture_inventory(FIXTURE):
        raise ValueError("project edit source bytes differ")
    assert_edit_scope(inputs)
    capture_root = Path(result["guest_capture_root"])
    if not capture_root.is_absolute():
        raise ValueError("guest capture path is malformed")
    for name, item in result["tool_pins"].items():
        if item["path"] != Path(GUEST_TOOL_PATHS[name]).name or item["sha256"] != TOOL_SHA256[name] or item["bytes"] <= 0:
            raise ValueError("tool identity differs")
    observed = result.get("observations", {})
    expected_names = {f"{lane}_{state}" for lane in LANES for state in (*STATES, "attack")}
    expected_names |= {f"bend_{state}_verdict" for state in STATES}
    if set(observed) != expected_names:
        raise ValueError("project edit route inventory differs")
    for name, row in observed.items():
        lane, state = name.split("_", 1)
        verdict = state.endswith("_verdict")
        state = state.removesuffix("_verdict")
        if row.get("argv") != command(lane, state, capture_root / "inputs", GUEST_TOOL_PATHS["bun"], GUEST_TOOL_PATHS["semaprax"], GUEST_TOOL_PATHS["bend_main"], verdict):
            raise ValueError(f"{name} retained command differs")
        if not accepted(lane, state, row, raw, verdict):
            raise ValueError(f"{name} retained outcome differs")
    if len(result.get("samples", [])) != REPETITIONS:
        raise ValueError("project edit sample count differs")
    times = {f"{lane}_{state}": [] for lane in LANES for state in STATES}
    for ordinal, sample in enumerate(result["samples"], 1):
        if sample.get("ordinal") != ordinal or set(sample.get("checks", {})) != set(times):
            raise ValueError("project edit sample order differs")
        for name, row in sample["checks"].items():
            lane, state = name.split("_", 1)
            if row.get("argv") != command(lane, state, capture_root / "inputs", GUEST_TOOL_PATHS["bun"], GUEST_TOOL_PATHS["semaprax"], GUEST_TOOL_PATHS["bend_main"]):
                raise ValueError("project edit sample command differs")
            if not accepted(lane, state, row, raw):
                raise ValueError("project edit raw checker output differs")
            times[name].append(row["elapsed_ns"])
    return {"status": result["status"], "repetitions_per_ordinary_route": REPETITIONS,
            "ordinary_check_summaries": {name: quantiles(values) for name, values in times.items()},
            "negative_controls": ["bend_attack", "semaprax_attack"],
            "bend_verdict_routes": ["bend_before_verdict", "bend_after_verdict"], "nonclaims": NONCLAIMS}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", type=Path)
    parser.add_argument("--review", type=Path)
    parser.add_argument("--fixture", type=Path, default=FIXTURE)
    parser.add_argument("--bun", type=Path)
    parser.add_argument("--semaprax", type=Path)
    parser.add_argument("--bend-root", type=Path)
    parser.add_argument("--bendtt", type=Path)
    args = parser.parse_args()
    try:
        if args.review:
            value = review(args.review)
        else:
            if not args.capture or not all((args.bun, args.semaprax, args.bend_root, args.bendtt)):
                parser.error("capture needs output and all pinned tool paths")
            value = capture(args.capture, args.fixture, args.bun, args.semaprax, args.bend_root, args.bendtt)
        print(json.dumps(value, indent=2, sort_keys=True))
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
