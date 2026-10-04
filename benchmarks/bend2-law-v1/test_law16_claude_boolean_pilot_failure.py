#!/usr/bin/env python3
"""Verify the bounded Claude Bend failure capsule without provider access."""
import hashlib
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parent
CAPSULE_ROOT = ROOT / "evidence/law16-claude-boolean-pilot-v1"


class ClaudeBooleanPilotFailureTests(unittest.TestCase):
    def test_failed_attempts_keep_costs_artifacts_and_campaign_nonadmission_explicit(self):
        value = json.loads((CAPSULE_ROOT / "capsule.json").read_text())
        self.assertEqual(value["status"], "two_bounded_bend_attempts_nonadmitted")
        self.assertFalse(value["campaign_admission"])
        self.assertEqual([row["id"] for row in value["attempts"]], ["bend-syntax-1", "bend-syntax-2"])
        self.assertEqual(value["attempts"][0]["verdict"], {"attack_exit_code": 1, "candidate_exit_code": 1})
        self.assertIn("response_parse_failed", value["attempts"][1]["result"])
        config = ROOT / value["config"]["path"]
        self.assertEqual(config.stat().st_size, value["config"]["bytes"])
        self.assertEqual("sha256:" + hashlib.sha256(config.read_bytes()).hexdigest(), value["config"]["sha256"])
        for ref in value["attempts"][0]["artifacts"].values():
            path = CAPSULE_ROOT / ref["path"]
            body = path.read_bytes()
            self.assertEqual(len(body), ref["bytes"])
            self.assertEqual("sha256:" + hashlib.sha256(body).hexdigest(), ref["sha256"])


if __name__ == "__main__":
    unittest.main()
