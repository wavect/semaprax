#!/usr/bin/env python3
"""Self-tests for the install-claim checks in scripts/release-reconcile.py.

Pure and offline: no network, no `gh`, no file writes. Run with
`python3 scripts/test-release-reconcile-install-claims.py`.
"""
import copy
import json
import runpy
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULE = runpy.run_path(str(ROOT / "scripts" / "release-reconcile.py"))
reconcile = MODULE["reconcile_install_claims"]
validate_channels = MODULE["validate_channels"]
live_assets_agree = MODULE["live_assets_agree"]
advertised = MODULE["advertised_release_assets"]
strip_history = MODULE["strip_history"]

COMMIT = "a" * 40
ARCHIVE = "semaprax-v0.8.0-aarch64-apple-darwin.tar.gz"
ASSETS = sorted(
    [
        "SHA256SUMS",
        "release-manifest.json",
        ARCHIVE,
        "semaprax-v0.8.0-x86_64-pc-windows-msvc.zip",
        "semaprax-v0.8.0-x86_64-unknown-linux-gnu.tar.gz",
    ]
)
CHANNELS = {
    "schema": "semaprax.channel-status.v1",
    "releases": {"v0.8.0": {"commit": COMMIT, "published_at": "2026-10-06T07:37:34Z", "assets": ASSETS}},
    "installers": {"first_release": None},
    "homebrew": {"status": "pending", "version": None, "verified_at": None, "command": "brew install wavect/tap/semaprax"},
    "winget": {"status": "pending", "version": None, "verified_at": None, "command": "winget install --id Wavect.Semaprax --exact"},
}
CHANGELOG = {"0.8.0": "2026-10-04", "0.7.0": "2026-10-01"}
GOOD_GUIDE = f"""# Install

Newest release: [v0.8.0](https://github.com/wavect/semaprax/releases/tag/v0.8.0).

```sh
TAG=v0.8.0
BASE=https://github.com/wavect/semaprax/releases/download/v0.8.0
curl -fLO "$BASE/{ARCHIVE}"
curl -fLO "$BASE/SHA256SUMS"
grep " {ARCHIVE}$" SHA256SUMS | shasum -a 256 -c -
```
"""


def check(text, channels=CHANNELS, evidence=None, changelog=CHANGELOG, path="guide.md"):
    return reconcile({path: text}, channels, evidence or {}, changelog)


