#!/usr/bin/env python3
"""Generate package-manager channel manifests from one published release.

Reads a release's `SHA256SUMS` and aggregate `release-manifest.json` and
writes, deterministically and without any network access:

* `homebrew`: a binary formula `semaprax.rb` for the tap `wavect/tap`;
* `winget`: the three-file WinGet manifest set for `Wavect.Semaprax`.

This tool only renders text. It never creates a repository, pull request, tap
or release, and it never talks to GitHub, Homebrew or WinGet; publication and
the pending/published record (`packaging/channels.json`) are separate,
reviewed steps. It fails closed: a missing archive, a digest absent from
`SHA256SUMS`, or a manifest that disagrees with `SHA256SUMS` or the tag
produces no output at all.

Usage:
    channel-manifests.py {homebrew,winget,all} --tag v0.8.0 \\
        --sums SHA256SUMS --manifest release-manifest.json --out DIR
"""

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO_URL = "https://github.com/wavect/semaprax"
TAG_RE = re.compile(r"v((?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*))")
DIGEST_RE = re.compile(r"[0-9a-f]{64}")
SUMS_LINE_RE = re.compile(r"([0-9a-f]{64})  ([^\s/\\]+)")

# target -> archive extension, as published by the release workflow.
TARGETS = {
    "x86_64-unknown-linux-gnu": "tar.gz",
    "aarch64-unknown-linux-gnu": "tar.gz",
    "aarch64-apple-darwin": "tar.gz",
    "x86_64-apple-darwin": "tar.gz",
    "x86_64-pc-windows-msvc": "zip",
}

# (target) -> (Homebrew OS block, Homebrew arch block, depends_on arch symbol)
BREW_TARGETS = {
    "aarch64-apple-darwin": ("macos", "arm", ":arm64"),
    "x86_64-apple-darwin": ("macos", "intel", ":x86_64"),
    "aarch64-unknown-linux-gnu": ("linux", "arm", ":arm64"),
    "x86_64-unknown-linux-gnu": ("linux", "intel", ":x86_64"),
}

DESCRIPTION = "Systems language: meaning in, verified machine code out"
WINGET_SCHEMA = "1.10.0"
WINGET_MIN_OS = "10.0.17763.0"
WINGET_TARGET = "x86_64-pc-windows-msvc"


class ChannelError(Exception):
    """The release material cannot back a channel manifest."""


def parse_sums(text):
    """`name -> lowercase sha256` from SHA256SUMS; malformed lines are errors."""
    sums = {}
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        match = SUMS_LINE_RE.fullmatch(line)
        if not match:
            raise ChannelError(f"SHA256SUMS line {number} is malformed: {line!r}")
        digest, name = match.groups()
        if name in sums:
            raise ChannelError(f"SHA256SUMS lists {name} twice")
        sums[name] = digest
    return sums


def archive_name(tag, target):
    return f"semaprax-{tag}-{target}.{TARGETS[target]}"


def release_archives(tag, sums_text, manifest_text):
    """`target -> (name, digest)` for every published archive, fully cross-checked."""
    if not TAG_RE.fullmatch(tag):
        raise ChannelError(f"tag {tag!r} is not vX.Y.Z")
    sums = parse_sums(sums_text)
    try:
        manifest = json.loads(manifest_text)
    except json.JSONDecodeError as error:
        raise ChannelError(f"release-manifest.json is not JSON: {error}") from error
    if not isinstance(manifest, dict) or manifest.get("tag") != tag:
        raise ChannelError(
            f"release-manifest.json tag is {manifest.get('tag')!r}, expected {tag!r}"
            if isinstance(manifest, dict)
            else "release-manifest.json is not an object"
        )
    version = tag[1:]
    if manifest.get("version") != version:
        raise ChannelError(
            f"release-manifest.json version is {manifest.get('version')!r}, expected {version!r}"
        )
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, list):
        raise ChannelError("release-manifest.json has no artifacts list")
    by_platform = {}
    for entry in artifacts:
        platform = entry.get("platform") if isinstance(entry, dict) else None
        if platform in by_platform:
            raise ChannelError(f"release-manifest.json lists {platform} twice")
        by_platform[platform] = entry
    found = {}
    for target in sorted(TARGETS):
        name = archive_name(tag, target)
        in_sums = name in sums
        entry = by_platform.get(target)
        if not in_sums and entry is None:
            continue  # this release did not publish that target
        if not in_sums:
            raise ChannelError(f"release-manifest.json lists {name} but SHA256SUMS does not")
        if entry is None:
            raise ChannelError(f"SHA256SUMS lists {name} but release-manifest.json does not")
        if entry.get("name") != name:
            raise ChannelError(
                f"release-manifest.json names {entry.get('name')!r} for {target}, expected {name!r}"
            )
        digest = sums[name]
        if entry.get("digest") != f"sha256:{digest}":
            raise ChannelError(
                f"digest disagreement for {name}: SHA256SUMS says {digest}, "
                f"release-manifest.json says {entry.get('digest')!r}"
            )
        found[target] = (name, digest)
    return found


