#!/usr/bin/env python3
"""Fixture tests for scripts/install.sh (docs/INSTALLER-V1.md).

Run: /usr/bin/python3 scripts/test-install-sh.py

Every test builds fake releases in a temporary directory (stub `semaprax` and
`semapraxd` shell scripts packed into correctly named archives) and runs the
installer through SEMAPRAX_INSTALL_DOWNLOAD_BASE=file://... with HOME pointed at
a temporary directory. The real home directory is never touched. Platform
detection is driven by the test-only SEMAPRAX_INSTALL_TEST_UNAME_S / _UNAME_M /
_LIBC / _ROSETTA overrides, so results do not depend on the host.
"""

import hashlib
import http.server
import io
import json
import os
import re
import shutil
import stat
import subprocess
import tarfile
import tempfile
import threading
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
INSTALL_SH = HERE / "install.sh"
RECONCILE = HERE / "release-reconcile.py"

LINUX = "x86_64-unknown-linux-gnu"
FIVE_TARGETS = {
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
}
WINDOWS = "x86_64-pc-windows-msvc"

# Commands the installer may use. The PATH given to it contains only these
# (symlinked), so `gh` is absent unless a test adds a stub.
TOOLS = [
    "tar", "awk", "sed", "grep", "tr", "mkdir", "mv", "ln", "rm", "rmdir", "cat",
    "wc", "dirname", "basename", "readlink", "find", "sort", "head", "cmp", "curl",
    "sha256sum", "shasum", "uname", "getconf", "sysctl", "gzip", "ls", "cp", "chmod",
    "mktemp", "env", "sleep", "touch", "printf", "cut", "xargs", "id",
]


def stub_semaprax(version, mode="ok"):
    if mode == "fail":
        return "#!/bin/sh\necho 'cannot load shared library' >&2\nexit 127\n"
    if mode == "wrongversion":
        version = "0.0.1"
    return (
        "#!/bin/sh\n"
        'case "$1" in\n'
        '  version) echo \'{"schema":"semaprax.version.v1","version":"%s","commit":"0000000000000000000000000000000000000000","maturity":"beta","rust_min":"1.88"}\' ;;\n'
        '  --version) echo "semaprax %s (stub)" ;;\n'
        "  *) exit 2 ;;\n"
        "esac\n"
    ) % (version, version)


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


