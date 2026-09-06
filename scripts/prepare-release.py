#!/usr/bin/env python3
"""Prepare or verify the mechanical surfaces of a SEMAPRAX version release."""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
VERSION_RE = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)")
DATE_RE = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")

# Current package-release surfaces only. Historical evidence, dependency test
# vectors, and frozen protocol/WIT identities are deliberately absent.
VERSION_FILES = (
    "Cargo.toml",
    "crates/semaprax-toolchain/Cargo.toml",
    "crates/semaprax-doctor-collector/Cargo.toml",
    "crates/semaprax-native-host/Cargo.toml",
    "crates/semaprax-native-rust-interop-builder/Cargo.toml",
    "crates/semaprax-offline-wasm-package/Cargo.toml",
    "platform-tests/component-runtime/Cargo.toml",
    "platform-tests/public-scalar-wit-interface/Cargo.toml",
    "tests/cli_version_v1.rs",
    "tests/agent_transport_v1.rs",
    "tests/component_runtime_ci_contract.rs",
    "tests/public_scalar_wit_interface_external_contract.rs",
    "crates/semaprax-doctor-collector/tests/provisioned.rs",
    "crates/semaprax-doctor-collector/tests/support/report.rs",
    "crates/semaprax-toolchain/tests/cli_doctor_v1.rs",
    ".github/workflows/doctor-provisioned-linux.yml",
    "README.md",
    "docs/INSTALL.md",
    "docs/index.md",
)
LOCK_MANIFESTS = (
    "Cargo.toml",
    "examples/calculator-rust/Cargo.toml",
    "examples/owned-data-rust/Cargo.toml",
    "platform-tests/component-runtime/Cargo.toml",
    "platform-tests/public-scalar-wit-interface/Cargo.toml",
)


def reject(message):
    raise ValueError(message)


def root_version():
    text = (ROOT / "Cargo.toml").read_text()
    match = re.search(r'^version = "([^"]+)"$', text, re.MULTILINE)
    if not match or not VERSION_RE.fullmatch(match.group(1)):
        reject("root Cargo package version is missing or noncanonical")
    return match.group(1)


def release_date():
    text = (ROOT / "CITATION.cff").read_text()
    match = re.search(r"^date-released: ([0-9-]+)$", text, re.MULTILINE)
    if not match or not DATE_RE.fullmatch(match.group(1)):
        reject("CITATION.cff release date is missing or noncanonical")
    return match.group(1)


def replace_required(text, old, new, label):
    if old not in text:
        reject(f"{label} does not contain expected release identity {old!r}")
    return text.replace(old, new)


def verify(version):
    if root_version() != version:
        reject("requested version does not match root Cargo.toml")
    tag = f"v{version}"
    for relative in VERSION_FILES:
        text = (ROOT / relative).read_text()
        if version not in text and tag not in text:
            reject(f"{relative} does not carry {version}")
    date = release_date()
    citation = (ROOT / "CITATION.cff").read_text()
    if f'version: "{version}"' not in citation:
        reject("CITATION.cff version disagrees")
    metadata = json.loads((ROOT / "codemeta.json").read_text())
    if metadata.get("version") != version or metadata.get("dateModified") != date:
        reject("CodeMeta version/date disagrees with citation metadata")
    if f"## {version} — {date}" not in (ROOT / "CHANGELOG.md").read_text():
        reject("dated changelog release heading is missing")
    for manifest in LOCK_MANIFESTS:
        subprocess.run(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1", "--manifest-path", manifest],
            cwd=ROOT,
            stdout=subprocess.DEVNULL,
            check=True,
        )
    print(f"release surfaces agree on {tag} ({date})")


def write(version, date):
    old = root_version()
    if old == version:
        reject("new version already equals the root package version; use --check")
    dirty = subprocess.run(
        ["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout
    if dirty:
        reject("release preparation requires a clean worktree")
    old_tag, tag = f"v{old}", f"v{version}"
    updates = {}
    for relative in VERSION_FILES:
        text = (ROOT / relative).read_text()
        if old in text:
            text = replace_required(text, old, version, relative)
        if old_tag in text:
            text = replace_required(text, old_tag, tag, relative)
        updates[relative] = text
    citation = (ROOT / "CITATION.cff").read_text()
    citation = replace_required(
        citation, f'version: "{old}"', f'version: "{version}"', "CITATION.cff"
    )
    citation = re.sub(
        r"^date-released: [0-9-]+$",
        f"date-released: {date}",
        citation,
        count=1,
        flags=re.MULTILINE,
    )
    updates["CITATION.cff"] = citation
    metadata = json.loads((ROOT / "codemeta.json").read_text())
    metadata["version"] = version
    metadata["dateModified"] = date
    updates["codemeta.json"] = json.dumps(metadata, indent=2, ensure_ascii=False) + "\n"
    changelog = (ROOT / "CHANGELOG.md").read_text()
    marker = "## Unreleased\n"
    if marker not in changelog or f"## {version} —" in changelog:
        reject("changelog cannot accept the requested release heading")
    updates["CHANGELOG.md"] = changelog.replace(
        marker, f"{marker}\n## {version} — {date}\n", 1
    )
    for relative, text in updates.items():
        (ROOT / relative).write_text(text)
    for manifest in LOCK_MANIFESTS:
        subprocess.run(
            [
                "cargo",
                "update",
                "--offline",
                "--manifest-path",
                manifest,
                "-p",
                "semaprax",
                "--precise",
                version,
            ],
            cwd=ROOT,
            check=True,
        )
    print(f"prepared mechanical release surfaces for {tag} ({date})")
    print("review release notes and RELEASE-PROCESS.md, run gates, commit, then tag")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--date")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    args = parser.parse_args(argv)
    if not VERSION_RE.fullmatch(args.version):
        reject("--version must be canonical major.minor.patch")
    if args.write:
        if args.date is None or not DATE_RE.fullmatch(args.date):
            reject("--write requires canonical --date YYYY-MM-DD")
        write(args.version, args.date)
    else:
        if args.date is not None:
            reject("--date is derived from release metadata in --check mode")
        verify(args.version)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"release preparation rejected: {error}", file=sys.stderr)
        sys.exit(2)
