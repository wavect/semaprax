"""Independently pinned v3-only correction; the original corpus stays frozen."""
import base64
import json
from pathlib import Path
import runnable_adapter as v1
import runnable_v3_provenance as p

PATH = Path(__file__).resolve().parent / "provenance/typescript-official-v3-corrections.json"
HASH = "7ebeac85f9443b5d90ffaf22bdb03fa5bd26db9260d1fe3ff03e9f99f1b59f3c"
SIZE = 15706
TASK = "stale-edit-preservation-v1"
PREFIX = "benchmarks/cross-language-v1/tasks/" + TASK + "/"
FILES = ["public/typescript/candidate.ts", "public/typescript/index.ts", "hidden/typescript/index.ts"]
MUTANTS = ["stale-edit-original-trunc-formula", "stale-edit-missing-zero-clamp", "stale-edit-damaged-stale-helper"]


def payload(row):
    if (set(row) != {"bytes", "sha256", "base64"} or type(row["bytes"]) is not int
            or not 0 <= row["bytes"] <= v1.MAX_SOURCE_FILE_BYTES
            or not isinstance(row["sha256"], str) or not isinstance(row["base64"], str)):
        raise p.Error("correction_payload_invalid")
    data = base64.b64decode(row["base64"], validate=True)
    if (base64.b64encode(data).decode() != row["base64"] or len(data) != row["bytes"]
            or p.digest(data) != row["sha256"]):
        raise p.Error("correction_payload_identity_drifted")
    return data


def admit(manifest, sources):
    raw = p.read_regular(PATH, 65536)
    if len(raw) != SIZE or p.digest(raw) != HASH:
        raise p.Error("approved_source_correction_drifted")
    row = json.loads(raw)
    if (p.canonical(row) != raw or set(row) != {"schema", "baseline_source_origin", "baseline_manifest_sha256",
            "task_id", "oracle", "files", "equivalence_review", "mutants"}
            or row["schema"] != "benchmark.cross_language.official_ts_correction.v1"
            or row["baseline_source_origin"] != p.SOURCE_COMMIT
            or row["baseline_manifest_sha256"] != p.SOURCE_HASH or row["task_id"] != TASK
            or p.digest(p.canonical(manifest)) != p.SOURCE_HASH):
        raise p.Error("correction_baseline_binding_refused")
    pins = {entry["path"]: entry for entry in manifest["files"]}
    if set(sources) != set(pins):
        raise p.Error("correction_baseline_binding_refused")
    for name, data in sources.items():
        if len(data) != pins[name]["bytes"] or "sha256:" + p.digest(data) != pins[name]["sha256"]:
            raise p.Error("correction_baseline_binding_refused")
    oracle = row["oracle"]
    if set(oracle) != {"path", "bytes", "sha256", "base64"} or oracle["path"] != PREFIX + "EQUIVALENCE.md":
        raise p.Error("correction_oracle_binding_refused")
    oracle_data = payload({key: value for key, value in oracle.items() if key != "path"})
    if oracle_data != sources[oracle["path"]]:
        raise p.Error("correction_oracle_binding_refused")
    if not isinstance(row["equivalence_review"], str) or not row["equivalence_review"]:
        raise p.Error("correction_review_refused")
    if [entry["path"] for entry in row["files"]] != [PREFIX + name for name in FILES]:
        raise p.Error("correction_file_inventory_refused")
    effective = dict(sources)
    artifacts = []
    def artifact(name, data):
        artifacts.append({"path": name, "bytes": len(data), "sha256": p.digest(data),
                          "base64": base64.b64encode(data).decode()})
        return name
    artifact("correction-subject.json", raw)
    oracle_artifact = artifact("corrections/" + TASK + "/EQUIVALENCE.md", oracle_data)
    references = []
    for entry, name in zip(row["files"], FILES):
        if set(entry) != {"path", "base", "effective"}:
            raise p.Error("correction_file_inventory_refused")
        before, after = payload(entry["base"]), payload(entry["effective"])
        if before != sources[entry["path"]]:
            raise p.Error("correction_source_binding_refused")
        effective[entry["path"]] = after
        references.append({"path": entry["path"],
                           "base_artifact": artifact("corrections/" + TASK + "/base/" + name, before),
                           "effective_artifact": artifact("corrections/" + TASK + "/effective/" + name, after)})
    if sum(map(len, effective.values())) > v1.MAX_SOURCE_TOTAL_BYTES:
        raise p.Error("corrected_source_snapshot_exceeds_bound")
    if [entry["id"] for entry in row["mutants"]] != MUTANTS:
        raise p.Error("correction_mutant_inventory_refused")
    candidate = effective[PREFIX + FILES[0]].decode()
    for mutant in row["mutants"]:
        if (set(mutant) != {"id", "path", "target", "replacement", "public_passed", "hidden_passed"}
                or mutant["path"] != "candidate.ts" or type(mutant["public_passed"]) is not bool
                or type(mutant["hidden_passed"]) is not bool or candidate.count(mutant["target"]) != 1):
            raise p.Error("correction_mutant_inventory_refused")
    return {"sources": effective, "artifacts": artifacts, "references": references,
            "oracle_artifact": oracle_artifact, "mutants": {entry["id"]: entry for entry in row["mutants"]}}
