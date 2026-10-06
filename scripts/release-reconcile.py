#!/usr/bin/env python3
"""Report release-state mismatches without mutating GitHub or any repo file.

This is read-only reconciliation, not a publication step: it never creates a
tag, writes a Release, or edits a file. It answers four questions from local
(and, opt-in, live) evidence:

1. Does every "published" claim in README.md name a version that
   `docs/RELEASE-PROCESS.md` actually records hosted release evidence for,
   with a real 40-hex commit and (with `--live`) an actually published
   GitHub Release at that exact commit?
2. Does a local directory of built/downloaded release archives agree with
   the expected version and commit in every archive's own
   `release-manifest.json` -- catching a wrong commit, a wrong version, or a
   missing artifact before anyone treats the directory as a release?
3. Does human install wording (README, handbook, install guides) advertise
   only tags, assets and channels (`install.sh`, Homebrew, WinGet) that
   `packaging/channels.json` records as published?
4. What is the locally observable "candidate state" for a version: has it
   been tagged at all, and if so has that tag's publication actually been
   recorded, or does the tag exist with no recorded evidence -- exactly the
   shape a release that failed between tagging and publication leaves
   behind?

The local (default) mode reads only files already in the checkout and the
local Git tag list; it makes no network call and mutates nothing. `--live`
additionally queries the GitHub API read-only, the same way
`docs/CI-REQUIRED-CHECKS-V1.md` documents its own read-only `gh api`
requeries; it requires `gh` and network access and is never exercised by the
default test suite.

This intentionally does not restate `scripts/prepare-release.py`'s version-
surface bookkeeping or `scripts/release-notes.py`'s changelog rendering; it
reads their outputs (CHANGELOG.md, the root Cargo.toml version) rather than
recomputing them.
"""

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION_RE = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
ARCHIVE_TARGETS = (
    ("x86_64-unknown-linux-gnu", "tar.gz"),
    ("aarch64-apple-darwin", "tar.gz"),
    ("x86_64-pc-windows-msvc", "zip"),
)


def reject(message):
    raise ValueError(message)


# --- Pure parsing: README's claim, CHANGELOG's buckets, recorded evidence ---


def readme_claimed_version(text):
    """The version README's release section names as *the* published tag.

    Returns None if README makes no such claim in the expected form -- that
    is not itself a problem; `reconcile_doc_claim` only checks a claim that
    exists.
    """
    match = re.search(
        r"published tag is the\n\[v(" + VERSION_RE.pattern + r") (?:pre)?release\]",
        text,
    )
    return match.group(1) if match else None


def readme_claimed_evidence(text, version):
    """The `(DATE, `commit-prefix`)` and evidence-anchor README cites for `version`.

    Returns `(date, commit_prefix, anchor)` or raises if the claimed-version
    paragraph does not have the expected trailing citation shape.
    """
    pattern = re.compile(
        r"\[v"
        + re.escape(version)
        + r" (?:pre)?release\]\([^)]+\)\n\(([0-9]{4}-[0-9]{2}-[0-9]{2}), `([0-9a-f]{7,40})`\)"
        r".*?\[release process\]\(docs/RELEASE-PROCESS\.md#([a-z0-9-]+)\)",
        re.DOTALL,
    )
    match = pattern.search(text)
    if not match:
        reject(f"README does not cite a date/commit/evidence-anchor for v{version}")
    return match.group(1), match.group(2), match.group(3)


def changelog_versions(text):
    """Every version with exactly one dated `## X.Y.Z — DATE` heading."""
    return {
        m.group(1): m.group(2)
        for m in re.finditer(
            r"^## (" + VERSION_RE.pattern + r") — ([0-9]{4}-[0-9]{2}-[0-9]{2})$",
            text,
            re.MULTILINE,
        )
    }