class Release:
    """One fixture release directory: <base>/<tag>/..."""

    def __init__(self, base, tag, target=LINUX, smoke="ok", members=None, att=True,
                 manifest_style="pretty", installers=False):
        self.base, self.tag, self.target = Path(base), tag, target
        self.dir = self.base / tag
        self.dir.mkdir(parents=True, exist_ok=True)
        self.version = tag[1:]
        self.top = "semaprax-%s-%s" % (tag, target)
        self.name = self.top + ".tar.gz"
        self.archive = self.dir / self.name
        self.manifest_style = manifest_style
        self.installers = installers
        if members is None:
            members = self.default_members(smoke)
        self.write_archive(members)
        self.write_metadata(att)

    def default_members(self, smoke):
        t = self.top
        ex = 0o755
        return [
            (t, "dir", None, ex),
            (t + "/semaprax", "file", stub_semaprax(self.version, smoke).encode(), ex),
            (t + "/semapraxd", "file", b"#!/bin/sh\nexit 0\n", ex),
            (t + "/LICENSE", "file", b"license\n", 0o644),
            (t + "/README.md", "file", b"readme\n", 0o644),
            (t + "/release-manifest.json", "file", b"{}\n", 0o644),
            (t + "/smoke", "dir", None, ex),
            (t + "/smoke/meaning.spx", "file", b"x\n", 0o644),
        ]

    def write_archive(self, members):
        with tarfile.open(self.archive, "w:gz") as tf:
            for name, kind, data, mode in members:
                ti = tarfile.TarInfo(name)
                ti.mode = mode if isinstance(mode, int) else 0o755
                ti.mtime = 0
                if kind == "dir":
                    ti.type = tarfile.DIRTYPE
                    tf.addfile(ti)
                elif kind == "file":
                    ti.size = len(data)
                    tf.addfile(ti, io.BytesIO(data))
                elif kind == "symlink":
                    ti.type = tarfile.SYMTYPE
                    ti.linkname = data
                    tf.addfile(ti)
                elif kind == "hardlink":
                    ti.type = tarfile.LNKTYPE
                    ti.linkname = data
                    tf.addfile(ti)

    def write_metadata(self, att):
        data = self.archive.read_bytes()
        self.sha = sha256_bytes(data)
        other = "semaprax-%s-%s.zip" % (self.tag, WINDOWS)
        other_sha = "f" * 64
        sums = "%s  %s\n%s  %s\n" % (other_sha, other, self.sha, self.name)
        (self.dir / "SHA256SUMS").write_text(sums)
        manifest = {
            "schema": "semaprax.release-manifest.v1",
            "version": self.version,
            "tag": self.tag,
            "commit": "0" * 40,
            "prerelease": False,
            "required_checks": ["verify", "installed-journey"],
            "changelog_section_digest": "sha256:" + "1" * 64,
            "artifacts": [
                {
                    "name": self.name,
                    "platform": self.target,
                    "size": len(data),
                    "digest": "sha256:" + self.sha,
                },
                {
                    "name": other,
                    "platform": WINDOWS,
                    "size": 10,
                    "digest": "sha256:" + other_sha,
                },
            ],
        }
        if self.installers:
            manifest["installers"] = [
                {"name": "install.ps1", "size": 1, "digest": "sha256:" + "2" * 64},
                {"name": "install.sh", "size": 1, "digest": "sha256:" + "3" * 64},
            ]
        self.write_manifest(manifest)
        if att:
            (self.dir / ("release-attestation-%s.json" % self.target)).write_text(
                '{"dummy":"attestation"}\n'
            )

    def write_manifest(self, manifest):
        if self.manifest_style == "compact":
            text = json.dumps(manifest, separators=(",", ":"))
        else:
            text = json.dumps(manifest, indent=2)
        (self.dir / "release-manifest.json").write_text(text + "\n")

    def edit_manifest(self, fn):
        path = self.dir / "release-manifest.json"
        manifest = json.loads(path.read_text())
        fn(manifest)
        self.write_manifest(manifest)

    def edit_sums(self, fn):
        path = self.dir / "SHA256SUMS"
        path.write_text(fn(path.read_text()))

    def truncate_archive(self):
        data = self.archive.read_bytes()
        self.archive.write_bytes(data[: len(data) // 2])


class InstallerCase(unittest.TestCase):
    maxDiff = None

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="spx-install-test-"))
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.home = self.tmp / "home dir"
        self.home.mkdir()
        self.base = self.tmp / "base"  # no spaces: it is part of a file:// URL
        self.base.mkdir()
        self.prefix = self.tmp / "with space" / "semaprax"
        self.farm = self.make_farm(with_gh=None)

    def make_farm(self, with_gh):
        farm = Path(tempfile.mkdtemp(prefix="farm-", dir=self.tmp))
        for tool in TOOLS:
            found = shutil.which(tool)
            if found:
                os.symlink(found, farm / tool)
        if with_gh is not None:
            self.gh_log = self.tmp / "gh.log"
            script = (
                "#!/bin/sh\n"
                'if [ "$1" = attestation ] && [ "${2:-}" = --help ]; then exit 0; fi\n'
                'echo "$@" >> "%s"\n'
                "%s\n"
            ) % (self.gh_log, "exit 0" if with_gh else "echo 'attestation failed' >&2; exit 1")
            gh = farm / "gh"
            gh.write_text(script)
            gh.chmod(0o755)
        return farm

    def run_install(self, *args, release_env=None, farm=None, shell="/bin/zsh",
                    uname_s="Linux", uname_m="x86_64", libc="glibc 2.35", prefix=True,
                    base=None, extra_env=None):
        env = {
            "HOME": str(self.home),
            "PATH": str(farm or self.farm),
            "SHELL": shell,
            "SEMAPRAX_INSTALL_DOWNLOAD_BASE": "file://" + str(base or self.base),
            "SEMAPRAX_INSTALL_TEST_UNAME_S": uname_s,
            "SEMAPRAX_INSTALL_TEST_UNAME_M": uname_m,
            "SEMAPRAX_INSTALL_TEST_LIBC": libc,
            "SEMAPRAX_INSTALL_TEST_ROSETTA": "0",
        }
        if extra_env:
            env.update(extra_env)
        cmd = ["/bin/sh", str(INSTALL_SH)]
        cmd += list(args)
        if prefix:
            cmd += ["--prefix", str(self.prefix)]
        return subprocess.run(
            cmd, env=env, stdin=subprocess.DEVNULL, capture_output=True, text=True,
            timeout=120, cwd=str(self.tmp),
        )

    def install(self, tag, *extra, **kw):
        return self.run_install("--version", tag, *extra, **kw)

    def release(self, tag, **kw):
        return Release(self.base, tag, **kw)

    # Assertions -----------------------------------------------------------

    def assertOk(self, proc):
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)

    def assertFailed(self, proc, text=None):
        self.assertNotEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        out = proc.stdout + proc.stderr
        self.assertNotRegex(out, r"installed semaprax|already installed and active")
        if text:
            self.assertIn(text, out)

    def current_tag(self):
        return os.readlink(self.prefix / "current")

    def receipt(self):
        return json.loads((self.prefix / "install-receipt.json").read_text())

    def staging_leftovers(self):
        return sorted(p.name for p in self.prefix.glob(".*")) if self.prefix.exists() else []

    def installed_version(self):
        out = subprocess.run(
            [str(self.prefix / "bin" / "semaprax"), "--version"],
            capture_output=True, text=True,
        ).stdout
        return out.split()[1]

    def count_blocks(self, path):
        return path.read_text().count("# >>> semaprax installer >>>")

    def assertUntouchedHome(self):
        self.assertEqual(list(self.home.iterdir()), [])


