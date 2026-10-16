#!/usr/bin/env python3
"""Capture the exact current macOS libtest executable for bounded SSD replay.

This builds nothing locally in the agent workflow: CI calls it after compiling
and failing its unit lane. Cargo selects the executable; globs and old target
files cannot substitute a binary. Existing test execution and results remain
unchanged. The short-lived artifact grants no release or installation claim.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

COMMAND = [
    "cargo", "test", "--locked", "--offline", "-p", "semaprax",
    "--all-features", "--lib", "--no-run", "--message-format=json",
]
CHUNK = 512 * 1024


def capture(root, destination, commit):
    root = Path(root).resolve()
    destination = Path(destination)
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("expected a complete lowercase Git commit")
    held = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    if held != commit:
        raise ValueError("checkout does not match the requested commit")
    for command in (["git", "diff", "--exit-code"], ["git", "diff", "--cached", "--exit-code"]):
        subprocess.run(command, cwd=root, check=True, stdout=subprocess.DEVNULL)
    candidates = []
    with subprocess.Popen(COMMAND, cwd=root, stdout=subprocess.PIPE, text=True) as process:
        for line in process.stdout:
            row = json.loads(line)
            if row.get("reason") != "compiler-artifact":
                continue
            target = row["target"]
            if (target.get("name") == "semaprax" and target.get("kind") == ["lib"]
                    and Path(target["src_path"]).resolve() == root / "src/lib.rs"
                    and row.get("profile", {}).get("test") is True and row.get("executable")):
                candidates.append(Path(row["executable"]).resolve())
        if process.wait():
            raise ValueError("Cargo did not finish the unit executable selection")
    if len(candidates) != 1:
        raise ValueError("Cargo must identify exactly one current semaprax libtest executable")
    executable = candidates[0]
    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", root / "target")).resolve()
    executable.relative_to(target_dir)
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise ValueError("selected unit executable is missing or not executable")
    compiler = subprocess.check_output(["rustc", "-vV"], cwd=root, text=True)
    destination.mkdir(parents=True, exist_ok=False)
    digest = hashlib.sha256()
    with executable.open("rb") as source, (destination / "semaprax-unit").open("xb") as output:
        before = os.fstat(source.fileno())
        for chunk in iter(lambda: source.read(CHUNK), b""):
            digest.update(chunk)
            output.write(chunk)
        after = os.fstat(source.fileno())
    if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
            after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns):
        raise ValueError("selected executable changed while copied")
    (destination / "semaprax-unit").chmod(0o755)
    receipt = {
        "schema": "semaprax.ci-unit-executable.v1", "commit": held,
        "cargo_command": COMMAND, "rustc_verbose_version": compiler,
        "source": str(executable.relative_to(target_dir)),
        "bytes": before.st_size, "sha256": digest.hexdigest(),
        "cargo_lock_sha256": hashlib.sha256((root / "Cargo.lock").read_bytes()).hexdigest(),
        "scope": "CI unit executable only; no release, install or completed-test claim",
    }
    (destination / "provenance.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    receipt = capture(Path.cwd(), args.output, args.commit)
    print(json.dumps({"commit": receipt["commit"], "bytes": receipt["bytes"],
                      "sha256": receipt["sha256"]}))


if __name__ == "__main__":
    main()
