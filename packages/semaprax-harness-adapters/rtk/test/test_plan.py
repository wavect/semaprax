import os
import unittest

from helpers import RTK, PINNED_SHA256, Scratch, drive, one


class PlanRouting(Scratch):
    def plan(self, argv, **payload):
        r = one("plan", {"argv": argv, "cwd_rel": ".", **payload}, self.ret)
        self.assertEqual(r["status"], "complete", r)
        return r["payload"]

    def assertBypass(self, argv, reason, **payload):
        p = self.plan(argv, **payload)
        self.assertEqual((p["route"], p["reason"]), ("bypass", reason), argv)

    def test_pinned_binary_identity(self):
        import hashlib
        with open(RTK, "rb") as f:
            self.assertEqual(hashlib.sha256(f.read()).hexdigest(), PINNED_SHA256)

    def test_allowlisted_families_route_post_execution(self):
        for argv, fam in [(["git", "diff"], "git-diff"), (["git", "diff", "--cached", "--", "src"], "git-diff"),
                          (["git", "log", "-n", "20"], "git-log"),
                          (["rg", "-n", "TODO", "src"], "rg"), (["grep", "-rn", "TODO", "src"], "grep"),
                          (["find", "src", "-name", "*.txt"], "find"), (["cargo", "test"], "cargo-test"),
                          (["pytest", "-q"], "pytest")]:
            p = self.plan(argv)
            self.assertEqual((p["route"], p["family"], p["operation"]), ("post-execution", fam, "view"), argv)

    def test_already_wrapped_bypasses_exactly_once(self):
        self.assertBypass([RTK, "git", "status"], "already-wrapped")
        self.assertBypass(["rtk", "cargo", "test"], "already-wrapped")

    def test_existing_external_hook_or_lineage_bypasses(self):
        self.assertBypass(["git", "diff"], "external-hook-owns-rewrite", external_hooks=["rtk"])
        self.assertBypass(["git", "diff"], "external-hook-owns-rewrite", lineage=["rtk"])

    def test_unsupported_command_bypasses(self):
        self.assertBypass(["npm", "test"], "unsupported-command")
        self.assertBypass(["echo", "hi"], "unsupported-command")
        self.assertBypass(["git", "commit", "-m", "x"], "unsupported-command")
        self.assertBypass(["cargo", "build"], "unsupported-command")
        self.assertBypass(["python", "-m", "pytest"], "unsupported-command")

    def test_small_output_bypasses(self):
        self.assertBypass(["git", "diff"], "small-output", estimated_output_bytes=100)
        p = self.plan(["git", "diff"], estimated_output_bytes=100000)
        self.assertEqual(p["route"], "post-execution")

    def test_machine_output_and_authoritative_envelopes_bypass(self):
        for argv in (["git", "diff", "--name-only"], ["git", "status", "--porcelain"], ["git", "log", "--format=%H"],
                     ["cargo", "test", "--message-format=json"], ["cargo", "test", "--", "--format", "json"],
                     ["rg", "--json", "-n", "x"], ["find", ".", "-print0"], ["git", "diff", "-z"]):
            p = self.plan(argv)
            self.assertEqual(p["route"], "bypass", argv)
            self.assertIn(p["reason"], ("machine-output-flag", "unverified-flag", "unsupported-command"), argv)
        self.assertBypass(["semaprax", "check", "x.spx"], "authoritative-envelope")
        self.assertBypass(["/opt/bin/semaprax-harness", "exec"], "authoritative-envelope")

    def test_shell_syntax_and_malformed_argv_bypass(self):
        self.assertBypass(["git", "status", "|", "wc"], "shell-syntax")
        self.assertBypass(["FOO=1", "cargo", "test"], "shell-syntax")
        self.assertBypass(["cargo", "test", "&&", "git", "push"], "shell-syntax")
        self.assertBypass([], "malformed-argv")
        self.assertBypass(["git", ""], "malformed-argv")

    def test_unverified_flags_bypass(self):
        self.assertBypass(["git", "-C", "x", "status"], "unverified-flag")
        self.assertBypass(["git", "diff", "--stat"], "unverified-flag")
        self.assertBypass(["git", "log", "--oneline"], "unverified-flag")
        self.assertBypass(["find", ".", "-exec", "rm", "{}", ";"], "shell-syntax")
        self.assertBypass(["find", ".", "-delete"], "machine-output-flag")
        self.assertBypass(["rg", "x"], "unparsable-output-shape")
        self.assertBypass(["rg", "-nl", "x"], "unparsable-output-shape")

    def test_wrapper_requires_explicit_opt_in_and_incomplete_recovery_is_bypass(self):
        self.assertBypass(["ls", "-la"], "wrapper-recovery-incomplete")
        self.assertBypass(["git", "status"], "wrapper-recovery-incomplete")
        p = self.plan(["ls", "-la"], config={"allow_wrapper": True})
        self.assertEqual(p["route"], "wrapped")
        self.assertEqual(p["argv"], [RTK, "ls", "-la"])
        self.assertEqual(p["recovery"]["coverage"], "failure-or-truncation-only")
        self.assertEqual(p["resolves_via"], "PATH")
        self.assertTrue(p["env"]["RTK_RECALL_DB"].startswith(self.ret))
        self.assertEqual(p["env"]["RTK_TELEMETRY_DISABLED"], "1")
        # absolute argv[0]: rtk would resolve through PATH, so it could substitute the executable
        self.assertEqual(self.plan(["/bin/ls"], config={"allow_wrapper": True})["route"], "bypass")
        p = self.plan(["git", "diff"], config={"allow_wrapper": True}, form="wrapper")
        self.assertEqual((p["route"], p["argv"][:3]), ("wrapped", [RTK, "git", "diff"]))

    def test_rtk_missing_or_wrong_version_is_unavailable_not_launch(self):
        r = one("plan", {"argv": ["git", "diff"]}, self.ret, upstream="/nonexistent/rtk")
        self.assertEqual(r["status"], "unavailable")
        fake = os.path.join(self.tmp, "rtk")
        with open(fake, "w") as f:
            f.write("#!/bin/sh\necho 'rtk 9.9.9'\n")
        os.chmod(fake, 0o755)
        r = one("view", {"argv": ["git", "diff"], "stdout_b64": "", "min_bytes": 0}, self.ret, upstream=fake)
        self.assertEqual((r["status"], r["diagnostics"][0]["code"]), ("unavailable", "rtk-version"))

    def test_unknown_operation_is_unsupported(self):
        self.assertEqual(one("recover", {}, self.ret)["status"], "unsupported")


if __name__ == "__main__":
    unittest.main()
