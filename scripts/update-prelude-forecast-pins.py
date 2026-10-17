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
    ("context", "26296de75d133f1563b1b4fc23049b6eb29f4b407a2369d4849a12e9a379f225"),
    ("forward", "190a00dfa539b7cef26a50dcbda97d522bb22699b02c6b904682d524806266c8"),
    ("capability-context", "7ba1554e318ce8d56739e03a2987c90d5d7fdf1ff301b528ad67c1f8cd432a42"),
    ("type-context", "bccc97fe0ad0026b44544e1e7c27c72f4d266c6773de01e157dcc9492317dd83"),
    ("impact", "6e0ea8e0fc10e31b325fdc33e41ab9030b75f393c2f0e4fec916be6a19abab62"),
    ("custom-impact", "421e0b0fba5f10c294d13bbfde592c9837468e074cacf6b113ab62051299b3bc"),
    ("review", "864367757119c169b5f049542de643b2825bcc64794ae91784920e8d73d18faf"),
    ("capability-review", "812fa5e3a129b96f36eed9e686ec88e3b5118813f31106c42adae637c6c14b21"),
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

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--expected-capture-head", required=True)
    parser.add_argument("--expected-build-receipt-sha256", required=True)
    parser.add_argument("--expected-producer-receipt-sha256", required=True)
    parser.add_argument("--round", required=True, type=Path,
                        help="trusted ROOT round holding existing build and producer receipts")
    parser.add_argument("--workflow-record", required=True, type=Path)
    parser.add_argument("--expected-workflow-record-sha256", required=True)
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--evidence-dir", type=Path)
    args = parser.parse_args()
    capture = args.capture.resolve()
    repo = args.repo.resolve()
    manifest = json.loads((capture / "candidates.json").read_text())
    require(manifest["schema"] == "semaprax.prelude-forecast-pin-candidates.v1", "wrong capture schema")
    require(not manifest["missing"], "incomplete capture; do not update pins")
    head = (capture / "source-head.txt").read_text().strip()
    require(re.fullmatch(r"[0-9a-f]{40}", args.expected_capture_head) is not None
            and head == args.expected_capture_head, "capture head differs from full ROOT-approved build")
    require(head == manifest["source_head"], "capture head manifest mismatch")
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
    # The descriptor is independently reviewed; its own contents grant no authority.
    import importlib.util
    generation_path = Path(__file__).resolve().parent / "prelude_forecast_committed_generation.py"
    require(hashlib.sha256(generation_path.read_bytes()).hexdigest() == "cb2ea06b1f01b302b20bfbc07a543adba577da5acd9ed92aa117119352bab3d2", "committed-generation module drift")
    spec = importlib.util.spec_from_file_location("reviewed_committed_generation", generation_path)
    generation = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(generation)
    baseline_capture, baseline_hashes = generation.preflight(args, expected, build)
    baseline_public = json.loads((baseline_capture / "wires/public-graph.json").read_bytes())
    baseline_amount = baseline_public["budget"]["used_builder_bytes"]
    analysis = [(name, baseline_hashes[name + ".json"]) for name, _ in ANALYSIS]
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
    names = ["public-graph"] + [name for name, _ in analysis] + ["projection-wire"]
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
    whole_wire_review = generation.validate_capture(capture, baseline_capture)
    graph = values["public-graph"]
    require(graph["schema"] == "semaprax.workspace-semantic-graph.v1", "public graph schema drift")
    amount = graph["budget"]["used_builder_bytes"]
    require(type(amount) is int and 0 < amount <= 67108864, "actual builder amount outside unchanged cap")
    for name in ["public-graph", "projection-wire"]:
        value = values[name]
        digest = value["graph_digest"]
        field = (',"graph_digest":"' + digest + '"').encode()
        require(wires[name].count(field) == 1, "graph digest field framing drift")
        require(workspace_artifact_digest(wires[name].replace(field, b"", 1)) == digest, "graph digest does not bind domain/length/full payload")
    for name, _ in analysis:
        require(values[name]["workspace_graph_digest"] == graph["graph_digest"], "analysis graph binding differs: " + name)
    edits = []
    def edit(path, before, after, evidence):
        edits.append(dict(path=path, before=before, after=after, actual_wire=evidence))
    owner = "tests/workspace/semantic_graph.rs"
    edit(owner, "assert_eq!(graph.budget().used_builder_bytes(), " + format(baseline_amount, "_") + ");",
         "assert_eq!(graph.budget().used_builder_bytes(), " + format(amount, "_") + ");", "public-graph")
    edit(owner, "sha256:" + baseline_hashes["public-graph.json"],
         sha(wires["public-graph"]), "public-graph")
    for name, old in analysis:
        edit(owner, "sha256:" + old, sha(wires[name]), name)
    edit("src/workspace_graph/type_proof_tests.rs",
         "sha256:" + baseline_hashes["projection-wire.json"],
         sha(wires["projection-wire"]), "projection-wire")
    before_sources = {}
    after_sources = {}
    for path in sorted({record["path"] for record in edits}):
        before_sources[path] = (repo / path).read_bytes()
        require(b"\r" not in before_sources[path], "unexpected CRLF/CR source; do not normalize: " + path)
        before_sources[path].decode("utf-8")
        frozen_bytes = subprocess.check_output(["git", "show", head + ":" + path], cwd=trusted_root)
        require(before_sources[path] == frozen_bytes, "pin owner differs from trusted frozen source: " + path)
        after_sources[path] = before_sources[path]
    for record in edits:
        path = record["path"]
        before, after = record["before"].encode(), record["after"].encode()
        require(after_sources[path].count(before) == 1, "old pin not unique/current: " + path)
        after_sources[path] = after_sources[path].replace(before, after, 1)
    # Exit 101 authorizes only the exact old-pin assertion, with both actual
    # left and frozen historical right values. A complete capture alone grants
    # no permission to ignore any other getter/locking/CLI assertion failure.
    owner_source = before_sources[owner].decode("utf-8")
    for label in ["public-graph", "analysis"]:
        if statuses[label][0] != 101:
            continue
        log = (capture / (label + ".log")).read_text()
        require(log.count("panicked at ") == 1, "unexpected/multiple panic: " + label)
        match = re.search(r"panicked at ([^\n]+):(\d+):\d+:\nassertion `left == right` failed\n\s*left: ([^\n]+)\n\s*right: ([^\n]+)", log)
        require(match is not None and match[1].endswith("tests/workspace/semantic_graph.rs"),
                "not an exact owning stale-pin assertion: " + label)
        left, right = json.loads(match[3]), json.loads(match[4])
        permitted = [(edits[0]["before"], amount, baseline_amount),
                     (edits[1]["before"], sha(wires["public-graph"]), edits[1]["before"])]
        if label == "analysis":
            permitted = [("sha256:" + analysis[0][1],
                          [sha(wires[name]) for name, _ in analysis],
                          ["sha256:" + old for _, old in analysis])]
        authorized = False
        for needle, actual, historical in permitted:
            offset = owner_source.index(needle)
            assertion = owner_source.rfind("assert_eq!(", 0, offset + len(needle))
            line = owner_source.count("\n", 0, assertion) + 1
            authorized |= int(match[2]) == line and left == actual and right == historical
        require(authorized, "non-pin assertion or mismatched actual/old failure: " + label)
    complete_source_projection = generation.source_projection(trusted_root, head, before_sources, after_sources)
    report = dict(complete_wire_review=whole_wire_review, complete_source_projection=complete_source_projection,
                  schema="semaprax.prelude-forecast-reviewed-pin-plan.v1", capture_head=head,
                  status="actual-candidate-plan-requires-wire-review", edits=edits,
                  source_sha256={path: sha(data) for path, data in before_sources.items()},
                  after_sha256={path: sha(data) for path, data in after_sources.items()},
                  trusted_build_receipt_sha256=sha(build_bytes),
                  trusted_producer_receipt_sha256=sha(producer_bytes))
    print(json.dumps(report, indent=2))
    if not args.apply:
        return
    require(args.evidence_dir is not None, "--apply requires --evidence-dir")
    evidence = args.evidence_dir.resolve()
    require(not evidence.exists(), "evidence directory must be fresh")
    require(not evidence.is_relative_to(capture) and not evidence.is_relative_to(repo),
            "evidence must be outside capture and target repository")
    # All inputs, pins and replacements validate before creating any candidate.
    evidence.mkdir(parents=True)
    shutil.copytree(capture, evidence / "actual-capture")
    for path, data in before_sources.items():
        (evidence / (path.replace("/", "__") + ".before.txt")).write_bytes(data)
    (evidence / "reviewed-pin-plan.json").write_text(json.dumps(report, indent=2) + "\n")
    # Preflight every owner before any source write; preserve raw LF bytes.
    for path, data in before_sources.items():
        require((repo / path).read_bytes() == data, "source drift before pin update")
    for path, data in after_sources.items():
        (repo / path).write_bytes(data)

if __name__ == "__main__":
    main()
