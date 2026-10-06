#!/usr/bin/env python3
"""Self-tests for scripts/channel-manifests.py. Offline; writes only to temp dirs."""
import json
import runpy
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULE = runpy.run_path(str(ROOT / "scripts" / "channel-manifests.py"))
ChannelError = MODULE["ChannelError"]
release_archives = MODULE["release_archives"]
render_formula = MODULE["render_formula"]
render_winget = MODULE["render_winget"]
parse_sums = MODULE["parse_sums"]
main = MODULE["main"]

TAG = "v0.8.0"
MAC = "aarch64-apple-darwin"
WIN = "x86_64-pc-windows-msvc"
LIN = "x86_64-unknown-linux-gnu"
INTEL = "x86_64-apple-darwin"
DIGESTS = {
    MAC: "c281aa274650e9003487f738a349fb746b27bc5575276cae63f50cc81a155ff6",
    WIN: "f78f626c524510ac45f9a7cb136d1f77be0fba2528efacdc94871888de23ed32",
    LIN: "246c8e981a12cedaac10f102ce1e6c76a8835cbc647b8d44a2e34fbd9918affe",
    INTEL: "1" * 64,
}
EXT = {MAC: "tar.gz", WIN: "zip", LIN: "tar.gz", INTEL: "tar.gz"}


def material(targets=(MAC, WIN, LIN), tag=TAG, mutate=None):
    sums = ""
    artifacts = []
    for target in targets:
        name = f"semaprax-{tag}-{target}.{EXT[target]}"
        sums += f"{DIGESTS[target]}  {name}\n"
        artifacts.append(
            {"name": name, "platform": target, "size": 1, "digest": f"sha256:{DIGESTS[target]}"}
        )
    manifest = {
        "schema": "semaprax.release-manifest.v1",
        "version": tag[1:],
        "tag": tag,
        "artifacts": artifacts,
    }
    if mutate:
        sums, manifest = mutate(sums, manifest)
    return sums, json.dumps(manifest)


class Archives(unittest.TestCase):
    def test_cross_checked_archives(self):
        archives = release_archives(TAG, *material())
        self.assertEqual(sorted(archives), sorted([MAC, WIN, LIN]))
        self.assertEqual(archives[MAC][1], DIGESTS[MAC])

    def test_installer_assets_in_sums_are_ignored(self):
        def add(sums, manifest):
            return sums + f"{'2' * 64}  install.sh\n{'3' * 64}  install.ps1\n", manifest

        self.assertEqual(sorted(release_archives(TAG, *material(mutate=add))), sorted([MAC, WIN, LIN]))

    def test_digest_disagreement_fails_closed(self):
        def bad(sums, manifest):
            manifest["artifacts"][0]["digest"] = "sha256:" + "0" * 64
            return sums, manifest

        with self.assertRaisesRegex(ChannelError, "digest disagreement"):
            release_archives(TAG, *material(mutate=bad))

    def test_tag_and_version_mismatch_fail_closed(self):
        sums, manifest = material()
        with self.assertRaisesRegex(ChannelError, "tag is"):
            release_archives("v0.9.0", sums, manifest)
        with self.assertRaises(ChannelError):
            release_archives("0.8.0", sums, manifest)

        def wrong_version(s, m):
            m["version"] = "0.7.0"
            return s, m

        with self.assertRaisesRegex(ChannelError, "version is"):
            release_archives(TAG, *material(mutate=wrong_version))

    def test_one_sided_listings_fail_closed(self):
        def drop_manifest_entry(sums, manifest):
            manifest["artifacts"].pop()
            return sums, manifest

        with self.assertRaisesRegex(ChannelError, "SHA256SUMS lists"):
            release_archives(TAG, *material(mutate=drop_manifest_entry))

        def drop_sums_line(sums, manifest):
            return "".join(sums.splitlines(True)[:-1]), manifest

        with self.assertRaisesRegex(ChannelError, "SHA256SUMS does not"):
            release_archives(TAG, *material(mutate=drop_sums_line))

    def test_malformed_inputs_fail_closed(self):
        with self.assertRaises(ChannelError):
            parse_sums("not a checksum line\n")
        with self.assertRaises(ChannelError):
            parse_sums(f"{'a' * 64}  x\n{'b' * 64}  x\n")
        with self.assertRaises(ChannelError):
            release_archives(TAG, material()[0], "{")


