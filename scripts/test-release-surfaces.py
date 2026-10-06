#!/usr/bin/env python3
"""Self-tests for the generated release surfaces (INSTALL-04/05/10).

Covers `release-notes.py` (install table, installer one-liners, bounded
changelog), `release-manifest.py` (optional `installers` inventory), the
archive README templates, and the closed target inventory shared by the
packagers, the notes renderer and `release-reconcile.py`.

Pure Python, offline, no network. Run: python3 scripts/test-release-surfaces.py
"""

import hashlib
import importlib.util
import re
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TAG = "v9.9.9"
VERSION = "9.9.9"
COMMIT = "0123456789abcdef0123456789abcdef01234567"


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


notes = load("t_release_notes", "release-notes.py")
manifest = load("t_release_manifest", "release-manifest.py")
reconcile = load("t_release_reconcile", "release-reconcile.py")

TARGETS = [target for target, _ in reconcile.ARCHIVE_TARGETS]
CHANGELOG = f"## {VERSION} — 2027-01-01\n\n- a change\n\n## 9.9.8 — 2026-12-01\n\nolder\n"
WORKFLOW = "jobs:\n  release-gate:\n    needs:\n      - alpha\n    runs-on: x\n"


def write_assets(directory, installers=(), skip=()):
    for target, extension in reconcile.ARCHIVE_TARGETS:
        name = f"semaprax-{TAG}-{target}.{extension}"
        if name not in skip:
            (directory / name).write_bytes(f"archive for {target}\n".encode())
    for name in installers:
        (directory / name).write_bytes(f"#installer {name}\n".encode())


def render(**kwargs):
    return notes.render_release_body(VERSION, "- a change", **kwargs)


class ReleaseNotes(unittest.TestCase):
    def test_labels_cover_exactly_the_admitted_targets(self):
        self.assertEqual(set(notes.TARGET_LABELS), set(TARGETS))

    def test_every_link_names_an_asset_in_the_directory(self):
        with tempfile.TemporaryDirectory() as scratch:
            assets = Path(scratch)
            write_assets(assets, installers=("install.sh", "install.ps1"))
            body = render(assets_dir=assets)
            prefix = f"https://github.com/wavect/semaprax/releases/download/{TAG}/"
            linked = set(re.findall(re.escape(prefix) + r"([A-Za-z0-9._-]+)", body))
            self.assertTrue(linked)
            for name in linked:
                self.assertTrue((assets / name).is_file(), name)
            for target, extension in reconcile.ARCHIVE_TARGETS:
                self.assertIn(f"semaprax-{TAG}-{target}.{extension}", linked)

    def test_install_section_precedes_collapsed_changelog(self):
        with tempfile.TemporaryDirectory() as scratch:
            assets = Path(scratch)
            write_assets(assets, installers=("install.sh", "install.ps1"))
            body = render(assets_dir=assets)
        self.assertTrue(body.startswith(f"SEMAPRAX {TAG} is beta software.\n"))
        install = body.index("## Install")
        details = body.index("<details>")
        self.assertLess(install, details)
        self.assertIn(f"<summary>Full changelog for {TAG}</summary>", body)
        self.assertIn("</details>", body)
        self.assertLess(body.index("## Changes"), body.index("</details>"))
        self.assertIn(
            f"https://github.com/wavect/semaprax/blob/{TAG}/handbook/getting-started/install.md",
            body,
        )
        self.assertIn("automatic repository snapshots", body)

    def test_installer_one_liners_only_when_installers_present(self):
        with tempfile.TemporaryDirectory() as scratch:
            assets = Path(scratch)
            write_assets(assets)
            bare = render(assets_dir=assets)
            self.assertNotIn("install.sh", bare)
            self.assertNotIn("install.ps1", bare)
            write_assets(assets, installers=("install.sh",))
            unix_only = render(assets_dir=assets)
            self.assertIn("install.sh | sh -s -- --version " + TAG, unix_only)
            self.assertIn("releases/latest/download/install.sh | sh", unix_only)
            self.assertNotIn("install.ps1", unix_only)
            write_assets(assets, installers=("install.sh", "install.ps1"))
            both = render(assets_dir=assets)
            self.assertIn(f"/download/{TAG}/install.ps1", both)
            self.assertIn("releases/latest/download/install.ps1 | iex", both)

    def test_missing_admitted_archive_fails_closed(self):
        for target, extension in reconcile.ARCHIVE_TARGETS:
            name = f"semaprax-{TAG}-{target}.{extension}"
            with tempfile.TemporaryDirectory() as scratch:
                assets = Path(scratch)
                write_assets(assets, skip=(name,))
                with self.assertRaises(ValueError) as context:
                    render(assets_dir=assets)
                self.assertIn(name, str(context.exception))

    def test_no_assets_mode_has_no_table_and_no_one_liners(self):
        body = render()
        self.assertNotIn("| Your computer |", body)
        self.assertNotIn("install.sh", body)
        self.assertIn("## Changes", body)

    def test_oversized_changelog_is_bounded_with_the_frame_counted(self):
        section = "- padding line that makes the section huge\n" * 6_000
        with tempfile.TemporaryDirectory() as scratch:
            assets = Path(scratch)
            write_assets(assets, installers=("install.sh", "install.ps1"))
            body = notes.render_release_body(VERSION, section, assets_dir=assets)
        self.assertLess(len(body), notes.GITHUB_BODY_LIMIT)
        self.assertIn("truncated for GitHub's 125,000-byte release-note limit", body)
        self.assertIn("</details>", body)

    def test_small_changelog_is_not_truncated(self):
        self.assertNotIn("truncated for GitHub", render())


