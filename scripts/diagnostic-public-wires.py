#!/usr/bin/env python3
"""Retain complete fixture bytes and original test verdicts, never qualify CI."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SELECTORS = [
    "semantic_workspace_change::artifact::tests::literal_kats_wire_order_domains_and_reference_parity",
    "semantic_workspace_structural_change::artifact::tests::whole_document_kats_keys_domains_refs_paths_and_fixed_point_are_exact",
    "semantic_workspace_structural_change::tests::managed_generate_and_verify_are_exact_read_only_kats_under_one_shared_lock",
    "semantic_workspace_structural_change::tests::structural_apply_publishes_exact_candidate_once_without_raw_writes",
    "semantic_workspace_operations::tests::authenticated_derivation_kat_binds_refs_budget_nonclaims_and_build_counts",
    "semantic_workspace_operations::tests::operations_evidence_and_verification_are_exact_one_build_kats",
    "semantic_workspace_operations::tests::operations_apply_is_exact_stale_and_zero_write_before_replay",
    "workspace_graph::type_proof_tests::projection_tests::rendered_document_has_literal_sha_exact_wire_order_and_digest_binding",
    "workspace_analysis::tests::context_impact_and_review_documents_have_frozen_kats_and_exact_digest_replay",
]
CAPTURE = "semantic_workspace_operations::tests::current_wire_capture::capture_all_original_operations_fixture_receipts"
HEAD = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
assert HEAD == os.environ["GITHUB_SHA"]
provenance = json.loads((ROOT / ".wire-diagnostic/provenance.json").read_bytes())
def verify_sources():
    expected = {item["source"] for item in provenance["files"]}
    assert expected == {"src/semantic_workspace_change/artifact/tests.rs",
                        "src/semantic_workspace_structural_change/artifact.rs",
                        "src/semantic_workspace_structural_change/tests.rs",
                        "src/semantic_workspace_operations/tests.rs",
                        "src/workspace_analysis/tests.rs", "src/workspace_graph/type_proof_tests.rs"}
    changed = set(subprocess.check_output(["git", "diff", "--name-only", "--", "src", "std", "Cargo.toml", "Cargo.lock"], cwd=ROOT, text=True).splitlines())
    assert changed == expected
    for item in provenance["files"]:
        original = subprocess.check_output(["git", "show", f'HEAD:{item["source"]}'], cwd=ROOT)
        assert hashlib.sha256(original).hexdigest() == item["original_sha256"]
        assert hashlib.sha256((ROOT / item["source"]).read_bytes()).hexdigest() == item["diagnostic_sha256"]
    for item in provenance["fixtures"]:
        raw = (ROOT / ".wire-diagnostic" / item["generated"]).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == item["generated_sha256"]
verify_sources()
rows = [json.loads(line) for line in (ROOT / "wire-build.jsonl").read_text().splitlines() if line.startswith("{")]
binaries = [Path(row["executable"]).resolve() for row in rows
            if row.get("reason") == "compiler-artifact" and row.get("executable")
            and row.get("target", {}).get("name") == "semaprax"
            and row.get("target", {}).get("kind") == ["lib"] and row.get("profile", {}).get("test")]
assert len(binaries) == 1
binary = binaries[0]
assert binary.is_relative_to((ROOT / "target").resolve())
assert binary.is_file() and 0 < binary.stat().st_size <= 2 * 1024**3
inventory = subprocess.check_output([str(binary), "--list", "--format", "terse"], cwd=ROOT, text=True)
names = {line.rsplit(": ", 1)[0] for line in inventory.splitlines() if line.endswith(": test")}
replicas = [name.rsplit("::", 1)[0] + "::capture_complete_" + name.rsplit("::", 1)[1] for name in SELECTORS]
assert all(selector in names for selector in [*SELECTORS, *replicas, CAPTURE])
out = ROOT / "target/wire-diagnostic"
out.mkdir(parents=True, exist_ok=True)
env = dict(os.environ, SEMAPRAX_CAPTURE_PUBLIC_WIRES="1", RUST_TEST_THREADS="1")
results = []
wires = []
pattern = re.compile(r"COMPLETE_UNIT_WIRE_V1 label=([a-z-]+) bytes=(\d+) hex=([a-f0-9]+)")
replicas = [name.rsplit("::", 1)[0] + "::capture_complete_" + name.rsplit("::", 1)[1] for name in SELECTORS]
for index, selector in enumerate([*SELECTORS, *replicas, CAPTURE]):
    args = [str(binary), selector, "--exact", "--test-threads=1", "--nocapture"]
    if selector == CAPTURE or selector in replicas:
        args.append("--ignored")
    log = out / f"{index:02}.log"
    with log.open("wb") as stream:
        result = subprocess.run(args, cwd=ROOT, env=env, stdout=stream, stderr=subprocess.STDOUT,
                                timeout=900, check=False)
    count = 0
    with log.open(errors="replace") as stream:
        for line in stream:
            match = pattern.search(line)
            if not match:
                continue
            body = bytes.fromhex(match[3])
            assert len(body) == int(match[2]) and len(body) <= 65536
            value = json.loads(body)
            digest = hashlib.sha256(body).hexdigest()
            (out / f"{digest}.json").write_bytes(body)
            wires.append(dict(selector=selector, label=match[1], schema=value.get("schema") if isinstance(value, dict) else None,
                              bytes=len(body), sha256=digest, file=f"{digest}.json"))
            count += 1
    print(selector, "original_exit", result.returncode, "complete_wires", count, flush=True)
    results.append(dict(selector=selector, exit_code=result.returncode, log=log.name, wires=count))
assert results[-1]["exit_code"] == 0 and results[-1]["wires"] == 6
assert all(row["exit_code"] == 0 and row["wires"] > 0 for row in results if row["selector"] in replicas)
verify_sources()
with binary.open("rb") as stream:
    digest = hashlib.file_digest(stream, "sha256").hexdigest()
(out / "manifest.json").write_text(json.dumps({
    "schema": "semaprax.complete-wire-diagnostic.v1", "head": HEAD,
    "run_id": os.environ["GITHUB_RUN_ID"], "host": os.environ["RUNNER_OS"],
    "rustc": subprocess.check_output(["rustc", "-vV"], text=True),
    "profile": "test-semaprax-opt0-dependencies-opt1-debug0-incremental0-all-features", "binary_sha256": digest,
    "qualification": "diagnostic_capture_only; original test verdicts retained; no passing-CI claim",
    "source_variant": provenance, "tests": results, "wires": wires,
}, indent=2) + "\n")