class Homebrew(unittest.TestCase):
    def formula(self, targets=(MAC, WIN, LIN), **kwargs):
        return render_formula(TAG, release_archives(TAG, *material(targets)), **kwargs)

    def test_apple_silicon_only_formula(self):
        text = self.formula()
        self.assertIn("class Semaprax < Formula", text)
        self.assertIn('license "Apache-2.0"', text)
        self.assertIn("depends_on :macos", text)
        self.assertIn("depends_on arch: :arm64", text)
        self.assertIn(
            f'url "https://github.com/wavect/semaprax/releases/download/{TAG}/semaprax-{TAG}-{MAC}.tar.gz"',
            text,
        )
        self.assertIn(f'sha256 "{DIGESTS[MAC]}"', text)
        self.assertNotIn("on_intel", text)
        self.assertNotIn("on_linux", text)
        self.assertNotIn(DIGESTS[LIN], text)
        self.assertNotIn(DIGESTS[WIN], text)
        self.assertNotIn("\n  version ", text)
        self.assertIn('bin.install "semaprax", "semapraxd"', text)

    def test_formula_test_block_runs_the_beginner_journey(self):
        text = self.formula()
        for needle in (
            'system bin/"semaprax", "new", "first-semaprax"',
            'system bin/"semaprax", "check", "first-semaprax/semaprax.toml"',
            'system bin/"semaprax", "test", "first-semaprax/semaprax.toml"',
            'assert_equal "42"',
            'assert_path_exists bin/"semapraxd"',
        ):
            self.assertIn(needle, text)

    def test_intel_macos_is_added_only_when_published(self):
        text = self.formula(targets=(MAC, INTEL, WIN))
        self.assertIn("on_arm do", text)
        self.assertIn("on_intel do", text)
        self.assertIn(f'sha256 "{DIGESTS[INTEL]}"', text)
        self.assertNotIn("depends_on arch:", text)

    def test_linux_is_opt_in(self):
        self.assertNotIn("on_linux", self.formula())
        text = self.formula(include_linux=True)
        self.assertIn("on_linux do", text)
        self.assertIn(f'sha256 "{DIGESTS[LIN]}"', text)
        self.assertNotIn("depends_on :macos", text)

    def test_missing_macos_archive_fails_closed(self):
        with self.assertRaises(ChannelError):
            self.formula(targets=(WIN,))

    def test_deterministic(self):
        self.assertEqual(self.formula(), self.formula())