def rendered_readme(kind, target):
    template = (ROOT / "packaging" / "archive" / f"README.{kind}.md").read_text(
        encoding="utf-8"
    )
    return (
        template.replace("{{TAG}}", TAG)
        .replace("{{VERSION}}", VERSION)
        .replace("{{TARGET}}", target)
    )


class ArchiveReadmes(unittest.TestCase):
    def readmes(self):
        yield "unix", "x86_64-unknown-linux-gnu"
        yield "unix", "aarch64-apple-darwin"
        yield "windows", "x86_64-pc-windows-msvc"

    def test_no_unreplaced_placeholders(self):
        for kind, target in self.readmes():
            self.assertNotIn("{{", rendered_readme(kind, target))

    def test_every_link_is_absolute_and_pinned_to_the_tag(self):
        for kind, target in self.readmes():
            text = rendered_readme(kind, target)
            links = re.findall(r"\]\(([^)]*)\)", text)
            self.assertTrue(links, kind)
            for link in links:
                self.assertTrue(
                    link.startswith("https://github.com/wavect/semaprax/"), link
                )
                if "/blob/" in link or "/tree/" in link or "/releases/tag/" in link:
                    self.assertIn(f"/{TAG}", link)
                self.assertNotIn("/main/", link)
                self.assertNotIn("/latest", link)
            self.assertNotRegex(text, r"\]\((?!https://)")

    def test_no_checkout_only_references(self):
        for kind, target in self.readmes():
            text = rendered_readme(kind, target)
            self.assertNotIn("examples/", text)
            self.assertNotIn("examples\\", text)
            self.assertNotIn("cargo run", text)

    def test_executable_names_match_the_platform(self):
        unix = rendered_readme("unix", "x86_64-unknown-linux-gnu")
        self.assertIn("./semaprax --version", unix)
        self.assertNotIn(".exe", unix)
        self.assertNotIn("PowerShell", unix)
        windows = rendered_readme("windows", "x86_64-pc-windows-msvc")
        self.assertIn(".\\semaprax.exe --version", windows)
        self.assertIn("```powershell", windows)
        self.assertNotIn("```sh", windows)
        self.assertNotIn("./semaprax", windows)

    def test_beginner_journey_is_present_and_literal(self):
        for kind, target in self.readmes():
            text = rendered_readme(kind, target)
            sep = "\\" if kind == "windows" else "/"
            for command in (
                "semaprax --version",
                "semaprax new first-semaprax",
                f"semaprax check first-semaprax{sep}semaprax.toml",
                f"semaprax test first-semaprax{sep}semaprax.toml",
                f"semaprax run first-semaprax{sep}semaprax.toml",
            ):
                self.assertIn(command, text, (kind, command))

    def test_templates_use_lf_line_endings(self):
        for kind in ("unix", "windows"):
            data = (ROOT / "packaging" / "archive" / f"README.{kind}.md").read_bytes()
            self.assertNotIn(b"\r", data)
            self.assertTrue(data.endswith(b"\n"))


