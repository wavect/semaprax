"""MF-05: lossy stderr decoding is reported identically on the normal and small-output paths.

Drives adapter.view in-process with a local identity filter, so no rtk binary is needed
and no command is ever executed.
"""
import os
import subprocess
import sys
import unittest
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import adapter  # noqa: E402

FFFD = "�"
NONMERGED, MERGED = ["git", "diff"], ["cargo", "test"]


def run_view(argv, out, err, min_bytes):
    """Returns (view, subprocess.run call count). The fake filter echoes stdin unchanged."""
    calls = []

    def fake_run(cmd, **kw):
        calls.append(cmd)
        return subprocess.CompletedProcess(cmd, 0, kw["input"], b"")

    payload = {"argv": argv, "stdout_b64": adapter.base64.b64encode(out).decode(),
               "stderr_b64": adapter.base64.b64encode(err).decode(), "min_bytes": min_bytes,
               "recovery_handle": "rec-1"}
    with mock.patch.object(adapter, "upstream", return_value="/x/rtk"), \
            mock.patch.object(adapter, "retention_dir", return_value="/x"), \
            mock.patch.object(adapter, "probe_identity"), \
            mock.patch.object(adapter, "rtk_env", return_value={}), \
            mock.patch.object(adapter.subprocess, "run", side_effect=fake_run):
        status, res, _ = adapter.view({"payload": payload})
    assert status == "complete"
    return res["view"], len(calls)


class StderrLossy(unittest.TestCase):
    def both_paths(self, argv, out, err):
        small, n_small = run_view(argv, out, err, 10**9)
        normal, n_normal = run_view(argv, out, err, 0)
        self.assertEqual((n_small, n_normal), (0, 1))  # filter runs only on the normal path, never the command
        self.assertEqual(small["recovery_handle"], "rec-1")
        self.assertEqual(normal["recovery_handle"], "rec-1")
        return small, normal

    def test_nonmerged_invalid_stderr_is_never_lossless_on_either_path(self):
        small, normal = self.both_paths(NONMERGED, b"ok\n", b"warn \xff\xfe\n")
        for v in (small, normal):
            self.assertFalse(v["lossless"])
            self.assertEqual(v["omissions"], 2)
            self.assertIn("\n[stderr]\n", v["text"])
        self.assertEqual(small["text"], normal["text"])

    def test_nonmerged_invalid_stdout_only(self):
        small, normal = self.both_paths(NONMERGED, b"ok \xff\n", b"clean\n")
        for v in (small, normal):
            self.assertFalse(v["lossless"])
            self.assertEqual(v["omissions"], 1)

    def test_nonmerged_both_invalid_counts_each_stream_once(self):
        small, normal = self.both_paths(NONMERGED, b"a \xff\n", b"b \xfe\xfd\n")
        for v in (small, normal):
            self.assertFalse(v["lossless"])
            self.assertEqual(v["omissions"], 3)

    def test_merged_invalid_stderr_is_counted_once(self):
        small, normal = self.both_paths(MERGED, b"ok\n", b"warn \xff\n")
        self.assertFalse(small["lossless"])
        self.assertFalse(normal["lossless"])
        self.assertEqual(small["omissions"], 1)
        self.assertEqual(normal["omissions"], 1)
        self.assertNotIn("[stderr]", normal["text"])

    def test_merged_invalid_stdout_and_both(self):
        for out, err, want in ((b"a \xff\n", b"clean\n", 1), (b"a \xff\n", b"b \xfe\n", 2)):
            small, normal = self.both_paths(MERGED, out, err)
            for v in (small, normal):
                self.assertFalse(v["lossless"])
                self.assertEqual(v["omissions"], want)

    def test_authored_replacement_character_is_not_corruption(self):
        authored = f"keep {FFFD} here\n".encode()
        for argv in (NONMERGED, MERGED):
            small, normal = self.both_paths(argv, authored, authored)
            for v in (small, normal):
                self.assertTrue(v["lossless"], argv)
                self.assertEqual(v["omissions"], 0)
                self.assertIn(FFFD, v["text"])

    def test_authored_fffd_next_to_invalid_byte_counts_only_the_invalid_byte(self):
        err = f"{FFFD}".encode() + b"\xff\n"
        small, normal = self.both_paths(NONMERGED, b"ok\n", err)
        for v in (small, normal):
            self.assertFalse(v["lossless"])
            self.assertEqual(v["omissions"], 1)

    def test_clean_streams_stay_lossless(self):
        for argv in (NONMERGED, MERGED):
            small, normal = self.both_paths(argv, b"ok\n", b"fine\n")
            self.assertTrue(small["lossless"])
            self.assertTrue(normal["lossless"])
            self.assertEqual((small["omissions"], normal["omissions"]), (0, 0))


if __name__ == "__main__":
    unittest.main()
