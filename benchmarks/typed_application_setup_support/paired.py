"""Complete paired-plan byte binding; evidence grants no installation authority."""
from __future__ import annotations
import json
from pathlib import Path
import re

BUNDLE_LIMIT = 8 * 1024 * 1024
SOURCE_LIMIT = 2 * 1024 * 1024


def _object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate paired-plan JSON key")
        result[key] = value
    return result


def plan(application, api):
    entries = []
    for source, record, profile, flags in api.generator_jobs(application):
        entry = {"source": source, "record": record, "profile": profile}
        if flags:
            if flags != ["--max-string-bytes", "16"]:
                raise ValueError("paired setup profile flags differ")
            entry["max_string_bytes"] = 16
        entries.append(entry)
    return {"schema": "semaprax.json-codec-plan.v1", "entries": entries}


def plan_bytes(application, api):
    return (json.dumps(plan(application, api), sort_keys=True, separators=(",", ":")) + "\n").encode()


def validate(encoded, application, authored_root: Path, api, *, bundled_sources):
    if not encoded or len(encoded) > BUNDLE_LIMIT:
        raise ValueError("paired bundle exceeds byte bound")
    value = json.loads(encoded, object_pairs_hook=_object)
    fields = {"schema", "authored_project_revision", "candidate_project_revision",
              "candidate_workspace_revision", "manifest", "authored_sources",
              "candidate_sources", "entries"}
    if not isinstance(value, dict) or set(value) != fields or value["schema"] != "semaprax.json-codec-bundle.v1":
        raise ValueError("paired bundle schema differs")
    for field in ("authored_project_revision", "candidate_project_revision", "candidate_workspace_revision"):
        if not isinstance(value[field], str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", value[field]):
            raise ValueError("paired bundle revision differs")
    manifest = api.validate_manifest(application, api.read_regular(authored_root / "semaprax.toml"))
    if not isinstance(value["manifest"], str) or api.tomllib.loads(value["manifest"]) != manifest:
        raise ValueError("paired bundle manifest differs from authored original")
    authored_paths = set(manifest["modules"]["sources"])
    paths = sorted(authored_paths | bundled_sources.keys())
    inventories = []
    for field in ("authored_sources", "candidate_sources"):
        rows = value[field]
        if not isinstance(rows, list) or len(rows) != len(paths) or len(rows) > 64:
            raise ValueError("paired complete source inventory differs")
        sources = {}
        total = 0
        for path, row in zip(paths, rows):
            if (not isinstance(row, dict) or set(row) != {"path", "source", "sha256"}
                    or row["path"] != path or not isinstance(row["source"], str)):
                raise ValueError("paired complete source path differs")
            data = row["source"].encode()
            total += len(data)
            if total > SOURCE_LIMIT or row["sha256"] != "sha256:" + api.sha(data):
                raise ValueError("paired source bytes or digest differ")
            if path in bundled_sources and data != bundled_sources[path]:
                raise ValueError("paired bundled source differs from committed dependency")
            if (field == "authored_sources" and path in authored_paths
                    and data != api.read_regular(authored_root / path)):
                raise ValueError("paired authored original source differs")
            sources[path] = data
        inventories.append(sources)
    authored, candidate = inventories
    expected = sorted(plan(application, api)["entries"], key=lambda row: row["source"])
    receipts = value["entries"]
    if not isinstance(receipts, list) or len(receipts) != len(expected):
        raise ValueError("paired entry receipt inventory differs")
    outputs = {}
    keys = {"source_path", "record_id", "profile", "max_string_bytes", "max_array_items",
            "authored_sha256", "candidate_sha256", "generated_declarations"}
    identities = set()
    for entry, receipt in zip(expected, receipts):
        path = entry["source"]
        if (not isinstance(receipt, dict) or set(receipt) != keys
                or receipt["source_path"] != path or receipt["record_id"] != entry["record"]
                or receipt["profile"] != entry["profile"]
                or receipt["max_string_bytes"] != entry.get("max_string_bytes")
                or receipt["max_array_items"] is not None
                or receipt["authored_sha256"] != "sha256:" + api.sha(authored[path])
                or receipt["candidate_sha256"] != "sha256:" + api.sha(candidate[path])):
            raise ValueError("paired entry receipt or selector differs")
        ids = receipt["generated_declarations"]
        if (not isinstance(ids, list) or not ids or any(not isinstance(i, str) or not i for i in ids)
                or ids != sorted(set(ids)) or identities.intersection(ids)):
            raise ValueError("paired generated declaration inventory differs")
        identities.update(ids)
        outputs[path] = candidate[path]
    for path in paths:
        if path not in outputs and candidate[path] != authored[path]:
            raise ValueError("paired unrelated candidate source changed")
    return outputs


def derive(application, compiler, bootstrap, generated, run, unchanged, api, on_replayed=None, *, bundled_sources):
    policy = generated / "codec-plan.json"
    policy.write_bytes(plan_bytes(application, api))
    for label, output in (("derive-plan", generated / "candidate.json"),
                          ("replay-plan", generated / "candidate.replay.json")):
        unchanged()
        run([str(compiler), "json-codec", str(bootstrap / "semaprax.toml"),
             "--plan", str(policy), "--output", str(output)], label)
        unchanged()
    first = api.read_regular(generated / "candidate.json", BUNDLE_LIMIT)
    replay = api.read_regular(generated / "candidate.replay.json", BUNDLE_LIMIT)
    if first != replay:
        raise ValueError("complete paired candidate differs from same-bootstrap replay")
    outputs = validate(first, application, bootstrap, api, bundled_sources=bundled_sources)
    unchanged()
    if on_replayed is not None:
        for path, *_ in api.generator_jobs(application):
            on_replayed(path, outputs[path])
    return outputs


def retained(application, generated, authored, api, *, bundled_sources):
    if api.read_regular(generated / "codec-plan.json", 16384) != plan_bytes(application, api):
        raise ValueError("retained paired plan differs")
    first = api.read_regular(generated / "candidate.json", BUNDLE_LIMIT)
    if first != api.read_regular(generated / "candidate.replay.json", BUNDLE_LIMIT):
        raise ValueError("retained complete paired replay differs")
    return validate(first, application, authored, api, bundled_sources=bundled_sources), api.sha(first)