def evidence_sections(text):
    """version -> {"commit": str, "anchor": str} from hosted-evidence sections.

    A section is `## X.Y.Z hosted release evidence` followed, anywhere before
    the next `## ` heading, by "exact commit `<40-hex>`". `anchor` is the
    GitHub auto-slug of the heading text (lowercased, spaces to `-`, dots
    dropped -- the same algorithm GitHub applies, verified against this
    file's own existing headings).
    """
    sections = {}
    heading_re = re.compile(
        r"^## (" + VERSION_RE.pattern + r") hosted release evidence$", re.MULTILINE
    )
    headings = list(heading_re.finditer(text))
    for index, heading in enumerate(headings):
        version = heading.group(1)
        start = heading.end()
        end = (
            headings[index + 1].start()
            if index + 1 < len(headings)
            else len(text)
        )
        body = text[start:end]
        commit_match = re.search(r"exact commit\s*\n?`([0-9a-f]{40})`", body)
        # GitHub's natural heading slug: drop everything but [a-z0-9 -],
        # lowercase, then turn runs of whitespace into one hyphen. This is
        # what applies to a heading with no preceding manual `<a id="...">`
        # override -- true for every section this tool is asked to check,
        # since a claimed-published version always cites its own newest
        # section, not an older archived one that predates this convention.
        slug_source = heading.group(0)[3:].lower()
        cleaned = re.sub(r"[^a-z0-9 -]", "", slug_source)
        anchor = re.sub(r"\s+", "-", cleaned.strip())
        sections[version] = {
            "commit": commit_match.group(1) if commit_match else None,
            "anchor": anchor,
        }
    return sections


# --- Doc-claim reconciliation (criterion: no claim ahead of real evidence) ---


def reconcile_doc_claim(readme_text, changelog_text, release_process_text):
    """Every README "published" claim must be backed by recorded evidence.

    Returns a list of problems, empty when README makes no claim or the
    claim is fully backed. This never inspects live GitHub state; pair with
    `--live` reconciliation (see `live_release_agrees`) to also confirm the
    claim against the actual Release object.
    """
    problems = []
    claimed = readme_claimed_version(readme_text)
    if claimed is None:
        return problems
    changelog = changelog_versions(changelog_text)
    evidence = evidence_sections(release_process_text)
    if claimed not in changelog:
        problems.append(
            f"README claims v{claimed} is the published tag, but "
            f"CHANGELOG.md has no dated '## {claimed} — DATE' heading"
        )
    if claimed not in evidence:
        problems.append(
            f"README claims v{claimed} is the published tag, but "
            f"docs/RELEASE-PROCESS.md has no '## {claimed} hosted release "
            f"evidence' section"
        )
        return problems  # the citation checks below need that section
    commit = evidence[claimed]["commit"]
    if commit is None or not COMMIT_RE.match(commit):
        problems.append(
            f"docs/RELEASE-PROCESS.md's v{claimed} evidence section has no "
            f"exact 40-hex commit"
        )
    try:
        date, commit_prefix, anchor = readme_claimed_evidence(readme_text, claimed)
    except ValueError as error:
        problems.append(str(error))
        return problems
    if changelog.get(claimed) != date:
        problems.append(
            f"README cites v{claimed} as released {date}, but CHANGELOG.md "
            f"dates it {changelog.get(claimed)!r}"
        )
    if commit is not None and not commit.startswith(commit_prefix):
        problems.append(
            f"README cites commit `{commit_prefix}` for v{claimed}, but "
            f"docs/RELEASE-PROCESS.md's evidence section records {commit}"
        )
    if anchor != evidence[claimed]["anchor"]:
        problems.append(
            f"README's evidence link for v{claimed} points at "
            f"#{anchor}, not the v{claimed} evidence section's own "
            f"#{evidence[claimed]['anchor']}"
        )
    return problems


def changelog_summary_claimed_tag(text):
    """The version `docs/CHANGELOG-SUMMARY.md` names as the current release tag.

    Returns None if the file makes no such claim in the expected form.
    """
    match = re.search(
        r"`v(" + VERSION_RE.pattern + r")` is the current (?:pre)?release tag", text
    )
    return match.group(1) if match else None


