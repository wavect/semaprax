#!/usr/bin/env python3
"""Build (or check) `semaprax.release-provenance.v1` (#168).

Issue #167 built `scripts/release-manifest.py`: version, tag, commit, the
required-check inventory, the artifact inventory (platform/size/digest), the
prerelease flag, and a changelog-section digest. Issue #168 additionally asks
for provenance that binds **source commit, workflow identity, toolchain
inputs, build host class, artifact digests, and publication event**. This
script is the aggregate provenance document over an *already-built* manifest:
it never restates the manifest's own fields by re-deriving them independently
(that would be a second, potentially-drifting source of truth) -- it copies
them verbatim from the manifest's own bytes and additionally records a
byte-exact `manifest_digest` of those bytes, so the pair can be independently
rebound later by `src/release_provenance.rs`.

This is generated evidence, not a publication decision, exactly like
`scripts/release-manifest.py`: building a provenance document here creates no
GitHub Release, calls no network API, signs nothing, and grants no authority.
It also does not itself claim publication occurred -- the real GitHub Release
publication event remains recorded only in `docs/RELEASE-PROCESS.md`'s dated
`## X.Y.Z hosted release evidence` section, written by hand after a real,
already-published Release is confirmed (see that document's "failure and
recovery path").

Every provenance-only input (workflow identity, run id/attempt, toolchain
version, build host class) is an explicit required argument. None of it is
read from the ambient environment: `AGENTS.md` grants generated code no
ambient authority, and a provenance document that silently trusted whatever
environment variables happened to be set when this script ran would be a
weaker claim than one whose caller had to state each field. The real CI job
that would run this (see `docs/RELEASE-SIGNING-POLICY-V1.md`) reads those
values from its own `github.*`/`runner.*` context and passes them explicitly
on the command line; this script never assumes it is running inside CI.

Two modes, mirroring `release-manifest.py`:

  --manifest PATH --workflow-identity ... --run-id ... --run-attempt ...
  --rustc-version ... --host-class ... [--output PATH]
      Build the provenance document and print it (or write it to --output).

  --check PATH (with the same other flags)
      Recompute the provenance document the same way, then diff it
      field-by-field against the document already on disk at PATH. Exit 0
      only if every field agrees.
"""

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

SCHEMA = "semaprax.release-provenance.v1"
MANIFEST_SCHEMA = "semaprax.release-manifest.v1"

# Trusted identity policy v1. Must exactly match the Rust constants of the
# same name in `src/release_provenance.rs` and the "Trusted identity policy
# v1" table in `docs/RELEASE-SIGNING-POLICY-V1.md` --
# `tests/offline_package/release_provenance.rs` cross-checks all three.
TRUSTED_REPOSITORY = "wavect/semaprax"
TRUSTED_WORKFLOW_PATH = ".github/workflows/ci.yml"

HOST_CLASSES = (
    "github-hosted-ubuntu-24.04",
    "github-hosted-macos-15",
    "github-hosted-windows-2025",
)

VERSION_RE = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")

NONCLAIMS = [
    "unsigned_without_a_paired_signature_claim",
    "not_a_reproducible_build_claim",
    "not_a_notarization_or_code_signing_claim",
    "not_a_publication_confirmation",
    "not_a_production_support_or_safety_claim",
    "not_a_semantic_or_compiler_correctness_verification",
    "not_a_runtime_or_package_authority_grant",
]


def reject(message):
    raise ValueError(message)