class InstallLifecycle(InstallerCase):
    def test_fresh_install(self):
        rel = self.release("v1.0.0")
        proc = self.install("v1.0.0")
        self.assertOk(proc)
        out = proc.stdout
        self.assertIn("installed semaprax v1.0.0", out)
        self.assertIn("publisher: not verified (gh not found)", out)
        self.assertIn("checksum only", out)
        self.assertIn('export PATH="%s/bin:$PATH"' % self.prefix, out)
        self.assertEqual(self.current_tag(), "versions/v1.0.0")
        for n in ("semaprax", "semapraxd"):
            link = self.prefix / "bin" / n
            self.assertEqual(os.readlink(link), "../current/" + n)
            self.assertTrue(os.access(link, os.X_OK))
        self.assertEqual(self.installed_version(), "1.0.0")
        rc = self.receipt()
        self.assertEqual(rc["schema"], "semaprax.install-receipt.v1")
        self.assertEqual(rc["installer"], "install.sh")
        self.assertEqual((rc["version"], rc["tag"], rc["target"]), ("1.0.0", "v1.0.0", LINUX))
        self.assertEqual(rc["archive_sha256"], rel.sha)
        self.assertEqual(rc["source"], "file://%s/v1.0.0/%s" % (self.base, rel.name))
        self.assertEqual(rc["publisher_verification"], "not-verified")
        self.assertEqual(rc["path_modification"]["kind"], "profile")
        self.assertEqual(rc["path_modification"]["location"], str(self.home / ".zshrc"))
        for f in rc["files"]:
            self.assertTrue(os.path.lexists(self.prefix / f), f)
        self.assertIn("versions/v1.0.0/semapraxd", rc["files"])
        self.assertIn("current", rc["files"])
        self.assertEqual(self.staging_leftovers(), [])
        self.assertEqual(self.count_blocks(self.home / ".zshrc"), 1)

    def test_compact_manifest_and_installers_key(self):
        self.release("v1.0.0", manifest_style="compact", installers=True)
        self.assertOk(self.install("v1.0.0", "--no-modify-path"))

    def test_same_version_rerun_is_idempotent(self):
        self.release("v1.0.0")
        self.assertOk(self.install("v1.0.0"))
        first = (self.prefix / "install-receipt.json").read_text()
        profile = (self.home / ".zshrc").read_text()
        proc = self.install("v1.0.0")
        self.assertOk(proc)
        self.assertIn("already installed and active", proc.stdout)
        self.assertEqual((self.prefix / "install-receipt.json").read_text(), first)
        self.assertEqual((self.home / ".zshrc").read_text(), profile)
        self.assertEqual(self.current_tag(), "versions/v1.0.0")
        self.assertEqual(self.installed_version(), "1.0.0")

    def test_upgrade_then_explicit_downgrade(self):
        self.release("v1.0.0")
        self.release("v2.0.0")
        self.assertOk(self.install("v1.0.0", "--no-modify-path"))
        self.assertOk(self.install("v2.0.0", "--no-modify-path"))
        self.assertEqual(self.current_tag(), "versions/v2.0.0")
        self.assertEqual(self.installed_version(), "2.0.0")
        self.assertEqual(sorted(p.name for p in (self.prefix / "versions").iterdir()), ["v2.0.0"])
        self.assertEqual(self.receipt()["tag"], "v2.0.0")
        self.assertOk(self.install("v1.0.0", "--no-modify-path"))
        self.assertEqual(self.current_tag(), "versions/v1.0.0")
        self.assertEqual(self.installed_version(), "1.0.0")
        self.assertEqual(sorted(p.name for p in (self.prefix / "versions").iterdir()), ["v1.0.0"])
        self.assertEqual(self.staging_leftovers(), [])

    def test_version_without_v_prefix_is_accepted(self):
        self.release("v1.0.0")
        self.assertOk(self.run_install("--version", "1.0.0", "--no-modify-path"))
        self.assertEqual(self.receipt()["tag"], "v1.0.0")

    def test_no_modify_path_touches_nothing_outside_prefix(self):
        self.release("v1.0.0")
        proc = self.install("v1.0.0", "--no-modify-path")
        self.assertOk(proc)
        self.assertUntouchedHome()
        self.assertEqual(self.receipt()["path_modification"], {"kind": "none", "location": None})
        self.assertIn('export PATH="%s/bin:$PATH"' % self.prefix, proc.stdout)

    def test_yes_flag_is_accepted(self):
        self.release("v1.0.0")
        self.assertOk(self.install("v1.0.0", "--yes", "--no-modify-path"))

    def test_help(self):
        proc = self.run_install("--help", prefix=False)
        self.assertOk(proc)
        for word in ("--version", "--prefix", "--no-modify-path", "--uninstall",
                     "--require-publisher-verification", "--yes"):
            self.assertIn(word, proc.stdout)

    def test_unknown_option_and_bad_tag(self):
        self.assertFailed(self.run_install("--bogus"), "unknown option")
        self.assertFailed(self.install("v1.0"), "invalid release tag")
        self.assertFailed(self.install("v1.0.0/../x"), "invalid release tag")

    def test_version_required_with_download_override(self):
        self.assertFailed(self.run_install(), "--version is required")
        self.assertFalse(self.prefix.exists())