def license_spdx():
    text = (ROOT / "LICENSE").read_text(encoding="utf-8")
    if "Apache License" in text and "Version 2.0" in text:
        return "Apache-2.0"
    raise ChannelError("LICENSE is not recognised as Apache-2.0; update channel-manifests.py")


# --- Homebrew --------------------------------------------------------------


def render_formula(tag, archives, include_linux=False):
    """The binary formula text. Only macOS arm64 is required to exist.

    Linux archives are emitted only with `include_linux`: Homebrew on Linux
    uses the host glibc, so a Linux formula is a support claim that needs its
    own `brew test` evidence on a host at the archive's glibc baseline.
    """
    # The version is deliberately not declared: Homebrew derives it from the
    # tag-bearing archive URL and audit rejects a redundant `version` line.
    present = {
        t: v
        for t, v in archives.items()
        if t in BREW_TARGETS and (include_linux or BREW_TARGETS[t][0] == "macos")
    }
    if not present:
        raise ChannelError("no macOS or Linux archive is published for Homebrew")
    by_os = {}
    for target, (name, digest) in sorted(present.items()):
        os_name, arch_block, arch_symbol = BREW_TARGETS[target]
        by_os.setdefault(os_name, {})[arch_block] = (target, name, digest, arch_symbol)

    lines = [
        "class Semaprax < Formula",
        f'  desc "{DESCRIPTION}"',
        f'  homepage "{REPO_URL}"',
        f'  license "{license_spdx()}"',
        "",
        "  livecheck do",
        "    url :stable",
        "    strategy :github_latest",
        "  end",
        "",
    ]
    if "linux" not in by_os:
        lines += ["  depends_on :macos", ""]
    elif "macos" not in by_os:
        lines += ["  depends_on :linux", ""]
    for os_name in ("macos", "linux"):
        arches = by_os.get(os_name)
        if not arches:
            continue
        lines.append(f"  on_{os_name} do")
        if len(arches) == 1:
            (only,) = arches.values()
            lines += [f"    depends_on arch: {only[3]}", ""]
        for arch_block in ("arm", "intel"):
            if arch_block not in arches:
                continue
            _target, name, digest, _symbol = arches[arch_block]
            lines += [
                f"    on_{arch_block} do",
                f'      url "{REPO_URL}/releases/download/{tag}/{name}"',
                f'      sha256 "{digest}"',
                "    end",
            ]
        lines.append("  end")
        lines.append("")
    lines += [
        "  def install",
        '    bin.install "semaprax", "semapraxd"',
        '    pkgshare.install "release-manifest.json"',
        '    prefix.install "LICENSE"',
        "  end",
        "",
        "  test do",
        "    # Both executables ship in the same archive, so they are the same release.",
        '    assert_path_exists bin/"semapraxd"',
        '    assert_predicate bin/"semapraxd", :executable?',
        '    assert_match version.to_s, shell_output("#{bin}/semaprax --version")',
        '    assert_match version.to_s, (pkgshare/"release-manifest.json").read',
        "",
        '    system bin/"semaprax", "new", "first-semaprax"',
        '    system bin/"semaprax", "check", "first-semaprax/semaprax.toml"',
        '    system bin/"semaprax", "test", "first-semaprax/semaprax.toml"',
        '    assert_equal "42", shell_output("#{bin}/semaprax run first-semaprax/semaprax.toml").strip',
        "  end",
        "end",
        "",
    ]
    return "\n".join(lines)


# --- WinGet ----------------------------------------------------------------


def _winget_header(kind):
    return (
        "# Created by scripts/channel-manifests.py from the published release; do not edit by hand.\n"
        f"# yaml-language-server: $schema=https://aka.ms/winget-manifest.{kind}.{WINGET_SCHEMA}.schema.json\n\n"
    )


