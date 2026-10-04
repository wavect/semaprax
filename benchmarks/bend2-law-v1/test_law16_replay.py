#!/usr/bin/env python3
import importlib.util
import contextlib
import io
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("law16_replay", ROOT / "law16_replay.py")
REPLAY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPLAY)


class Law16ReplayTests(unittest.TestCase):
    def test_fresh_physical_routes_are_distinct_from_offline_capsule_replay(self):
        sequence = [*REPLAY.FRESH_CAPTURE_ROUTES, *REPLAY.OFFLINE_REPLAY_ROUTES]
        classified = REPLAY.classify_execution_routes(sequence)
        self.assertEqual(classified["fresh_capture_routes"], list(REPLAY.FRESH_CAPTURE_ROUTES))
        self.assertEqual(classified["offline_replay_routes"], list(REPLAY.OFFLINE_REPLAY_ROUTES))
        self.assertEqual(classified["live_agent_routes"], [])
        with_agent = REPLAY.classify_execution_routes([*sequence, REPLAY.LIVE_AGENT_ROUTE])
        self.assertEqual(with_agent["live_agent_routes"], [REPLAY.LIVE_AGENT_ROUTE])
        with self.assertRaisesRegex(ValueError, "route inventory drifted"):
            REPLAY.classify_execution_routes([*REPLAY.FRESH_CAPTURE_ROUTES, "bounded_balance_agent_evidence_reexecution"])

    def test_live_agent_campaign_receipt_binds_the_pinned_executable_path(self):
        with tempfile.TemporaryDirectory() as directory:
            codex = Path(directory) / "pinned-codex"
            output = Path(directory) / "campaign"
            command = REPLAY.agent_campaign_argv(str(codex), output)
        self.assertEqual(command[-2:], ["--codex", str(codex)])
        self.assertEqual(command[command.index("--output") + 1], str(output))

    def test_completed_fresh_capture_authenticates_and_rejects_failure_or_raw_drift(self):
        capsule = ROOT / "evidence/law16-unified-fresh-v1"
        result = REPLAY.verify_fresh_capture(capsule)
        self.assertEqual(result["status"], "fresh_capture_authenticated")
        self.assertEqual(result["fresh_route_count"], 6)
        self.assertEqual(result["retained_agent_replay_count"], 1)
        with tempfile.TemporaryDirectory() as directory:
            copied = Path(directory) / "capsule"
            shutil.copytree(capsule, copied)
            status_path = copied / "replay-status.json"
            original = status_path.read_text()
            status = json.loads(original)
            status["status"] = "failed_closed"
            status_path.write_text(json.dumps(status))
            with self.assertRaisesRegex(ValueError, "not a completed"):
                REPLAY.verify_fresh_capture(copied)
            status_path.write_text(original)
            (copied / "lean-law15-raw/kernel-test.stdout").write_bytes(b"forged pass\n")
            with self.assertRaisesRegex(ValueError, "inventory drifted"):
                REPLAY.verify_fresh_capture(copied)

    def test_route_receipt_survives_success_failure_and_timeout(self):
        cases = [
            ("raise SystemExit(0)", None, 0),
            ("raise SystemExit(2)", "failed", 2),
            ("import time; time.sleep(30)", "timed out", None),
        ]
        for action, error, code in cases:
            with self.subTest(error=error), tempfile.TemporaryDirectory() as directory:
                target = Path(directory)
                source = "import sys; print('partial', flush=True); print('diagnostic', file=sys.stderr, flush=True); " + action
                argv = [REPLAY.sys.executable, "-c", source]
                if error:
                    with self.assertRaisesRegex(RuntimeError, error):
                        REPLAY.command(argv, target, "route", timeout=1)
                else:
                    REPLAY.command(argv, target, "route", timeout=1)
                receipt = json.loads((target / "route.command.json").read_text())
                self.assertEqual(receipt["timed_out"], code is None)
                self.assertEqual(receipt["timeout_seconds"], 1)
                self.assertEqual(receipt["exit_code"], code)
                for channel, expected in (("stdout", b"partial\n"), ("stderr", b"diagnostic\n")):
                    path = target / receipt[channel]["path"]
                    self.assertEqual(path.read_bytes(), expected)
                    self.assertEqual(REPLAY.sha(path), receipt[channel]["sha256"])

    def test_failed_replay_retains_an_inventory_of_partial_output(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "out"
            def fail(output):
                output.mkdir()
                (output / "partial.stdout").write_bytes(b"prior route output\n")
                raise RuntimeError("route timed out")
            with mock.patch.object(REPLAY, "verify_retained", side_effect=fail), contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                REPLAY.main(["--verify-retained", "--output-dir", str(target)])
            result = json.loads((target / "replay-status.json").read_text())
            self.assertEqual(result["status"], "failed_closed")
            self.assertEqual(result["generated_artifacts"], REPLAY.output_inventory(target))
            self.assertEqual(result["generated_artifacts"][0]["path"], "partial.stdout")

    def test_retained_mode_reauthenticates_existing_cells_without_execution_claim(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "retained"
            result = REPLAY.verify_retained(target)
            self.assertEqual(result["mode"], "retained_evidence_replay")
            self.assertIn("was rerun", result["execution_claim"])
            self.assertEqual(result["report_status"], "incomplete")
            self.assertIn("no:", result["report_closure_statement"])
            self.assertEqual(result["status"], "retained_evidence_verified")
            self.assertEqual(result["unavailable_cells"], [])
            self.assertTrue(result["raw_artifacts"])
            inventory = {row["path"] for row in result["raw_artifacts"]}
            for cell in ("law16-guest-cache-thirty-v1", "law16-native-phase-thirty-v1",
                         "law16-project-incremental-cell-v1", "law16-boolean-refactor-cell-v1"):
                self.assertTrue(any(path.startswith(f"benchmarks/bend2-law-v1/evidence/{cell}/") for path in inventory), cell)
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
