"""Pure provisioning refusals; no network, CLI start, credentials or model call."""
import copy
import pathlib
import sys
import tempfile
import unittest
from unittest import mock

SUITE = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(SUITE))
from agent import pilot_linux_provider as provider


class LinuxProviderTests(unittest.TestCase):
    def test_exact_release_manifest_refuses_other_platform_version_or_hash(self):
        manifest = {"version": provider.VERSION, "platforms": {"linux-arm64": {
            "binary": "claude", "checksum": provider.BINARY_SHA256, "size": provider.BINARY_BYTES}}}
        self.assertEqual(provider.manifest_admission(provider.p.canonical(manifest)), manifest)
        for key, value in (("binary", "installer.sh"), ("checksum", "0" * 64), ("size", provider.BINARY_BYTES - 1)):
            changed = copy.deepcopy(manifest)
            changed["platforms"]["linux-arm64"][key] = value
            with self.assertRaisesRegex(ValueError, "release_identity_refused"):
                provider.manifest_admission(provider.p.canonical(changed))
        manifest["version"] = "latest"
        with self.assertRaisesRegex(ValueError, "release_identity_refused"):
            provider.manifest_admission(provider.p.canonical(manifest))

    def test_command_vocabulary_excludes_model_and_token_operations(self):
        for arguments in (["--version"], ["auth", "status", "--json"], ["auth", "login", "--claudeai"]):
            result = provider.exec_arguments(arguments)
            self.assertIn("/opt/claude/claude", result)
            self.assertIn("-i", result)
            self.assertNotIn("--interactive", result)
        for arguments in (["--print", "hello"], ["setup-token"], ["auth", "login", "--token", "secret"], []):
            with self.assertRaisesRegex(ValueError, "provision_command_refused"):
                provider.exec_arguments(arguments)
        self.assertIn("--tty", provider.exec_arguments(["auth", "login", "--claudeai"], interactive=True))

    def test_launch_uses_only_new_private_home_public_binary_and_scratch(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            args = provider.launch_arguments(root, {"image": "reviewed-image"})
            mounts = [args[i + 1] for i, value in enumerate(args) if value == "--mount"]
            self.assertEqual(mounts, [f"type=bind,source={root / 'public'},target=/opt/claude,readonly",
                                      f"type=bind,source={root / 'private-home'},target=/home/pilot",
                                      f"type=bind,source={root / 'scratch'},target=/work"])
            self.assertEqual(args[args.index("--network") + 1], "default")
            self.assertIn("--read-only", args)
            self.assertIn("DISABLE_UPDATES=1", args)
            self.assertNotIn("ANTHROPIC_API_KEY", " ".join(args))
            self.assertNotIn("CLAUDE_CODE_OAUTH_TOKEN", " ".join(args))
            with self.assertRaisesRegex(ValueError, "launch_identity_refused"):
                provider.launch_arguments(root, {"image": "reviewed-image"}, "../escape")

    def test_existing_root_refuses_before_any_verification_or_mutation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            (root / "sentinel").write_text("unchanged")
            with self.assertRaisesRegex(ValueError, "fresh_private_provider_root_required"):
                provider.provision("/nonexistent", root, "/nonexistent", "0" * 64, "/nonexistent")
            self.assertEqual((root / "sentinel").read_text(), "unchanged")
            self.assertEqual(list(root.iterdir()), [root / "sentinel"])


    def test_login_is_terminal_only_and_launcher_is_private_create_new(self):
        with mock.patch.object(provider, "checked_root", return_value=(None, None, None, None)), \
                mock.patch.object(provider.sys.stdin, "isatty", return_value=False):
            with self.assertRaisesRegex(ValueError, "login_requires_user_terminal"):
                provider.login("/unused", "0" * 64)
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            provider.write_login_launcher(root, "0" * 64)
            script = root / "login.sh"
            self.assertEqual(script.stat().st_mode & 0o777, 0o700)
            self.assertIn("agent.pilot_linux_provider login", script.read_text())
            self.assertNotIn("setup-token", script.read_text())
            with self.assertRaises(FileExistsError):
                provider.write_login_launcher(root, "0" * 64)


if __name__ == "__main__":
    unittest.main()
