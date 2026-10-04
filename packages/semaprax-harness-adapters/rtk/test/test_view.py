import os
import shutil
import subprocess
import unittest

from helpers import Scratch, b64, measure, one


class PostExecutionView(Scratch):
    def view(self, argv, out, err=b"", **kw):
        r = one("view", {"argv": argv, "stdout_b64": b64(out), "stderr_b64": b64(err), **kw}, self.ret)
        return r

    def text(self, r):
        self.assertEqual(r["status"], "complete", r)
        return r["payload"]["view"]["text"]

    def fake_cargo(self, mode):
        r = self.run_cmd(["cargo", "test"], env=self.env(FAKE_CARGO_MODE=mode))
        return r

    def test_family_byte_reductions_measured_independently(self):
        repo = self.make_repo()
        pytest_bin = shutil.which("pytest")
        cases = [["git", "diff"], ["git", "log"], ["rg", "-n", "TODO", "src"], ["grep", "-rn", "TODO", "src"],
                 ["find", "src", "-name", "*.txt"], ["cargo", "test"]]
        for argv in cases:
            r = self.run_cmd(argv, cwd=repo, env=self.env(FAKE_CARGO_MODE="ok"))
            res = self.view(argv, r.stdout, r.stderr, min_bytes=0)
            text = self.text(res)
            raw = len(r.stdout) + len(r.stderr)
            view = len(text.encode())
            measure(" ".join(argv), raw_bytes=raw, view_bytes=view, saved_bytes=raw - view,
                    lossless=res["payload"]["view"]["lossless"], source="adapter-view")
            self.assertLess(view, raw, argv)
            self.assertFalse(res["payload"]["view"]["lossless"])
            self.assertGreater(res["payload"]["view"]["omissions"], 0)

    def test_raw_recovery_is_host_owned_and_runs_command_once(self):
        # The adapter never executes the command: the view op only reads captured bytes.
        r = self.fake_cargo("fail")
        self.assertEqual(self.runs(), 1)
        res = self.view(["cargo", "test"], r.stdout, r.stderr, recovery_handle="rec-1")
        self.assertEqual(res["payload"]["view"]["recovery_handle"], "rec-1")
        for _ in range(3):  # repeated views over the same retained bytes
            self.view(["cargo", "test"], r.stdout, r.stderr)
        self.assertEqual(self.runs(), 1)

    def test_planted_failure_survives_in_view(self):
        r = self.fake_cargo("fail")
        self.assertEqual(r.returncode, 101)
        self.assertIn("CRITICAL-PLANTED-7731", self.text(self.view(["cargo", "test"], r.stdout, r.stderr)))

    def test_planted_error_inside_large_stderr_noise_is_kept_or_flagged(self):
        r = self.fake_cargo("ok")
        noise = b"warning: unused variable in some module path here\n" * 3000
        err = noise + b"error: CRITICAL-STDERR-55 disk on fire\n" + noise
        res = self.view(["cargo", "test"], r.stdout, err)
        self.assertIn("CRITICAL-STDERR-55", self.text(res))
        self.assertFalse(res["payload"]["view"]["lossless"])

    def test_planted_failure_in_real_pytest(self):
        pytest_bin = shutil.which("pytest")
        if not pytest_bin:
            self.skipTest("pytest not installed")
        d = os.path.join(self.tmp, "py")
        os.makedirs(d)
        with open(os.path.join(d, "test_big.py"), "w") as f:
            f.write("import pytest\n@pytest.mark.parametrize('i', range(400))\ndef test_ok(i):\n    assert i >= 0\n"
                    "def test_planted():\n    assert False, 'CRITICAL-PY-9921'\n")
        r = self.run_cmd(["pytest"], cwd=d)
        self.assertEqual(r.returncode, 1)
        res = self.view(["pytest"], r.stdout, r.stderr)
        self.assertIn("CRITICAL-PY-9921", self.text(res))
        raw, view = len(r.stdout) + len(r.stderr), len(self.text(res).encode())
        measure("pytest 400 ok + 1 failing", raw_bytes=raw, view_bytes=view, saved_bytes=raw - view, source="adapter-view")
        self.assertLess(view, raw)

    # ---- HN-12 planted-error matrix: real `rtk pipe` over synthetic-but-faithful tool output ----

    def libtest(self, failing_block, totals="test result: FAILED. 300 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s",
                passing=300):
        out = "running %d tests\n" % (passing + 1) + "".join(f"test tests::case_{i} ... ok\n" for i in range(passing))
        return (out + failing_block + "\n" + totals + "\n").encode()

    def test_nested_multiline_failure_block_is_kept_whole(self):
        block = ("test tests::nested_case ... FAILED\n\nfailures:\n\n---- tests::nested_case stdout ----\n"
                 "thread 'tests::nested_case' panicked at src/lib.rs:41:9:\n"
                 "assertion `left == right` failed: outer context\n  left: Config { name: \"alpha\",\n    inner: Inner {\n"
                 "        depth: 3, tag: \"CRITICAL-NEST-31\" } }\n right: Config { name: \"beta\" }\n"
                 "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n\n"
                 "failures:\n    tests::nested_case\n")
        text = self.text(self.view(["cargo", "test"], self.libtest(block), b""))
        for needle in ("CRITICAL-NEST-31", "tests::nested_case", "left: Config", "right: Config", "300 passed", "1 failed"):
            self.assertIn(needle, text)

    def test_long_paths_and_negations_keep_their_meaning(self):
        long_path = "/".join(["very"] * 30 + ["deep", "module"]) + "/lib.rs"
        block = ("test tests::path_case ... FAILED\n\nfailures:\n\n---- tests::path_case stdout ----\n"
                 f"thread 'tests::path_case' panicked at {long_path}:7:5:\n"
                 "assertion failed: !config.is_not_enabled() -- value must NOT be absent (CRITICAL-NEG-8)\n\n"
                 "failures:\n    tests::path_case\n")
        text = self.text(self.view(["cargo", "test"], self.libtest(block), b""))
        self.assertIn(long_path, text)
        self.assertIn("!config.is_not_enabled()", text)
        self.assertIn("must NOT be absent", text)
        self.assertIn("CRITICAL-NEG-8", text)

    def test_critical_stderr_and_unicode_survive_for_cargo_and_pytest(self):
        err = ("warning: unused variable `x`\n" * 3000 + "error[E0308]: mismatched types: 期待 i64, 得到 &str — CRITICAL-ERR-ü9\n"
               + "warning: unused variable `y`\n" * 3000).encode()
        block = "test tests::ünïcode_失败 ... FAILED\n\nfailures:\n\n---- tests::ünïcode_失败 stdout ----\nthread 'x' panicked: 日本語 ❌\n\nfailures:\n    tests::ünïcode_失败\n"
        res = self.view(["cargo", "test"], self.libtest(block), err)
        text = self.text(res)
        for needle in ("CRITICAL-ERR-ü9", "tests::ünïcode_失败", "日本語 ❌"):
            self.assertIn(needle, text)
        py_out = ("=" * 20 + " test session starts " + "=" * 20 + "\n" + "test_big.py " + "." * 400 + "F\n"
                  + "=" * 20 + " FAILURES " + "=" * 20 + "\n_____ test_unicode_ü _____\n    def test_unicode_ü():\n>       assert 'ü' == 'u'\nE       AssertionError: CRITICAL-PY-ü5\n"
                  + "=" * 10 + " short test summary info " + "=" * 10 + "\nFAILED test_big.py::test_unicode_ü - AssertionError: CRITICAL-PY-ü5\n"
                  + "=" * 10 + " 1 failed, 400 passed in 0.31s " + "=" * 10 + "\n").encode()
        text = self.text(self.view(["pytest"], py_out, b"pytest: stderr says CRITICAL-PYERR-4\n"))
        for needle in ("CRITICAL-PY-ü5", "test_unicode_ü", "1 failed", "400 passed", "CRITICAL-PYERR-4"):
            self.assertIn(needle, text)

    def test_test_totals_are_exact_when_everything_passes_and_when_a_filter_matches_nothing(self):
        ok = self.libtest("", totals="test result: ok. 300 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s")
        text = self.text(self.view(["cargo", "test"], ok, b"", min_bytes=0))
        self.assertIn("300 passed", text)
        none = b"running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 301 filtered out; finished in 0.00s\n" * 30
        text = self.text(self.view(["cargo", "test", "nosuch"], none, b"", min_bytes=0))
        self.assertIn("0 passed", text)
        self.assertNotIn("1 failed", text)

    def test_no_match_and_error_status_are_not_blurred(self):
        repo = self.make_repo()
        nomatch = self.run_cmd(["rg", "-n", "ZZZ_NO_SUCH_TOKEN", "src"], cwd=repo)
        self.assertEqual((nomatch.returncode, nomatch.stdout), (1, b""))  # authoritative: exit 1, nothing printed
        v = self.view(["rg", "-n", "ZZZ_NO_SUCH_TOKEN", "src"], nomatch.stdout, nomatch.stderr)["payload"]["view"]
        self.assertEqual((v["text"], v["lossless"]), ("", True))
        bad = self.run_cmd(["rg", "-n", "(unclosed", "src"], cwd=repo)
        self.assertEqual(bad.returncode, 2)
        text = self.text(self.view(["rg", "-n", "(unclosed", "src"], bad.stdout, bad.stderr, min_bytes=0))
        self.assertIn("regex parse error", text)
        self.assertIn("unclosed", text)
        missing = self.run_cmd(["find", "nosuchdir", "-name", "x"], cwd=repo)
        self.assertEqual(missing.returncode, 1)
        text = self.text(self.view(["find", "nosuchdir", "-name", "x"], missing.stdout, missing.stderr, min_bytes=0))
        self.assertIn("nosuchdir", text)  # the error text is delivered; the exit status stays the host's

    def test_ctest_is_admitted_with_failure_and_totals_preserved(self):
        out = "".join(f"  {i + 1}/151 Test #{i + 1}: case_{i} ...................   Passed    0.01 sec\n" for i in range(150))
        out += ("151/151 Test #151: planted_zeta_9f3a ...***Failed    0.02 sec\n\n99% tests passed, 1 tests failed out of 151\n\n"
                "The following tests FAILED:\n\t151 - planted_zeta_9f3a (Failed)\n").encode().decode()
        res = self.view(["ctest"], out.encode(), b"")
        text = self.text(res)
        raw, view = len(out.encode()), len(text.encode())
        measure("ctest 150 pass + 1 fail (synthetic output)", raw_bytes=raw, view_bytes=view, saved_bytes=raw - view, source="adapter-view")
        self.assertLess(view, raw)
        self.assertIn("planted_zeta_9f3a", text)
        self.assertIn("1 failed", text)

    def test_malformed_utf8_is_reported_not_hidden(self):
        r = self.fake_cargo("badutf8")
        self.assertIn(b"\xff\xfe", r.stdout)
        res = self.view(["cargo", "test"], r.stdout, r.stderr, min_bytes=0)
        v = res["payload"]["view"]
        self.assertFalse(v["lossless"])
        self.assertGreaterEqual(v["omissions"], 1)
        self.assertIn("CRITICAL-PLANTED-7731", v["text"])
        # also below the small-output threshold the lossy decode is never reported as lossless
        v = self.view(["git", "diff"], b"ok\n\xff\xfe\n", b"")["payload"]["view"]
        self.assertFalse(v["lossless"])
        self.assertIn("�", v["text"])

    def test_small_output_passes_through_losslessly(self):
        res = self.view(["git", "diff"], b"diff --git a/x b/x\n+one\n")
        v = res["payload"]["view"]
        self.assertTrue(v["lossless"])
        self.assertEqual((v["text"], v["omissions"]), ("diff --git a/x b/x\n+one\n", 0))
        self.assertEqual(res["diagnostics"][0]["code"], "rtk.small-output-bypass")

    def test_negative_saving_case_is_recorded(self):
        # Tiny stdout plus a short stderr: the stream-distinction header makes the view larger than raw.
        out, err = b"src/a.rs:3:fn main() {}\n", b"rg: ./locked: Permission denied\n"
        res = self.view(["rg", "-n", "main"], out, err, min_bytes=0)
        text = self.text(res)
        raw, view = len(out) + len(err), len(text.encode())
        measure("rg -n tiny + stderr (adapter overhead)", raw_bytes=raw, view_bytes=view, saved_bytes=raw - view, source="adapter-view")
        self.assertGreater(view, raw)
        self.assertIn("Permission denied", text)
        self.assertIn("[stderr]", text)

    def test_stderr_of_non_merged_families_stays_separated(self):
        repo = self.make_repo()
        r = self.run_cmd(["rg", "-n", "TODO", "src", "nosuchdir"], cwd=repo)
        self.assertEqual(r.returncode, 2)
        text = self.text(self.view(["rg", "-n", "TODO", "src", "nosuchdir"], r.stdout, r.stderr))
        self.assertIn("\n[stderr]\n", text)
        self.assertIn("nosuchdir", text.split("[stderr]")[1])

    def test_large_input_by_retained_path_and_path_escape_refused(self):
        big = b"INFO noise noise noise\n" * 100000 + b"test result: FAILED. 0 passed; 1 failed; 0 ignored\n"
        with open(os.path.join(self.ret, "stdout.bin"), "wb") as f:
            f.write(big)
        r = one("view", {"argv": ["cargo", "test"], "stdout_path": "stdout.bin"}, self.ret)
        self.assertEqual(r["status"], "complete", r)
        self.assertLess(len(r["payload"]["view"]["text"]), len(big))
        r = one("view", {"argv": ["cargo", "test"], "stdout_path": "../outside"}, self.ret)
        self.assertEqual((r["status"], r["diagnostics"][0]["code"]), ("refused", "path-outside-retention"))

    def test_non_admitted_commands_are_unsupported_not_compressed(self):
        for argv in (["semaprax", "check", "x"], ["git", "diff", "--name-only"], ["cargo", "test", "--message-format=json"],
                     ["npm", "test"], ["git", "status"], [os.path.join(self.bin, "x") + "=1"]):
            r = self.view(argv, b'{"a":1}\n' * 400)
            self.assertEqual(r["status"], "unsupported", argv)

    def test_rtk_state_confined_to_retention(self):
        before = set(os.listdir(self.tmp))
        r = self.fake_cargo("fail")
        self.view(["cargo", "test"], r.stdout, r.stderr, min_bytes=0)
        self.assertEqual(set(os.listdir(self.tmp)) - before, set())
        self.assertTrue(os.path.isdir(os.path.join(self.ret, "rtk-home")))  # adapter's private HOME lives under retention


if __name__ == "__main__":
    unittest.main()
