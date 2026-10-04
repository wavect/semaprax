#!/usr/bin/env python3
import importlib.util, pathlib, unittest
PATH = pathlib.Path(__file__).with_name("law16_cold_warm_cell.py")
SPEC = importlib.util.spec_from_file_location("law16_cold_warm_cell", PATH)
CELL = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(CELL)
class CellTests(unittest.TestCase):
    def test_summary_retains_all_samples_and_refuses_failed_cell(self):
        samples = [{"elapsed_ns": value, "exit_code": 0} for value in range(1, 31)]
        row = CELL.summarize(samples); self.assertEqual(row["status"], "completed"); self.assertEqual(len(row["samples"]), 30); self.assertEqual(row["summary"]["p50_ns"], 15.5)
        samples[0]["exit_code"] = 1; self.assertEqual(CELL.summarize(samples)["status"], "unavailable")
if __name__ == "__main__": unittest.main()
