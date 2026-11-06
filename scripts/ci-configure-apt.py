#!/usr/bin/env python3
"""Use Ubuntu's canonical mirror and bound dependency acquisition in CI."""

import argparse
from pathlib import Path
import re


AZURE_MIRROR = re.compile(r"\bhttps?://azure\.archive\.ubuntu\.com/ubuntu(?=/|\s|$)")
NETWORK_CONFIG = '''Acquire::http::Timeout "30";
Acquire::https::Timeout "30";
Acquire::Retries "2";
DPkg::Lock::Timeout "120";
'''


def configure(root):
    sources = [root / "sources.list"]
    sources.extend(sorted((root / "sources.list.d").glob("*.list")))
    sources.extend(sorted((root / "sources.list.d").glob("*.sources")))
    for path in sources:
        if not path.is_file():
            continue
        before = path.read_text(encoding="utf-8")
        after = AZURE_MIRROR.sub("https://archive.ubuntu.com/ubuntu", before)
        if after != before:
            path.write_text(after, encoding="utf-8")
            print(f"Replaced Azure Ubuntu mirror in {path}")
    config = root / "apt.conf.d" / "99-semaprax-ci-network"
    config.parent.mkdir(parents=True, exist_ok=True)
    config.write_text(NETWORK_CONFIG, encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apt-root", type=Path, default=Path("/etc/apt"))
    args = parser.parse_args()
    configure(args.apt_root)


if __name__ == "__main__":
    main()
