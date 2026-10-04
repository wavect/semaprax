#!/usr/bin/env python3
"""Focused regressions for LAW-16 raw agent telemetry capture."""
import hashlib
import importlib.util
import json
import pathlib
import tempfile
import unittest

MODULE = pathlib.Path(__file__).with_name("agent_trial_capture.py")
SPEC = importlib.util.spec_from_file_location("bend2_agent_capture", MODULE)
CAPTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPTURE)


def sha(body):
    return "sha256:" + hashlib.sha256(body.encode()).hexdigest()


def artifact(root, name, body):
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body)
    return {"path": name, "sha256": sha(body), "bytes": len(body.encode())}


def trial(identifier, language):
    return {"id": identifier, "language": language, "status": "not_run", "acceptance": {
        "success_witnesses": [{}], "rejected_law_gaming_attacks": {"weakened-postcondition": [{}]},
    }}


def observed(identifier, artifacts):
    return {
        "id": identifier,
        "transcript": artifact(artifacts, identifier + "/transcript.jsonl", identifier + " transcript"),
        "telemetry_events": [
            {"event_id": identifier + ":tokens", "kind": "token_usage", "value": 123,
             "artifact": artifact(artifacts, identifier + "/tokens.json", identifier + " tokens")},
            {"event_id": identifier + ":cost", "kind": "cost_usage", "value": "0.01",
             "artifact": artifact(artifacts, identifier + "/cost.json", identifier + " cost")},
        ],
        "phases": {phase: {"status": "completed", "wall_ms": 1.0,
                             "artifact": artifact(artifacts, identifier + "/" + phase + ".json", identifier + " " + phase)}
                   for phase in CAPTURE.PHASES},
        "success_witnesses": [{"outcome": "accepted", "artifact": artifact(artifacts, identifier + "/success.json", identifier + " success")}],
        "attacks": {"weakened-postcondition": [{"outcome": "rejected", "artifact": artifact(artifacts, identifier + "/attack.json", identifier + " attack")}]},
    }


class AgentTrialCaptureTests(unittest.TestCase):
    def plan(self, root):
        path = root / "plan.json"
        path.write_text(json.dumps({"schema": CAPTURE.PLAN_SCHEMA, "cells": [{"status": "preregistered", "trials": [
            trial("boolean:bend2:1", "bend2"), trial("boolean:semaprax-scalar-v1:1", "semaprax-scalar-v1"),
        ]}]}))
        return path

    def raw(self, root, plan, rows):
        path = root / "raw.json"
        path.write_text(json.dumps({"schema": CAPTURE.RAW_SCHEMA, "plan_sha256": CAPTURE.digest(plan),
            "exporter": {"kind": "existing_agent_telemetry_export", "exported_at": "2026-10-04T00:00:00Z"}, "trials": rows}))
        return path

    def test_completed_capture_rehashes_regular_raw_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); artifacts = root / "artifacts"; artifacts.mkdir(); plan = self.plan(root)
            raw = self.raw(root, plan, [observed("boolean:bend2:1", artifacts), observed("boolean:semaprax-scalar-v1:1", artifacts)])
            document = CAPTURE.capture(plan, raw, artifacts)
        self.assertEqual(document["status"], "completed")
        self.assertEqual(document["trial_counts"]["bend2"], {"captured": 1, "required": 1})
        self.assertEqual(document["captured_trials"][0]["telemetry"]["token_usage"]["value"], 123)
        self.assertEqual(set(document["captured_trials"][0]["phase_measurements"]), set(CAPTURE.PHASES))

    def test_partial_capture_is_not_a_trial_result(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); artifacts = root / "artifacts"; artifacts.mkdir(); plan = self.plan(root)
            raw = self.raw(root, plan, [observed("boolean:bend2:1", artifacts)])
            document = CAPTURE.capture(plan, raw, artifacts)
        self.assertEqual(document["status"], "partial")
        self.assertEqual(document["missing_trial_ids"], ["boolean:semaprax-scalar-v1:1"])

    def test_tampered_or_linked_raw_artifacts_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); artifacts = root / "artifacts"; artifacts.mkdir(); plan = self.plan(root)
            row = observed("boolean:bend2:1", artifacts)
            (artifacts / row["transcript"]["path"]).write_text("changed")
            raw = self.raw(root, plan, [row])
            with self.assertRaisesRegex(ValueError, "bytes disagree"):
                CAPTURE.capture(plan, raw, artifacts)
            row = observed("boolean:bend2:1", artifacts)
            linked = artifacts / "linked.jsonl"; linked.symlink_to(artifacts / row["transcript"]["path"])
            row["transcript"] = {"path": "linked.jsonl", "sha256": row["transcript"]["sha256"], "bytes": row["transcript"]["bytes"]}
            raw = self.raw(root, plan, [row])
            with self.assertRaisesRegex(ValueError, "symbolic link"):
                CAPTURE.capture(plan, raw, artifacts)

    def test_phase_measurements_must_have_distinct_authenticated_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); artifacts = root / "artifacts"; artifacts.mkdir(); plan = self.plan(root)
            row = observed("boolean:bend2:1", artifacts)
            row["phases"][CAPTURE.PHASES[1]]["artifact"] = row["phases"][CAPTURE.PHASES[0]]["artifact"]
            raw = self.raw(root, plan, [row])
            with self.assertRaisesRegex(ValueError, "reuses one measurement artifact"):
                CAPTURE.capture(plan, raw, artifacts)


if __name__ == "__main__":
    unittest.main()