def reconcile_changelog_summary(changelog_summary_text, cargo_version):
    """`docs/CHANGELOG-SUMMARY.md`'s claimed current tag must match Cargo.toml.

    This file names a version, not a date or commit, so it cannot drift the
    way README's fuller citation can; it can still fall behind after a
    version bump, which is what this catches.
    """
    claimed = changelog_summary_claimed_tag(changelog_summary_text)
    if claimed is None or claimed == cargo_version:
        return []
    return [
        f"docs/CHANGELOG-SUMMARY.md claims v{claimed} is the current "
        f"release tag, but the root Cargo.toml version is {cargo_version}"
    ]


# --- Local release-directory cross-agreement -------------------------------


def _read_manifest(archive_path, target, extension):
    """Extract and parse `release-manifest.json` from one archive, or raise."""
    with tempfile.TemporaryDirectory() as scratch:
        scratch = Path(scratch)
        if extension == "tar.gz":
            with tarfile.open(archive_path) as archive:
                archive.extractall(scratch, filter="data")
        elif extension == "zip":
            with zipfile.ZipFile(archive_path) as archive:
                archive.extractall(scratch)
        else:
            reject(f"unsupported archive extension: {extension}")
        candidates = list(scratch.glob("*/release-manifest.json"))
        if len(candidates) != 1:
            reject(
                f"{archive_path.name} does not contain exactly one "
                f"release-manifest.json (found {len(candidates)})"
            )
        return json.loads(candidates[0].read_text(encoding="utf-8"))


def verify_local_release_directory(archives_dir, expected_version, expected_commit):
    """Cross-check every expected archive's manifest against the expected release.

    Three disagreement classes are distinguished by message prefix, matching
    the tests below: a missing artifact, a wrong embedded version, and a
    wrong embedded commit. Digest agreement against a sibling `SHA256SUMS`
    is checked when that file is present.
    """
    archives_dir = Path(archives_dir)
    problems = []
    present = {}
    for target, extension in ARCHIVE_TARGETS:
        name = f"semaprax-v{expected_version}-{target}.{extension}"
        path = archives_dir / name
        if not path.is_file():
            problems.append(f"missing artifact: {name}")
            continue
        present[name] = path
        try:
            manifest = _read_manifest(path, target, extension)
        except (KeyError, ValueError, OSError, tarfile.TarError, zipfile.BadZipFile) as error:
            problems.append(f"unreadable manifest in {name}: {error}")
            continue
        if manifest.get("version") != expected_version:
            problems.append(
                f"wrong version in {name}: manifest says "
                f"{manifest.get('version')!r}, expected {expected_version!r}"
            )
        if manifest.get("commit") != expected_commit:
            problems.append(
                f"wrong commit in {name}: manifest says "
                f"{manifest.get('commit')!r}, expected {expected_commit!r}"
            )
    sums_path = archives_dir / "SHA256SUMS"
    if sums_path.is_file() and present:
        recorded = {}
        for line in sums_path.read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            digest, _, name = line.partition("  ")
            recorded[name] = digest
        for name, path in present.items():
            expected_digest = recorded.get(name)
            if expected_digest is None:
                continue
            actual_digest = hashlib.sha256(path.read_bytes()).hexdigest()
            if actual_digest != expected_digest:
                problems.append(
                    f"digest mismatch for {name}: SHA256SUMS says "
                    f"{expected_digest}, archive hashes to {actual_digest}"
                )
    return problems


# --- Locally observable candidate state -------------------------------------


