#!/usr/bin/env python3
import importlib.util, os, pathlib, subprocess, sys, tempfile, unittest
from unittest.mock import patch
PATH = pathlib.Path(__file__).with_name("law16_cold_warm_cell.py")
SPEC = importlib.util.spec_from_file_location("law16_cold_warm_cell", PATH)
CELL = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(CELL)
class CellTests(unittest.TestCase):
    def test_summary_retains_all_samples_and_refuses_failed_cell(self):
        samples = [{"elapsed_ns": value, "outcome": "completed", "exit_code": 0} for value in range(1, 31)]
        row = CELL.summarize(samples); self.assertEqual(row["status"], "completed"); self.assertEqual(len(row["samples"]), 30); self.assertEqual(row["summary"]["p50_ns"], 15.5)
        samples[0]["exit_code"] = 1; self.assertEqual(CELL.summarize(samples)["status"], "unavailable")

    def test_invoke_retains_exact_child_streams_and_command_binding(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            row = CELL.invoke(
                [sys.executable, "-c", "import sys; print('out'); print('err', file=sys.stderr)"],
                dict(os.environ, BEND_NO_TELEMETRY="1"), root, "fresh-process", 1, 17,
            )
            self.assertEqual(row["exit_code"], 0)
            self.assertEqual((root / row["stdout"]["path"]).read_bytes(), b"out\n")
            self.assertEqual((root / row["stderr"]["path"]).read_bytes(), b"err\n")
            self.assertEqual(row["stdout"]["bytes"], 4)
            self.assertTrue(row["command_sha256"].startswith("sha256:"))

    def test_timeout_is_a_nonresult_with_retained_partial_streams(self):
        expired = subprocess.TimeoutExpired(["tool"], 17, output=b"partial stdout", stderr=b"partial stderr")
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            with patch.object(CELL.subprocess, "run", side_effect=expired) as run:
                sample = CELL.invoke(["tool"], {"BEND_NO_TELEMETRY": "1"}, root, "fresh-process", 1, 17)
            run.assert_called_once_with(["tool"], capture_output=True, env={"BEND_NO_TELEMETRY": "1"}, timeout=17)
            self.assertEqual(sample["outcome"], "nonresult_timeout")
            self.assertIsNone(sample["exit_code"])
            self.assertEqual((root / sample["stdout"]["path"]).read_bytes(), b"partial stdout")
            self.assertEqual((root / sample["stderr"]["path"]).read_bytes(), b"partial stderr")
            self.assertEqual(CELL.summarize([sample])["status"], "unavailable")

    def test_parse_command_rejects_non_array_and_empty_parts(self):
        with self.assertRaises(ValueError): CELL.parse_command("{}", "test")
        with self.assertRaises(ValueError): CELL.parse_command('["", "x"]', "test")

    def test_main_refuses_distinct_fresh_and_repeat_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            fresh, repeat = root / "fresh.spx", root / "repeat.spx"
            fresh.write_text("first\n"); repeat.write_text("second\n")
            with self.assertRaises(SystemExit) as exit_code:
                CELL.main([
                    "--fresh-command", '["tool", "fresh.spx"]',
                    "--repeat-command", '["tool", "repeat.spx"]',
                    "--fresh-input", str(fresh), "--repeat-input", str(repeat),
                    "--timeout-seconds", "17",
                    "--raw-artifact-dir", str(root / "raw"),
                    "--output", str(root / "result.json"),
                ])
            self.assertEqual(exit_code.exception.code, 2)
if __name__ == "__main__": unittest.main()
