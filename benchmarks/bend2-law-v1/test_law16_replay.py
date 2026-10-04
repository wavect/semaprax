#!/usr/bin/env python3
import importlib.util
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("law16_replay", ROOT / "law16_replay.py")
REPLAY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPLAY)


class Law16ReplayTests(unittest.TestCase):
    def test_retained_mode_reauthenticates_existing_cells_without_execution_claim(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "retained"
            result = REPLAY.verify_retained(target)
            self.assertEqual(result["mode"], "retained_evidence_replay")
            self.assertIn("was rerun", result["execution_claim"])
            self.assertEqual(result["report_status"], "incomplete")
            self.assertIn("no:", result["report_closure_statement"])
            self.assertEqual(result["status"], "partial_retained_evidence")
            self.assertEqual(result["unavailable_cells"][0]["cell"], "bounded_balance_agent_campaign")
            self.assertEqual(result["unavailable_cells"][0]["status"], "incomplete_missing_raw_artifact")
            self.assertTrue(result["raw_artifacts"])
            for row in result["raw_artifacts"]:
                path = REPLAY.PROJECT / row["path"]
                self.assertEqual(path.stat().st_size, row["bytes"])
                self.assertEqual(REPLAY.sha(path), row["sha256"])

    def test_live_pin_loader_fails_closed_when_required_tool_pins_are_missing(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "pins.json"
            path.write_text(json.dumps({
                "schema": "semaprax.bend2-law-benchmark.local-replay-pins.v1",
                "bend_commit": REPLAY.BEND_PIN,
                "semaprax_build_commit": "0" * 40,
                "tools": {},
            }))
            with self.assertRaisesRegex(ValueError, "tools.bun.path"):
                REPLAY.load_pins(path)

    def test_live_execution_requires_explicit_pins(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "out"
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                REPLAY.main(["--execute", "--output-dir", str(target)])
            self.assertFalse(target.exists())


if __name__ == "__main__":
    unittest.main()
