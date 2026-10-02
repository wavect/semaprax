#!/usr/bin/env python3
"""Offline unit and subprocess checks for ``scripts/token_report.py``."""

import importlib.util
import json
import stat
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
spec = importlib.util.spec_from_file_location("token_report", ROOT / "scripts" / "token_report.py")
report = importlib.util.module_from_spec(spec)
spec.loader.exec_module(report)


class ArithmeticTests(unittest.TestCase):
    def counts(self, baseline, actual):
        with mock.patch.object(report, "measure_utf8", side_effect=[(baseline, {"name": "cl100k_base"}), (actual, {"name": "cl100k_base"})]):
            return report.comparison_counts(b"baseline", b"actual", "cl100k_base", False)[0]

    def test_signed_arithmetic_and_exact_fraction(self):
        self.assertEqual(self.counts(1000, 800), {
            "measurement_status": "measured", "baseline_tokens": 1000, "actual_tokens": 800,
            "delta_tokens": 200, "delta_fraction": {"numerator": 200, "denominator": 1000},
            "delta_percentage": 20.0,
        })
        growth = self.counts(1000, 1100)
        self.assertEqual(growth["delta_tokens"], -100)
        self.assertEqual(growth["delta_fraction"], {"numerator": -100, "denominator": 1000})
        self.assertEqual(growth["delta_percentage"], -10.0)
        zero = self.counts(0, 0)
        self.assertEqual(zero["delta_tokens"], 0)
        self.assertIsNone(zero["delta_fraction"])
        self.assertIsNone(zero["delta_percentage"])

    def test_bytes_only_never_claims_a_token_saving(self):
        with mock.patch.object(report, "measure_utf8", side_effect=report.TokenizerUnavailable("missing")):
            result, metadata = report.comparison_counts(b"a", b"b", "cl100k_base", True)
        self.assertIsNone(metadata)
        self.assertEqual(result["measurement_status"], "tokenizer_unavailable")
        self.assertTrue(all(result[key] is None for key in ("baseline_tokens", "actual_tokens", "delta_tokens", "delta_fraction", "delta_percentage")))


class BoundaryTests(unittest.TestCase):
    def test_framed_metadata_retains_unicode_and_whitespace_boundary(self):
        payload = "SEMAPRAX-MODEL-TEXT 2\nprofile 5 graph\nroot 5 é id\nsource_revision 3 rev\n".encode("utf-8")
        self.assertEqual(report.framed_metadata(payload), {"profile": "graph", "root": "é id", "source_revision": "rev"})

    def test_executable_hash_accepts_non_utf8_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            executable = Path(directory) / "compiler"
            executable.write_bytes(b"\xff\x00compiler")
            self.assertEqual(report.executable_sha256(executable), report.sha256(b"\xff\x00compiler"))

    def test_projection_report_uses_replay_and_does_not_copy_input_text_or_path(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            source = directory / "secret.spx"
            source.write_text("TOP_SECRET_123\n", encoding="utf-8")
            executable = directory / "fake-semaprax"
            executable.write_text(
                "#!" + sys.executable + "\n"
                "import json, sys\n"
                "if sys.argv[1:3] == ['version', '--json']:\n"
                " print(json.dumps({'schema':'semaprax.version.v1','version':'0.test','commit':None}))\n"
                "elif '--replay' in sys.argv:\n"
                " sys.stdout.buffer.write(b'{\\\"selected\\\":true}')\n"
                "else:\n"
                " sys.stdout.buffer.write(b'SEMAPRAX-COMPACT 1\\nprofile 5 graph\\nroot 1 *\\nsource_revision 3 rev\\nsource_digest 1 x\\ndict 0\\nbody\\n{}')\n",
                encoding="utf-8",
            )
            executable.chmod(executable.stat().st_mode | stat.S_IXUSR)
            args = report.parser().parse_args([
                "projection", "--semaprax", str(executable), "--input", str(source), "--profile", "graph",
                "--encoding", "model-text", "--measurement-tokenizer", "cl100k_base", "--output", str(directory / "out.json"),
            ])
            with mock.patch.object(report, "measure_utf8", side_effect=[(5, {"name": "cl100k_base"}), (4, {"name": "cl100k_base"})]):
                document = report.projection(args)
            rendered = json.dumps(document)
            self.assertEqual(document["baseline_kind"], "same_selected_json")
            self.assertEqual(document["counts"]["delta_tokens"], 1)
            self.assertNotIn("TOP_SECRET_123", rendered)
            self.assertNotIn(str(source), rendered)
            self.assertIn("root_sha256", document)

    def test_closed_producer_options_reject_unrelated_flags(self):
        with self.assertRaisesRegex(report.ReportError, "not admitted"):
            report.validate_producer_options("context", [["--replay", "surprise"]])

    def test_session_aggregates_only_metadata_by_tokenizer_boundary_and_reference(self):
        with tempfile.TemporaryDirectory() as directory:
            events = Path(directory) / "events.jsonl"
            events.write_text(
                '{"schema":"semaprax.token-observation.v1","eventId":"one","sessionId":"secret","attemptSequence":1,"deliverySequence":1,"method":"compact","boundary":"envelope","subjectRevision":"rev","outcome":"ok","status":0,"bytes":10,"digest":"sha256:a","tokenizer":"cl100k_base","tokenizerFingerprint":"sha256:t","tokens":8,"referenceKind":"same_selected_json","baselineTokens":10}\n'
                '{"schema":"semaprax.token-observation.v1","eventId":"two","sessionId":"secret","attemptSequence":2,"deliverySequence":2,"method":"compact","boundary":"envelope","subjectRevision":"rev","outcome":"ok","status":0,"bytes":12,"digest":"sha256:b","tokenizer":"cl100k_base","tokenizerFingerprint":"sha256:t","tokens":9,"referenceKind":"same_selected_json","baselineTokens":10}\n',
                encoding="utf-8",
            )
            args = report.parser().parse_args(["session", "--events", str(events), "--output", str(Path(directory) / "out.json")])
            document = report.session(args)
        self.assertEqual(document["events"], 2)
        self.assertNotIn("secret", json.dumps(document))
        self.assertEqual(document["groups"][0]["coverage"], {"events": 2, "token_measured": 2, "baseline_available": 2})
        self.assertEqual(document["groups"][0]["tokens"], 17)


if __name__ == "__main__":
    unittest.main()
