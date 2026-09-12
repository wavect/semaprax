#!/usr/bin/env python3
"""Offline checks for scripts/opencode-provider-smoke.py.

Run directly: python3 scripts/test-opencode-provider-smoke.py
"""

import importlib.util
import json
import sys
import tempfile
import time
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("opencode_provider_smoke", ROOT / "scripts/opencode-provider-smoke.py")
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class OpenCodeSmokeTests(unittest.TestCase):
    def test_policy_denies_every_tool_for_the_dedicated_agent(self):
        policy = smoke.policy_document()
        self.assertEqual(policy["agent"][smoke.SMOKE_AGENT]["permission"], {"*": "deny"})
        self.assertNotIn("api_key_env", json.dumps(policy))

    def test_command_pins_one_model_and_never_auto_approves(self):
        line = smoke.command("/opt/homebrew/bin/opencode", "opencode/muse-spark-1.3-contributor-free", "/tmp/sandbox", "frozen")
        self.assertEqual(line[:3], ["/opt/homebrew/bin/opencode", "run", "--pure"])
        self.assertIn("--format", line)
        self.assertEqual(line[line.index("--format") + 1], "json")
        self.assertEqual(line[line.index("--model") + 1], "opencode/muse-spark-1.3-contributor-free")
        self.assertNotIn("--auto", line)

    def test_raw_events_require_an_exact_identity_self_report_but_not_completion(self):
        raw = b'{"type":"session","model":"opencode/muse-spark-1.3-contributor-free"}\n'
        smoke.verify_raw_events(raw, "opencode/muse-spark-1.3-contributor-free")
        with self.assertRaises(smoke.SmokeFailure):
            smoke.verify_raw_events(b'{"text":"opencode/muse-spark-1.3-contributor-free"}\n', "opencode/muse-spark-1.3-contributor-free")
        with self.assertRaises(smoke.SmokeFailure):
            smoke.verify_raw_events(b"not-json\n", "opencode/muse-spark-1.3-contributor-free")

    def test_capture_stdout_records_a_successful_local_json_stream(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "events.jsonl"
            command = [sys.executable, "-c", "print('{\\\"model\\\":\\\"opencode/muse-spark-1.3-contributor-free\\\"}')"]
            raw = smoke.capture_stdout(command, output, timeout_seconds=1, max_output_bytes=1_024)
            self.assertEqual(raw, output.read_bytes())
            smoke.verify_raw_events(raw, "opencode/muse-spark-1.3-contributor-free")

    def test_capture_stdout_kills_and_reaps_on_output_cap(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "events.jsonl"
            command = [sys.executable, "-c", "import sys; sys.stdout.write('x' * 4096); sys.stdout.flush()"]
            with self.assertRaisesRegex(smoke.SmokeFailure, "max-output-bytes"):
                smoke.capture_stdout(command, output, timeout_seconds=1, max_output_bytes=32)

    def test_capture_stdout_kills_and_reaps_on_timeout(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "events.jsonl"
            command = [sys.executable, "-c", "import time; time.sleep(5)"]
            started = time.monotonic()
            with self.assertRaisesRegex(smoke.SmokeFailure, "timed out"):
                smoke.capture_stdout(command, output, timeout_seconds=0.05, max_output_bytes=1_024)
            self.assertLess(time.monotonic() - started, 2)

    def test_capture_stdout_refuses_an_existing_output_before_starting_child(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "events.jsonl"
            output.write_text("reserved", encoding="utf-8")
            marker = Path(directory) / "child-was-started"
            command = [sys.executable, "-c", f"from pathlib import Path; Path({str(marker)!r}).write_text('started')"]
            with self.assertRaisesRegex(smoke.SmokeFailure, "create raw event output"):
                smoke.capture_stdout(command, output, timeout_seconds=1, max_output_bytes=1_024)
            self.assertFalse(marker.exists())

    def test_frozen_prompt_read_is_bounded_before_hashing(self):
        with tempfile.TemporaryDirectory() as directory:
            prompt = Path(directory) / "oversized-prompt.txt"
            prompt.write_bytes(b"x" * (smoke.MAX_PROMPT_BYTES + 1))
            with self.assertRaisesRegex(smoke.SmokeFailure, "exceeds"):
                smoke.load_frozen_prompt(prompt, "0" * 64)

    def test_sandbox_must_be_empty_before_the_adapter_writes_its_policy(self):
        with tempfile.TemporaryDirectory() as directory:
            sandbox = Path(directory)
            _, policy_path, policy = smoke.prepare_empty_sandbox(sandbox)
            self.assertTrue(policy_path.is_file())
            self.assertEqual(policy_path.read_bytes(), policy)
        with tempfile.TemporaryDirectory() as directory:
            sandbox = Path(directory)
            (sandbox / "task.spx").write_text("protected", encoding="utf-8")
            with self.assertRaises(smoke.SmokeFailure):
                smoke.prepare_empty_sandbox(sandbox)

    def test_execute_requires_an_external_raw_event_destination_before_any_call(self):
        arguments = SimpleNamespace(raw_events_output=None)
        with self.assertRaisesRegex(smoke.SmokeFailure, "raw-events-output"):
            smoke.execute(arguments)


if __name__ == "__main__":
    unittest.main()