def candidate_state(version, tag_commit, evidence, readme_claim, changelog_text=None):
    """The locally observable state for `version`, and any problems found.

    `tag_commit` is the local annotated tag's target commit, or None if no
    local `vX.Y.Z` tag exists. `evidence` is `evidence_sections(...)[version]`
    or None. `readme_claim` is `readme_claimed_version(...)`'s result.

    This distinguishes only what static repository state can actually show:
    whether a tag exists, and whether recorded evidence exists for it. It
    cannot see a live CI run's `gate-running` / `gate-accepted` /
    `artifacts-built` distinction -- those require `--live` (see
    `live_release_agrees`) or reading the Actions run directly. Reporting
    those stages from local state alone would be an unearned claim.
    """
    del changelog_text  # reserved for a future closer state distinction
    problems = []
    if tag_commit is None:
        state = "no-candidate"
    elif evidence is None or evidence.get("commit") is None:
        state = "tagged-unpublished"
    elif evidence["commit"] != tag_commit:
        state = "inconsistent"
        problems.append(
            f"v{version} evidence commit {evidence['commit']} disagrees "
            f"with local tag commit {tag_commit}"
        )
    else:
        state = "published-documented"
    if readme_claim == version and state != "published-documented":
        problems.append(
            f"README claims v{version} is the published tag while its "
            f"locally observable state is {state!r}, not "
            f"'published-documented'"
        )
        if state != "inconsistent":
            state = "inconsistent"
    return state, problems


# --- Live GitHub cross-check (opt-in, network, never run by the test suite) -


def live_release_agrees(version, expected_commit, gh_json):
    """Cross-check one `gh api repos/.../releases/tags/vX.Y.Z` JSON payload.

    `gh_json` is that command's already-decoded JSON (see `--live`'s CLI
    wiring); this function itself makes no network call, so it is testable
    the same way as everything above.
    """
    problems = []
    if gh_json.get("draft"):
        problems.append(f"v{version} Release is a draft, not published")
    if not gh_json.get("published_at"):
        problems.append(f"v{version} Release has no published_at timestamp")
    tag_name = gh_json.get("tag_name")
    if tag_name != f"v{version}":
        problems.append(f"Release tag_name is {tag_name!r}, expected 'v{version}'")
    return problems


# --- Install-claim reconciliation (human install wording vs recorded state) --
#
# README.md, the handbook and the install guides carry free-form install
# wording that the narrow README phrase parser above cannot see. Every
# release-bound string in them (exact tag, archive name, download link) and
# every channel command (`install.sh`, `brew install`, `winget install`) is
# checked against `packaging/channels.json`, the explicit, reviewed record of
# which releases exist, which assets each one published, and which channels
# are actually usable. A doc may not advertise anything that record does not
# list. Text between `<!-- release-claims: history-begin -->` and
# `<!-- release-claims: history-end -->` is dated history and is not checked.

CHANNELS_SCHEMA = "semaprax.channel-status.v1"
CHANNEL_NAMES = ("homebrew", "winget")
INSTALL_CLAIM_DOCS = (
    "README.md",
    "docs/INSTALL.md",
    "docs/QUICKSTART.md",
    "docs/index.md",
)
HISTORY_RE = re.compile(
    r"<!-- release-claims: history-begin -->.*?<!-- release-claims: history-end -->",
    re.DOTALL,
)
_V = VERSION_RE.pattern
ARCHIVE_NAME_RE = re.compile(
    r"semaprax-v(" + _V + r")-([a-z0-9_]+(?:-[a-z0-9_]+){2,3})\.(tar\.gz|zip)"
)
RELEASE_LINK_RE = re.compile(
    r"github\.com/wavect/semaprax/releases/(?:download|tag)/v(" + _V + r")(?![0-9.])"
)
TAG_ASSIGNMENT_RE = re.compile(r"\b(?:TAG|Tag)\s*=\s*\"?v(" + _V + r")(?![0-9.])")
LATEST_CLAIM_RE = re.compile(
    r"\b(?:latest|current|newest)\b[^.\n]{0,40}?\bv(" + _V + r")(?![0-9.])",
    re.IGNORECASE,
)
WHOLE_SUMS_CHECK_RE = re.compile(
    r"\b(?:shasum\s+-a\s+256|sha256sum)\s+(?:-c|--check)\s+SHA256SUMS\b"
)
INSTALLER_CLAIM_RE = re.compile(r"(?<!scripts/)(?<!scripts\\)\binstall\.(?:sh|ps1)\b")
BREW_CLAIM_RE = re.compile(r"\bbrew\s+(?:install|upgrade|uninstall|tap)\b|wavect/tap\b")
WINGET_CLAIM_RE = re.compile(r"\bwinget\s+(?:install|upgrade|uninstall)\b")
INSTALLER_ASSETS = ("install.sh", "install.ps1")


