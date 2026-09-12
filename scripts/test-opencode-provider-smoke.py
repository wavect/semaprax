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
FIXTURES = ROOT / "scripts" / "fixtures" / "opencode-provider-smoke-v1"
spec = importlib.util.spec_from_file_location("opencode_provider_smoke", ROOT / "scripts/opencode-provider-smoke.py")
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class OpenCodeSmokeTests(unittest.TestCase):
    model = "opencode/muse-spark-1.3-contributor-free"

    def archived_fixture(self):
        return (
            (FIXTURES / "events.jsonl").read_bytes(),
            json.loads((FIXTURES / "session.json").read_bytes()),
        )

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

    def test_archived_session_binds_observed_completion_usage_and_model(self):
        raw, session = self.archived_fixture()
        result = smoke.validate_archived_session(raw, json.dumps(session).encode("utf-8"), self.model)
        self.assertEqual(result["completion"], "stopped_text_completion")
        self.assertEqual(result["usage"], {
            "total": 4651, "input": 4437, "output": 18, "reasoning": 196,
            "cache_read": 0, "cache_write": 0,
        })
        self.assertEqual(result["model"], self.model)
        self.assertIn("raw_events_sha256", result)
        self.assertIn("session_export_sha256", result)

    def test_arbitrary_nested_model_text_cannot_satisfy_archive_validation(self):
        with self.assertRaises(smoke.SmokeFailure):
            smoke.validate_archived_session(
                b'{"type":"text","model":"opencode/muse-spark-1.3-contributor-free"}\n',
                b'{"info":{},"messages":[]}', self.model,
            )

    def test_archive_rejects_model_message_and_usage_mismatches(self):
        raw, session = self.archived_fixture()
        model_mismatch = json.loads(json.dumps(session))
        model_mismatch["info"]["model"]["id"] = "different-model"
        with self.assertRaises(smoke.SmokeFailure):
            smoke.validate_archived_session(raw, json.dumps(model_mismatch).encode("utf-8"), self.model)
        message_mismatch = json.loads(json.dumps(session))
        message_mismatch["messages"][0]["info"]["id"] = "msg_other"
        with self.assertRaises(smoke.SmokeFailure):
            smoke.validate_archived_session(raw, json.dumps(message_mismatch).encode("utf-8"), self.model)
        usage_mismatch = raw.replace(b'"output":18', b'"output":19')
        with self.assertRaises(smoke.SmokeFailure):
            smoke.validate_archived_session(usage_mismatch, json.dumps(session).encode("utf-8"), self.model)

    def test_archive_does_not_call_a_start_only_stream_a_completion(self):
        raw, session = self.archived_fixture()
        with self.assertRaisesRegex(smoke.SmokeFailure, "stopped text completion"):
            smoke.validate_archived_session(raw.splitlines()[0] + b"\n", json.dumps(session).encode("utf-8"), self.model)

    def test_capture_stdout_records_a_successful_local_json_stream(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "events.jsonl"
            command = [sys.executable, "-c", "print('{\\\"model\\\":\\\"opencode/muse-spark-1.3-contributor-free\\\"}')"]
            raw = smoke.capture_stdout(command, output, timeout_seconds=1, max_output_bytes=1_024)
            self.assertEqual(raw, output.read_bytes())
            self.assertEqual(raw, b'{"model":"opencode/muse-spark-1.3-contributor-free"}\n')

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