class Profile(InstallerCase):
    def test_block_is_idempotent_and_preserves_other_content(self):
        self.release("v1.0.0")
        self.release("v2.0.0")
        zshrc = self.home / ".zshrc"
        zshrc.write_text("export FOO=1\nalias ll='ls -l'\n")
        self.assertOk(self.install("v1.0.0"))
        self.assertOk(self.install("v1.0.0"))
        self.assertOk(self.install("v2.0.0"))
        text = zshrc.read_text()
        self.assertEqual(self.count_blocks(zshrc), 1)
        self.assertEqual(text.count("# <<< semaprax installer <<<"), 1)
        self.assertTrue(text.startswith("export FOO=1\nalias ll='ls -l'\n"))
        self.assertIn('export PATH="%s/bin:$PATH"' % self.prefix, text)

    def test_shell_profile_selection(self):
        self.release("v1.0.0")
        cases = [
            ("/bin/zsh", "Linux", [".zshrc"]),
            ("/bin/bash", "Linux", [".bashrc"]),
            ("/bin/bash", "Darwin", [".bashrc", ".bash_profile"]),
            ("/usr/bin/fish", "Linux", [".config/fish/conf.d/semaprax.fish"]),
            ("/bin/dash", "Linux", [".profile"]),
        ]
        for shell, os_name, files in cases:
            with self.subTest(shell=shell, os=os_name):
                shutil.rmtree(self.home)
                self.home.mkdir()
                shutil.rmtree(self.prefix.parent, ignore_errors=True)
                shutil.rmtree(self.base)
                self.base.mkdir()
                kw = {}
                if os_name == "Darwin":
                    self.release("v1.0.0", target="aarch64-apple-darwin")
                    kw = {"uname_s": "Darwin", "uname_m": "arm64"}
                else:
                    self.release("v1.0.0")
                proc = self.install("v1.0.0", shell=shell, **kw)
                self.assertOk(proc)
                for f in files:
                    self.assertEqual(self.count_blocks(self.home / f), 1, f)
                if shell.endswith("fish"):
                    self.assertIn("set -gx PATH", (self.home / files[0]).read_text())

    def test_uninstall_removes_only_block(self):
        self.release("v1.0.0")
        zshrc = self.home / ".zshrc"
        zshrc.write_text("export FOO=1\n")
        self.assertOk(self.install("v1.0.0"))
        self.assertOk(self.run_install("--uninstall"))
        self.assertEqual(zshrc.read_text(), "export FOO=1\n")


class Uninstall(InstallerCase):
    def test_uninstall_scope(self):
        self.release("v1.0.0")
        outside_bin = self.tmp / "usr-local-bin"
        outside_bin.mkdir()
        (outside_bin / "semaprax").write_text("#!/bin/sh\necho foreign\n")
        project = self.tmp / "my-project"
        project.mkdir()
        (project / "semaprax.toml").write_text("keep\n")
        agent = self.home / ".claude"
        agent.mkdir()
        (agent / "settings.json").write_text("{}\n")
        self.assertOk(self.install("v1.0.0"))
        (self.prefix / "notes.txt").write_text("mine\n")
        proc = self.run_install("--uninstall")
        self.assertOk(proc)
        self.assertIn("uninstalled semaprax", proc.stdout)
        self.assertEqual(sorted(p.name for p in self.prefix.iterdir()), ["notes.txt"])
        self.assertEqual((self.prefix / "notes.txt").read_text(), "mine\n")
        self.assertEqual((outside_bin / "semaprax").read_text(), "#!/bin/sh\necho foreign\n")
        self.assertEqual((project / "semaprax.toml").read_text(), "keep\n")
        self.assertEqual((agent / "settings.json").read_text(), "{}\n")
        self.assertFalse((self.home / ".zshrc").exists())

    def test_uninstall_removes_empty_prefix(self):
        self.release("v1.0.0")
        self.assertOk(self.install("v1.0.0"))
        self.assertOk(self.run_install("--uninstall"))
        self.assertFalse(self.prefix.exists())

    def test_uninstall_refuses_without_receipt(self):
        self.prefix.mkdir(parents=True)
        (self.prefix / "keep.txt").write_text("x")
        proc = self.run_install("--uninstall")
        self.assertFailed(proc, "no semaprax install receipt")
        self.assertTrue((self.prefix / "keep.txt").exists())
        missing = self.run_install("--uninstall", prefix=False,
                                   extra_env=None)
        self.assertNotEqual(missing.returncode, 0)

    def test_uninstall_refuses_unsafe_receipt_path(self):
        self.release("v1.0.0")
        self.assertOk(self.install("v1.0.0", "--no-modify-path"))
        victim = self.tmp / "victim.txt"
        victim.write_text("keep")
        receipt = self.prefix / "install-receipt.json"
        text = receipt.read_text().replace('    "current"', '    "../../victim.txt",\n    "current"')
        receipt.write_text(text)
        self.assertFailed(self.run_install("--uninstall"), "unsafe path")
        self.assertTrue(victim.exists())
        self.assertTrue((self.prefix / "bin" / "semaprax").is_symlink())

    def test_uninstall_leaves_other_prefix_block(self):
        self.release("v1.0.0")
        zshrc = self.home / ".zshrc"
        self.assertOk(self.install("v1.0.0", "--no-modify-path"))
        zshrc.write_text(
            "# >>> semaprax installer >>>\nexport PATH=\"/elsewhere/bin:$PATH\"\n"
            "# <<< semaprax installer <<<\n"
        )
        self.assertOk(self.run_install("--uninstall"))
        self.assertIn("/elsewhere/bin", zshrc.read_text())


