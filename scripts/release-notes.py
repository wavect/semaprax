#!/usr/bin/env python3
"""Render one GitHub release body from the matching CHANGELOG section."""

import argparse
import importlib.util
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
VERSION_RE = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)")

REPOSITORY_URL = "https://github.com/wavect/semaprax"
INSTALLER_NAMES = ("install.sh", "install.ps1")

# Human label per admitted target. The key set must equal ARCHIVE_TARGETS in
# `release-reconcile.py`; `render_install_section` fails closed on any drift
# in either direction rather than guessing a label or a filename.
TARGET_LABELS = {
    "aarch64-apple-darwin": "macOS, Apple Silicon (arm64), macOS 11+",
    "x86_64-apple-darwin": "macOS, Intel (x86_64), macOS 10.12+",
    "x86_64-unknown-linux-gnu": "Linux, x86_64, glibc 2.35+ (Ubuntu 22.04+)",
    "aarch64-unknown-linux-gnu": "Linux, arm64, glibc 2.35+ (Ubuntu 22.04+)",
    "x86_64-pc-windows-msvc": "Windows, x64, Windows 10 1809+ / Server 2019+",
}


def _archive_targets():
    path = ROOT / "scripts" / "release-reconcile.py"
    spec = importlib.util.spec_from_file_location("semaprax_release_reconcile_notes", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.ARCHIVE_TARGETS

# GitHub's release-body limit is 125,000 bytes; `gh release create` rejects a
# larger body outright with no partial publication. The fixed frame this
# script wraps the section in (title, "## Changes" heading, and the trailing
# nonclaim paragraph) costs a few hundred bytes, so the section itself is
# bounded well under the hard limit rather than exactly at it.
BOUNDED_NOTES_LIMIT = 118_000
GITHUB_BODY_LIMIT = 125_000
# Slack for multi-byte characters and the truncation notice.
RESERVE = 1_000


def bound_section(section, version, limit=BOUNDED_NOTES_LIMIT):
    """Truncate an oversized changelog section to one deterministic bounded form.

    Detects an oversized section BEFORE publication -- this runs inside
    `publish-release` before `gh release create` -- rather than letting the
    live API call fail after every other release blocker already passed.
    Applies to every version, not one hard-coded past release: any future
    section that grows past the limit is truncated the same way. The cut
    lands on a line boundary and a fixed notice names the version and points
    at the untouched `CHANGELOG.md`, which this function never writes.
    """
    if len(section) <= limit:
        return section
    return (
        section[:limit].rsplit("\n", 1)[0]
        + "\n\n… (truncated for GitHub's 125,000-byte release-note limit; "
        f"see CHANGELOG.md for the complete v{version} notes)"
    )


def changelog_section(text, version):
    heading = re.compile(
        rf"^## {re.escape(version)} — [0-9]{{4}}-[0-9]{{2}}-[0-9]{{2}}$",
        re.MULTILINE,
    )
    matches = list(heading.finditer(text))
    if len(matches) != 1:
        raise ValueError(f"expected exactly one changelog heading for {version}")
    start = matches[0].end()
    next_heading = re.search(r"^## ", text[start:], re.MULTILINE)
    end = start + next_heading.start() if next_heading else len(text)
    section = text[start:end].strip()
    if not section:
        raise ValueError(f"changelog section for {version} is empty")
    return section


def render_install_section(version, assets_dir):
    """The leading `## Install` section, derived from the asset inventory.

    Every link names a file that exists in `assets_dir` and is one of the
    admitted archive names (or one of the two installer scripts); nothing is
    guessed. A missing admitted archive raises, so a release body can never
    advertise a download that is not attached. With `assets_dir=None` only the
    asset-independent guidance is rendered.
    """
    tag = f"v{version}"
    download = f"{REPOSITORY_URL}/releases/download/{tag}"
    guide = f"{REPOSITORY_URL}/blob/{tag}/handbook/getting-started/install.md"
    lines = ["## Install", "", f"SEMAPRAX {tag} (beta).", ""]
    present_installers = set()
    if assets_dir is not None:
        assets_dir = Path(assets_dir)
        targets = _archive_targets()
        if set(TARGET_LABELS) != {target for target, _ in targets}:
            raise ValueError(
                "release-notes TARGET_LABELS disagrees with ARCHIVE_TARGETS in "
                "release-reconcile.py"
            )
        rows = []
        missing = []
        for target, extension in targets:
            name = f"semaprax-{tag}-{target}.{extension}"
            if not (assets_dir / name).is_file():
                missing.append(name)
                continue
            rows.append(f"| {TARGET_LABELS[target]} | [`{name}`]({download}/{name}) |")
        if missing:
            raise ValueError(f"missing admitted release archive(s): {', '.join(missing)}")
        present_installers = {n for n in INSTALLER_NAMES if (assets_dir / n).is_file()}
        lines += [
            "Download the archive for your computer:",
            "",
            "| Your computer | Download |",
            "| --- | --- |",
            *rows,
            "",
        ]
    if present_installers:
        lines += ["Or install with one command:", ""]
        if "install.sh" in present_installers:
            lines += [
                "macOS and Linux:",
                "",
                "```sh",
                f"curl -fsSL {download}/install.sh | sh -s -- --version {tag}",
                "```",
                "",
                "Always the latest stable release:",
                "",
                "```sh",
                f"curl -fsSL {REPOSITORY_URL}/releases/latest/download/install.sh | sh",
                "```",
                "",
            ]
        if "install.ps1" in present_installers:
            lines += [
                "Windows PowerShell:",
                "",
                "```powershell",
                f'powershell -ExecutionPolicy Bypass -c "& ([scriptblock]::Create((irm {download}/install.ps1))) -Version {tag}"',
                "```",
                "",
                "Always the latest stable release:",
                "",
                "```powershell",
                f'powershell -ExecutionPolicy Bypass -c "irm {REPOSITORY_URL}/releases/latest/download/install.ps1 | iex"',
                "```",
                "",
            ]
    lines += [
        f"Full instructions, the first-project walkthrough and uninstall steps: [Install SEMAPRAX]({guide}).",
        "",
        "Only the `semaprax-<tag>-<target>` archives are runnable programs. "
        "GitHub's \"Source code\" downloads are automatic repository snapshots, and "
        "`SHA256SUMS`, `release-*.json`, `*.bundle` and `trusted_root.jsonl` are verification metadata.",
        "",
        "To verify a download, compare it with `SHA256SUMS` and check its attestation "
        f"as described in the [release process]({REPOSITORY_URL}/blob/{tag}/docs/RELEASE-PROCESS.md).",
        "",
    ]
    return "\n".join(lines)


def render_release_body(version, section, assets_dir=None):
    """The complete release body with the changelog bounded to fit the frame."""
    install = render_install_section(version, assets_dir)

    def frame(changelog):
        return (
            f"SEMAPRAX v{version} is beta software.\n\n"
            + install
            + "\n"
            + f"<details>\n<summary>Full changelog for v{version}</summary>\n\n"
            + "## Changes\n\n"
            + changelog
            + "\n\n</details>\n"
            + "\nThese archives are not notarized and make no cross-host "
            "reproducible-build claim.\n"
            "SHA-256 checksums alone are integrity facts, not signatures; "
            "check the published signature bundle against the release policy.\n"
        )

    # The frame (install section, details wrapper, closing note) shares the
    # hard limit with the changelog, so the changelog bound shrinks by the
    # frame's size instead of assuming the frame is a few hundred bytes.
    overhead = len(frame(""))
    limit = min(BOUNDED_NOTES_LIMIT, GITHUB_BODY_LIMIT - RESERVE - overhead)
    body = frame(bound_section(section, version, limit))
    if len(body) >= GITHUB_BODY_LIMIT:
        raise ValueError("release body still exceeds the GitHub release-body limit")
    return body


def main(argv=None):
    # Windows' default stdout/stderr encoding is still a legacy charmap
    # (cp1252) when PYTHONUTF8 is not set; the release notes contain `→`
    # and `…` from CHANGELOG.md, so printing with `print()` would raise
    # `UnicodeEncodeError: 'charmap' codec can't encode` on that platform.
    # Reconfigure to UTF-8 when available so the same bytes are emitted on
    # every host; fall back to binary buffer writes for older interpreters.
    if hasattr(sys.stdout, "reconfigure"):
        try:
            sys.stdout.reconfigure(encoding="utf-8")
            sys.stderr.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--changelog", type=Path, default=ROOT / "CHANGELOG.md")
    parser.add_argument(
        "--assets-dir",
        type=Path,
        help="directory of release assets; renders the install table from it and "
        "fails if an admitted archive is missing (omit for the guidance-only form)",
    )
    args = parser.parse_args(argv)
    if not VERSION_RE.fullmatch(args.version):
        raise ValueError("--version must be canonical major.minor.patch")
    section = changelog_section(
        args.changelog.read_text(encoding="utf-8"), args.version
    )
    body = render_release_body(args.version, section, args.assets_dir)
    # Use `write` rather than `print` after reconfigure so the UTF-8
    # setting is respected even if `print` would still use the legacy
    # encoding on some Windows runners.
    sys.stdout.write(body)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError) as error:
        print(f"release notes rejected: {error}", file=sys.stderr)
        sys.exit(2)
