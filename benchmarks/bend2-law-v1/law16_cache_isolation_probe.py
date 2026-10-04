#!/usr/bin/env python3
"""Validate the retained, non-destructive LAW-16 cache-isolation probe.

This receipt is capability evidence only. It contains no checker timings and
does not call a fresh process or path an OS-cache-cold observation.
"""
import argparse
import hashlib
import json
import pathlib

SCHEMA = "semaprax.bend2-law-benchmark.cache-isolation-probe.v1"
IMAGE = "rust:1.98.0-slim-bookworm"
IMAGE_DIGEST = "sha256:1469a27c125cb5a3aebfa4f4e4665d935b02fb72cc093b2c974b3d740e43f157"
ROOT = pathlib.Path(__file__).parent
RECEIPT = ROOT / "evidence/law16-cache-isolation-probe-v1"


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def review(root=RECEIPT):
    receipt = json.loads((root / "receipt.json").read_text())
    if receipt.get("schema") != SCHEMA or receipt.get("status") != "capability_probe_only":
        raise ValueError("cache isolation receipt schema or status drifted")
    image = receipt.get("probe_image", {})
    if image.get("name") != IMAGE or image.get("digest") != IMAGE_DIGEST:
        raise ValueError("receipt does not bind the exact inspected local image")
    probe = receipt.get("guest_probe", {})
    argv = probe.get("argv")
    if not isinstance(argv, list) or probe.get("command_sha256") != digest(json.dumps(argv, separators=(",", ":")).encode()):
        raise ValueError("guest probe command or digest is missing")
    if probe.get("exit_code") != 0 or probe.get("measurement_samples") != 0:
        raise ValueError("guest probe must complete without producing timing samples")
    required_args = ["run", "--rm", "--memory", "256M", "--cpus", "1", "--arch", "amd64", "--rosetta", "--cap-add", "ALL", IMAGE]
    if argv[1:1 + len(required_args)] != required_args:
        raise ValueError("guest probe command no longer matches the recorded ephemeral capability probe")
    transcript_ref = probe.get("transcript")
    if not isinstance(transcript_ref, dict):
        raise ValueError("guest probe transcript reference is missing")
    transcript_path = root / transcript_ref.get("path", "")
    if not transcript_path.is_file():
        raise ValueError("guest probe transcript is missing")
    transcript = transcript_path.read_bytes()
    if transcript_ref.get("bytes") != len(transcript) or transcript_ref.get("sha256") != digest(transcript):
        raise ValueError("guest probe transcript digest or size drifted")
    transcript_text = transcript.decode("utf-8")
    if "uid=0(root)" not in transcript_text or "proc /proc/sys proc ro," not in transcript_text or "not_writable" not in transcript_text:
        raise ValueError("guest transcript does not establish the read-only cache-control limitation")
    if probe.get("drop_caches_writable") is not False or probe.get("measurement_samples") != 0:
        raise ValueError("probe must fail closed on cache-control access and report zero measurements")
    containers = receipt.get("container_state", {})
    if containers.get("before_running") != 0 or containers.get("after_running") != 0:
        raise ValueError("receipt must show no existing or leftover running containers")
    cold_state = receipt.get("cold_state", {})
    if cold_state.get("status") != "unavailable":
        raise ValueError("a capability-only probe cannot qualify cache-cold measurements")
    if receipt.get("checking_measurements") != {"status": "not_collected", "samples": 0}:
        raise ValueError("receipt must not imply checker measurements")
    return receipt


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--receipt-root", type=pathlib.Path, default=RECEIPT)
    args = parser.parse_args(argv)
    try:
        review(args.receipt_root)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        parser.error(str(error))
    print("cache isolation unavailable; zero checking measurements authenticated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
