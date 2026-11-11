"""Exercise the actual CI APT configuration without privileges or networking."""

import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("ci-configure-apt.py")
SPEC = importlib.util.spec_from_file_location("ci_configure_apt", SCRIPT)
APT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(APT)


class AptConfigurationTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="semaprax-apt-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / "sources.list.d").mkdir()

    def test_deb822_replaces_both_azure_schemes_and_preserves_signing_and_suites(self):
        path = self.root / "sources.list.d/ubuntu.sources"
        source = ("Types: deb\nURIs: http://azure.archive.ubuntu.com/ubuntu/ "
                  "https://azure.archive.ubuntu.com/ubuntu\n"
                  "Suites: noble noble-updates noble-security\nComponents: main universe\n"
                  "Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg\n")
        path.write_text(source, encoding="utf-8")
        APT.configure(self.root)
        expected = source.replace("http://azure.archive.ubuntu.com/ubuntu",
                                  "https://archive.ubuntu.com/ubuntu")
        expected = expected.replace("https://azure.archive.ubuntu.com/ubuntu",
                                    "https://archive.ubuntu.com/ubuntu")
        self.assertEqual(path.read_text(encoding="utf-8"), expected)

    def test_legacy_lists_preserve_unrelated_repositories_and_comments(self):
        path = self.root / "sources.list"
        source = ("# trusted repositories\n"
                  "deb http://azure.archive.ubuntu.com/ubuntu noble main\n"
                  "deb https://packages.microsoft.com/repos/azure-cli/ noble main\n"
                  "deb https://archive.ubuntu.com/ubuntu noble-security main\n")
        path.write_text(source, encoding="utf-8")
        unrelated = self.root / "sources.list.d/third-party.list"
        unrelated.write_bytes(b"deb https://example.org/ubuntu noble main\n")
        APT.configure(self.root)
        self.assertEqual(path.read_text(encoding="utf-8"), source.replace(
            "http://azure.archive.ubuntu.com/ubuntu", "https://archive.ubuntu.com/ubuntu"))
        self.assertEqual(unrelated.read_bytes(), b"deb https://example.org/ubuntu noble main\n")

    def test_runner_mirror_list_rewrites_azure_uri_and_preserves_other_entries(self):
        sources = self.root / "sources.list.d/ubuntu.sources"
        source = b"Types: deb\nURIs: mirror+file:/etc/apt/apt-mirrors.txt\nSuites: noble-updates\n"
        sources.write_bytes(source)
        mirrors = self.root / "apt-mirrors.txt"
        mirrors.write_bytes(
            b"http://azure.archive.ubuntu.com/ubuntu priority:1\n"
            b"https://security.ubuntu.com/ubuntu priority:2\n"
        )

        APT.configure(self.root)

        self.assertEqual(sources.read_bytes(), source)
        self.assertEqual(mirrors.read_bytes(),
                         b"https://archive.ubuntu.com/ubuntu priority:1\n"
                         b"https://security.ubuntu.com/ubuntu priority:2\n")

    def test_command_is_idempotent_and_writes_finite_timeouts_without_disabling_verification(self):
        args = [sys.executable, str(SCRIPT), "--apt-root", str(self.root)]
        subprocess.run(args, check=True, capture_output=True)
        before = {p.relative_to(self.root): p.read_bytes()
                  for p in self.root.rglob("*") if p.is_file()}
        subprocess.run(args, check=True, capture_output=True)
        after = {p.relative_to(self.root): p.read_bytes()
                 for p in self.root.rglob("*") if p.is_file()}
        self.assertEqual(before, after)
        self.assertEqual(after, {Path("apt.conf.d/99-semaprax-ci-network"):
                                b'Acquire::http::Timeout "30";\n'
                                b'Acquire::https::Timeout "30";\n'
                                b'Acquire::Retries "2";\n'
                                b'DPkg::Lock::Timeout "120";\n'})


if __name__ == "__main__":
    unittest.main()
