#!/usr/bin/env python3
"""Measure the local interpreter development loops using one exact fixture.

The runner never builds a compiler.  Supply an already-built ``semaprax``
binary, so Cargo time cannot enter a sample.  It records real JSONL replies and
refuses a missing or changed fixture instead of turning a failed run into a
small timing number.
"""
from __future__ import annotations

import argparse, hashlib, json, os, pathlib, platform, shutil, statistics
import subprocess, tempfile, time

SUITE = pathlib.Path(__file__).resolve().parent
ROOT = SUITE.parent.parent
MANIFEST = SUITE / "manifest.json"
SCHEMA = "semaprax.hot-reload-benchmark.v1"
CONTROL = "semaprax.hot-reload-control.v1"


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"


def validate(manifest):
    if set(manifest) != {"schema", "scope", "compiler", "lanes", "fixture", "transitions", "scenario_matrix", "required_test_counts", "nonclaims"}:
        raise ValueError("acceptance manifest has an unknown or missing top-level field")
    if manifest["schema"] != "semaprax.hot-reload-acceptance.v1" or manifest["scope"] != "interpreter-only":
        raise ValueError("acceptance manifest does not select the interpreter-only v1 contract")
    if manifest["compiler"] != {"revision": "git-head-at-run", "binary_digest": "captured-in-report"}:
        raise ValueError("acceptance manifest does not bind the compiler subject at run time")
    if manifest["required_test_counts"] != {"interpreter-save-to-ack": 1, "full-restart": 1, "authenticated-warm-restart": 1}:
        raise ValueError("acceptance manifest has an incomplete required-test inventory")
    if "source-Agent journey" not in manifest["nonclaims"]:
        raise ValueError("interpreter benchmark must explicitly exclude the Agent journey")
    scenarios = manifest["scenario_matrix"]
    expected_scenarios = {
        "cold-small-a-to-b", "warm-repeated-a-b-a", "no-op", "multi-module-import-closure", "failed-edit-repair"
    }
    if {item.get("id") for item in scenarios} != expected_scenarios:
        raise ValueError("acceptance manifest has an incomplete scenario matrix")
    if any(set(item) != {"id", "availability", "description"} or item["availability"] != "local" for item in scenarios):
        raise ValueError("scenario matrix has an invalid local scenario")
    for item in manifest["fixture"]["sources"]:
        path = SUITE / item["path"]
        if set(item) != {"path", "sha256"} or not path.is_file() or digest(path) != item["sha256"]:
            raise ValueError("fixture digest mismatch: " + item.get("path", "<missing>"))


