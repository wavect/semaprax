#!/usr/bin/env python3
"""Plan/apply reviewed pins from complete actual ROOT producer captures only.

No compiler/build invocation, synthesized wire, browser update or gate bypass.
Default is read-only. --apply requires a fresh evidence directory.
"""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path
import shutil

ANALYSIS = [
    ("context", "ab1dd558d5cc2033dcf94540c4f648efba1e48c4f8a10cf5b0a89d49bad6908f"),
    ("forward", "bd9f88f8d1b821d14ff92d0e3697ecd4a91868bb4c87bc93d0085665d470a61f"),
    ("capability-context", "a7a13a15a56550eaa35ea6609f3fd169fe33066c054ba6ba23212b9836d848b3"),
    ("type-context", "45961d6e8efaeb42948626d238ca39eb966d8c15023c04356d7e27c3051762e0"),
    ("impact", "a1b8abdea4abd3e4e93bfac7a80456d5a2097c8599b68fa9d5bbc5d210cb017a"),
    ("custom-impact", "239e02032ae054db62e3c3033518acb2a7ad9fa96a35c304c1e90a03a3b02b0b"),
    ("review", "004de4419f867bfc5b2abd840779901908f0fbbb1f52193e6eea28a987d47574"),
    ("capability-review", "50bcee1a9faf8470dc076592d1b0d7625ea9949c7db363ae560c69701c500706"),
]
OWNERS = {
    "public-graph": "semantic_graph::public_api_cli_bytes_getters_and_read_only_locking_are_exact",
    "analysis": "semantic_graph::public_workspace_analysis_api_cli_kats_and_locking_are_exact",
    "projection-wire": "workspace_graph::type_proof_tests::projection_tests::emit_projection_wire_pin_candidate",
}

def sha(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()

def workspace_artifact_digest(payload):
    # Exact owning formula: docs/WORKSPACE-SEMANTIC-GRAPH-V1.md and
    # workspace_graph::artifact_digest. Document pin SHAs still hash full wires.
    domain = b"semaprax.workspace-semantic-graph.artifact-digest.v1\0"
    return sha(domain + len(payload).to_bytes(8, byteorder="little") + payload)

def require(condition, reason):
    if not condition:
        raise SystemExit(reason)

def validate_actual_capture(args):
    capture = args.capture.resolve()
    repo = args.repo.resolve()
    manifest = json.loads((capture / "candidates.json").read_text())
    require(manifest["schema"] == "semaprax.prelude-forecast-pin-candidates.v1", "wrong capture schema")
    require(not manifest["missing"], "incomplete capture; do not update pins")
    head = (capture / "source-head.txt").read_text().strip()
    require(re.fullmatch(r"[0-9a-f]{40}", args.expected_capture_head) is not None
            and head == args.expected_capture_head, "capture head differs from full ROOT-approved build")
    require(head == manifest["source_head"], "capture head manifest mismatch")
    require(head == args.expected_capture_head, "this one-off updater accepts only its explicitly frozen source capture")
    source_patch = (capture / "source-local.patch").read_bytes()
    require(not source_patch, "capture includes source drift; ROOT must audit and recapture frozen source")
    require(hashlib.sha256(source_patch).hexdigest() == manifest["source_patch_sha256"], "capture patch manifest mismatch")
    binaries = json.loads((capture / "binaries.json").read_text())
    # Consume ROOT's existing frozen-build wrapper proof; do not infer build
    # provenance merely from the standalone capture's self-reported manifest.
    trusted_round = args.round.resolve()
    build_bytes = (trusted_round / "owning-build-result.json").read_bytes()
    build = json.loads(build_bytes)
    producer_bytes = (trusted_round / "prelude-pin-producers-result.json").read_bytes()
    for data, approved in [(build_bytes, args.expected_build_receipt_sha256),
                           (producer_bytes, args.expected_producer_receipt_sha256)]:
        require(re.fullmatch(r"[0-9a-f]{64}", approved) is not None
                and hashlib.sha256(data).hexdigest() == approved,
                "external ROOT-approved receipt hash mismatch")
    producer = json.loads(producer_bytes)
    expected = json.loads((trusted_round / "expected-counts.json").read_bytes())
    require(build["exit_code"] == producer["exit_code"] == 0
            and not producer["resource_refusal"], "trusted ROOT build/producer failed")
    require(build["source_subject"] == producer["source_subject"]
            == expected["source_subject"] == head, "trusted ROOT frozen source mismatch")
    require(producer["build_receipt_sha256"] == hashlib.sha256(build_bytes).hexdigest(),
            "trusted build receipt hash mismatch")
    trusted_root = Path(expected["draft_repository"]).resolve()
    command = producer["command"]
    require(len(command) == 5 and command[0] == "/bin/sh"
            and Path(command[1]).resolve() == trusted_root / "scripts/capture-prelude-forecast-pins.sh"
            and Path(command[4]).resolve() == capture
            and Path(producer["capture_out"]).resolve() == capture,
            "trusted wrapper command/capture binding mismatch")
    built = {str(Path(row["executable"]).resolve()): row for row in build["artifacts"]}
    require(len(binaries) == 2, "expected exactly ROOT lib/workspace binary roles")
    for record, role, executable in zip(binaries, ["semaprax", "workspace"], command[2:4]):
        key = str(Path(executable).resolve())
        require(str(Path(record["path"]).resolve()) == key and key in built,
                "trusted binary role path mismatch")
        require(built[key]["target"]["kind"] == (["lib"] if role == "semaprax" else ["test"]), "trusted binary kind mismatch")
        require(built[key]["target"]["name"] == role
                and record["sha256"] == built[key]["executable_sha256"],
                "trusted binary role SHA mismatch")
    require(subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=trusted_root).strip().decode() == head,
            "ROOT head drift since capture")
    require(not subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=all"], cwd=trusted_root).strip(),
            "ROOT source drift including untracked since trusted capture")
    statuses = {}
    for line in (capture / "status.tsv").read_text().splitlines():
        label, code, selector = line.split("\t")
        require(label not in statuses, "duplicate producer status")
        statuses[label] = (int(code), selector)
    for label, selector in OWNERS.items():
        require(label in statuses and statuses[label][1] == selector, "missing exact owning producer: " + label)
        require(statuses[label][0] in (0, 101), "non-test producer failure: " + label)
        log = (capture / (label + ".log")).read_text()
        require(selector in log and "running 1 test" in log, "not one exact owning test: " + label)
        if statuses[label][0] == 0:
            require("test result: ok. 1 passed; 0 failed" in log, "missing actual owner success: " + label)
    require(statuses["projection-wire"][0] == 0, "ignored wire producer failed")
    names = ["public-graph"] + [name for name, _ in ANALYSIS] + ["projection-wire"]
    index = {record["name"]: record for record in manifest["candidates"]}
    require(len(index) == len(manifest["candidates"]), "duplicate wire candidate")
    wires = {}
    values = {}
    for name in names:
        require(name in index, "missing actual wire: " + name)
        data = (capture / "wires" / (name + ".json")).read_bytes()
        record = index[name]
        require(record["file"] == "wires/" + name + ".json", "unexpected candidate path")
        require(sha(data) == record["document_sha256"] and len(data) == record["bytes"], "actual bytes differ from manifest: " + name)
        require(not data.endswith(b"\n"), "wire has noncanonical terminal LF: " + name)
        values[name] = json.loads(data)
        wires[name] = data
    return manifest