def sha256_digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def build_provenance(manifest_bytes, workflow_identity, run_id, run_attempt, rustc_version, host_class):
    """Assemble the complete `semaprax.release-provenance.v1` document.

    Trusts nothing about the manifest beyond its own bytes: parses it,
    validates its schema and shape, and copies its version/tag/commit/
    prerelease/required_checks/artifacts fields verbatim rather than
    re-deriving them from CHANGELOG.md or the CI workflow a second time --
    `release-manifest.py` already did that derivation once.
    """
    manifest = json.loads(manifest_bytes)
    if manifest.get("schema") != MANIFEST_SCHEMA:
        reject(f"manifest schema must be {MANIFEST_SCHEMA}, found {manifest.get('schema')!r}")
    version = manifest.get("version")
    if not isinstance(version, str) or not VERSION_RE.fullmatch(version):
        reject(f"manifest version is not a canonical version: {version!r}")
    tag = manifest.get("tag")
    if tag != f"v{version}":
        reject(f"manifest tag {tag!r} does not equal v plus the version {version!r}")
    commit = manifest.get("commit")
    if not isinstance(commit, str) or not COMMIT_RE.match(commit):
        reject(f"manifest commit must be exactly 40 lowercase hexadecimal characters: {commit!r}")
    prerelease = manifest.get("prerelease")
    if not isinstance(prerelease, bool):
        reject("manifest prerelease must be a boolean")
    required_checks = manifest.get("required_checks")
    if not isinstance(required_checks, list) or not required_checks:
        reject("manifest required_checks must be a non-empty array")
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        reject("manifest artifacts must be a non-empty array")

    expected_identity = f"{TRUSTED_REPOSITORY}/{TRUSTED_WORKFLOW_PATH}@refs/tags/{tag}"
    if workflow_identity != expected_identity:
        reject(
            f"--workflow-identity {workflow_identity!r} does not match the expected trusted "
            f"identity {expected_identity!r} for tag {tag!r} (see "
            "docs/RELEASE-SIGNING-POLICY-V1.md's trusted identity policy)"
        )
    if host_class not in HOST_CLASSES:
        reject(f"--host-class {host_class!r} is not one of the admitted host classes {HOST_CLASSES}")
    if not rustc_version:
        reject("--rustc-version must not be empty")
    if not run_id:
        reject("--run-id must not be empty")
    if not run_attempt:
        reject("--run-attempt must not be empty")

    manifest_digest = sha256_digest(manifest_bytes)

    return {
        "schema": SCHEMA,
        "version": version,
        "tag": tag,
        "commit": commit,
        "prerelease": prerelease,
        "required_checks": required_checks,
        "artifacts": artifacts,
        "manifest_digest": manifest_digest,
        "source": {"repository": TRUSTED_REPOSITORY, "commit": commit, "tag": tag},
        "builder": {
            "workflow_identity": workflow_identity,
            "run_id": run_id,
            "run_attempt": run_attempt,
        },
        "toolchain": {"rustc_version": rustc_version, "cargo_locked": True},
        "build_host_class": host_class,
        "nonclaims": NONCLAIMS,
    }


def diff_provenance(existing, expected):
    """Every field disagreement between an on-disk provenance document and a
    freshly recomputed one, empty when they fully agree."""
    problems = []
    for key in (
        "schema",
        "version",
        "tag",
        "commit",
        "prerelease",
        "manifest_digest",
        "build_host_class",
    ):
        if existing.get(key) != expected.get(key):
            problems.append(
                f"{key} mismatch: document has {existing.get(key)!r}, recomputed {expected.get(key)!r}"
            )
    for key in ("required_checks", "artifacts", "source", "builder", "toolchain", "nonclaims"):
        if existing.get(key) != expected.get(key):
            problems.append(f"{key} disagrees with the recomputed document")
    return problems


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        required=True,
        help="path to an already-built semaprax.release-manifest.v1 document",
    )
    parser.add_argument(
        "--workflow-identity",
        required=True,
        help="e.g. wavect/semaprax/.github/workflows/ci.yml@refs/tags/v0.4.2",
    )
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--run-attempt", required=True)
    parser.add_argument("--rustc-version", required=True)
    parser.add_argument("--host-class", required=True, choices=HOST_CLASSES)
    parser.add_argument("--output", type=Path, help="write the document here instead of stdout")
    parser.add_argument(
        "--check",
        type=Path,
        help="diff an existing document at this path against the recomputed one instead of writing",
    )
    args = parser.parse_args(argv)

    manifest_bytes = args.manifest.read_bytes()
    provenance = build_provenance(
        manifest_bytes,
        args.workflow_identity,
        args.run_id,
        args.run_attempt,
        args.rustc_version,
        args.host_class,
    )

    if args.check is not None:
        existing = json.loads(args.check.read_text(encoding="utf-8"))
        problems = diff_provenance(existing, provenance)
        for problem in problems:
            print(f"release provenance: {problem}", file=sys.stderr)
        if problems:
            return 1
        print(f"release provenance: {args.check} agrees with recomputed evidence for {provenance['tag']}")
        return 0

    rendered = json.dumps(provenance, indent=2) + "\n"
    if args.output is not None:
        args.output.write_text(rendered, encoding="utf-8")
        print(f"release provenance: wrote {args.output}")
    else:
        sys.stdout.write(rendered)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print(f"release provenance rejected: {error}", file=sys.stderr)
        sys.exit(2)