def run(command, *, input=None):
    started = time.perf_counter_ns()
    result = subprocess.run(command, input=input, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    elapsed = (time.perf_counter_ns() - started) / 1_000_000
    if result.returncode:
        raise RuntimeError("command failed: %s\n%s" % (" ".join(map(str, command)), result.stderr))
    return elapsed, result.stdout


def reply(process, request_id, op):
    sent = time.perf_counter_ns()
    process.stdin.write(canonical({"schema": CONTROL, "id": request_id, "op": op}))
    process.stdin.flush()
    line = process.stdout.readline()
    elapsed = (time.perf_counter_ns() - sent) / 1_000_000
    if not line:
        raise RuntimeError("development process closed before its reply")
    value = json.loads(line)
    if value.get("schema") != CONTROL or value.get("id") != request_id:
        raise RuntimeError("development process returned an uncorrelated reply")
    return elapsed, value


def fixture():
    # macOS often exposes the default temporary directory through `/var`, a
    # symlink that the Project loader deliberately refuses as an ancestor.
    root = pathlib.Path(
        tempfile.mkdtemp(
            prefix="semaprax-hot-reload-benchmark-",
            dir=pathlib.Path(tempfile.gettempdir()).resolve(),
        )
    )
    for relative, destination in [("semaprax.toml", "semaprax.toml"), ("a/src/app.spx", "src/app.spx"),
                                  ("b/src/app.spx", "b/src/app.spx"), ("shared/src/core.spx", "src/core.spx"),
                                  ("shared/src/tests.spx", "src/tests.spx")]:
        target = root / destination
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(SUITE / "fixtures" / relative, target)
    return root


def outcome(row, expected):
    actual = row.get("invocation", {}).get("outcome")
    if actual != expected:
        raise RuntimeError("unexpected invocation outcome: " + repr(actual))


def phase_ms(reply_value, key, *, required):
    timings = reply_value.get("phase_timings_ns")
    value = timings.get(key) if isinstance(timings, dict) else None
    if value is None:
        if required:
            raise RuntimeError("JSONL response omitted required phase timing: " + key)
        return None
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise RuntimeError("JSONL response returned invalid phase timing: " + key)
    return value / 1_000_000


def start_session(binary, root):
    process = subprocess.Popen([binary, "dev", str(root / "semaprax.toml"), "--jsonl", "--interpreter"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    _, started = reply(process, 1, "start")
    if started.get("event") != "started":
        raise RuntimeError("interpreter session did not start")
    return process, 2


def stop_session(process, request_id):
    _, stopped = reply(process, request_id, "stop")
    if stopped.get("event") != "stopped":
        raise RuntimeError("session did not stop")
    process.wait(timeout=10)
    if process.returncode:
        raise RuntimeError(process.stderr.read())


def invoke(process, request_id, expected):
    _, value = reply(process, request_id, "invoke")
    outcome(value, expected)
    return request_id + 1


def plan_after_write(process, request_id, root, writes, expected_event, expected_decision="eligible_code_replacement", expected_reason=None):
    saved = time.perf_counter_ns()
    for source, destination in writes:
        shutil.copyfile(SUITE / "fixtures" / source, root / destination)
    write_ms = (time.perf_counter_ns() - saved) / 1_000_000
    plan_ms, planned = reply(process, request_id, "plan")
    if planned.get("event") != expected_event:
        raise RuntimeError("unexpected candidate plan event: " + repr(planned.get("event")))
    if expected_event == "candidate_admitted" and planned.get("plan", {}).get("decision") != expected_decision:
        raise RuntimeError("saved source produced the wrong replacement decision")
    if expected_reason is not None and planned.get("plan", {}).get("reason") != expected_reason:
        raise RuntimeError("saved source produced the wrong replacement reason")
    admitted = expected_event == "candidate_admitted"
    source_admission_check_ms = phase_ms(planned, "source_admission_check_ns", required=admitted)
    candidate_preparation_ms = phase_ms(planned, "candidate_preparation_ns", required=admitted)
    return request_id + 1, {
        "write_ms": write_ms,
        "save_to_plan_response_ms": (time.perf_counter_ns() - saved) / 1_000_000,
        "plan_control_round_trip_ms": plan_ms,
        "debounce_wait_ms": 0.0,
        "source_admission_check_ms": source_admission_check_ms,
        "candidate_preparation_ms": candidate_preparation_ms,
        "safe_point_wait_ms": 0.0,
        "stage_limitations": [
            "timings cover authenticated admission/check and HR-01 candidate admission; unchanged or rejected polls report null",
            "the synchronous plan request triggers polling directly, with no debounce interval",
            "the fixture has no outstanding invocation, so safe-point wait is exactly zero",
        ],
    }


def activate(process, request_id, phase):
    round_trip_ms, activated = reply(process, request_id, "activate")
    if activated.get("event") != "activated":
        raise RuntimeError("candidate did not receive an activation acknowledgement")
    phase["activation_control_round_trip_ms"] = round_trip_ms
    phase["activation_pivot_ms"] = phase_ms(activated, "activation_pivot_ns", required=True)
    phase["save_to_ack_ms"] = phase["save_to_plan_response_ms"] + round_trip_ms
    return request_id + 1


def scenario(binary, scenario_id):
    root = fixture()
    process = None
    try:
        process, request_id = start_session(binary, root)
        request_id = invoke(process, request_id, {"kind": "returned", "value": 42})
        phases = []
        if scenario_id == "no-op":
            saved = time.perf_counter_ns()
            shutil.copyfile(SUITE / "fixtures/a/src/app.spx", root / "src/app.spx")
            plan_ms, planned = reply(process, request_id, "plan")
            if planned.get("event") != "unchanged":
                raise RuntimeError("unchanged source did not produce an unchanged plan")
            phases.append({"save_to_plan_response_ms": (time.perf_counter_ns() - saved) / 1_000_000,
                           "plan_control_round_trip_ms": plan_ms,
                           "debounce_wait_ms": 0.0,
                           "source_admission_check_ms": phase_ms(planned, "source_admission_check_ns", required=False),
                           "candidate_preparation_ms": phase_ms(planned, "candidate_preparation_ns", required=False), "safe_point_wait_ms": 0.0,
                           "stage_limitations": ["no candidate is built for an unchanged Project"]})
            request_id += 1
        elif scenario_id == "cold-small-a-to-b":
            request_id, phase = plan_after_write(process, request_id, root, [("b/src/app.spx", "src/app.spx")], "candidate_admitted")
            request_id = activate(process, request_id, phase); phases.append(phase)
            request_id = invoke(process, request_id, {"kind": "returned", "value": 48})
        elif scenario_id == "warm-repeated-a-b-a":
            for source, expected in (("b/src/app.spx", 48), ("a/src/app.spx", 42)):
                request_id, phase = plan_after_write(process, request_id, root, [(source, "src/app.spx")], "candidate_admitted")
                request_id = activate(process, request_id, phase); phases.append(phase)
                request_id = invoke(process, request_id, {"kind": "returned", "value": expected})
        elif scenario_id == "multi-module-import-closure":
            request_id, phase = plan_after_write(process, request_id, root, [("c/src/core.spx", "src/core.spx"), ("c/src/tests.spx", "src/tests.spx")], "candidate_admitted", "unsupported_restart_required", "incompatible_closure")
            phase["activation_control_round_trip_ms"] = None; phase["activation_pivot_ms"] = None; phase["save_to_ack_ms"] = None
            phase["decision"] = "unsupported_restart_required"; phase["reason"] = "incompatible_closure"
            phases.append(phase)
            request_id = invoke(process, request_id, {"kind": "returned", "value": 42})
        elif scenario_id == "failed-edit-repair":
            request_id, rejected = plan_after_write(process, request_id, root, [("invalid/src/app.spx", "src/app.spx")], "candidate_rejected")
            rejected["activation_control_round_trip_ms"] = None; rejected["activation_pivot_ms"] = None; rejected["save_to_ack_ms"] = None
            phases.append(rejected)
            request_id = invoke(process, request_id, {"kind": "returned", "value": 42})
            request_id, repaired = plan_after_write(process, request_id, root, [("b/src/app.spx", "src/app.spx")], "candidate_admitted")
            request_id = activate(process, request_id, repaired); phases.append(repaired)
            request_id = invoke(process, request_id, {"kind": "returned", "value": 48})
        else:
            raise ValueError("unsupported benchmark scenario: " + scenario_id)
        stop_session(process, request_id)
        process = None
        return {"scenario": scenario_id, "phases": phases, "clean_stop": {"event": "stopped", "process_exit": 0}}
    finally:
        if process is not None and process.poll() is None:
            process.kill()
            process.communicate(timeout=10)
        shutil.rmtree(root, ignore_errors=True)


def hot(binary):
    """Compatibility projection for the legacy single A-to-B loop."""
    record = scenario(binary, "cold-small-a-to-b")
    return {**record["phases"][0], "clean_stop": record["clean_stop"]}


def full_restart(binary):
    root = fixture()
    process = None
    try:
        save = time.perf_counter_ns(); shutil.copyfile(SUITE / "fixtures/b/src/app.spx", root / "src/app.spx")
        launched = time.perf_counter_ns()
        process = subprocess.Popen([binary, "dev", str(root / "semaprax.toml"), "--jsonl", "--interpreter"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        _, started = reply(process, 1, "start")
        if started.get("event") != "started": raise RuntimeError("restart did not start")
        _, invoked = reply(process, 2, "invoke")
        outcome(invoked, {"kind": "returned", "value": 48})
        acknowledged = time.perf_counter_ns()
        _, stopped = reply(process, 3, "stop")
        if stopped.get("event") != "stopped": raise RuntimeError("restart did not stop")
        process.wait(timeout=10)
        if process.returncode: raise RuntimeError(process.stderr.read())
        return {"save_to_ack_ms": (acknowledged - save) / 1_000_000, "process_start_to_ack_ms": (acknowledged - launched) / 1_000_000, "clean_stop": {"event": "stopped", "process_exit": 0}}
    finally:
        if process is not None and process.poll() is None:
            process.kill()
            process.communicate(timeout=10)
        shutil.rmtree(root, ignore_errors=True)


def warm_restart(binary):
    root = fixture(); store = root / "store"; store.mkdir(mode=0o700)
    try:
        run([binary, "semantic-cache-init", str(store)])
        _, receipt = run([binary, "semantic-cache-persist", str(root / "semaprax.toml"), str(store)])
        old = json.loads(receipt)["entry_digest"]
        save = time.perf_counter_ns(); shutil.copyfile(SUITE / "fixtures/b/src/app.spx", root / "src/app.spx")
        refresh_ms, refreshed = run([binary, "semantic-cache-refresh", str(root / "semaprax.toml"), str(store), old])
        entry = json.loads(refreshed)["entry_digest"]
        warm_ms, opened = run([binary, "semantic-cache-warm-open", str(root / "semaprax.toml"), str(store), entry])
        if json.loads(opened).get("schema") != "semaprax.semantic-cache-warm-open.v1": raise RuntimeError("warm restart did not authenticate and open B")
        return {"save_to_ack_ms": (time.perf_counter_ns() - save) / 1_000_000, "refresh_ms": refresh_ms, "warm_open_ms": warm_ms}
    finally:
        shutil.rmtree(root, ignore_errors=True)


def summary(samples):
    ordered = sorted(samples); n = len(ordered)
    return {"samples": n, "median_ms": round(statistics.median(ordered), 3), "p95_ms": round(ordered[min(n - 1, max(0, (95 * n + 99) // 100 - 1))], 3), "values_ms": [round(value, 3) for value in samples]}


def scenario_summary(records):
    values = [
        phase.get("save_to_ack_ms", phase["plan_control_round_trip_ms"])
        for record in records
        for phase in record["phases"]
        if phase.get("save_to_ack_ms") is not None
    ]
    if not values:
        values = [phase["plan_control_round_trip_ms"] for record in records for phase in record["phases"]]
    return summary(values)


def compiler_subject(binary, expected_commit):
    _, output = run([binary, "version", "--json"])
    try:
        version = json.loads(output)
    except json.JSONDecodeError as error:
        raise RuntimeError("compiler version output is not JSON") from error
    if set(version) != {"schema", "version", "commit", "maturity", "rust_min"} or version["schema"] != "semaprax.version.v1":
        raise RuntimeError("compiler version output has an unexpected schema")
    commit = version["commit"]
    if commit is not None and (not isinstance(commit, str) or len(commit) != 40 or any(character not in "0123456789abcdef" for character in commit)):
        raise RuntimeError("compiler version output has an invalid commit")
    if expected_commit is not None and commit != expected_commit:
        raise RuntimeError("compiler embedded commit does not match --expected-commit")
    return {"path": binary, "digest": digest(pathlib.Path(binary)), "embedded_commit": commit, "version": version["version"]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--semaprax")
    parser.add_argument("--samples", type=int, default=11)
    parser.add_argument("--warmups", type=int, default=3)
    parser.add_argument("--expected-commit", help="exact 40-character commit embedded by the supplied CLI binary")
    parser.add_argument("--output", type=pathlib.Path, default=pathlib.Path("/tmp/hot-reload-benchmark.json"))
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_text()); validate(manifest)
    if args.samples < 1: raise SystemExit("--samples must be positive")
    if args.warmups < 0: raise SystemExit("--warmups must be nonnegative")
    if args.expected_commit is not None and (len(args.expected_commit) != 40 or any(character not in "0123456789abcdef" for character in args.expected_commit)):
        raise SystemExit("--expected-commit must be exactly 40 lowercase hexadecimal characters")
    if args.dry_run:
        args.output.write_text(canonical({"schema": SCHEMA, "mode": "plan", "acceptance_manifest_digest": digest(MANIFEST), "fixture": manifest["fixture"], "lanes": manifest["lanes"], "samples": args.samples, "warmups": args.warmups})); return
    if not args.semaprax or not pathlib.Path(args.semaprax).is_file(): raise SystemExit("--semaprax must name an already-built executable")
    binary = str(pathlib.Path(args.semaprax).resolve())
    subject = compiler_subject(binary, args.expected_commit)
    scenario_ids = [item["id"] for item in manifest["scenario_matrix"]]
    scenario_records = {identifier: [] for identifier in scenario_ids}
    records = {"full-restart": [], "authenticated-warm-restart": []}
    for _ in range(args.warmups):
        for identifier in scenario_ids:
            scenario(binary, identifier)
        full_restart(binary); warm_restart(binary)
    for _ in range(args.samples):
        for identifier in scenario_ids:
            scenario_records[identifier].append(scenario(binary, identifier))
        records["full-restart"].append(full_restart(binary)); records["authenticated-warm-restart"].append(warm_restart(binary))
    report = {"schema": SCHEMA, "acceptance_manifest_digest": digest(MANIFEST), "compiler": {**subject, "git_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()}, "host": {"system": platform.system(), "release": platform.release(), "machine": platform.machine(), "cpu_count": os.cpu_count()}, "samples": args.samples, "warmups": args.warmups, "peak_rss_bytes": None, "peak_rss_basis": "unavailable: portable per-child peak measurement is not implemented", "loops": {"interpreter-save-to-ack": {"summary": scenario_summary(scenario_records["cold-small-a-to-b"]), "records": scenario_records["cold-small-a-to-b"]}, **{name: {"summary": summary([item["save_to_ack_ms"] for item in rows]), "records": rows} for name, rows in records.items()}}, "scenario_matrix": {name: {"summary": scenario_summary(rows), "records": rows} for name, rows in scenario_records.items()}, "nonclaims": manifest["nonclaims"]}
    args.output.parent.mkdir(parents=True, exist_ok=True); args.output.write_text(canonical(report))


if __name__ == "__main__": main()
