#!/usr/bin/env python3
"""Manual production-algorithm debit census; no passing-product claim."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib

root = Path.cwd().resolve()
out = root / "diagnostic-resolved-core"
out.mkdir()
head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
assert head == os.environ["GITHUB_SHA"]
subprocess.run(["git", "diff", "--exit-code"], check=True)
artifacts = []
with Path(sys.argv[1]).open() as stream:
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
rows = []
for i, subject in enumerate(subjects):
    manifest = root / subject
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
subprocess.run(["git", "diff", "--exit-code"], check=True)
def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            value.update(chunk)
    return value.hexdigest()

(out / "provenance.json").write_text(json.dumps({"head": head, "run_id": os.environ["GITHUB_RUN_ID"],
    "qualification": "diagnostic_only; original refusal preserved; no passing application claim",
    "library_sha256": digest(lib),
    "probe_sha256": digest(binary),
    "rustc": subprocess.check_output([compiler, "-vV"], text=True), "subjects": rows}, indent=2) + "\n")