def render_winget(tag, archives):
    """`filename -> text` for the version, installer and defaultLocale manifests."""
    if WINGET_TARGET not in archives:
        raise ChannelError(f"no {WINGET_TARGET} archive is published for WinGet")
    version = tag[1:]
    name, digest = archives[WINGET_TARGET]
    folder = name[: -len(".zip")]
    identifier = "Wavect.Semaprax"
    version_manifest = (
        _winget_header("version")
        + f"PackageIdentifier: {identifier}\n"
        + f"PackageVersion: {version}\n"
        + "DefaultLocale: en-US\n"
        + "ManifestType: version\n"
        + f"ManifestVersion: {WINGET_SCHEMA}\n"
    )
    installer_manifest = (
        _winget_header("installer")
        + f"PackageIdentifier: {identifier}\n"
        + f"PackageVersion: {version}\n"
        + "InstallerLocale: en-US\n"
        + f"MinimumOSVersion: {WINGET_MIN_OS}\n"
        + "InstallerType: zip\n"
        + "NestedInstallerType: portable\n"
        + "NestedInstallerFiles:\n"
        + f"- RelativeFilePath: {folder}\\semaprax.exe\n"
        + "  PortableCommandAlias: semaprax\n"
        + f"- RelativeFilePath: {folder}\\semapraxd.exe\n"
        + "  PortableCommandAlias: semapraxd\n"
        + "Installers:\n"
        + "- Architecture: x64\n"
        + f"  InstallerUrl: {REPO_URL}/releases/download/{tag}/{name}\n"
        + f"  InstallerSha256: {digest.upper()}\n"
        + "ManifestType: installer\n"
        + f"ManifestVersion: {WINGET_SCHEMA}\n"
    )
    locale_manifest = (
        _winget_header("defaultLocale")
        + f"PackageIdentifier: {identifier}\n"
        + f"PackageVersion: {version}\n"
        + "PackageLocale: en-US\n"
        + "Publisher: Wavect\n"
        + "PublisherUrl: https://github.com/wavect\n"
        + f"PublisherSupportUrl: {REPO_URL}/issues\n"
        + "PackageName: Semaprax\n"
        + f"PackageUrl: {REPO_URL}\n"
        + f"License: {license_spdx()}\n"
        + f"LicenseUrl: {REPO_URL}/blob/{tag}/LICENSE\n"
        + f"ShortDescription: \"{DESCRIPTION}.\"\n"
        + f"ReleaseNotesUrl: {REPO_URL}/releases/tag/{tag}\n"
        + "Moniker: semaprax\n"
        + "Tags:\n"
        + "- compiler\n"
        + "- programming-language\n"
        + "- semaprax\n"
        + "ManifestType: defaultLocale\n"
        + f"ManifestVersion: {WINGET_SCHEMA}\n"
    )
    return {
        f"{identifier}.yaml": version_manifest,
        f"{identifier}.installer.yaml": installer_manifest,
        f"{identifier}.locale.en-US.yaml": locale_manifest,
    }


def winget_directory(tag):
    """Relative winget-pkgs path for the package version."""
    return Path("manifests") / "w" / "Wavect" / "Semaprax" / tag[1:]


# --- CLI -------------------------------------------------------------------


def write_if_changed(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.is_file() and path.read_text(encoding="utf-8") == text:
        return
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("channel", choices=("homebrew", "winget", "all"))
    parser.add_argument("--tag", required=True, help="exact release tag, e.g. v0.8.0")
    parser.add_argument("--sums", type=Path, required=True, help="the release's SHA256SUMS")
    parser.add_argument("--manifest", type=Path, required=True, help="the aggregate release-manifest.json")
    parser.add_argument("--out", type=Path, required=True, help="output directory")
    parser.add_argument(
        "--include-linux",
        action="store_true",
        help="also emit on_linux blocks for published Linux archives (needs Linux brew test evidence)",
    )
    args = parser.parse_args(argv)
    try:
        archives = release_archives(
            args.tag,
            args.sums.read_text(encoding="utf-8"),
            args.manifest.read_text(encoding="utf-8"),
        )
        outputs = {}
        if args.channel in ("homebrew", "all"):
            outputs[Path("homebrew") / "semaprax.rb"] = render_formula(args.tag, archives, args.include_linux)
        if args.channel in ("winget", "all"):
            for filename, text in render_winget(args.tag, archives).items():
                outputs[Path("winget") / winget_directory(args.tag) / filename] = text
    except (ChannelError, OSError) as error:
        print(f"channel manifests rejected: {error}", file=sys.stderr)
        return 1
    for relative, text in sorted(outputs.items()):
        write_if_changed(args.out / relative, text)
        print(f"wrote {args.out / relative}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