class Refusals(InstallerCase):
    def test_nonempty_prefix_without_receipt(self):
        self.release("v1.0.0")
        self.prefix.mkdir(parents=True)
        (self.prefix / "precious.txt").write_text("x")
        proc = self.install("v1.0.0", "--no-modify-path")
        self.assertFailed(proc, "no semaprax install receipt")
        self.assertEqual(sorted(p.name for p in self.prefix.iterdir()), ["precious.txt"])

    def test_empty_existing_prefix_is_fine_and_survives_failure(self):
        self.prefix.mkdir(parents=True)
        rel = self.release("v1.0.0")
        rel.edit_sums(lambda s: s.replace(rel.sha, "0" * 64))
        self.assertFailed(self.install("v1.0.0", "--no-modify-path"))
        self.assertTrue(self.prefix.is_dir())
        self.assertEqual(list(self.prefix.iterdir()), [])

    def test_foreign_bin_is_not_replaced(self):
        self.release("v1.0.0")
        self.release("v2.0.0")
        self.assertOk(self.install("v1.0.0", "--no-modify-path"))
        link = self.prefix / "bin" / "semaprax"
        link.unlink()
        link.write_text("#!/bin/sh\necho foreign\n")
        proc = self.install("v2.0.0", "--no-modify-path")
        self.assertFailed(proc, "not owned by this installer")
        self.assertEqual(link.read_text(), "#!/bin/sh\necho foreign\n")
        self.assertEqual(self.current_tag(), "versions/v1.0.0")

    def test_current_that_is_a_directory(self):
        self.release("v1.0.0")
        self.prefix.mkdir(parents=True)
        (self.prefix / "install-receipt.json").write_text(
            '{\n  "schema": "semaprax.install-receipt.v1",\n  "tag": "v0.0.1",\n'
            '  "archive_sha256": "x",\n  "files": [\n  ]\n}\n'
        )
        (self.prefix / "current").mkdir()
        proc = self.install("v1.0.0", "--no-modify-path")
        self.assertFailed(proc, "not a symbolic link")
        self.assertTrue((self.prefix / "current").is_dir())

    def test_prefix_is_a_file(self):
        self.prefix.parent.mkdir(parents=True)
        self.prefix.write_text("x")
        self.release("v1.0.0")
        self.assertFailed(self.install("v1.0.0"), "not a directory")

    def test_prefix_with_dollar_is_rejected(self):
        self.release("v1.0.0")
        proc = self.run_install("--version", "v1.0.0", "--prefix", str(self.tmp / "a$b"),
                                prefix=False)
        self.assertFailed(proc, "may not contain")

    def test_same_tag_different_archive_is_refused(self):
        rel = self.release("v1.0.0")
        self.assertOk(self.install("v1.0.0", "--no-modify-path"))
        shutil.rmtree(self.base / "v1.0.0")
        self.release("v1.0.0", members=[
            (rel.top, "dir", None, 0o755),
            (rel.top + "/semaprax", "file", stub_semaprax("1.0.0").encode() + b"# changed\n", 0o755),
            (rel.top + "/semapraxd", "file", b"#!/bin/sh\n", 0o755),
        ])
        self.assertFailed(self.install("v1.0.0", "--no-modify-path"), "different archive")
        self.assertEqual(self.installed_version(), "1.0.0")


