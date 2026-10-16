#!/usr/bin/env python3
"""Trace the unchanged low-stack unit case after its owning Linux shard fails."""

import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
CASE = (
    "hir::validation::iterative_while_admission_tests::"
    "exact_512_nested_indexed_matches_validate_on_a_low_stack"
)


def select_executable(messages, root, target):
    """Select one current Cargo root-library test artifact, never a guessed path."""
    # https://doc.rust-lang.org/cargo/reference/external-tools.html#artifact-messages
    candidates = []
    for line in messages.splitlines():
        item = json.loads(line)
        if item.get("reason") != "compiler-artifact":
            continue
        artifact = item.get("target", {})
        if artifact.get("name") != "semaprax" or artifact.get("kind") != ["lib"]:
            continue
        if item.get("profile", {}).get("test") is not True:
            continue
        if Path(item.get("manifest_path", "")).resolve() != (root / "Cargo.toml").resolve():
            continue
        if Path(artifact.get("src_path", "")).resolve() != (root / "src/lib.rs").resolve():
            raise ValueError("root library artifact has a foreign source")
        executable = item.get("executable")
        if not executable:
            raise ValueError("root library test artifact has no executable")
        path = Path(executable).resolve()
        if not path.is_relative_to(target.resolve()) or not path.is_file():
            raise ValueError("root library test executable escaped the current target directory")
        if not os.access(path, os.X_OK):
            raise ValueError("root library test artifact is not executable")
        candidates.append(path)
    if len(candidates) != 1:
        raise ValueError(f"expected one current root library test artifact, found {len(candidates)}")
    return candidates[0]


def main():
    if sys.platform != "linux" or os.environ.get("CI") != "true":
        raise ValueError("the stack trace runs only on the owning hosted Linux CI runner")
    expected = os.environ.get("GITHUB_SHA", "")
    current = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if not re.fullmatch(r"[0-9a-f]{40}", expected) or current != expected:
        raise ValueError("stack trace checkout differs from the current CI commit")
    debugger = shutil.which("gdb")
    if debugger is None:
        raise ValueError("the Linux runner has no gdb; no stack trace was captured")
    router = runpy.run_path(str(ROOT / "scripts/ci-msrv.py"))
    environment = router["cargo_environment"]()
    # Match the unit shard's complete feature unification. This only asks Cargo
    # for its current artifact records; it neither substitutes a saved executable
    # nor changes the owning shard's command, flags, result or test inventory.
    messages = router["capture_listing"](
        [*router["TEST"], "--offline", "--lib", "--no-run", "--message-format=json"],
        environment,
    )
    target = Path(environment.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    executable = select_executable(messages, ROOT, target)
    listing = router["parse_listing"](router["capture_listing"](
        [str(executable), CASE, "--exact", "--list"], environment,
    ))
    ignored = router["parse_listing"](router["capture_listing"](
        [str(executable), CASE, "--exact", "--list", "--ignored"], environment,
    ))
    if listing != [CASE] or ignored:
        raise ValueError("the current executable does not contain the exact runnable stack case")
    digest = hashlib.sha256()
    with executable.open("rb") as stream:
        for chunk in iter(lambda: stream.read(64 * 1024), b""):
            digest.update(chunk)
    print(json.dumps({
        "diagnostic_only": True, "head": current, "case": CASE,
        "executable": str(executable.relative_to(ROOT)),
        "executable_sha256": digest.hexdigest(),
        "qualification": "owning shard failure remains authoritative",
    }, sort_keys=True), flush=True)
    commands = [
        "set pagination off", "set print thread-events off",
        "set print frame-arguments none", "set auto-load off",
        "set debuginfod enabled off", "set disable-randomization off",
        "handle SIGPIPE nostop noprint pass", "handle SIGSEGV stop print nopass",
        "handle SIGBUS stop print nopass", "handle SIGABRT stop print nopass",
        "run", "thread apply all bt 48",
    ]
    command = [debugger, "--batch", "--nx"]
    for instruction in commands:
        command.extend(["-ex", instruction])
    command.extend(["--args", str(executable), CASE, "--exact", "--nocapture", "--test-threads=1"])
    # The original test creates its original 128 KiB thread. Stop at the first
    # fault and print bounded frames, without increasing that stack or depth.
    return subprocess.run(command, cwd=ROOT, env=environment, timeout=180).returncode


if __name__ == "__main__":
    raise SystemExit(main())