class InstallClaims(unittest.TestCase):
    def test_current_guide_is_clean(self):
        self.assertEqual(check(GOOD_GUIDE), [])

    def test_selected_checksum_example_is_accepted_and_whole_inventory_rejected(self):
        self.assertEqual(check(GOOD_GUIDE), [])
        for command in ("shasum -a 256 -c SHA256SUMS", "sha256sum --check SHA256SUMS"):
            problems = check(GOOD_GUIDE + f"\n```sh\n{command}\n```\n")
            self.assertEqual(len(problems), 1, problems)
            self.assertIn("whole SHA256SUMS inventory", problems[0])

    def test_unrecorded_published_version_is_rejected(self):
        problems = check(GOOD_GUIDE.replace("v0.8.0", "v9.9.9"))
        self.assertTrue(problems)
        self.assertTrue(any("v9.9.9" in p and "no recorded published" in p for p in problems), problems)

    def test_stale_latest_claim_is_rejected(self):
        channels = copy.deepcopy(CHANNELS)
        channels["releases"]["v0.7.0"] = {
            "commit": "b" * 40,
            "published_at": "2026-10-01T00:00:00Z",
            "assets": ["SHA256SUMS", "release-manifest.json"],
        }
        problems = check("The latest published [v0.7.0 archive](x) is current.", channels=channels)
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("newest recorded release is v0.8.0", problems[0])

    def test_dated_history_is_not_checked(self):
        text = (
            GOOD_GUIDE
            + "\n<!-- release-claims: history-begin -->\nThe latest published v0.7.0 "
            "semaprax-v0.7.0-x86_64-unknown-linux-gnu.tar.gz; run install.sh; brew install x\n"
            "<!-- release-claims: history-end -->\n"
        )
        self.assertEqual(check(text), [])
        self.assertNotIn("v0.7.0", strip_history(text))

    def test_asset_absent_from_the_recorded_release_is_rejected(self):
        channels = copy.deepcopy(CHANNELS)
        channels["releases"]["v0.8.0"]["assets"] = [a for a in ASSETS if a != ARCHIVE]
        problems = check(GOOD_GUIDE, channels=channels)
        self.assertEqual(len(problems), 1, problems)
        self.assertIn(ARCHIVE, problems[0])

    def test_archive_with_wrong_extension_or_target_is_rejected(self):
        for bad in (
            "semaprax-v0.8.0-aarch64-apple-darwin.zip",
            "semaprax-v0.8.0-riscv64-unknown-linux-gnu.tar.gz",
        ):
            problems = check(GOOD_GUIDE + f"\n`{bad}`\n")
            self.assertTrue(any("not a published release target" in p for p in problems), problems)

    def test_exact_tag_link_must_match_its_asset(self):
        text = GOOD_GUIDE + (
            "\n[x](https://github.com/wavect/semaprax/releases/download/v0.7.0/"
            + ARCHIVE
            + ")\n"
        )
        channels = copy.deepcopy(CHANNELS)
        channels["releases"]["v0.7.0"] = {
            "commit": "b" * 40,
            "published_at": "2026-10-01T00:00:00Z",
            "assets": ["SHA256SUMS", "release-manifest.json"],
        }
        problems = check(text, channels=channels)
        self.assertTrue(any("links release tag v0.7.0 to asset" in p for p in problems), problems)

    def test_installer_claims_need_a_release_that_published_them(self):
        for claim in (
            "curl -fsSL https://github.com/wavect/semaprax/releases/latest/download/install.sh | sh",
            "irm https://github.com/wavect/semaprax/releases/latest/download/install.ps1 | iex",
        ):
            problems = check(GOOD_GUIDE + f"\n```sh\n{claim}\n```\n")
            self.assertEqual(len(problems), 1, problems)
            self.assertIn("records no release that published them", problems[0])
        # The repository source path is not an advertisement.
        self.assertEqual(check(GOOD_GUIDE + "\nSee scripts/install.sh.\n"), [])

    def test_installer_first_release_must_list_both_assets(self):
        channels = copy.deepcopy(CHANNELS)
        channels["installers"]["first_release"] = "v0.8.0"
        problems = validate_channels(channels)
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("install.sh, install.ps1", problems[0])
        channels["releases"]["v0.8.0"]["assets"] = sorted(ASSETS + ["install.sh", "install.ps1"])
        self.assertEqual(validate_channels(channels), [])
        self.assertEqual(check(GOOD_GUIDE + "\nrun install.sh\n", channels=channels), [])

    def test_package_manager_claims_follow_channel_status(self):
        for claim, name in (
            ("brew install wavect/tap/semaprax", "homebrew"),
            ("winget install --id Wavect.Semaprax --exact", "winget"),
        ):
            problems = check(GOOD_GUIDE + f"\n```sh\n{claim}\n```\n")
            self.assertEqual(len(problems), 1, problems)
            self.assertIn(f"advertises the {name} channel", problems[0])
            published = copy.deepcopy(CHANNELS)
            published[name].update(status="published", version="v0.8.0", verified_at="2026-10-07")
            self.assertEqual(check(GOOD_GUIDE + f"\n{claim}\n", channels=published), [])

    def test_published_channel_needs_a_recorded_release_and_verification(self):
        channels = copy.deepcopy(CHANNELS)
        channels["homebrew"].update(status="published", version="v9.9.9")
        problems = validate_channels(channels)
        self.assertTrue(any("not a recorded release" in p for p in problems), problems)
        self.assertTrue(any("without verified_at" in p for p in problems), problems)

    def test_malformed_record_fails_closed(self):
        self.assertTrue(validate_channels({}))
        broken = copy.deepcopy(CHANNELS)
        broken["releases"]["v0.8.0"]["assets"] = ["z", "a"]
        self.assertTrue(validate_channels(broken))
        broken = copy.deepcopy(CHANNELS)
        del broken["installers"]
        self.assertTrue(validate_channels(broken))

    def test_live_assets_must_exist_on_the_release(self):
        live = {"assets": [{"name": a} for a in ASSETS]}
        want = advertised({"g.md": GOOD_GUIDE}, CHANNELS)
        self.assertEqual(want, {"0.8.0": {ARCHIVE}})
        self.assertEqual(live_assets_agree("0.8.0", want["0.8.0"], live), [])
        problems = live_assets_agree("0.8.0", {"install.sh"}, live)
        self.assertEqual(problems, ["--live: v0.8.0 Release has no asset install.sh"])

    def test_real_repository_docs_agree_with_the_recorded_state(self):
        docs, channels = MODULE["load_install_claim_inputs"]()
        self.assertIsNotNone(channels, "packaging/channels.json must exist")
        self.assertIn("handbook/getting-started/install.md", docs)
        evidence = MODULE["evidence_sections"]((ROOT / "docs/RELEASE-PROCESS.md").read_text(encoding="utf-8"))
        changelog = MODULE["changelog_versions"]((ROOT / "CHANGELOG.md").read_text(encoding="utf-8"))
        self.assertEqual(reconcile(docs, channels, evidence, changelog), [])
        # The record itself is JSON that round-trips deterministically.
        text = (ROOT / "packaging/channels.json").read_text(encoding="utf-8")
        self.assertEqual(json.dumps(json.loads(text), indent=2) + "\n", text)


if __name__ == "__main__":
    unittest.main()