class Verification(InstallerCase):
    def assertFreshFailure(self, proc, text):
        self.assertFailed(proc, text)
        self.assertFalse(self.prefix.exists(), "a failed fresh install must leave no prefix")
        self.assertUntouchedHome()

    def test_checksum_mismatch(self):
        rel = self.release("v1.0.0")
        rel.edit_sums(lambda s: s.replace(rel.sha, "0" * 64))
        self.assertFreshFailure(self.install("v1.0.0"), "checksum mismatch")

    def test_missing_sums_line(self):
        rel = self.release("v1.0.0")
        rel.edit_sums(lambda s: "\n".join(l for l in s.splitlines() if rel.name not in l) + "\n")
        self.assertFreshFailure(self.install("v1.0.0"), "no line for")

    def test_duplicate_sums_line(self):
        rel = self.release("v1.0.0")
        rel.edit_sums(lambda s: s + "%s  %s\n" % (rel.sha, rel.name))
        self.assertFreshFailure(self.install("v1.0.0"), "more than one line")

    def test_missing_sums_file(self):
        rel = self.release("v1.0.0")
        (rel.dir / "SHA256SUMS").unlink()
        self.assertFreshFailure(self.install("v1.0.0"), "SHA256SUMS")

    def test_manifest_mismatches(self):
        cases = {
            "size": (lambda m: m["artifacts"][0].__setitem__("size", 12345), "size"),
            "digest": (lambda m: m["artifacts"][0].__setitem__("digest", "sha256:" + "0" * 64), "digest"),
            "platform": (lambda m: m["artifacts"][0].__setitem__("platform", "aarch64-apple-darwin"), "expected"),
            "tag": (lambda m: m.__setitem__("tag", "v9.9.9"), "tag"),
            "entry": (lambda m: m.__setitem__("artifacts", m["artifacts"][1:]), "no artifacts entry"),
            "schema": (lambda m: m.__setitem__("schema", "other"), "schema"),
            "duplicate": (lambda m: m["artifacts"].append(dict(m["artifacts"][0])), "more than once"),
        }
        for label, (edit, text) in cases.items():
            with self.subTest(label):
                shutil.rmtree(self.base, ignore_errors=True)
                self.base.mkdir()
                self.release("v1.0.0").edit_manifest(edit)
                self.assertFreshFailure(self.install("v1.0.0"), text)

    def test_missing_or_malformed_manifest(self):
        rel = self.release("v1.0.0")
        (rel.dir / "release-manifest.json").write_text("not json at all")
        self.assertFreshFailure(self.install("v1.0.0"), "schema")
        (rel.dir / "release-manifest.json").unlink()
        self.assertFreshFailure(self.install("v1.0.0"), "release-manifest.json")

    def test_missing_attestation(self):
        rel = self.release("v1.0.0", att=False)
        self.assertFreshFailure(self.install("v1.0.0"), "release-attestation-%s.json" % rel.target)

    def test_empty_attestation(self):
        rel = self.release("v1.0.0")
        (rel.dir / ("release-attestation-%s.json" % rel.target)).write_text("")
        self.assertFreshFailure(self.install("v1.0.0"), "empty")

    def test_only_selected_assets_are_requested(self):
        # Other hosts' archives and attestations are absent; install still works.
        rel = self.release("v1.0.0")
        self.assertEqual(
            sorted(p.name for p in rel.dir.iterdir()),
            sorted([rel.name, "SHA256SUMS", "release-manifest.json",
                    "release-attestation-%s.json" % LINUX]),
        )
        self.assertOk(self.install("v1.0.0", "--no-modify-path"))

    def test_wrong_top_level_dir(self):
        t = "semaprax-v1.0.0-%s" % LINUX
        members = [("other-dir", "dir", None, 0o755),
                   ("other-dir/semaprax", "file", stub_semaprax("1.0.0").encode(), 0o755),
                   ("other-dir/semapraxd", "file", b"x", 0o755)]
        self.release("v1.0.0", members=members)
        self.assertFreshFailure(self.install("v1.0.0"), "outside %s/" % t)

    def test_unsafe_members(self):
        t = "semaprax-v1.0.0-%s" % LINUX
        good = [(t, "dir", None, 0o755),
                (t + "/semaprax", "file", stub_semaprax("1.0.0").encode(), 0o755),
                (t + "/semapraxd", "file", b"x", 0o755)]
        bad = {
            "dotdot": (t + "/../evil", "file", b"x", 0o644),
            "dotdot-deep": (t + "/a/../../evil", "file", b"x", 0o644),
            "absolute": ("/tmp/evil", "file", b"x", 0o644),
            "symlink": (t + "/link", "symlink", "/etc/passwd", 0o777),
            "hardlink": (t + "/hard", "hardlink", t + "/semaprax", 0o644),
        }
        for label, member in bad.items():
            with self.subTest(label):
                shutil.rmtree(self.base, ignore_errors=True)
                self.base.mkdir()
                self.release("v1.0.0", members=good + [member])
                proc = self.install("v1.0.0")
                self.assertFreshFailure(proc, "")
                self.assertFalse((self.tmp / "evil").exists())

    def test_missing_executable(self):
        t = "semaprax-v1.0.0-%s" % LINUX
        self.release("v1.0.0", members=[
            (t, "dir", None, 0o755),
            (t + "/semaprax", "file", stub_semaprax("1.0.0").encode(), 0o755),
        ])
        self.assertFreshFailure(self.install("v1.0.0"), "missing the executable semapraxd")

    def test_non_executable_member(self):
        t = "semaprax-v1.0.0-%s" % LINUX
        self.release("v1.0.0", members=[
            (t, "dir", None, 0o755),
            (t + "/semaprax", "file", stub_semaprax("1.0.0").encode(), 0o644),
            (t + "/semapraxd", "file", b"x", 0o755),
        ])
        self.assertFreshFailure(self.install("v1.0.0"), "missing the executable semaprax")

    def test_failed_staged_smoke_keeps_prior_install(self):
        self.release("v1.0.0")
        self.assertOk(self.install("v1.0.0"))
        before = (self.prefix / "install-receipt.json").read_text()
        for mode in ("fail", "wrongversion"):
            with self.subTest(mode):
                shutil.rmtree(self.base / "v2.0.0", ignore_errors=True)
                self.release("v2.0.0", smoke=mode)
                proc = self.install("v2.0.0")
                self.assertFailed(proc, "staged smoke failed")
                self.assertEqual(self.current_tag(), "versions/v1.0.0")
                self.assertEqual(self.installed_version(), "1.0.0")
                self.assertEqual((self.prefix / "install-receipt.json").read_text(), before)
                self.assertFalse((self.prefix / "versions" / "v2.0.0").exists())
                self.assertEqual(self.staging_leftovers(), [])

    def test_failed_smoke_on_fresh_install_leaves_nothing(self):
        self.release("v1.0.0", smoke="fail")
        self.assertFreshFailure(self.install("v1.0.0"), "staged smoke failed")

    def test_interrupted_download_keeps_prior_install(self):
        self.release("v1.0.0")
        self.assertOk(self.install("v1.0.0"))
        rel = self.release("v2.0.0")
        rel.truncate_archive()
        proc = self.install("v2.0.0")
        self.assertFailed(proc)
        self.assertEqual(self.current_tag(), "versions/v1.0.0")
        self.assertEqual(self.installed_version(), "1.0.0")
        self.assertEqual(self.receipt()["tag"], "v1.0.0")
        self.assertEqual(self.staging_leftovers(), [])
        self.assertNotIn("v2.0.0", proc.stdout.split("installed semaprax")[-1] if "installed semaprax" in proc.stdout else "")

    def test_missing_archive_download(self):
        rel = self.release("v1.0.0")
        rel.archive.unlink()
        self.assertFreshFailure(self.install("v1.0.0"), "could not download")

    def test_profile_untouched_on_verification_failure(self):
        rel = self.release("v1.0.0")
        rel.edit_sums(lambda s: s.replace(rel.sha, "0" * 64))
        self.assertFailed(self.install("v1.0.0"))
        self.assertUntouchedHome()


