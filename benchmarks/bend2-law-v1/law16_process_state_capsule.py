#!/usr/bin/env python3
"""Authenticate one bounded LAW-16 process-state evidence capsule offline."""
import argparse
import hashlib
import json
import pathlib
import stat
import statistics


SCHEMA = "semaprax.bend2-law-benchmark.cold-warm-process-cell.v2"
RESULT_SCHEMA = "semaprax.bend2-law-benchmark.process-state-capsule-review.v1"
MAX_STREAM_BYTES = 1024 * 1024


def digest(body):
    return "sha256:" + hashlib.sha256(body).hexdigest()


def read_json(path, label):
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {label}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} is not an object")
    return value


def raw_reference(root, reference):
    if not isinstance(reference, dict) or set(reference) != {"path", "bytes", "sha256"}:
        raise ValueError("raw stream reference is malformed")
    name, count, expected = reference["path"], reference["bytes"], reference["sha256"]
    if not isinstance(name, str) or not isinstance(count, int) or not isinstance(expected, str) or not 0 <= count <= MAX_STREAM_BYTES:
        raise ValueError("raw stream reference is invalid")
    relative = pathlib.PurePosixPath(name)
    if relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts):
        raise ValueError("raw stream path is unsafe")
    path = root / "raw"
    try:
        for part in relative.parts:
            path /= part
            details = path.lstat()
            if stat.S_ISLNK(details.st_mode):
                raise ValueError("raw stream crosses a symbolic link")
        if not stat.S_ISREG(details.st_mode) or details.st_size != count:
            raise ValueError("raw stream bytes disagree")
        body = path.read_bytes()
    except OSError as error:
        raise ValueError("raw stream is unavailable") from error
    if digest(body) != expected:
        raise ValueError("raw stream digest disagrees")


def review(root):
    root = root.resolve(strict=True)
    receipt = read_json(root / "receipt.json", "receipt")
    if receipt.get("schema") != SCHEMA:
        raise ValueError("receipt schema is unsupported")
    cells = receipt.get("cells")
    if not isinstance(cells, dict) or set(cells) != {"fresh_process", "repeat_process"}:
        raise ValueError("receipt does not contain both process states")
    artifacts = receipt.get("artifacts")
    if not isinstance(artifacts, list) or len(artifacts) < 2:
        raise ValueError("receipt does not bind fresh and repeat inputs")
    fresh, repeat = artifacts[:2]
    if fresh.get("role") != "fresh_input" or repeat.get("role") != "repeat_input":
        raise ValueError("receipt input roles are malformed")
    for name, reference in (("fresh", fresh), ("repeat", repeat)):
        path = root / f"{name}-input.spx"
        body = path.read_bytes()
        if len(body) != reference.get("bytes") or digest(body) != reference.get("sha256"):
            raise ValueError(f"{name} input does not match receipt")
    if fresh["bytes"] != repeat["bytes"] or fresh["sha256"] != repeat["sha256"]:
        raise ValueError("fresh and repeat inputs differ")
    stream_count = 0
    summaries = {}
    for state, cell in cells.items():
        if not isinstance(cell, dict) or cell.get("status") != "completed":
            raise ValueError(f"{state} is not a completed process observation")
        samples, summary = cell.get("samples"), cell.get("summary")
        if not isinstance(samples, list) or len(samples) != 30 or not isinstance(summary, dict):
            raise ValueError(f"{state} lacks exactly thirty samples")
        elapsed = []
        for sample in samples:
            if not isinstance(sample, dict) or sample.get("exit_code") != 0 or not isinstance(sample.get("elapsed_ns"), int):
                raise ValueError(f"{state} sample is not a successful timed child")
            raw_reference(root, sample.get("stdout"))
            raw_reference(root, sample.get("stderr"))
            elapsed.append(sample["elapsed_ns"])
            stream_count += 2
        ordered = sorted(elapsed)
        expected = {"count": 30, "p50_ns": statistics.median(elapsed), "p95_ns": ordered[(95 * len(ordered) + 99) // 100 - 1]}
        if summary != expected:
            raise ValueError(f"{state} summary disagrees with raw elapsed samples")
        summaries[state] = expected
    identity = read_json(root / "tool-identity.json", "tool identity")
    if identity.get("schema") != "semaprax.bend2-law-benchmark.local-tool-identity.v1":
        raise ValueError("tool identity schema is unsupported")
    return {
        "schema": RESULT_SCHEMA,
        "status": "local_process_state_authenticated",
        "tool": {"commit": identity.get("commit"), "executable_sha256": identity.get("executable_sha256")},
        "route": identity.get("route"),
        "states": summaries,
        "raw_streams": stream_count,
        "cold_state": receipt.get("cold_state"),
        "nonclaims": [
            "fresh-path and repeat-path processes do not isolate OS or tool caches",
            "no cold-versus-warm comparison, winner, proof, or runtime throughput result",
            "local pinned executable evidence is not current-head evidence",
        ],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capsule", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.output.exists() or not args.output.parent.is_dir():
        parser.error("--output must be new beneath an existing directory")
    try:
        result = review(args.capsule)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