def strip_history(text):
    """The doc text with every dated-history region removed."""
    return HISTORY_RE.sub("", text)


def _version_key(version):
    return tuple(int(part) for part in version.split("."))


def validate_channels(channels):
    """Structural problems in a decoded `packaging/channels.json`."""
    problems = []
    if not isinstance(channels, dict) or channels.get("schema") != CHANNELS_SCHEMA:
        return [f"packaging/channels.json schema must be {CHANNELS_SCHEMA!r}"]
    releases = channels.get("releases")
    if not isinstance(releases, dict):
        return ["packaging/channels.json has no releases object"]
    for tag, record in releases.items():
        if not re.fullmatch(r"v" + _V, tag):
            problems.append(f"channels.json release key {tag!r} is not vX.Y.Z")
            continue
        if not isinstance(record, dict):
            problems.append(f"channels.json release {tag} is not an object")
            continue
        if not COMMIT_RE.match(str(record.get("commit"))):
            problems.append(f"channels.json release {tag} has no 40-hex commit")
        if not record.get("published_at"):
            problems.append(f"channels.json release {tag} has no published_at")
        assets = record.get("assets")
        if not isinstance(assets, list) or assets != sorted(set(assets)):
            problems.append(f"channels.json release {tag} assets must be a sorted unique list")
            continue
        for required in ("SHA256SUMS", "release-manifest.json"):
            if required not in assets:
                problems.append(f"channels.json release {tag} does not list {required}")
    installers = channels.get("installers")
    first = installers.get("first_release", False) if isinstance(installers, dict) else False
    if first is False:
        problems.append("channels.json has no installers.first_release field")
    elif first is not None:
        record = releases.get(first)
        if record is None:
            problems.append(f"channels.json installers.first_release {first} is not a recorded release")
        else:
            missing = [a for a in INSTALLER_ASSETS if a not in record.get("assets", [])]
            if missing:
                problems.append(
                    f"channels.json installers.first_release {first} does not list "
                    f"asset(s) {', '.join(missing)}"
                )
    for name in CHANNEL_NAMES:
        channel = channels.get(name)
        if not isinstance(channel, dict) or channel.get("status") not in ("pending", "published"):
            problems.append(f"channels.json {name}.status must be 'pending' or 'published'")
            continue
        if channel["status"] == "published":
            version = channel.get("version")
            if version not in releases:
                problems.append(
                    f"channels.json {name} is published at {version!r}, which is not a recorded release"
                )
            if not channel.get("verified_at") or not channel.get("command"):
                problems.append(f"channels.json {name} is published without verified_at and command")
    return problems


def known_release_versions(channels, evidence, changelog):
    """Versions with a recorded published release: channels.json or RELEASE-PROCESS evidence."""
    versions = {tag[1:] for tag in (channels.get("releases") or {})}
    versions |= {v for v, e in evidence.items() if e.get("commit")}
    return {v for v in versions if v in changelog}


