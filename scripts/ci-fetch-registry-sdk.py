#!/usr/bin/env python3
"""Provision the frozen registry SDK closure before its offline physical test."""

import argparse
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ('[package]\nname="ri06-url-owner"\nversion="0.1.0"\nedition="2021"\npublish=false\n'
            '\n[lib]\npath="src/lib.rs"\n\n[workspace]\n\n[dependencies]\n'
            'url_alias={package="url",version="=2.5.8"}\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true", help="verify the provisioned cache without network")
    arguments = parser.parse_args()
    if sys.platform != "darwin" or platform.machine() not in ("arm64", "aarch64"):
        return
    lock = (ROOT / "crates/semaprax-toolchain/src/fixtures/ri06-url-2.5.8.Cargo.lock").read_bytes()
    # This is a dependency-provisioning step, never a build or a lock update.
    with tempfile.TemporaryDirectory(prefix="semaprax-registry-sdk-fetch-", dir=os.environ.get("RUNNER_TEMP")) as directory:
        fixture = Path(directory)
        (fixture / "src").mkdir()
        (fixture / "src/lib.rs").write_text("// Dependency provisioning only.\n")
        (fixture / "Cargo.toml").write_text(MANIFEST)
        (fixture / "Cargo.lock").write_bytes(lock)
        command = [os.environ.get("CARGO", "cargo"), "fetch", "--locked"]
        if arguments.offline:
            command.append("--offline")
        subprocess.run(command, cwd=fixture, check=True)
        if (fixture / "Cargo.lock").read_bytes() != lock:
            raise RuntimeError("registry SDK provisioning changed the frozen lock")


if __name__ == "__main__":
    main()
