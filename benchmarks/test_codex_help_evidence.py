import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import codex_help_evidence as evidence


def record(item_id, command, output, *, event_type="item.completed"):
    return {"type": event_type, "item": {"type": "command_execution", "id": item_id,
            "command": command, "aggregated_output": output}}


class HelpEvidenceTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.path = Path(directory.name) / "exec.jsonl"

    def write_events(self, *events):
        data = "".join(json.dumps(event, ensure_ascii=False) + "\n" for event in events).encode()
        self.path.write_bytes(data)
        return data

    def test_completed_help_uses_physical_line_ids_and_utf8_bytes(self):
        output = "größer\u2028字\n"
        direct = record("help", './tools/semaprax help authoring "file text"', output)
        data = self.write_events(
            direct | {"type": "item.started"},
            record("ordinary", "./tools/semaprax check x.spx -- --help", ""),
            direct,
            record("global", '/bin/zsh -lc \'"$SEMAPRAX_BIN" --help\'', ""),
            record("compound", "semaprax help language | head -20", "outside scope"),
            record("substitution", 'semaprax help "$(echo language)"', "outside scope"),
            record("quoted", 'printf "semaprax help language"', "outside scope"),
        )
        result = evidence.capture(self.path)
        self.assertEqual(result["transcript_sha256"], hashlib.sha256(data).hexdigest())
        self.assertTrue(result["complete_within_scope"])
        self.assertEqual(result["captured_help_calls"], 2)
        self.assertEqual(result["captured_help_utf8_bytes"], len(output.encode()))
        self.assertEqual(result["items"][0], {
            "line": 3, "item_id": "help", "arguments": ["help", "authoring", "file text"],
            "captured_utf8_bytes": len(output.encode()),
            "captured_sha256": hashlib.sha256(output.encode()).hexdigest(),
        })
        self.assertEqual(result["items"][1]["line"], 4)
        self.assertIsNone(result["delivered_help_utf8_bytes"])
        self.assertIsNone(result["delivered_help_tokens"])

    def test_missing_output_bad_encoding_and_duplicate_do_not_create_totals(self):
        data = self.write_events(
            record("one", "semaprax help language", "part"),
            record("one", "semaprax help language", "other"),
            record("two", "semaprax help library", None),
        )
        self.path.write_bytes(data + b'\xff\n' +
                              b'{"type":"item.completed","item":{"type":"command_execution","id":"three",'
                              b'"command":"semaprax --help","aggregated_output":"\\ud800"}}\n')
        result = evidence.capture(self.path)
        self.assertFalse(result["complete_within_scope"])
        self.assertIsNone(result["captured_help_calls"])
        self.assertIsNone(result["captured_help_utf8_bytes"])
        self.assertEqual([item["line"] for item in result["items"]], [1, 3, 5])
        self.assertEqual([item["captured_utf8_bytes"] for item in result["items"]], [4, None, None])

    def test_zero_is_distinct_from_missing_or_empty_transcript(self):
        self.assertIsNone(evidence.capture(self.path))
        self.path.touch()
        self.assertIsNone(evidence.capture(self.path)["captured_help_calls"])
        self.write_events({"type": "thread.started"})
        self.assertEqual(evidence.capture(self.path)["captured_help_calls"], 0)

    def test_attempt_summary_includes_failures_and_rejects_unknown_totals(self):
        self.write_events(record("one", "semaprax help language", "part"))
        capture = evidence.capture(self.path)
        rows = [{"status": "accepted", "help_output": capture},
                {"status": "not_accepted", "help_output": capture}]
        totals = evidence.summarize(rows, accepted=1)
        self.assertEqual(totals["captured_help_utf8_bytes"], {
            "total_all_attempts": 8, "per_accepted_task_including_failures": 8,
        })
        self.assertIsNone(totals["delivered_help_tokens"]["total_all_attempts"])
        rows.append({"status": "failed"})
        self.assertIsNone(evidence.summarize(rows, accepted=1)["captured_help_calls"]["total_all_attempts"])


if __name__ == "__main__":
    unittest.main()