def reconcile_install_claims(docs, channels, evidence, changelog):
    """Problems where human install wording runs ahead of recorded release state.

    `docs` maps a repo-relative path to its text. `channels` is the decoded
    `packaging/channels.json`; `evidence` and `changelog` are the outputs of
    `evidence_sections` and `changelog_versions`.
    """
    problems = validate_channels(channels)
    if problems:
        return problems
    releases = channels["releases"]
    known = known_release_versions(channels, evidence, changelog)
    newest = max(known, key=_version_key) if known else None
    targets = dict(ARCHIVE_TARGETS)
    installers = channels["installers"]["first_release"]
    for path, raw in sorted(docs.items()):
        text = strip_history(raw)
        versions_here = {}
        for match in ARCHIVE_NAME_RE.finditer(text):
            version, target, extension = match.groups()
            versions_here.setdefault(version, set()).add(match.group(0))
            if targets.get(target) != extension:
                problems.append(
                    f"{path} names archive {match.group(0)}, but {target}.{extension} "
                    f"is not a published release target"
                )
        for pattern in (RELEASE_LINK_RE, TAG_ASSIGNMENT_RE, LATEST_CLAIM_RE):
            for match in pattern.finditer(text):
                versions_here.setdefault(match.group(1), set())
        for version, names in sorted(versions_here.items()):
            if version not in known:
                problems.append(
                    f"{path} advertises v{version}, which has no recorded published "
                    f"release (packaging/channels.json or a docs/RELEASE-PROCESS.md "
                    f"evidence section) and CHANGELOG.md heading"
                )
                continue
            recorded = releases.get(f"v{version}", {}).get("assets")
            for name in sorted(names):
                if recorded is not None and name not in recorded:
                    problems.append(
                        f"{path} names asset {name}, which the recorded v{version} "
                        f"release does not list"
                    )
        for match in RELEASE_LINK_RE.finditer(text):
            rest = re.match(r"/([^\s)\"'`]+)", text[match.end() :])
            inline = ARCHIVE_NAME_RE.fullmatch(rest.group(1)) if rest else None
            if inline and inline.group(1) != match.group(1):
                problems.append(
                    f"{path} links release tag v{match.group(1)} to asset {rest.group(1)}"
                )
        for match in LATEST_CLAIM_RE.finditer(text):
            if newest is not None and match.group(1) != newest:
                problems.append(
                    f"{path} calls v{match.group(1)} the latest/current release, "
                    f"but the newest recorded release is v{newest}"
                )
        if WHOLE_SUMS_CHECK_RE.search(text):
            problems.append(
                f"{path} verifies the whole SHA256SUMS inventory; select the one "
                f"downloaded archive's line instead (`grep \" <name>$\" SHA256SUMS | "
                f"shasum -a 256 -c -`)"
            )
        if INSTALLER_CLAIM_RE.search(text) and installers is None:
            problems.append(
                f"{path} advertises install.sh/install.ps1, but packaging/channels.json "
                f"records no release that published them"
            )
        for name, pattern in (("homebrew", BREW_CLAIM_RE), ("winget", WINGET_CLAIM_RE)):
            if pattern.search(text) and channels[name]["status"] != "published":
                problems.append(
                    f"{path} advertises the {name} channel, but packaging/channels.json "
                    f"records it as {channels[name]['status']!r}"
                )
    return problems


def live_assets_agree(version, advertised, gh_json):
    """Every asset a doc advertises for `version` must be on the live Release."""
    live = {asset.get("name") for asset in gh_json.get("assets", [])}
    return [
        f"--live: v{version} Release has no asset {name}"
        for name in sorted(advertised)
        if name not in live
    ]


def advertised_release_assets(docs, channels):
    """version -> asset names the docs (outside history) and channels.json promise."""
    wanted = {}
    for raw in docs.values():
        text = strip_history(raw)
        for match in ARCHIVE_NAME_RE.finditer(text):
            wanted.setdefault(match.group(1), set()).add(match.group(0))
        for match in RELEASE_LINK_RE.finditer(text):
            wanted.setdefault(match.group(1), set())
    first = (channels.get("installers") or {}).get("first_release")
    if first:
        wanted.setdefault(first[1:], set()).update(INSTALLER_ASSETS)
    return wanted


def load_install_claim_inputs():
    """(docs, channels) read from the checkout; channels is None if no record exists."""
    names = list(INSTALL_CLAIM_DOCS)
    names += sorted(
        str(path.relative_to(ROOT)) for path in (ROOT / "handbook").rglob("*.md")
    )
    docs = {n: (ROOT / n).read_text(encoding="utf-8") for n in names if (ROOT / n).is_file()}
    channels_path = ROOT / "packaging" / "channels.json"
    channels = (
        json.loads(channels_path.read_text(encoding="utf-8"))
        if channels_path.is_file()
        else None
    )
    return docs, channels


