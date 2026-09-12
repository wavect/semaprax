#!/usr/bin/env python3
"""Build (or verify) the canonical machine-readable release manifest (#167).

Issue #167's required outcome lists the fields a release manifest must carry:
version, tag, commit, the **required-check inventory**, the **artifact
inventory** (platform, size, digest), the **prerelease flag**, and a
**changelog-section digest**. The existing `semaprax.release-artifact.v1`
document that `scripts/package-release.sh`/`.ps1` embed inside each single
archive is narrower by design -- it is written *before* the sibling archives
or their digests exist, so it can only assert what one packaging run knows
about itself (version/commit/target/maturity/binaries/nonclaims). This script
is the aggregate, cross-archive document assembled once all target archives
exist: it never replaces or restates the per-archive manifest, only reads its
sibling archives' actual bytes.

This is generated evidence, not a publication decision: building or checking
a manifest here creates or authenticates no GitHub Release, calls no network
API, and grants no authority. The real `publish-release` CI job (off-limits
to this change; see docs/RELEASE-PROCESS.md) is the only place a Release is
actually created, and only after `release-gate` -- the exact-tag blocking
aggregate over the required-check inventory this manifest records -- has
already succeeded in the same run.

Reuse, not restatement: the supported archive targets and version/commit
patterns are imported from `release-reconcile.py`, and the exact changelog
section extraction is imported from `release-notes.py`, rather than
re-implementing either.

Two modes:

  --archives-dir DIR --commit C [--version V] [--tag T] [--output PATH]
      Recompute the manifest from a checkout's CHANGELOG.md and CI workflow
      plus a directory of already-built archives (and, if present, that
      directory's SHA256SUMS), then print it (or write it to --output).

  --check PATH (with the same other flags)
      Recompute the manifest the same way, then diff it field-by-field
      against the manifest already on disk at PATH, reporting every
      disagreement instead of writing anything. Exit 0 only if every field
      agrees.
"""

import argparse
import hashlib
import importlib.util
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCHEMA = "semaprax.release-manifest.v1"


def reject(message):
    raise ValueError(message)


