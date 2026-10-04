#!/usr/bin/env python3
"""Replay retained LAW16 evidence or execute the available local capture routes.

The default retained mode re-authenticates prior output; it does not rerun
benchmarks.  ``--execute`` requires explicit binary pins and delegates capture
to the existing route runners.  The live agent continuation is a separate,
explicitly expensive opt-in.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent
PROJECT = ROOT.parent.parent
BEND_PIN = "947db722640c86247849343657bf2f7ef01cb7f1"
Z3_VERSION = "Z3 version 4.12.5 - 64 bit"
PIN_KEYS = ("bun", "semaprax", "z3", "clang", "lean", "lean_test_binary", "bendtt")
RETAINED_DIRS = (
    "law16-boolean-negation-peak-rss-v1", "law16-boolean-negation-peak-rss-v2",
    "law16-boolean-negation-nonproof-process-v1", "law16-boolean-negation-proof-verdict-v1",
    "law16-boolean-negation-agent-pilot-v1", "law16-boolean-negation-agent-campaign-v1",
    "law16-boolean-negation-process-v1", "law16-boolean-negation-process-v2",
    "law16-guarded-i64-balance-smt-v1", "full-u32-encoding-controls-v1", "law16-i64-list-proof-v1",
    "law16-guarded-i64-profile-controls-v2", "law16-bounded-balance-v2",
    "bend-u32-sort-universal-v1",
)


def sha(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def module(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / filename)
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot load route verifier: {filename}")
    selected = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(selected)
    return selected


def load_pins(path: Path) -> dict:
    value = json.loads(path.read_text())
    if value.get("schema") != "semaprax.bend2-law-benchmark.local-replay-pins.v1":
        raise ValueError("unsupported local replay pin schema")
    if value.get("bend_commit") != BEND_PIN:
        raise ValueError("local pin file must select the exact committed Bend source revision")
    if not re.fullmatch(r"[0-9a-f]{40}", value.get("semaprax_build_commit", "")):
        raise ValueError("semaprax_build_commit must be a full caller-declared source SHA")
    for key in PIN_KEYS:
        row = value.get("tools", {}).get(key)
        if not isinstance(row, dict) or not isinstance(row.get("path"), str):
            raise ValueError(f"local replay pin missing tools.{key}.path")
        path_value = Path(row["path"]).expanduser().resolve()
        if not path_value.is_file():
            raise ValueError(f"pinned tool is unavailable: {key}")
        expected = row.get("sha256")
        if not isinstance(expected, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", expected):
            raise ValueError(f"local replay pin missing tools.{key}.sha256")
        if sha(path_value) != expected:
            raise ValueError(f"pinned tool digest mismatch: {key}")
        row["path"] = str(path_value)
    if "codex" in value.get("tools", {}):
        row = value["tools"]["codex"]
        if not isinstance(row, dict) or not isinstance(row.get("path"), str):
            raise ValueError("tools.codex must contain a path and sha256")
        codex_path = Path(row["path"]).expanduser().resolve()
        if not codex_path.is_file() or not re.fullmatch(r"sha256:[0-9a-f]{64}", row.get("sha256", "")) or sha(codex_path) != row["sha256"]:
            raise ValueError("optional Codex executable pin is missing or drifted")
        row["path"] = str(codex_path)
    bend_root = Path(value.get("bend_root", "")).expanduser().resolve()
    value["bend_root"] = str(bend_root)
    if not (bend_root / "bend2/main.ts").is_file():
        raise ValueError("pinned Bend source tree is unavailable")
    head = subprocess.check_output(["git", "-C", str(bend_root), "rev-parse", "HEAD"], text=True).strip()
    dirty = subprocess.run(["git", "-C", str(bend_root), "diff", "--quiet", "HEAD", "--", "bend2"], check=False).returncode
    if head != BEND_PIN or dirty:
        raise ValueError("Bend source is not the clean pinned bend2 tree")
    if Path(shutil.which("clang") or "").resolve() != Path(value["tools"]["clang"]["path"]):
        raise ValueError("the native route's PATH clang differs from the pinned clang executable")
    if Path.home() / ".bend" / "bendtt" not in Path(value["tools"]["bendtt"]["path"]).parents:
        raise ValueError("BendTT pin is outside the cache location used by the theorem runner")
    bendtt_source = (bend_root / "bend2/bendtt.lean").read_bytes()
    expected_bendtt = Path.home() / ".bend" / "bendtt" / hashlib.sha256(bendtt_source).hexdigest()[:16] / "bendtt"
    if Path(value["tools"]["bendtt"]["path"]) != expected_bendtt.resolve():
        raise ValueError("BendTT pin does not match the cache key derived by the theorem runner")
    return value


def observe(path: Path, args: list[str], env: dict | None = None) -> dict:
    completed = subprocess.run([str(path), *args], capture_output=True, check=False, timeout=30, env=env)
    if completed.returncode:
        raise ValueError(f"pinned tool version probe failed: {path}")
    return {"argv": [str(path), *args], "stdout": completed.stdout.decode("utf-8", "replace").strip(), "sha256": sha(path)}


def command(argv: list[str], output: Path, name: str, *, env: dict | None = None, timeout: int = 3600) -> dict:
    try:
        result = subprocess.run(argv, cwd=PROJECT, env=env, capture_output=True, timeout=timeout, check=False)
        code, stdout, stderr, timed_out = result.returncode, result.stdout, result.stderr, False
    except subprocess.TimeoutExpired as error:
        code, stdout, stderr, timed_out = None, error.stdout or b"", error.stderr or b"", True
    out_path, err_path = output / f"{name}.stdout", output / f"{name}.stderr"
    out_path.write_bytes(stdout)
    err_path.write_bytes(stderr)
    record = {"argv": argv, "exit_code": code, "timed_out": timed_out, "timeout_seconds": timeout,
              "stdout": {"path": out_path.relative_to(output).as_posix(), "bytes": out_path.stat().st_size, "sha256": sha(out_path)},
              "stderr": {"path": err_path.relative_to(output).as_posix(), "bytes": err_path.stat().st_size, "sha256": sha(err_path)}}
    (output / f"{name}.command.json").write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    if timed_out:
        raise RuntimeError(f"route command timed out ({name}); see retained partial stdout/stderr")
    if code:
        raise RuntimeError(f"route command failed ({name}); see retained stderr")
    return record


def raw_inventory() -> list[dict]:
    rows = []
    base = ROOT / "evidence"
    for dirname in RETAINED_DIRS:
        folder = base / dirname
        if not folder.is_dir() or folder.is_symlink():
            raise ValueError(f"required retained evidence directory is missing: {dirname}")
        for path in sorted(folder.rglob("*")):
            if path.is_symlink():
                raise ValueError(f"symlink found in retained evidence: {path.relative_to(PROJECT)}")
            if path.is_file():
                rows.append({"path": path.relative_to(PROJECT).as_posix(), "bytes": path.stat().st_size, "sha256": sha(path)})
    return rows


def output_inventory(output: Path) -> list[dict]:
    rows = []
    for path in sorted(output.rglob("*")):
        if path.is_symlink():
            raise ValueError(f"symlink found in generated replay output: {path.relative_to(output)}")
        if path.is_file() and path.name != "replay-status.json":
            rows.append({"path": path.relative_to(output).as_posix(), "bytes": path.stat().st_size, "sha256": sha(path)})
    return rows


def verify_fresh_capture(capsule: Path) -> dict:
    """Authenticate a completed non-agent capture without invoking its tools."""
    result = json.loads((capsule / "replay-status.json").read_text())
    if result.get("mode") != "fresh_execution" or result.get("status") != "executed_without_live_agent_campaign_opt_in":
        raise ValueError("fresh capsule is not a completed non-agent execution")
    if result.get("generated_artifacts") != output_inventory(capsule):
        raise ValueError("fresh capsule artifact inventory drifted")
    if result.get("tool_pins") != {"path": "tool-pins.json", "sha256": sha(capsule / "tool-pins.json")}:
        raise ValueError("fresh capsule tool pins drifted")
    pins = json.loads((capsule / "tool-pins.json").read_text())
    if pins.get("bend_commit") != BEND_PIN or any(not re.fullmatch(r"sha256:[0-9a-f]{64}", pins.get("tools", {}).get(key, {}).get("sha256", "")) for key in PIN_KEYS):
        raise ValueError("fresh capsule lacks exact tool pins")
    expected = {
        "boolean_ordinary_check": "boolean_ordinary_check",
        "boolean_verdict_and_z3_process": "boolean_process",
        "boolean_peak_rss": "boolean_rss",
        "guarded_i64_balance_and_sort_controls": "guarded_i64_controls",
        "bend_u32_universal_sort_source_proof": "bend_universal_sort",
        "supplemental_law15_lean_list_theorem": "lean_law15",
        "bounded_balance_agent_evidence_replay": "bounded_balance",
    }
    steps = result.get("steps", [])
    if [step.get("id") for step in steps] != list(expected):
        raise ValueError("fresh capsule route inventory drifted")
    for step in steps:
        record = json.loads((capsule / "logs" / f"{expected[step['id']]}.command.json").read_text())
        if step != {"id": step["id"], **record} or record.get("exit_code") != 0 or record.get("timed_out") is not False:
            raise ValueError("fresh capsule route did not complete successfully")
        if step["id"] == "guarded_i64_balance_and_sort_controls":
            argv = record["argv"]
            if "--semaprax-sha256" not in argv or argv[argv.index("--semaprax-sha256") + 1] != pins["tools"]["semaprax"]["sha256"]:
                raise ValueError("guarded-i64 route lost the prefixed compiler digest")
    module("fresh_nonproof_review", "law16_boolean_negation_nonproof_capsule.py").review(capsule / "boolean-nonproof")
    module("fresh_process_review", "law16_boolean_negation_process_capsule.py").review(capsule / "boolean-process")
    module("fresh_rss_review", "law16_boolean_negation_rss_capsule.py").review_current(capsule / "boolean-rss")
    module("fresh_sort_review", "law16_bend_u32_sort_proof.py").verify(capsule / "bend-u32-sort-proof/capsule.json")
    controls = json.loads((capsule / "guarded-i64-controls/report.json").read_text())
    control_rows = controls.get("cases", []) + controls.get("domain_controls", [])
    if controls.get("status") != "supplemental_controls_pass" or len(control_rows) != 16 or not all(row.get("expected_outcome_observed") is True for row in control_rows):
        raise ValueError("fresh supplemental controls did not pass")
    lean = json.loads((capsule / "lean-law15-capsule.json").read_text())
    route = module("fresh_lean_review", "law16_i64_list_proof_capsule.py")
    raw = b"".join((capsule / "lean-law15-raw" / f"kernel-test.{name}").read_bytes() for name in ("stdout", "stderr"))
    if lean.get("status") != "supplemental_i64_list_profile_proved" or not route.accepted(0, raw):
        raise ValueError("fresh supplemental Lean test did not pass")
    return {"status": "fresh_capture_authenticated", "fresh_route_count": 6, "retained_agent_replay_count": 1,
            "artifact_count": len(result["generated_artifacts"]), "tool_pins": result["tool_pins"],
            "execution_claim": "offline authentication of retained fresh execution; tools were not rerun by this review",
            "agent_campaign": result["agent_campaign"], "nonclaims": result["nonclaims"]}


def verify_retained(output: Path) -> dict:
    output.mkdir(parents=True)
    steps = []
    unavailable = []
    python = sys.executable
    calls = [
        ([python, str(ROOT / "law16_current_report.py"), "--output", str(output / "current-report.json")], "current_report"),
        ([python, str(ROOT / "full_u32_guarded_i64_profile_v2.py"), "--profile", str(ROOT / "fixtures/full-u32-guarded-i64-profile-v2.json")], "guarded_i64_v2"),
        ([python, str(ROOT / "law16_bend_u32_sort_proof.py"), "--verify", str(ROOT / "evidence/bend-u32-sort-universal-v1/capsule.json")], "bend_u32_universal_sort"),
    ]
    for argv, name in calls:
        steps.append({"id": name, **command(argv, output, name)})
    report = json.loads((output / "current-report.json").read_text())
    if not report.get("pins_and_trust", {}).get("bend") or not report.get("pins_and_trust", {}).get("semaprax"):
        raise ValueError("retained Boolean report lacks source/tool pin identities")
    # The report renderer authenticates the LAW15 capsule and its universal
    # Bend comparator itself; this call adds the profile's stronger cross-route
    # boundary checks without depending on an older closure-audit schema.
    closure_audit = module("law16_closure_audit_for_retained_replay", "law16_closure_audit.py")
    lean_result = closure_audit.verify_i64_list_capsule()
    bounded_capsule = module("law16_bounded_balance_capsule_for_replay", "law16_bounded_balance_capsule.py")
    try:
        bounded_review = bounded_capsule.review(ROOT / "evidence/law16-bounded-balance-v2")
        (output / "bounded-balance-review.json").write_text(json.dumps(bounded_review, indent=2, sort_keys=True) + "\n")
    except FileNotFoundError as error:
        unavailable.append({"cell": "bounded_balance_agent_campaign", "status": "incomplete_missing_raw_artifact", "reason": str(error)})
    if report.get("status") != "incomplete" or not report.get("supplemental_universal_list_theorems", {}).get("bend"):
        raise ValueError("current report failed to retain incomplete status or Bend universal theorem evidence")
    (output / "law15-list-proof-review.json").write_text(json.dumps(lean_result, indent=2, sort_keys=True) + "\n")
    return {"mode": "retained_evidence_replay", "status": "retained_evidence_verified" if not unavailable else "partial_retained_evidence",
            "execution_claim": "offline re-authentication only; no benchmark command or timing was rerun",
            "steps": steps, "raw_artifacts": raw_inventory(), "report_status": report["status"], "report_closure_statement": report["closure"],
            "unavailable_cells": unavailable,
            "agent_campaign": "retained capsule authenticated by current-report; no live agent turn invoked",
            "nonclaims": ["does not reproduce cell execution or timings", "does not update closure status", "retained paths and tool identities are historical observations"]}


def execute(output: Path, pins_path: Path, include_agent_campaign: bool) -> dict:
    pins = load_pins(pins_path)
    campaign = None
    if include_agent_campaign:
        campaign = module("law16_live_campaign_constants", "law16_boolean_negation_agent_campaign.py")
        fixed = {"bend_root": Path(campaign.BEND_ROOT).resolve(), "bun": Path(campaign.BUN).resolve(), "semaprax": Path(campaign.SEM).resolve(), "z3": Path(campaign.Z3).resolve()}
        expected = {"bend_root": Path(pins["bend_root"]).resolve(), "bun": Path(pins["tools"]["bun"]["path"]),
                    "semaprax": Path(pins["tools"]["semaprax"]["path"]), "z3": Path(pins["tools"]["z3"]["path"])}
        if fixed != expected:
            raise ValueError("agent campaign runner has fixed tool paths that differ from the supplied pins")
        codex = pins["tools"].get("codex")
        if not codex or not Path(codex.get("path", "")).is_file() or sha(Path(codex["path"])) != codex.get("sha256"):
            raise ValueError("agent campaign opt-in requires a present SHA-pinned Codex executable")
    output.mkdir()
    retained = verify_retained(output / "retained-evidence")
    tools = pins["tools"]
    bend = Path(pins["bend_root"])
    bun = tools["bun"]["path"]
    sem = tools["semaprax"]["path"]
    z3 = tools["z3"]["path"]
    clang = tools["clang"]["path"]
    for name, args in (("bun", ["--version"]), ("semaprax", ["--version"]), ("z3", ["--version"]), ("clang", ["--version"])):
        tools[name]["version"] = observe(Path(tools[name]["path"]), args)
    lean_pin = (PROJECT / "proofs/kernel0-lean/lean-toolchain").read_text().strip()
    lean_env = dict(os.environ, ELAN_TOOLCHAIN=lean_pin)
    tools["lean"]["version"] = observe(Path(tools["lean"]["path"]), ["--version"], lean_env)
    if tools["z3"]["version"]["stdout"] != Z3_VERSION:
        raise ValueError("Z3 version differs from the exact version required by Boolean project proof-check")
    bendtt = Path(tools["bendtt"]["path"])
    version_manifest = output / "tool-pins.json"
    version_manifest.write_text(json.dumps({"schema": "semaprax.bend2-law-benchmark.live-tool-pins.v1", "bend_commit": BEND_PIN,
        "semaprax_build_commit": pins["semaprax_build_commit"], "semaprax_build_association": "caller-declared; not a build attestation",
        "tools": tools}, indent=2, sort_keys=True) + "\n")
    steps = []
    logs = output / "logs"; logs.mkdir()
    env = dict(os.environ, PATH=str(Path(clang).parent) + os.pathsep + os.environ.get("PATH", ""), BEND_NO_TELEMETRY="1")

    # Exact ordinary-check route. Inputs are copies of committed fixtures; the route runner verifies identity.
    inputs = output / "nonproof-inputs"; inputs.mkdir()
    bend_fresh, bend_repeat = inputs / "bend-fresh.bend", inputs / "bend-repeat.bend"
    sem_fresh, sem_repeat = inputs / "semaprax-fresh.spx", inputs / "semaprax-repeat.spx"
    for dest, source in ((bend_fresh, ROOT / "fixtures/bend-boolean-negation-v1.bend"), (bend_repeat, ROOT / "fixtures/bend-boolean-negation-v1.bend"),
                         (sem_fresh, ROOT / "fixtures/semaprax-boolean-negation-v1.spx"), (sem_repeat, ROOT / "fixtures/semaprax-boolean-negation-v1.spx")):
        shutil.copyfile(source, dest)
    nonproof = output / "boolean-nonproof"
    argv = [sys.executable, str(ROOT / "law16_boolean_negation_nonproof_process.py"), "--bend-root", str(bend), "--bun", bun, "--semaprax", sem,
            "--bend-fresh", str(bend_fresh), "--bend-repeat", str(bend_repeat), "--semaprax-fresh", str(sem_fresh), "--semaprax-repeat", str(sem_repeat), "--output-root", str(nonproof)]
    steps.append({"id": "boolean_ordinary_check", **command(argv, logs, "boolean_ordinary_check", env=env)})
    nonproof_review = module("law16_nonproof_review_for_replay", "law16_boolean_negation_nonproof_capsule.py").review(nonproof)
    (output / "boolean-nonproof-review.json").write_text(json.dumps(nonproof_review, indent=2, sort_keys=True) + "\n")

    process = output / "boolean-process"
    argv = [sys.executable, str(ROOT / "law16_boolean_negation_process.py"), "--bend-root", str(bend), "--bun", bun, "--semaprax", sem, "--z3", z3, "--output", str(process)]
    steps.append({"id": "boolean_verdict_and_z3_process", **command(argv, logs, "boolean_process", env=env)})
    process_review = module("law16_process_review_for_replay", "law16_boolean_negation_process_capsule.py").review(process)
    (output / "boolean-process-review.json").write_text(json.dumps(process_review, indent=2, sort_keys=True) + "\n")

    rss = output / "boolean-rss"
    argv = [sys.executable, str(ROOT / "law16_boolean_negation_rss_current.py"), "--bend-root", str(bend), "--bun", bun, "--semaprax", sem, "--z3", z3, "--output", str(rss)]
    steps.append({"id": "boolean_peak_rss", **command(argv, logs, "boolean_rss", env=env)})
    rss_review = module("law16_rss_review_for_replay", "law16_boolean_negation_rss_capsule.py").review_current(rss)
    (output / "boolean-rss-review.json").write_text(json.dumps(rss_review, indent=2, sort_keys=True) + "\n")

    controls = output / "guarded-i64-controls"
    argv = [sys.executable, str(ROOT / "full_u32_encoding_controls.py"), "--bend-root", str(bend), "--bun", bun, "--semaprax", sem, "--z3", z3,
            "--semaprax-sha256", tools["semaprax"]["sha256"], "--semaprax-build-commit", pins["semaprax_build_commit"], "--artifacts", str(controls)]
    steps.append({"id": "guarded_i64_balance_and_sort_controls", **command(argv, logs, "guarded_i64_controls", env=env)})

    bend_proof = output / "bend-u32-sort-proof"
    argv = [sys.executable, str(ROOT / "law16_bend_u32_sort_proof.py"), "--bend-root", str(bend), "--bun", bun, "--output-dir", str(bend_proof)]
    steps.append({"id": "bend_u32_universal_sort_source_proof", **command(argv, logs, "bend_universal_sort", env=env)})

    lean_raw, lean_output = output / "lean-law15-raw", output / "lean-law15-capsule.json"
    argv = [sys.executable, str(ROOT / "law16_i64_list_proof_capsule.py"), "--test-binary", tools["lean_test_binary"]["path"], "--lean", tools["lean"]["path"],
            "--raw-artifact-dir", str(lean_raw), "--output", str(lean_output)]
    steps.append({"id": "supplemental_law15_lean_list_theorem", **command(argv, logs, "lean_law15", env=env)})

    bounded_review = output / "retained-bounded-balance-agent-review.json"
    argv = [sys.executable, str(ROOT / "law16_bounded_balance_capsule.py"), "--capsule", str(ROOT / "evidence/law16-bounded-balance-v2"), "--output", str(bounded_review)]
    steps.append({"id": "bounded_balance_agent_evidence_replay", **command(argv, logs, "bounded_balance", env=env)})

    agent_status = "not_reexecuted; existing capsule is available in retained-evidence mode"
    if include_agent_campaign:
        codex = tools["codex"]
        codex["version"] = observe(Path(codex["path"]), ["--version"])
        env["PATH"] = str(Path(codex["path"]).parent) + os.pathsep + env["PATH"]
        agent_output = output / "boolean-agent-campaign"
        argv = [sys.executable, str(ROOT / "law16_boolean_negation_agent_campaign.py"), "--first", "2", "--last", "10", "--output", str(agent_output)]
        steps.append({"id": "boolean_ten_pair_agent_campaign_continuation", "opt_in": True, **command(argv, logs, "agent_campaign", env=env, timeout=9000)})
        reviewed = module("law16_agent_campaign_review_for_replay", "law16_boolean_negation_agent_campaign_capsule.py").review(agent_output)
        (output / "agent-campaign-review.json").write_text(json.dumps(reviewed, indent=2, sort_keys=True) + "\n")
        agent_status = "nine new matched pairs executed; together with retained pilot ordinal 1 this is a ten-pair series"

    result_status = "executed_with_live_agent_campaign" if include_agent_campaign else "executed_without_live_agent_campaign_opt_in"
    if retained["status"] != "retained_evidence_verified":
        result_status = "partial_retained_evidence_after_fresh_capture"
    return {"mode": "fresh_execution", "status": result_status,
            "tool_pins": {"path": version_manifest.name, "sha256": sha(version_manifest)}, "retained_evidence_replay": retained,
            "steps": steps, "generated_artifacts": output_inventory(output),
            "agent_campaign": agent_status,
            "nonclaims": ["supplemental guarded-i64 controls are not checked-u32 manifest admission", "LAW15 Lean proof is not a LAW16 source identity", "Bend universal sort proof has no matched Semaprax law16.* source certificate or timing", "Semaprax build commit is caller-declared and not a build attestation", "no issue closure or cross-route winner is implied"]}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--verify-retained", action="store_true", help="cheap offline review of retained capsules; does not rerun a cell")
    mode.add_argument("--verify-fresh", type=Path, metavar="CAPSULE", help="offline authenticate a completed non-agent fresh capture")
    mode.add_argument("--execute", action="store_true", help="fresh capture through existing route runners; requires an explicit pin file")
    parser.add_argument("--pins", type=Path, help="local JSON tool pins required by --execute")
    parser.add_argument("--include-agent-campaign", action="store_true", help="opt in to nine new Codex pairs; the retained ordinal-1 pilot supplies pair ten")
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args(argv)
    if args.output_dir.exists() or not args.output_dir.parent.is_dir():
        parser.error("output directory must be new below an existing parent")
    args.output_dir = args.output_dir.resolve()
    if args.pins:
        args.pins = args.pins.expanduser().resolve()
    if not args.execute and (args.pins or args.include_agent_campaign):
        parser.error("--pins and --include-agent-campaign require --execute")
    if args.execute and not args.pins:
        parser.error("--execute requires --pins so every executable and source revision is explicitly identified")
    try:
        if args.verify_fresh:
            result = verify_fresh_capture(args.verify_fresh.expanduser().resolve())
            args.output_dir.mkdir()
        else:
            result = verify_retained(args.output_dir) if args.verify_retained else execute(args.output_dir, args.pins, args.include_agent_campaign)
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        if args.output_dir.exists():
            failure = {"mode": "fresh_execution" if args.execute else "retained_evidence_replay", "status": "failed_closed", "reason": str(error),
                       "generated_artifacts": output_inventory(args.output_dir)}
            (args.output_dir / "replay-status.json").write_text(json.dumps(failure, indent=2, sort_keys=True) + "\n")
        parser.error(str(error))
    (args.output_dir / "replay-status.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(f"{result['status'] if 'status' in result else 'retained_evidence_verified'}: {args.output_dir / 'replay-status.json'}")
    return 0 if result.get("status") in ("retained_evidence_verified", "fresh_capture_authenticated", "executed_without_live_agent_campaign_opt_in", "executed_with_live_agent_campaign") else 1


if __name__ == "__main__":
    raise SystemExit(main())