class PublisherVerification(InstallerCase):
    def test_require_flag_without_gh_fails_before_download(self):
        self.release("v1.0.0")
        proc = self.install("v1.0.0", "--require-publisher-verification")
        self.assertFailed(proc, "--require-publisher-verification needs the GitHub CLI")
        self.assertFalse(self.prefix.exists())

    def test_verified_with_gh_uses_exact_arguments(self):
        rel = self.release("v1.0.0")
        farm = self.make_farm(with_gh=True)
        proc = self.install("v1.0.0", "--require-publisher-verification", farm=farm)
        self.assertOk(proc)
        self.assertIn("publisher: verified", proc.stdout)
        self.assertEqual(self.receipt()["publisher_verification"], "verified")
        call = self.gh_log.read_text().strip()
        self.assertTrue(call.startswith("attestation verify "), call)
        self.assertIn(rel.name + " --bundle", call)
        self.assertIn("--bundle", call)
        self.assertIn("release-attestation-%s.json" % LINUX, call)
        self.assertIn("--repo wavect/semaprax", call)
        self.assertIn("--signer-workflow wavect/semaprax/.github/workflows/ci.yml", call)
        self.assertIn("--source-ref refs/tags/v1.0.0", call)
        self.assertIn("--deny-self-hosted-runners", call)

    def test_gh_failure_blocks_install(self):
        self.release("v1.0.0")
        farm = self.make_farm(with_gh=False)
        proc = self.install("v1.0.0", farm=farm)
        self.assertFailed(proc, "publisher verification failed")
        self.assertFalse(self.prefix.exists())

    def test_gh_failure_keeps_prior_install(self):
        self.release("v1.0.0")
        self.assertOk(self.install("v1.0.0", "--no-modify-path"))
        self.release("v2.0.0")
        farm = self.make_farm(with_gh=False)
        self.assertFailed(self.install("v2.0.0", "--no-modify-path", farm=farm))
        self.assertEqual(self.installed_version(), "1.0.0")