def _load_sibling(module_name, filename):
    """Import a hyphenated sibling script as a module, unmodified.

    Python module names cannot contain `-`, so `release-notes.py` and
    `release-reconcile.py` cannot be imported with a plain `import` statement;
    this is the same `importlib.util.spec_from_file_location` technique
    `scripts/agent-task-comparison-runner.py` already uses to reuse a
    hyphenated sibling script without copying its logic.
    """
    path = ROOT / "scripts" / filename
    spec = importlib.util.spec_from_file_location(module_name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


_notes = _load_sibling("semaprax_release_notes", "release-notes.py")
_reconcile = _load_sibling("semaprax_release_reconcile", "release-reconcile.py")

changelog_section = _notes.changelog_section
ARCHIVE_TARGETS = _reconcile.ARCHIVE_TARGETS
VERSION_RE = _reconcile.VERSION_RE
COMMIT_RE = _reconcile.COMMIT_RE


def sha256_digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def changelog_section_digest(changelog_text, version):
    """`sha256:`-prefixed digest of the exact dated CHANGELOG.md section.

    Raises the same `ValueError` `changelog_section` does when the heading is
    missing, duplicated, or empty -- there is no silent "no digest" case.
    """
    return sha256_digest(changelog_section(changelog_text, version).encode("utf-8"))


def job_block(workflow_text, name):
    """The exact text of one top-level workflow job, `name:` through the line
    before the next top-level (2-space-indented) key.

    Ported from the identical `job()` helper in
    `tests/offline_package/ci_release_gate.rs` and
    `tests/offline_package/release_workflow.rs` -- both walk the same
    `.github/workflows/ci.yml` text the same way, so a future workflow
    restructuring that breaks one breaks the other too, rather than only one
    silently reading a stale job boundary.
    """
    marker = f"  {name}:\n"
    index = workflow_text.find(marker)
    if index == -1:
        reject(f"missing `{name}` job in the CI workflow")
    tail = workflow_text[index + len(marker) :]
    lines = tail.splitlines(keepends=True)
    position = 0
    for line_index, line in enumerate(lines):
        if line_index > 0:
            content = line.rstrip("\n")
            if (
                content.startswith("  ")
                and not content.startswith("    ")
                and content.endswith(":")
            ):
                return tail[:position]
        position += len(line)
    return tail


def parse_required_checks(workflow_text):
    """The `release-gate` job's `needs:` list, in declaration order.

    This is the required-check inventory: exactly the release blockers that
    must all report `success` before `release-gate` -- and therefore
    `release-artifacts`/`publish-release` -- can proceed. Read from the CI
    workflow rather than hard-coded, so a blocker added to or removed from
    `release-gate` changes this manifest's next build instead of silently
    drifting from what the gate actually requires.
    """
    block = job_block(workflow_text, "release-gate")
    marker = "    needs:\n"
    start = block.find(marker)
    if start == -1:
        reject("release-gate job has no `needs:` list")
    tail = block[start + len(marker) :]
    end = tail.find("    runs-on:")
    if end == -1:
        reject("release-gate job's `needs:` list has no following `runs-on:`")
    needs_block = tail[:end]
    checks = []
    for line in needs_block.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        if not stripped.startswith("- "):
            reject(f"unexpected line in release-gate's needs: list: {line!r}")
        checks.append(stripped[2:])
    if not checks:
        reject("release-gate job's needs: list is empty")
    return checks


def collect_artifacts(archives_dir, tag):
    """Every admitted target's archive: name, platform, byte size, digest.

    Reads each archive's actual bytes rather than trusting a caller-supplied
    size or digest -- the manifest's artifact inventory is derived evidence,
    not a restated claim. Fails closed (raises) if any admitted target's
    archive is absent; a manifest cannot honestly describe artifacts that do
    not exist on disk.
    """
    archives_dir = Path(archives_dir)
    artifacts = []
    missing = []
    for target, extension in ARCHIVE_TARGETS:
        name = f"semaprax-{tag}-{target}.{extension}"
        path = archives_dir / name
        if not path.is_file():
            missing.append(name)
            continue
        data = path.read_bytes()
        artifacts.append(
            {
                "name": name,
                "platform": target,
                "size": len(data),
                "digest": sha256_digest(data),
            }
        )
    if missing:
        reject(f"missing release artifact(s): {', '.join(missing)}")
    artifacts.sort(key=lambda entry: entry["platform"])
    return artifacts


def verify_against_sums(archives_dir, artifacts):
    """Cross-check computed digests against a sibling `SHA256SUMS`, if present.

    `publish-release` generates `SHA256SUMS` from the same archive bytes this
    function just hashed; agreement here is not new evidence by itself, but a
    disagreement means one of the two computations read different bytes --
    exactly the class of bug this check exists to catch before either is
    trusted.
    """
    sums_path = Path(archives_dir) / "SHA256SUMS"
    if not sums_path.is_file():
        return
    recorded = {}
    for line in sums_path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        digest, _, name = line.partition("  ")
        recorded[name] = f"sha256:{digest}"
    for artifact in artifacts:
        expected = recorded.get(artifact["name"])
        if expected is not None and expected != artifact["digest"]:
            reject(
                f"digest mismatch for {artifact['name']}: SHA256SUMS says "
                f"{expected}, computed {artifact['digest']}"
            )


def build_manifest(version, tag, commit, prerelease, workflow_text, changelog_text, archives_dir):
    """Assemble the complete `semaprax.release-manifest.v1` document."""
    if not VERSION_RE.fullmatch(version):
        reject(f"not a canonical version: {version!r}")
    if tag != f"v{version}":
        reject(f"tag {tag!r} does not equal v plus the version {version!r}")
    if not COMMIT_RE.match(commit):
        reject(f"commit must be exactly 40 lowercase hexadecimal characters: {commit!r}")
    required_checks = parse_required_checks(workflow_text)
    digest = changelog_section_digest(changelog_text, version)
    artifacts = collect_artifacts(archives_dir, tag)
    verify_against_sums(archives_dir, artifacts)
    return {
        "schema": SCHEMA,
        "version": version,
        "tag": tag,
        "commit": commit,
        "prerelease": bool(prerelease),
        "required_checks": required_checks,
        "changelog_section_digest": digest,
        "artifacts": artifacts,
    }


def diff_manifest(existing, expected):
    """Every field disagreement between an on-disk manifest and a freshly
    recomputed one, empty when they fully agree.

    Distinguishes the same disagreement classes `--check` callers need to
    report precisely: a scalar field, the required-check inventory as a
    whole, a per-artifact field, a missing artifact entry, and an artifact
    entry with no matching archive on disk.
    """
    problems = []
    for key in (
        "schema",
        "version",
        "tag",
        "commit",
        "prerelease",
        "changelog_section_digest",
    ):
        if existing.get(key) != expected.get(key):
            problems.append(
                f"{key} mismatch: manifest has {existing.get(key)!r}, "
                f"recomputed {expected.get(key)!r}"
            )
    if existing.get("required_checks") != expected.get("required_checks"):
        problems.append(
            "required_checks disagrees with the current release-gate needs: list: "
            f"manifest has {existing.get('required_checks')!r}, recomputed "
            f"{expected.get('required_checks')!r}"
        )
    existing_artifacts = {
        entry["name"]: entry
        for entry in existing.get("artifacts", [])
        if isinstance(entry, dict) and "name" in entry
    }
    expected_artifacts = {entry["name"]: entry for entry in expected.get("artifacts", [])}
    for name, expected_entry in expected_artifacts.items():
        actual_entry = existing_artifacts.get(name)
        if actual_entry is None:
            problems.append(f"manifest is missing an artifact entry for {name}")
            continue
        for field in ("platform", "size", "digest"):
            if actual_entry.get(field) != expected_entry.get(field):
                problems.append(
                    f"artifact {name} {field} mismatch: manifest has "
                    f"{actual_entry.get(field)!r}, recomputed {expected_entry.get(field)!r}"
                )
    for name in existing_artifacts:
        if name not in expected_artifacts:
            problems.append(
                f"manifest has an artifact entry with no matching archive on disk: {name}"
            )
    return problems


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", help="defaults to the root Cargo.toml version")
    parser.add_argument("--tag", help="defaults to v<version>")
    parser.add_argument("--commit", required=True, help="exact 40-hex release commit")
    parser.add_argument("--changelog", type=Path, default=ROOT / "CHANGELOG.md")
    parser.add_argument(
        "--workflow", type=Path, default=ROOT / ".github" / "workflows" / "ci.yml"
    )
    parser.add_argument(
        "--archives-dir",
        type=Path,
        required=True,
        help="directory already containing every admitted target's built archive",
    )
    parser.add_argument("--output", type=Path, help="write the manifest here instead of stdout")
    parser.add_argument(
        "--check",
        type=Path,
        help="diff an existing manifest at this path against the recomputed one instead of writing",
    )
    prerelease_group = parser.add_mutually_exclusive_group()
    prerelease_group.add_argument(
        "--prerelease",
        dest="prerelease",
        action="store_true",
        help="SEMAPRAX is pre-alpha; every tag today is a GitHub prerelease (default)",
    )
    prerelease_group.add_argument("--no-prerelease", dest="prerelease", action="store_false")
    parser.set_defaults(prerelease=True)
    args = parser.parse_args(argv)

    cargo_text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    cargo_match = re.search(r'^version = "([^"]+)"$', cargo_text, re.MULTILINE)
    version = args.version or (cargo_match.group(1) if cargo_match else None)
    if version is None:
        print(
            "release manifest: --version not given and root Cargo.toml has no version",
            file=sys.stderr,
        )
        return 2
    tag = args.tag or f"v{version}"

    workflow_text = args.workflow.read_text(encoding="utf-8")
    changelog_text = args.changelog.read_text(encoding="utf-8")

    manifest = build_manifest(
        version, tag, args.commit, args.prerelease, workflow_text, changelog_text, args.archives_dir
    )

    if args.check is not None:
        existing = json.loads(args.check.read_text(encoding="utf-8"))
        problems = diff_manifest(existing, manifest)
        for problem in problems:
            print(f"release manifest: {problem}", file=sys.stderr)
        if problems:
            return 1
        print(f"release manifest: {args.check} agrees with recomputed evidence for {tag}")
        return 0

    rendered = json.dumps(manifest, indent=2) + "\n"
    if args.output is not None:
        args.output.write_text(rendered, encoding="utf-8")
        print(f"release manifest: wrote {args.output}")
    else:
        sys.stdout.write(rendered)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print(f"release manifest rejected: {error}", file=sys.stderr)
        sys.exit(2)
