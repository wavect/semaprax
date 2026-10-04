"""Static contract tests for the compact HR-07 evidence capture."""
import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("hot_reload_capture", pathlib.Path(__file__).with_name("capture.py"))
CAPTURE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CAPTURE
SPEC.loader.exec_module(CAPTURE)


class CaptureContract(unittest.TestCase):
    def test_plan_retains_interpreter_and_honest_native_wasm_cells(self):
        value = CAPTURE.plan(11, 3)
        self.assertEqual(value["cells"]["interpreter-a-b"]["status"], "requires-prebuilt-semaprax")
        for identifier in CAPTURE.NATIVE_WASM_CELLS:
            self.assertEqual(value["cells"][identifier]["status"], "unavailable")
            self.assertIsNone(value["cells"][identifier]["selector"])

    def test_dry_run_writes_no_measurement(self):
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "plan.json"
            prior = sys.argv
            try:
                sys.argv = ["capture.py", "--dry-run", "--output", str(output)]
                CAPTURE.main()
            finally:
                sys.argv = prior
            value = json.loads(output.read_text())
        self.assertEqual(value["mode"], "plan")
        self.assertNotIn("benchmark_report_sha256", json.dumps(value))


if __name__ == "__main__":
    unittest.main()
