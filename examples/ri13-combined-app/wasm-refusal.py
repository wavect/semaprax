#!/usr/bin/env python3
"""Classify RI-13 Project Wasm requests without claiming Rust interop support."""
import argparse
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
from datetime import datetime, timezone

ROOT = pathlib.Path(__file__).resolve().parent
EXAMPLES = ROOT.parent
SCHEMA = "semaprax.ri13.wasm-target-classification.v1"
PROJECTS = {
    "m1": (EXAMPLES / "ri13-m1-regex-url/project/semaprax.toml", "implicit scalar.v1", "refused"),
    "m2": (EXAMPLES / "ri13-m2-record-iterator/project/semaprax.toml", "implicit scalar.v1", "supported"),
    "m3": (EXAMPLES / "ri13-m3-local-http/project/semaprax.toml", "source-local-future.v1", "refused"),
}


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def head(root):
    return subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True, stderr=subprocess.DEVNULL).strip()


def profile(manifest):
    matched = re.search(r'^profile\s*=\s*"([^"]+)"\s*$', manifest.read_text(), re.MULTILINE)
    return matched.group(1) if matched else "implicit scalar.v1"


def write_artifact(directory, name, content):
    path = directory / name
    path.write_text(content)
    return {"path": name, "sha256": digest(path)}


def run_one(binary, artifact_dir, label, manifest, selected_profile, expected):
    output = artifact_dir / f"{label}.wasm"
    command = [str(binary), "build", "--manifest-path", str(manifest), "--target", "wasm", "--output", str(output)]
    completed = subprocess.run(command, capture_output=True, text=True)
    stderr = write_artifact(artifact_dir, f"{label}.stderr", completed.stderr)
    stdout = write_artifact(artifact_dir, f"{label}.stdout", completed.stdout)
    codes = sorted(set(re.findall(r"SPX-[A-Z]\d{3}", completed.stderr)))
    result = {
        "requested_target": "wasm",
        "project_profile": selected_profile,
        "manifest": {"path": str(manifest.resolve()), "sha256": digest(manifest)},
        "command": command,
        "exit_code": completed.returncode,
        "stdout": stdout,
        "stderr": stderr,
        "diagnostic_codes": codes,
        "output_exists": output.exists(),
        "expected": expected,
    }
    if expected == "refused":
        if completed.returncode == 0 or output.exists():
            result.update({"status": "failed", "reason": "Wasm request emitted an artifact or did not refuse"})
        else:
            result["status"] = "refused"
    elif completed.returncode != 0 or not output.exists():
        result.update({"status": "failed", "reason": "supported scalar Project Wasm request did not emit its artifact"})
    else:
        result["status"] = "supported"
    return result


def run(binary, checkout, expected_commit, artifact_dir):
    observed = head(checkout)
    result = {
        "schema": SCHEMA,
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "compiler": {"path": str(binary.resolve()), "sha256": digest(binary)},
        "checkout": {"path": str(checkout.resolve()), "expected_commit": expected_commit, "observed_commit": observed},
        "paths": {},
        "nonclaims": [
            "M2's supported row covers only its ordinary scalar Project Wasm artifact",
            "M2's generated Serde record and Rust Iterator callback projection has no Wasm target subject in this command",
            "this command does not test native target behavior, runtime execution, or cross-target portability",
        ],
    }
    if observed != expected_commit:
        result.update({"status": "unavailable", "reason": "compiler checkout differs from requested commit"})
        return result
    artifact_dir.mkdir(parents=True, exist_ok=False)
    for label, (manifest, required_profile, expected) in PROJECTS.items():
        actual = profile(manifest)
        if actual != required_profile:
            raise ValueError(f"{label} profile changed from its reviewed refusal subject")
        result["paths"][label] = run_one(binary, artifact_dir, label, manifest, actual, expected)
    result["status"] = "completed" if all(
        row["status"] == row["expected"] for row in result["paths"].values()
    ) else "failed"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--semaprax", required=True, type=pathlib.Path)
    parser.add_argument("--checkout", required=True, type=pathlib.Path)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--artifact-dir", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args()
    if not args.semaprax.is_file():
        parser.error("--semaprax must name an existing compiler binary")
    try:
        document = run(args.semaprax, args.checkout, args.commit, args.artifact_dir)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.error(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    raise SystemExit(0 if document["status"] == "completed" else 1)


if __name__ == "__main__":
    main()