class Platform(InstallerCase):
    def assertSource(self, proc):
        out = proc.stderr
        self.assertIn("cargo install --locked --git https://github.com/wavect/semaprax --tag", out)
        self.assertIn("semaprax-toolchain --bin semaprax-full", out)
        self.assertIn("STANDALONE", out)
        self.assertFalse(self.prefix.exists())

    def test_supported_mappings(self):
        cases = [
            ("Linux", "x86_64", "glibc 2.35", "0", "x86_64-unknown-linux-gnu"),
            ("Linux", "x86_64", "glibc 2.39", "0", "x86_64-unknown-linux-gnu"),
            ("Linux", "aarch64", "glibc 2.35", "0", "aarch64-unknown-linux-gnu"),
            ("Linux", "arm64", "glibc 3.0", "0", "aarch64-unknown-linux-gnu"),
            ("Darwin", "arm64", "", "0", "aarch64-apple-darwin"),
            ("Darwin", "x86_64", "", "1", "aarch64-apple-darwin"),  # Rosetta
            ("Darwin", "x86_64", "", "0", "x86_64-apple-darwin"),
        ]
        for s, m, libc, rosetta, target in cases:
            with self.subTest(s=s, m=m, libc=libc, rosetta=rosetta):
                shutil.rmtree(self.base, ignore_errors=True)
                self.base.mkdir()
                shutil.rmtree(self.prefix.parent, ignore_errors=True)
                self.release("v1.0.0", target=target)
                proc = self.install("v1.0.0", "--no-modify-path", uname_s=s, uname_m=m,
                                    libc=libc, extra_env={"SEMAPRAX_INSTALL_TEST_ROSETTA": rosetta})
                self.assertOk(proc)
                self.assertEqual(self.receipt()["target"], target)
                self.assertIn("target " + target, proc.stdout)

    def test_unsupported_platforms(self):
        cases = [
            ("Darwin", "ppc64", "", "unsupported macOS CPU"),
            ("Linux", "riscv64", "glibc 2.39", "unsupported Linux CPU"),
            ("Linux", "i686", "glibc 2.39", "unsupported Linux CPU"),
            ("FreeBSD", "amd64", "", "unsupported operating system: FreeBSD"),
            ("MINGW64_NT-10.0", "x86_64", "", "unsupported operating system"),
        ]
        for s, m, libc, text in cases:
            with self.subTest(s=s, m=m):
                proc = self.install("v1.0.0", uname_s=s, uname_m=m, libc=libc)
                self.assertFailed(proc, text)
                self.assertSource(proc)
                self.assertIn("--tag v1.0.0", proc.stderr)

    def test_musl_is_refused_not_treated_as_gnu(self):
        proc = self.install("v1.0.0", libc="musl")
        self.assertFailed(proc, "musl")
        self.assertSource(proc)

    def test_old_glibc_is_refused(self):
        for libc in ("glibc 2.31", "glibc 2.34", "glibc 1.99"):
            with self.subTest(libc=libc):
                proc = self.install("v1.0.0", libc=libc)
                self.assertFailed(proc, "older than the supported baseline glibc 2.35")
                self.assertSource(proc)

    def test_unknown_libc_is_refused(self):
        proc = self.install("v1.0.0", libc="")
        self.assertFailed(proc, "cannot confirm GNU libc")
        self.assertSource(proc)

    def test_unparseable_glibc_is_refused(self):
        proc = self.install("v1.0.0", libc="glibc banana")
        self.assertFailed(proc)
        self.assertSource(proc)

    def test_target_list_matches_release_inventory(self):
        text = INSTALL_SH.read_text()
        match = re.search(r'^SUPPORTED_TARGETS="([^"]*)"$', text, re.M)
        self.assertIsNotNone(match)
        unix = set(match.group(1).split())
        self.assertEqual(len(unix), len(match.group(1).split()))
        everything = unix | {WINDOWS}  # install.ps1 owns the Windows target
        self.assertEqual(everything, FIVE_TARGETS)
        source = RECONCILE.read_text()
        tuple_text = re.search(r"ARCHIVE_TARGETS = \((.*?)\n\)", source, re.S).group(1)
        archive = set(re.findall(r'\("([a-z0-9_.-]+)",\s*"(?:tar\.gz|zip)"\)', tuple_text))
        self.assertTrue(archive)
        # The coordinator tightens this to equality once the tuple has five.
        self.assertTrue(archive <= everything, archive - everything)
        for target in unix:
            self.assertRegex(target, r"-(linux-gnu|apple-darwin)$")


class LatestResolution(InstallerCase):
    def test_latest_is_resolved_once(self):
        self.release("v1.2.3")
        hits = []

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                hits.append(self.path)
                if self.path == "/releases/latest":
                    self.send_response(302)
                    self.send_header("Location", "/releases/tag/v1.2.3")
                    self.end_headers()
                elif self.path == "/releases/tag/v1.2.3":
                    self.send_response(200)
                    self.send_header("Content-Length", "2")
                    self.end_headers()
                    self.wfile.write(b"ok")
                else:
                    self.send_response(404)
                    self.end_headers()

            def log_message(self, *a):
                pass

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        url = "http://127.0.0.1:%d/releases/latest" % server.server_port
        proc = self.run_install("--no-modify-path",
                                extra_env={"SEMAPRAX_INSTALL_LATEST_URL": url})
        self.assertOk(proc)
        self.assertEqual(hits.count("/releases/latest"), 1, hits)
        self.assertEqual(self.receipt()["tag"], "v1.2.3")
        self.assertIn("latest release resolved once: v1.2.3", proc.stdout)

    def test_latest_that_does_not_redirect_to_a_tag_fails(self):
        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(200)
                self.send_header("Content-Length", "0")
                self.end_headers()

            def log_message(self, *a):
                pass

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        url = "http://127.0.0.1:%d/releases/latest" % server.server_port
        proc = self.run_install("--no-modify-path",
                                extra_env={"SEMAPRAX_INSTALL_LATEST_URL": url})
        self.assertFailed(proc, "did not redirect to a tag page")


class StaticChecks(unittest.TestCase):
    def test_shellcheck(self):
        shellcheck = shutil.which("shellcheck")
        if not shellcheck:
            self.skipTest("shellcheck not installed")
        proc = subprocess.run([shellcheck, "-s", "sh", str(INSTALL_SH)],
                              capture_output=True, text=True)
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        journey = HERE / "install-journey.sh"
        if journey.exists():
            proc = subprocess.run([shellcheck, "-s", "sh", str(journey)],
                                  capture_output=True, text=True)
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)

    def test_posix_sh_parses_in_available_shells(self):
        for shell in ("/bin/sh", shutil.which("dash"), shutil.which("bash")):
            if shell:
                with self.subTest(shell=shell):
                    proc = subprocess.run([shell, "-n", str(INSTALL_SH)],
                                          capture_output=True, text=True)
                    self.assertEqual(proc.returncode, 0, proc.stderr)

    def test_installer_has_no_sudo_or_network_install_of_tools(self):
        text = INSTALL_SH.read_text()
        self.assertNotRegex(text, r"(?m)^\s*(sudo|doas)\b")
        self.assertNotIn("read -p", text)


if __name__ == "__main__":
    unittest.main()
