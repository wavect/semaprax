#!/usr/bin/env python3
"""Manual production-algorithm debit census; no passing-product claim."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("build_log", type=Path)
parser.add_argument("--applications", action="store_true",
                    help="diagnose the six unchanged application manifests instead of the four default subjects")
args = parser.parse_args()
root = Path.cwd().resolve()
out = root / "diagnostic-resolved-core"
out.mkdir()
head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
assert head == os.environ["GITHUB_SHA"]
subprocess.run(["git", "diff", "--exit-code"], check=True)
artifacts = []
with args.build_log.open() as stream:
    for line in stream:
        if line.startswith("{"):
            artifacts.append(json.loads(line))

def library(name, version=None):
    matches = [a for a in artifacts if a.get("reason") == "compiler-artifact"
               and a["target"]["name"] == name and not a["profile"]["test"]
               and (version is None or a["package_id"].endswith("@" + version))]
    assert len(matches) == 1, (name, len(matches))
    artifact = matches[0]
    if name == "semaprax":
        assert tomllib.loads((root / "Cargo.toml").read_text())["features"]["default"] == []
        assert set(artifact["features"]) in (
            {"unstable-workflow-profiling"}, {"default", "unstable-workflow-profiling"})
        assert Path(artifact["manifest_path"]).resolve() == root / "Cargo.toml"
        assert artifact["target"]["kind"] == ["lib"]
    paths = [Path(p).resolve() for p in artifact["filenames"] if p.endswith(".rlib")]
    assert len(paths) == 1 and paths[0].is_file()
    assert paths[0].is_relative_to((root / os.environ["CARGO_TARGET_DIR"]).resolve())
    return paths[0]

lib = library("semaprax")
sha = library("sha2", "0.11.1")
binary = out / "capture"
compiler = subprocess.check_output(["rustup", "which", "rustc"], text=True).strip()
subprocess.run([compiler, "--edition=2021", "-Cdebuginfo=0", "-Copt-level=1",
                str(root / "scripts/diagnose-resolved-core.rs"),
                "--extern", "semaprax=" + str(lib), "--extern", "sha2=" + str(sha),
                "-L", "dependency=" + str(lib.parent),
                "-L", "dependency=" + str(sha.parent), "-o", str(binary)], check=True)
subjects = [
    "examples/task-service-project/semaprax.toml",
    "std/test-bytes/semaprax.toml",
    "std/data-json-dec/semaprax.toml",
    "std/data-json-scan/semaprax.toml",
]
if args.applications:
    subjects = [
        "std/int-decimal/semaprax.toml",
        "examples/catalog-scoped-record-successor/semaprax.toml",
        "examples/shiftsim-typed-record-successor/semaprax.toml",
        "examples/catalog-normalizer-project/semaprax.toml",
        "experiments/ascii-pattern-source/loglens-reference/matcher-package-adapted/semaprax.toml",
        "experiments/selected-file-input/loglens/semaprax.toml",
    ]

def declared_sources(manifest):
    document = tomllib.loads(manifest.read_text())
    paths = [manifest] + [manifest.parent / source for source in document["modules"]["sources"]]
    return [{"path": str(path.relative_to(root)), "bytes": path.stat().st_size,
             "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for path in paths]

def final_attempt(log):
    lines = log.read_text().splitlines()
    summaries = [re.fullmatch(
        r"complete=true invalid=false lost=false overflow=false entered=(\d+) stored=(\d+) final=Some\((\d+)\)",
        line) for line in lines]
    summaries = [match for match in summaries if match is not None]
    assert len(summaries) == 1, "missing complete exclusive census"
    entered, stored, ordinal = map(int, summaries[0].groups())
    assert entered == stored and stored > 0
    starts = [i for i, line in enumerate(lines) if line.startswith(f"attempt={ordinal} ")]
    assert len(starts) == 1, "missing final exclusive attempt"
    start = starts[0]
    end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith("attempt=")), len(lines))
    detail = lines[start:end]
    assert " complete=true invalid=false " in detail[0]
    consumed = int(re.search(r" consumed=(\d+) ", detail[0]).group(1))
    for prefix in ("stage ", "operation "):
        assert sum(int(line.rsplit(": ", 1)[1]) for line in detail if line.startswith(prefix)) == consumed
    families = [json.loads(line.removeprefix("path-census:"))
                for line in detail if line.startswith("path-census:")]
    assert [row["family"] for row in families] == ["residual", "ordinary", "selected"]
    assert sum(row["debit_bytes"] for row in families) == consumed
    for row in families:
        assert row["schema"] == "semaprax.path-family-census.v1"
        assert set(row) == set("schema family entries successes errors unwinds refusals requests requested_bytes admitted_requests admitted_bytes debits debit_bytes terminal_paths successful_paths events event_labels foreign_events descendant_requests descendant_requested_bytes descendant_admitted_requests descendant_admitted_bytes descendant_entries descendant_events physical_allocator_census".split())
        for key, value in row.items():
            if key not in {"schema", "family", "events", "event_labels", "descendant_events", "physical_allocator_census"}:
                assert type(value) is int and value >= 0
        assert row["entries"] == sum(row[key] for key in ("successes", "errors", "unwinds", "refusals"))
        assert 0 <= row["admitted_requests"] <= row["requests"]
        assert 0 <= row["admitted_bytes"] <= row["requested_bytes"]
        assert row["foreign_events"] == 0 and row["physical_allocator_census"] is False
        assert len(row["events"]) == len(row["event_labels"]) == 12
        assert row["event_labels"] == ["body", "empty_publication", "one_promotion", "many_publication", "owned_box_formation", "held_box_transfer", "promotion", "equal_duplicate", "tree_insert_request", "incoming_empty", "incoming_one", "incoming_many"]
        for key in ("requests", "requested_bytes", "admitted_requests", "admitted_bytes", "entries"):
            assert 0 <= row["descendant_" + key] <= row[key]
        assert len(row["descendant_events"]) == 12
        assert all(type(n) is int and 0 <= n <= total for n, total in zip(row["descendant_events"], row["events"]))
        assert all(type(n) is int and n >= 0 for n in row["events"])
    return {"ordinal": ordinal, "entered": entered, "stored": stored,
            "path_family_request_and_source_event_census": families,
            "complete_exclusive_attempt_lines": detail}

def product(path):
    values = [line.removeprefix("product-json:") for line in path.read_text().splitlines()
              if line.startswith("product-json:")]
    assert len(values) == 1
    json.loads(values[0])
    return values[0]

def application_capture(i, subject):
    manifest = root / subject
    log = out / f"subject-{i}.log"
    baseline = out / f"subject-{i}-plain.log"
    row = {"manifest": subject, "log": log.name, "plain_log": baseline.name, "errors": []}

    def attempt(stage, action):
        try:
            return action()
        except Exception as error:
            detail = {"stage": stage, "type": type(error).__name__, "message": str(error)}
            if isinstance(error, subprocess.CalledProcessError):
                detail["process_exit_status"] = error.returncode
            if isinstance(error, subprocess.TimeoutExpired):
                detail["timeout_seconds"] = error.timeout
            row["errors"].append(detail)
            return None

    before = attempt("declared_source_inventory_before", lambda: declared_sources(manifest))
    if before is not None:
        row["declared_manifest_and_module_sources"] = before
        row["manifest_sha256"] = before[0]["sha256"]
    for label, path, extra in (("plain", baseline, ["plain"]), ("observed", log, [])):
        def run_capture():
            with path.open("w") as stream:
                return subprocess.run([str(binary), str(manifest), *extra], stdout=stream,
                                      stderr=subprocess.STDOUT, timeout=240, check=True).returncode
        status = attempt(label + "_process", run_capture)
        row[label + "_process_exit_status"] = status
        if path.exists():
            row["plain_log_sha256" if label == "plain" else "log_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
    plain = attempt("plain_product", lambda: product(baseline))
    observed = attempt("observed_product", lambda: product(log))
    if plain is not None and observed is not None:
        row["complete_products_equal"] = plain == observed
        def require_equality():
            assert plain == observed, "observer changed complete diagnostic bytes"
        attempt("complete_product_equality", require_equality)
    if observed is not None:
        def record_product():
            actual = json.loads(observed)
            assert set(actual) == {"ok", "diagnostics"}
            assert type(actual["ok"]) is bool and isinstance(actual["diagnostics"], list)
            row.update(complete_product_sha256=hashlib.sha256(observed.encode()).hexdigest(),
                       accepted=actual["ok"], product_status="accepted" if actual["ok"] else "rejected",
                       complete_product=actual)
        attempt("observed_product_schema", record_product)
    census = attempt("final_exclusive_attempt", lambda: final_attempt(log))
    row["final_exclusive_attempt_available"] = census is not None
    if census is not None:
        row["final_exclusive_attempt"] = census
    after = attempt("declared_source_inventory_after", lambda: declared_sources(manifest))
    if before is not None and after is not None:
        def require_same_sources():
            assert before == after, "application authored source drift"
        attempt("declared_source_equality", require_same_sources)
    if log.exists():
        attempt("print_observed_log", lambda: print(log.read_text(), flush=True))
    row["capture_complete"] = not row["errors"]
    return row

rows = []
for i, subject in enumerate(subjects):
    if args.applications:
        rows.append(application_capture(i, subject))
        continue
    manifest = root / subject
    log = out / f"subject-{i}.log"
    baseline = out / f"subject-{i}-plain.log"
    with baseline.open("w") as stream:
        subprocess.run([str(binary), str(manifest), "plain"], stdout=stream, stderr=subprocess.STDOUT,
                       timeout=240, check=True)
    with log.open("w") as stream:
        subprocess.run([str(binary), str(manifest)], stdout=stream, stderr=subprocess.STDOUT,
                       timeout=240, check=True)
    assert product(log) == product(baseline), "observer changed complete diagnostic bytes"
    print(log.read_text(), flush=True)
    rows.append({"manifest": subject, "manifest_sha256": hashlib.sha256(manifest.read_bytes()).hexdigest(),
                 "log": log.name, "log_sha256": hashlib.sha256(log.read_bytes()).hexdigest()})
subprocess.run(["git", "diff", "--exit-code"], check=True)
def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            value.update(chunk)
    return value.hexdigest()

provenance = {"head": head, "run_id": os.environ["GITHUB_RUN_ID"],
    "qualification": "diagnostic_only; original refusal preserved; no passing application claim",
    "library_sha256": digest(lib),
    "probe_sha256": digest(binary),
    "rustc": subprocess.check_output([compiler, "-vV"], text=True), "subjects": rows}
if args.applications:
    provenance.update(schema="semaprax.resolved-core-application-census.v1",
                      tree=subprocess.check_output(["git", "rev-parse", "HEAD^{tree}"], text=True).strip(),
                      profile="applications", original_caps_unchanged=True,
                      all_captures_complete=all(row["capture_complete"] for row in rows),
                      application_execution_or_cli49_acceptance_proven=False)
(out / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
if args.applications and not provenance["all_captures_complete"]:
    raise SystemExit(1)