def sha(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


class ManifestInstallers(unittest.TestCase):
    def build(self, assets):
        return manifest.build_manifest(
            VERSION, TAG, COMMIT, False, WORKFLOW, CHANGELOG, assets
        )

    def test_absent_installers_keep_the_original_shape(self):
        with tempfile.TemporaryDirectory() as scratch:
            assets = Path(scratch)
            write_assets(assets)
            self.assertNotIn("installers", self.build(assets))

    def test_installers_are_inventoried_sorted_with_real_digests(self):
        with tempfile.TemporaryDirectory() as scratch:
            assets = Path(scratch)
            write_assets(assets, installers=("install.sh", "install.ps1"))
            built = self.build(assets)
            self.assertEqual(
                [entry["name"] for entry in built["installers"]],
                ["install.ps1", "install.sh"],
            )
            for entry in built["installers"]:
                data = (assets / entry["name"]).read_bytes()
                self.assertEqual(entry["size"], len(data))
                self.assertEqual(entry["digest"], sha(data))

    def test_five_platform_artifacts(self):
        with tempfile.TemporaryDirectory() as scratch:
            assets = Path(scratch)
            write_assets(assets)
            platforms = [entry["platform"] for entry in self.build(assets)["artifacts"]]
            self.assertEqual(platforms, sorted(TARGETS))
            self.assertEqual(len(platforms), 5)

    def test_sha256sums_must_list_and_match_installers(self):
        with tempfile.TemporaryDirectory() as scratch:
            assets = Path(scratch)
            write_assets(assets, installers=("install.sh",))
            lines = []
            for path in sorted(assets.iterdir()):
                digest = hashlib.sha256(path.read_bytes()).hexdigest()
                lines.append(f"{digest}  {path.name}")
            (assets / "SHA256SUMS").write_text("\n".join(lines) + "\n", encoding="utf-8")
            self.build(assets)

            wrong = [
                line if not line.endswith("install.sh") else "0" * 64 + "  install.sh"
                for line in lines
            ]
            (assets / "SHA256SUMS").write_text("\n".join(wrong) + "\n", encoding="utf-8")
            with self.assertRaises(ValueError) as context:
                self.build(assets)
            self.assertIn("install.sh", str(context.exception))

            omitted = [line for line in lines if not line.endswith("install.sh")]
            (assets / "SHA256SUMS").write_text(
                "\n".join(omitted) + "\n", encoding="utf-8"
            )
            with self.assertRaises(ValueError):
                self.build(assets)

    def test_check_flags_installer_drift(self):
        with tempfile.TemporaryDirectory() as scratch:
            assets = Path(scratch)
            write_assets(assets, installers=("install.sh",))
            expected = self.build(assets)
            self.assertEqual(manifest.diff_manifest(expected, expected), [])
            drifted = dict(expected)
            drifted["installers"] = [dict(expected["installers"][0], size=1)]
            self.assertTrue(manifest.diff_manifest(drifted, expected))
            stripped = {k: v for k, v in expected.items() if k != "installers"}
            self.assertTrue(manifest.diff_manifest(stripped, expected))


class ClosedTargetInventory(unittest.TestCase):
    def test_five_targets_in_the_expected_order(self):
        self.assertEqual(
            TARGETS,
            [
                "x86_64-unknown-linux-gnu",
                "aarch64-unknown-linux-gnu",
                "aarch64-apple-darwin",
                "x86_64-apple-darwin",
                "x86_64-pc-windows-msvc",
            ],
        )

    def test_unix_packager_admits_every_unix_target(self):
        text = (ROOT / "scripts" / "package-release.sh").read_text(encoding="utf-8")
        for target in TARGETS:
            if "windows" not in target:
                self.assertIn(f"{target})", text, target)

    def test_container_packager_admits_the_linux_targets(self):
        text = (ROOT / "scripts" / "package-release-linux-container.sh").read_text(
            encoding="utf-8"
        )
        for target in TARGETS:
            if "linux" in target:
                self.assertIn(f"{target})", text, target)
        self.assertIn("1.97.1", text)

    def test_windows_packager_admits_the_windows_target(self):
        text = (ROOT / "scripts" / "package-release.ps1").read_text(encoding="utf-8")
        self.assertIn("x86_64-pc-windows-msvc", text)


if __name__ == "__main__":
    unittest.main()
