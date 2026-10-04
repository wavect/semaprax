"""Wrapper form (`rtk <argv>`): parity with the unwrapped command and raw-recovery coverage, real rtk 0.51.0."""
import os
import shutil
import signal
import subprocess
import time
import unittest

from helpers import RTK, Scratch, measure, one


class WrapperParity(Scratch):
    def wrapped_plan(self, argv):
        r = one("plan", {"argv": argv, "form": "wrapper", "config": {"allow_wrapper": True}}, self.ret)
        self.assertEqual(r["payload"]["route"], "wrapped", r)
        return r["payload"]

    def run_pair(self, argv, cwd=None, **env):
        """Run native and planned-wrapper argv once each; return (native, wrapped, plan)."""
        plan = self.wrapped_plan(argv)
        native = self.run_cmd(argv, cwd=cwd, env=self.env(**env))
        wrapped = self.run_cmd(plan["argv"], cwd=cwd, env=self.env(**env, **plan["env"]))
        return native, wrapped, plan

    def recall(self, plan, *args):
        env = {**self.env(), **plan["env"]}
        return subprocess.run([RTK, "recall", *args], env=env, capture_output=True, timeout=30)

    def test_exit_codes_and_native_stderr_agree_across_families(self):
        repo = self.make_repo()
        cases = [
            (["git", "diff"], 0, None), (["git", "log", "-n", "3"], 0, None),
            (["git", "diff", "nosuchrev"], 128, b"nosuchrev"),
            (["rg", "-n", "TODO", "src"], 0, None), (["rg", "-n", "zzzzNOPE", "src"], 1, None),
            (["rg", "-n", "("], 2, b"regex"),
                        (["ls", "-la", "src"], 0, None), (["ls", "nosuchdir"], 1, b"nosuchdir"),
            (["cargo", "test"], 0, None),
        ]
        for argv, want, err_has in cases:
            native, wrapped, _ = self.run_pair(argv, cwd=repo, FAKE_CARGO_MODE="ok")
            self.assertEqual((native.returncode, wrapped.returncode), (want, want), (argv, wrapped.stderr))
            if err_has:
                self.assertIn(err_has, native.stderr)
                self.assertIn(err_has, wrapped.stderr, argv)  # native failure text is preserved on stderr
        # nonzero test run
        native, wrapped, _ = self.run_pair(["cargo", "test"], FAKE_CARGO_MODE="fail")
        self.assertEqual((native.returncode, wrapped.returncode), (101, 101))

    def test_rtk_find_swallows_errors_so_find_has_no_wrapper_mapping(self):
        repo = self.make_repo()
        native = self.run_cmd(["find", "nosuchdir"], cwd=repo)
        wrapped = self.run_cmd([RTK, "find", "nosuchdir"], cwd=repo, env=self.env(RTK_TELEMETRY_DISABLED="1", RTK_DB_PATH=os.path.join(self.ret, "h.db")))
        self.assertEqual(native.returncode, 1)
        self.assertEqual((wrapped.returncode, wrapped.stdout, wrapped.stderr), (0, b"", b""))  # pinned defect
        r = one("plan", {"argv": ["find", "nosuchdir"], "form": "wrapper", "config": {"allow_wrapper": True}}, self.ret)
        self.assertEqual(r["payload"]["route"], "post-execution")  # never wrapped

    def test_real_pytest_exit_codes(self):
        if not shutil.which("pytest"):
            self.skipTest("pytest not installed")
        d = os.path.join(self.tmp, "py")
        os.makedirs(d)
        with open(os.path.join(d, "test_x.py"), "w") as f:
            f.write("def test_ok():\n    assert True\n")
        native, wrapped, _ = self.run_pair(["pytest", "-q"], cwd=d)
        self.assertEqual((native.returncode, wrapped.returncode), (0, 0))
        with open(os.path.join(d, "test_x.py"), "a") as f:
            f.write("def test_planted():\n    assert False, 'CRITICAL-PY-77'\n")
        native, wrapped, plan = self.run_pair(["pytest", "-q"], cwd=d)
        self.assertEqual((native.returncode, wrapped.returncode), (1, 1))
        self.assertIn(b"CRITICAL-PY-77", wrapped.stdout + wrapped.stderr)

    def test_wrapper_merges_streams_so_stream_identity_is_not_preserved(self):
        # Documented limitation: failure detail moves to stderr, summaries to stdout, regardless of origin.
        native, wrapped, _ = self.run_pair(["cargo", "test"], FAKE_CARGO_MODE="fail")
        self.assertNotIn(b"CRITICAL-PLANTED-7731", native.stderr)   # native: assertion text is on stdout
        self.assertIn(b"CRITICAL-PLANTED-7731", wrapped.stderr)      # wrapped: filtered failure on stderr

    def test_failing_run_is_recoverable_without_rerun(self):
        argv = ["cargo", "test"]
        plan = self.wrapped_plan(argv)
        w = self.run_cmd(plan["argv"], env=self.env(FAKE_CARGO_MODE="fail", **plan["env"]))
        self.assertEqual((w.returncode, self.runs()), (101, 1))
        hint = (w.stdout + w.stderr).decode()
        import re
        m = re.search(plan["recovery"]["handle_pattern"], hint)
        self.assertIsNotNone(m, hint)
        for _ in range(2):
            rec = self.recall(plan, m.group(1), "--full")
            self.assertEqual(rec.returncode, 0)
            self.assertIn(b"CRITICAL-PLANTED-7731", rec.stdout)
            self.assertIn(b"test tests::case_0 ... ok", rec.stdout)  # the elided passing lines are really there
        self.assertEqual(self.runs(), 1)
        self.assertTrue(os.path.exists(plan["env"]["RTK_RECALL_DB"]))
        self.assertTrue(plan["env"]["RTK_RECALL_DB"].startswith(self.ret))

    def test_successful_run_raw_is_not_retained_for_filtered_families(self):
        # The hole that makes wrapper routes bypass by default: success elides output and keeps no raw copy.
        repo = self.make_repo()
        for argv in (["cargo", "test"], ["git", "diff"], ["git", "log"]):
            plan = self.wrapped_plan(argv)
            before = self.runs()
            w = self.run_cmd(plan["argv"], cwd=repo, env=self.env(FAKE_CARGO_MODE="ok", **plan["env"]))
            self.assertEqual(w.returncode, 0)
            self.assertNotIn(b"rtk recall", w.stdout + w.stderr, argv)
            lst = self.recall(plan, "--list")
            self.assertNotIn(b"git", lst.stdout.lower().replace(b"hash", b""), argv)
        self.assertEqual(self.runs(), 1)
        raw = self.run_cmd(["git", "diff"], cwd=repo).stdout
        w = self.run_cmd(self.wrapped_plan(["git", "diff"])["argv"], cwd=repo, env=self.env(**self.wrapped_plan(["git", "diff"])["env"]))
        measure("rtk git diff (wrapper, success, no raw retained)", raw_bytes=len(raw), view_bytes=len(w.stdout), saved_bytes=len(raw) - len(w.stdout), source="wrapper")
        self.assertLess(len(w.stdout), len(raw))

    def test_truncating_rg_success_is_recoverable_across_handles_without_rerun(self):
        repo = self.make_repo()
        argv = ["rg", "-n", "TODO", "src"]
        plan = self.wrapped_plan(argv)
        raw = self.run_cmd(argv, cwd=repo).stdout
        w = self.run_cmd(plan["argv"], cwd=repo, env=self.env(**plan["env"]))
        import re
        hashes = re.findall(plan["recovery"]["handle_pattern"], (w.stdout + w.stderr).decode())
        self.assertGreater(len(hashes), 1)
        recovered = b"".join(self.recall(plan, h, "--full").stdout for h in hashes)
        # every elided line is in some handle; line ORDER is not preserved (per-file entries + one remainder entry)
        self.assertEqual(sorted(recovered.splitlines()), sorted(raw.splitlines()))
        self.assertNotEqual(recovered, raw)
        measure("rtk rg -n (wrapper, success)", raw_bytes=len(raw), view_bytes=len(w.stdout), saved_bytes=len(raw) - len(w.stdout), source="wrapper")

    def test_negative_saving_wrapper_case_ls_small_dir(self):
        d = os.path.join(self.tmp, "small")
        os.makedirs(d)
        for n in ("a.txt", "b.txt"):
            with open(os.path.join(d, n), "w") as f:
                f.write("x" * 4)
        native, wrapped, _ = self.run_pair(["ls"], cwd=d)
        measure("rtk ls small dir (wrapper)", raw_bytes=len(native.stdout), view_bytes=len(wrapped.stdout),
                saved_bytes=len(native.stdout) - len(wrapped.stdout), source="wrapper")
        self.assertGreater(len(wrapped.stdout), len(native.stdout))
        self.assertEqual((native.returncode, wrapped.returncode), (0, 0))

    def test_large_stderr_and_malformed_utf8_keep_exit_status_and_planted_failure(self):
        for mode in ("bigerr", "badutf8"):
            self.setUp()
            plan = self.wrapped_plan(["cargo", "test"])
            native = self.run_cmd(["cargo", "test"], env=self.env(FAKE_CARGO_MODE=mode))
            w = self.run_cmd(plan["argv"], env=self.env(FAKE_CARGO_MODE=mode, **plan["env"]))
            self.assertEqual((native.returncode, w.returncode), (101, 101), mode)
            self.assertIn(b"CRITICAL-PLANTED-7731", w.stdout + w.stderr, mode)
            import re
            m = re.search(plan["recovery"]["handle_pattern"], (w.stdout + w.stderr).decode("utf-8", "replace"))
            rec = self.recall(plan, m.group(1), "--full").stdout
            if mode == "bigerr":
                self.assertIn(b"unused variable number 3999", rec)
            else:
                # raw bytes are NOT byte-exact: rtk decodes lossily before storing
                self.assertIn(b"\xef\xbf\xbd", rec)
                self.assertNotIn(b"\xff\xfe", rec)
                self.assertIn(b"\xff\xfe", native.stdout)

    def test_interruption_matches_unwrapped_status(self):
        for sig in (signal.SIGINT, signal.SIGTERM):
            results = {}
            for label, argv, extra in (("native", ["cargo", "test"], {}), ("wrapped", None, {})):
                plan = self.wrapped_plan(["cargo", "test"])
                argv = argv or plan["argv"]
                env = self.env(FAKE_CARGO_MODE="sleep", **(plan["env"] if label == "wrapped" else {}))
                pidfile = self.counter + ".pid"
                if os.path.exists(pidfile):
                    os.remove(pidfile)
                p = subprocess.Popen(argv, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                     stdin=subprocess.DEVNULL, start_new_session=True)
                child = None
                for _ in range(100):
                    if os.path.exists(pidfile):
                        with open(pidfile) as f:
                            if f.read().strip():
                                break
                    time.sleep(0.05)
                with open(pidfile) as f:
                    child = int(f.read())
                os.killpg(p.pid, sig)  # the host kills the whole process group
                p.communicate(timeout=10)
                time.sleep(0.2)
                try:
                    os.kill(child, 0)
                    alive = True
                    os.kill(child, signal.SIGKILL)
                except ProcessLookupError:
                    alive = False
                results[label] = (p.returncode, alive)
            self.assertEqual(results["native"], (-sig, False), results)
            self.assertEqual(results["wrapped"], results["native"], (sig, results))

    def test_upstream_rewrite_agrees_with_allowlisted_mappings_and_is_never_used_as_authority(self):
        import shlex
        env = {"PATH": "/usr/bin:/bin", "HOME": os.path.join(self.ret, "rtk-home"), "RTK_TELEMETRY_DISABLED": "1",
               "RTK_DB_PATH": os.path.join(self.ret, "h.db"), "RTK_RECALL_DB": os.path.join(self.ret, "r.db")}
        os.makedirs(env["HOME"], exist_ok=True)
        for argv in (["git", "diff"], ["git", "log", "-n", "3"], ["rg", "-n", "TODO", "src"], ["cargo", "test"], ["ls", "-la"], ["pytest", "-q"]):
            r = subprocess.run([RTK, "rewrite", shlex.join(argv)], env=env, capture_output=True, timeout=20)
            self.assertIn(r.returncode, (0, 3), argv)  # 3 = "no explicit allow rule": rewrite printed, host must ask
            self.assertEqual(shlex.split(r.stdout.decode()), ["rtk", *argv], argv)
        # rewrite is a shell-string facility and diverges from the argv contract: it must not be trusted blindly
        for raw, rewritten in (("python -m pytest -q", ["rtk", "pytest", "-q"]),
                               ("head -n 3 a.txt", ["rtk", "read", "a.txt", "--head-lines", "3"]),
                               ("cargo test --message-format=json", ["rtk", "cargo", "test", "--message-format=json"])):
            r = subprocess.run([RTK, "rewrite", raw], env=env, capture_output=True, timeout=20)
            self.assertEqual(shlex.split(r.stdout.decode()), rewritten, raw)
        self.assertEqual(subprocess.run([RTK, "rewrite", "echo hi"], env=env, capture_output=True, timeout=20).returncode, 1)

    def test_rtk_writes_no_state_outside_retention(self):
        repo = self.make_repo()
        before = set(os.listdir(self.tmp))
        plan = self.wrapped_plan(["rg", "-n", "TODO", "src"])
        self.run_cmd(plan["argv"], cwd=repo, env=self.env(**plan["env"]))
        self.assertEqual(set(os.listdir(self.tmp)) - before, set())  # HOME (= tmp) untouched: no Library/.local/.config
        self.assertTrue(os.path.exists(os.path.join(self.ret, "rtk")))


if __name__ == "__main__":
    unittest.main()
