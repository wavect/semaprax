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
