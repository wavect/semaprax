#!/usr/bin/env python3
import hashlib, importlib.util, pathlib, subprocess, unittest
from unittest.mock import patch
PATH = pathlib.Path(__file__).with_name("law16_cold_warm_cell.py")
SPEC = importlib.util.spec_from_file_location("law16_cold_warm_cell", PATH)
CELL = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(CELL)
class CellTests(unittest.TestCase):
    def test_summary_retains_all_samples_and_refuses_failed_cell(self):
        samples = [{"elapsed_ns": value, "outcome": "completed", "exit_code": 0} for value in range(1, 31)]
        row = CELL.summarize(samples); self.assertEqual(row["status"], "completed"); self.assertEqual(len(row["samples"]), 30); self.assertEqual(row["summary"]["p50_ns"], 15.5)
        samples[0]["exit_code"] = 1; self.assertEqual(CELL.summarize(samples)["status"], "unavailable")

    def test_timeout_is_a_nonresult_with_retained_available_output_digests(self):
        expired = subprocess.TimeoutExpired(["tool"], 17, output=b"partial stdout", stderr=b"partial stderr")
        with patch.object(CELL.subprocess, "run", side_effect=expired) as run:
            sample = CELL.invoke(["tool"], {"BEND_NO_TELEMETRY": "1"}, 17)
        run.assert_called_once_with(["tool"], capture_output=True, env={"BEND_NO_TELEMETRY": "1"}, timeout=17)
        self.assertEqual(sample["outcome"], "nonresult_timeout")
        self.assertIsNone(sample["exit_code"])
        self.assertEqual(sample["timeout_seconds"], 17)
        self.assertEqual(sample["stdout_sha256"], "sha256:" + hashlib.sha256(b"partial stdout").hexdigest())
        self.assertEqual(sample["stderr_sha256"], "sha256:" + hashlib.sha256(b"partial stderr").hexdigest())
        self.assertEqual(CELL.summarize([sample])["status"], "unavailable")
if __name__ == "__main__": unittest.main()
