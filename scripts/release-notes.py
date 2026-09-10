#!/usr/bin/env python3
"""Render one GitHub release body from the matching CHANGELOG section."""

import argparse
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
VERSION_RE = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)")


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


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--changelog", type=Path, default=ROOT / "CHANGELOG.md")
    args = parser.parse_args(argv)
    if not VERSION_RE.fullmatch(args.version):
        raise ValueError("--version must be canonical major.minor.patch")
    section = changelog_section(
        args.changelog.read_text(encoding="utf-8"), args.version
    )
    # Single-release hotfix: 0.4.0 section is 126k (>125k GitHub limit); truncate
    # for this tag only so `gh release create` stops 422ing. Keep the full
    # CHANGELOG.md intact; this only affects the release notes file.
    if args.version == "0.4.0" and len(section) > 118_000:
        section = section[:118_000].rsplit("\n", 1)[0] + "\n\n… (truncated for GitHub 125k limit; see CHANGELOG.md for full 0.4.0 notes)"
    print(f"SEMAPRAX v{args.version} is pre-alpha research software.\n")
    print("## Changes\n")
    print(section)
    print(
        "\nThese unsigned archives are not notarized and make no cross-host "
        "reproducible-build claim.\n"
        "SHA-256 checksums are integrity facts, not signatures."
    )


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError) as error:
        print(f"release notes rejected: {error}", file=sys.stderr)
        sys.exit(2)
