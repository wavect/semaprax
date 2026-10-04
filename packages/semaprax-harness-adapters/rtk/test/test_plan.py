import os
import unittest

from helpers import RTK, PINNED_SHA256, Scratch, drive, one, write_exec


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
                          (["pytest", "-q"], "pytest"), (["ctest"], "ctest")]:
            p = self.plan(argv)
            self.assertEqual((p["route"], p.get("family"), p.get("operation")), ("post-execution", fam, "view"), (argv, p))

    def test_untested_pipe_filters_stay_bypassed(self):
        # measured against real rtk 0.51.0 (RESEARCH.md section 8): data loss or no gain
        for argv in (["go", "test", "./..."], ["mypy", "src"], ["ruff", "check", "."], ["tsc", "--noEmit"],
                     ["ctest", "-V"], ["ctest", "-j", "4"], ["ctest", "--output-on-failure"]):
            p = self.plan(argv)
            self.assertEqual(p["route"], "bypass", argv)

    def fake_rtk(self, version_line):
        path = os.path.join(self.tmp, "fake-rtk")
        write_exec(path, f"#!/bin/sh\n[ \"$1\" = --version ] && echo '{version_line}' && exit 0\nexit 9\n")
        return path

    def test_newer_or_unknown_rtk_versions_go_through_the_qualifier_not_blind_acceptance(self):
        from rtk_families import QUALIFIED_VERSIONS, Unqualified, qualify
        self.assertEqual(qualify("rtk 0.51.0\n"), "0.51.0")
        for text, reason in (("rtk 0.52.0", "rtk-version-unqualified"), ("rtk 1.0.0", "rtk-version-unqualified"),
                             ("rtk 0.51.0-rc1", "rtk-identity"), ("Rust Type Kit 0.51.0", "rtk-identity"), ("", "rtk-identity")):
            with self.assertRaises(Unqualified) as c:
                qualify(text)
            self.assertEqual(c.exception.reason, reason, text)
        self.assertEqual(sorted(QUALIFIED_VERSIONS), ["0.51.0"])
        newer = self.fake_rtk("rtk 0.52.0")
        r = one("plan", {"argv": ["git", "diff"], "cwd_rel": "."}, self.ret, upstream=newer)
        self.assertEqual((r["status"], r["payload"]["route"], r["payload"]["reason"]),
                         ("complete", "bypass", "rtk-version-unqualified"))
        v = one("view", {"argv": ["cargo", "test"], "stdout": "x\n" * 2000}, self.ret, upstream=newer)
        self.assertEqual((v["status"], v["diagnostics"][0]["code"]), ("unavailable", "rtk-version"))
        self.assertIn("rtk-version-unqualified", v["diagnostics"][0]["message"])
        # the shipped 0.51.0 mapping is unchanged
        self.assertEqual(self.plan(["git", "diff"])["route"], "post-execution")

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