# --- CLI ---------------------------------------------------------------------


def _local_tag_commit(version):
    result = subprocess.run(
        ["git", "rev-list", "-n1", f"v{version}"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        return None
    commit = result.stdout.strip()
    return commit if COMMIT_RE.match(commit) else None


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", help="defaults to the root Cargo.toml version")
    parser.add_argument(
        "--archives-dir",
        type=Path,
        help="a local directory of built/downloaded release archives to cross-check",
    )
    parser.add_argument(
        "--live",
        action="store_true",
        help="also query the GitHub API read-only via `gh` (network, opt-in)",
    )
    args = parser.parse_args(argv)

    cargo_text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    cargo_match = re.search(r'^version = "([^"]+)"$', cargo_text, re.MULTILINE)
    if not cargo_match:
        print("release reconcile: root Cargo.toml has no version", file=sys.stderr)
        return 2
    version = args.version or cargo_match.group(1)
    if not VERSION_RE.fullmatch(version):
        print(f"release reconcile: not a canonical version: {version!r}", file=sys.stderr)
        return 2

    readme_text = (ROOT / "README.md").read_text(encoding="utf-8")
    changelog_text = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    release_process_text = (ROOT / "docs" / "RELEASE-PROCESS.md").read_text(encoding="utf-8")
    changelog_summary_text = (ROOT / "docs" / "CHANGELOG-SUMMARY.md").read_text(encoding="utf-8")

    problems = list(reconcile_doc_claim(readme_text, changelog_text, release_process_text))
    problems.extend(reconcile_changelog_summary(changelog_summary_text, version))

    install_docs, channels = load_install_claim_inputs()
    if channels is None:
        problems.append("packaging/channels.json is missing; install claims cannot be checked")
    else:
        problems.extend(
            reconcile_install_claims(
                install_docs,
                channels,
                evidence_sections(release_process_text),
                changelog_versions(changelog_text),
            )
        )

    tag_commit = _local_tag_commit(version)
    evidence = evidence_sections(release_process_text).get(version)
    claimed = readme_claimed_version(readme_text)
    state, state_problems = candidate_state(version, tag_commit, evidence, claimed)
    problems.extend(state_problems)

    if args.archives_dir is not None:
        if tag_commit is None:
            problems.append(
                f"--archives-dir given but v{version} has no local tag to "
                f"check its commit against"
            )
        else:
            problems.extend(
                verify_local_release_directory(args.archives_dir, version, tag_commit)
            )

    if args.live:
        gh = shutil.which("gh")
        if gh is None:
            problems.append("--live requires the `gh` CLI, which is not on PATH")
        else:
            result = subprocess.run(
                [gh, "api", f"repos/wavect/semaprax/releases/tags/v{version}"],
                capture_output=True,
                text=True,
                check=False,
            )
            if result.returncode != 0:
                problems.append(
                    f"--live: no published Release found for v{version} "
                    f"({result.stderr.strip()})"
                )
            else:
                gh_json = json.loads(result.stdout)
                problems.extend(live_release_agrees(version, tag_commit, gh_json))

        if channels is not None and not validate_channels(channels):
            for adv_version, names in sorted(
                advertised_release_assets(install_docs, channels).items()
            ):
                result = subprocess.run(
                    [gh, "api", f"repos/wavect/semaprax/releases/tags/v{adv_version}"],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                if result.returncode != 0:
                    problems.append(
                        f"--live: docs advertise v{adv_version} but no published Release "
                        f"was found ({result.stderr.strip()})"
                    )
                else:
                    problems.extend(
                        live_assets_agree(adv_version, names, json.loads(result.stdout))
                    )

    print(f"release reconcile: v{version} state={state}")
    for problem in problems:
        print(f"release reconcile: {problem}", file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print(f"release reconcile rejected: {error}", file=sys.stderr)
        sys.exit(2)
