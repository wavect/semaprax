#!/usr/bin/env python3
"""Render one GitHub release body from the matching CHANGELOG section."""

import argparse
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
VERSION_RE = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)")

# GitHub's release-body limit is 125,000 bytes; `gh release create` rejects a
# larger body outright with no partial publication. The fixed frame this
# script wraps the section in (title, "## Changes" heading, and the trailing
# nonclaim paragraph) costs a few hundred bytes, so the section itself is
# bounded well under the hard limit rather than exactly at it.
BOUNDED_NOTES_LIMIT = 118_000


def bound_section(section, version):
    """Truncate an oversized changelog section to one deterministic bounded form.

    Detects an oversized section BEFORE publication -- this runs inside
    `publish-release` before `gh release create` -- rather than letting the
    live API call fail after every other release blocker already passed.
    Applies to every version, not one hard-coded past release: any future
    section that grows past the limit is truncated the same way. The cut
    lands on a line boundary and a fixed notice names the version and points
    at the untouched `CHANGELOG.md`, which this function never writes.
    """
    if len(section) <= BOUNDED_NOTES_LIMIT:
        return section
    return (
        section[:BOUNDED_NOTES_LIMIT].rsplit("\n", 1)[0]
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
    section = bound_section(section, args.version)
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