class Winget(unittest.TestCase):
    def manifests(self, targets=(MAC, WIN, LIN)):
        return render_winget(TAG, release_archives(TAG, *material(targets)))

    def test_installer_manifest(self):
        files = self.manifests()
        self.assertEqual(
            sorted(files),
            ["Wavect.Semaprax.installer.yaml", "Wavect.Semaprax.locale.en-US.yaml", "Wavect.Semaprax.yaml"],
        )
        installer = files["Wavect.Semaprax.installer.yaml"]
        folder = f"semaprax-{TAG}-{WIN}"
        for line in (
            "PackageIdentifier: Wavect.Semaprax",
            "PackageVersion: 0.8.0",
            "MinimumOSVersion: 10.0.17763.0",
            "InstallerType: zip",
            "NestedInstallerType: portable",
            f"- RelativeFilePath: {folder}\\semaprax.exe",
            "  PortableCommandAlias: semaprax",
            f"- RelativeFilePath: {folder}\\semapraxd.exe",
            "  PortableCommandAlias: semapraxd",
            "- Architecture: x64",
            f"  InstallerUrl: https://github.com/wavect/semaprax/releases/download/{TAG}/{folder}.zip",
            f"  InstallerSha256: {DIGESTS[WIN].upper()}",
            "ManifestType: installer",
            "ManifestVersion: 1.10.0",
        ):
            self.assertIn(line + "\n", installer)
        self.assertNotIn("arm64", installer)
        self.assertNotIn("Scope:", installer)  # winget validate warns: Scope unsupported for portable

    def test_other_manifests_are_consistent(self):
        files = self.manifests()
        self.assertIn("DefaultLocale: en-US\nManifestType: version\n", files["Wavect.Semaprax.yaml"])
        locale = files["Wavect.Semaprax.locale.en-US.yaml"]
        self.assertIn("License: Apache-2.0\n", locale)
        # A plain scalar containing ": " is a YAML scanner error (winget validate rejects it).
        self.assertIn('ShortDescription: "Systems language: meaning in, verified machine code out."\n', locale)
        for text in files.values():
            for line in text.splitlines():
                key, _, value = line.partition(": ")
                if line.startswith("#") or not value or value[0] in "\"'":
                    continue
                self.assertNotIn(": ", value, line)
        self.assertIn(f"LicenseUrl: https://github.com/wavect/semaprax/blob/{TAG}/LICENSE\n", locale)
        self.assertIn("ManifestType: defaultLocale\n", locale)
        for text in files.values():
            self.assertIn("PackageVersion: 0.8.0\n", text)
            self.assertTrue(text.endswith("\n"))
            self.assertNotIn("\r", text)

    def test_missing_windows_archive_fails_closed(self):
        with self.assertRaises(ChannelError):
            self.manifests(targets=(MAC, LIN))


class Cli(unittest.TestCase):
    def run_cli(self, channel, sums, manifest, tag=TAG, extra=()):
        with tempfile.TemporaryDirectory() as scratch:
            scratch = Path(scratch)
            (scratch / "SHA256SUMS").write_text(sums, encoding="utf-8")
            (scratch / "release-manifest.json").write_text(manifest, encoding="utf-8")
            out = scratch / "out"
            code = main(
                [channel, "--tag", tag, "--sums", str(scratch / "SHA256SUMS"),
                 "--manifest", str(scratch / "release-manifest.json"), "--out", str(out), *extra]
            )
            files = sorted(str(p.relative_to(out)) for p in out.rglob("*") if p.is_file()) if out.exists() else []
            contents = {f: (out / f).read_bytes() for f in files}
            second = None
            if code == 0:
                main([channel, "--tag", tag, "--sums", str(scratch / "SHA256SUMS"),
                      "--manifest", str(scratch / "release-manifest.json"), "--out", str(out), *extra])
                second = {f: (out / f).read_bytes() for f in files}
            return code, files, contents, second

    def test_all_channels_write_expected_tree_idempotently(self):
        code, files, contents, second = self.run_cli("all", *material())
        self.assertEqual(code, 0)
        self.assertEqual(
            files,
            [
                "homebrew/semaprax.rb",
                "winget/manifests/w/Wavect/Semaprax/0.8.0/Wavect.Semaprax.installer.yaml",
                "winget/manifests/w/Wavect/Semaprax/0.8.0/Wavect.Semaprax.locale.en-US.yaml",
                "winget/manifests/w/Wavect/Semaprax/0.8.0/Wavect.Semaprax.yaml",
            ],
        )
        self.assertEqual(contents, second)

    def test_rejection_writes_nothing(self):
        def bad(sums, manifest):
            manifest["artifacts"][0]["digest"] = "sha256:" + "0" * 64
            return sums, manifest

        code, files, _contents, _second = self.run_cli("all", *material(mutate=bad))
        self.assertEqual(code, 1)
        self.assertEqual(files, [])

    def test_winget_alone_does_not_need_macos(self):
        code, files, _c, _s = self.run_cli("winget", *material(targets=(WIN,)))
        self.assertEqual(code, 0)
        self.assertEqual(len(files), 3)
        code, files, _c, _s = self.run_cli("homebrew", *material(targets=(WIN,)))
        self.assertEqual((code, files), (1, []))


if __name__ == "__main__":
    unittest.main()
