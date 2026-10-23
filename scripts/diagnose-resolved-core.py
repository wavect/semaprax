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
    return {"ordinal": ordinal, "entered": entered, "stored": stored,
            "complete_exclusive_attempt_lines": detail}

rows = []
for i, subject in enumerate(subjects):
    manifest = root / subject
    if args.applications:
        before_sources = declared_sources(manifest)
    log = out / f"subject-{i}.log"
    baseline = out / f"subject-{i}-plain.log"
    with baseline.open("w") as stream:
        subprocess.run([str(binary), str(manifest), "plain"], stdout=stream, stderr=subprocess.STDOUT,
                       timeout=240, check=True)
    with log.open("w") as stream:
        subprocess.run([str(binary), str(manifest)], stdout=stream, stderr=subprocess.STDOUT,
                       timeout=240, check=True)
    def product(path):
        values = [line.removeprefix("product-json:") for line in path.read_text().splitlines()
                  if line.startswith("product-json:")]
        assert len(values) == 1
        json.loads(values[0])
        return values[0]
    assert product(log) == product(baseline), "observer changed complete diagnostic bytes"
    print(log.read_text(), flush=True)
    rows.append({"manifest": subject, "manifest_sha256": hashlib.sha256(manifest.read_bytes()).hexdigest(),
                 "log": log.name, "log_sha256": hashlib.sha256(log.read_bytes()).hexdigest()})
    if args.applications:
        assert declared_sources(manifest) == before_sources, "application authored source drift"
        actual_product = json.loads(product(log))
        assert set(actual_product) == {"ok", "diagnostics"}
        assert type(actual_product["ok"]) is bool and isinstance(actual_product["diagnostics"], list)
        rows[-1].update(declared_manifest_and_module_sources=before_sources,
                        plain_log=baseline.name,
                        plain_log_sha256=hashlib.sha256(baseline.read_bytes()).hexdigest(),
                        complete_product_sha256=hashlib.sha256(product(log).encode()).hexdigest(),
                        accepted=actual_product["ok"],
                        product_status="accepted" if actual_product["ok"] else "rejected",
                        complete_product=actual_product,
                        final_exclusive_attempt=final_attempt(log))
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
                      application_execution_or_cli49_acceptance_proven=False)
(out / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
