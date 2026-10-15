"""Verify complete merge-input custody with bounded, streaming reads."""
from pathlib import Path
import gzip
import hashlib
import json

HERE = Path(__file__).resolve().parent
index = json.loads((HERE / "index.json").read_bytes())
count = 0
total = 0
for conflict in index["conflicts"]:
    assert {entry["stage"] for entry in conflict["stages"]} == {1, 2, 3}
    for entry in conflict["stages"]:
        archive = HERE / entry["archive"]
        compressed = hashlib.sha256()
        with archive.open("rb") as stream:
            while chunk := stream.read(1024 * 1024):
                compressed.update(chunk)
        assert compressed.hexdigest() == entry["archive_sha256"], archive
        raw = hashlib.sha256()
        git = hashlib.sha1(
            b"blob " + str(entry["uncompressed_bytes"]).encode("ascii") + b"\0"
        )
        size = 0
        with gzip.open(archive, "rb") as stream:
            while chunk := stream.read(1024 * 1024):
                size += len(chunk)
                raw.update(chunk)
                git.update(chunk)
        assert size == entry["uncompressed_bytes"], archive
        assert raw.hexdigest() == entry["uncompressed_sha256"], archive
        assert git.hexdigest() == entry["git_blob"], archive
        count += 1
        total += size
assert count == 30
print(json.dumps({"complete_merge_inputs": count, "uncompressed_bytes": total,
                  "all_git_blob_ids_and_sha256_verified": True,
                  "stream_chunk_bytes": 1024 * 1024,
                  "compiler_or_runtime_qualification": False}))
