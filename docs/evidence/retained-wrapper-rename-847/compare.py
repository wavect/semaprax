#!/opt/homebrew/bin/python3
"""Extract exact wire artifacts and independently verify their real digests.
No accounting field normalization or semantic-field omission is permitted.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re

def external(value):
    path = Path(value).resolve()
    if not path.is_relative_to("/Volumes/Semaprax"):
        raise ValueError("SSD-only input/output")
    return path

def digest(domain, payload):
    return "sha256:" + hashlib.sha256(domain + len(payload).to_bytes(8, "little") + payload).hexdigest()

def frame(log):
    data = log.read_bytes()
    match = re.search(rb'(?:^|\n)@847 result ([0-9]+)\n', data)
    if not match:
        raise ValueError("missing explicit protocol frame")
    size = int(match[1])
    if size > 256 * 1024 * 1024:
        raise ValueError("closed output byte bound")
    raw = data[match.end():match.end() + size]
    if len(raw) != size or data[match.end() + size:match.end() + size + 1] != b"\n":
        raise ValueError("truncated frame")
    return json.loads(raw), hashlib.sha256(data).hexdigest()

def materialize(packet, out):
    result = packet["result"]
    passes = result.get("passes", [{"result":result.get("result", result)}])
    semantic = []
    for index, item in enumerate(passes):
        value = item["result"]
        artifacts = value.get("artifacts", value)
        row = {"diagnostics":artifacts["diagnostics"]} if "diagnostics" in artifacts else artifacts
        semantic.append({"work":value.get("work"),"artifacts":row})
        prefix = out / str(index)
        prefix.mkdir()
        (prefix / "complete.json").write_text(json.dumps(row, ensure_ascii=False, indent=2) + "\n")
        if "graph" not in row:
            continue
        graph = row["graph"].encode()
        parsed = json.loads(graph)
        if parsed["graph_digest"] != row["graph_digest"]:
            raise ValueError("graph declaration digest mismatch")
        # The owning renderer omits exactly this top-level field when hashing.
        # Preserve every remaining raw byte, including accounting and ordering.
        field = b',"graph_digest":"' + row["graph_digest"].encode() + b'"'
        if graph.count(field) != 1:
            raise ValueError("nonunique exact graph digest field")
        payload = graph.replace(field, b"", 1)
        actual = digest(b"semaprax.project-semantic-graph.artifact-digest.v1\0", payload)
        if actual != row["graph_digest"]:
            raise ValueError("independent project graph digest mismatch")
        (prefix / "graph.raw.json").write_bytes(graph)
        for role in ["entry_hir", "public_api_hir", "test_hir"]:
            (prefix / (role + ".txt")).write_text(row[role])
        image = row["image"]
        if "json" in image:
            raw_image = image["json"].encode()
            if digest(b"semaprax.semantic-workspace-image.digest.v1\0", raw_image) != image["digest"]:
                raise ValueError("independent image digest mismatch")
            (prefix / "image.raw.json").write_bytes(raw_image)
    return semantic

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--old-log", required=True)
    parser.add_argument("--current-log", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    out = external(args.output)
    out.mkdir(parents=True, exist_ok=False)
    compared = []
    receipt = {"schema":"semaprax.private-847-wire-comparison.v1","normalization":"none", "subjects":[]}
    for role, log in [("old",args.old_log),("current",args.current_log)]:
        log = external(log)
        packet, log_hash = frame(log)
        if packet["mode"] not in ["semantic", "cache"]:
            raise ValueError("wire comparison requires semantic or cache mode")
        target = out / role
        target.mkdir()
        compared.append(materialize(packet,target))
        receipt["subjects"].append({"role":role,"label":packet["subject"],"log":str(log),"log_sha256":log_hash,"request_sha256":packet["request_sha256"],"mode":packet["mode"]})
    if receipt["subjects"][0]["request_sha256"] != receipt["subjects"][1]["request_sha256"] or receipt["subjects"][0]["mode"] != receipt["subjects"][1]["mode"]:
        raise ValueError("paired protocol request bytes differ")
    receipt["exact_semantic_and_work_equal"] = compared[0] == compared[1]
    receipt["whole_raw_graph_and_image_equal"] = all((out / "current" / p.relative_to(out / "old")).exists() and p.read_bytes() == (out / "current" / p.relative_to(out / "old")).read_bytes() for p in (out / "old").rglob("*.raw.json"))
    (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt))
    if not receipt["exact_semantic_and_work_equal"] or not receipt["whole_raw_graph_and_image_equal"]:
        raise SystemExit(1)
if __name__ == "__main__":
    main()
