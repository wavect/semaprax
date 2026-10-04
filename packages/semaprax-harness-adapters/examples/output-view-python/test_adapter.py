import base64
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "tools"))
from drive import drive  # noqa: E402

ADAPTER = [sys.executable, os.path.join(HERE, "adapter.py")]


def view(payload):
    replies, _, code = drive(ADAPTER, "command.view", "view", payload)
    assert code == 0, replies
    return replies[1]["result"]


class OutputViewTest(unittest.TestCase):
    def test_dedupes_repeats_and_reports_omissions(self):
        r = view({"stdout": "ok\n" * 50 + "done\n", "stderr": "", "max_bytes": 4096})
        v = r["payload"]["view"]
        self.assertEqual(v["omissions"], 49)
        self.assertFalse(v["lossless"])
        self.assertIn("ok (x50)", v["text"])

    def test_no_loss_is_lossless(self):
        v = view({"stdout": "a\nb\n", "stderr": "c\n"})["payload"]["view"]
        self.assertTrue(v["lossless"])
        self.assertEqual(v["omissions"], 0)

    def test_critical_lines_always_kept_even_over_budget(self):
        noise = "".join(f"line {i}\n" for i in range(200))
        r = view({"stdout": noise + "thread panicked at x\nERROR: boom\n", "stderr": "Test FAILED\n", "max_bytes": 40})
        text = r["payload"]["view"]["text"]
        for needle in ("panicked", "ERROR: boom", "FAILED"):
            self.assertIn(needle, text)
        self.assertFalse(r["payload"]["view"]["lossless"])

    def test_repeated_error_lines_are_kept_not_collapsed(self):
        text = view({"stdout": "error: x\n" * 3})["payload"]["view"]["text"]
        self.assertEqual(text.count("error: x"), 3)

    def test_b64_input(self):
        b = base64.b64encode(b"hello\nhello\n").decode()
        self.assertIn("hello (x2)", view({"stdout_b64": b})["payload"]["view"]["text"])

    def test_never_returns_exit_status(self):
        r = view({"stdout": "x", "exit_status": 0})
        blob = repr(r).lower()
        self.assertNotIn("exit", blob)
        self.assertNotIn("status_code", blob)

    def test_bad_max_bytes_refused(self):
        self.assertEqual(view({"stdout": "x", "max_bytes": -1})["status"], "refused")


if __name__ == "__main__":
    unittest.main()
