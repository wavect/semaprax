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


def write(path, value):
    path.write_text(json.dumps(value, sort_keys=True) + "\n")


def sha(text):
    return "sha256:" + hashlib.sha256(text.encode()).hexdigest()


def trial(identifier, language):
    return {
        "id": identifier,
        "language": language,
        "status": "not_run",
        "acceptance": {
            "success_witnesses": [{"input": False, "output": True}],
            "rejected_law_gaming_attacks": {"weakened-postcondition": [{"input": False, "output": False}]},
        },
    }


def observed(identifier):
    return {
        "id": identifier,
        "transcript_sha256": sha(identifier + " transcript"),
        "telemetry_events": [
            {"event_id": identifier + ":tokens", "kind": "token_usage", "value": 123},
            {"event_id": identifier + ":cost", "kind": "cost_usage", "value": "0.01"},
        ],
        "phases": {phase: {"status": "completed", "wall_ms": 1.0,
                             "measurement_sha256": sha(identifier + " " + phase)}
                   for phase in CAPTURE.PHASES},
        "success_witnesses": [{"outcome": "accepted", "evidence_sha256": sha(identifier + " success")}],
        "attacks": {"weakened-postcondition": [{"outcome": "rejected", "evidence_sha256": sha(identifier + " attack")}]} ,
    }


class AgentTrialCaptureTests(unittest.TestCase):
    def plan(self, root):
        path = root / "plan.json"
        write(path, {
            "schema": CAPTURE.PLAN_SCHEMA,
            "cells": [{"status": "preregistered", "trials": [
                trial("boolean:bend2:1", "bend2"),
                trial("boolean:semaprax-scalar-v1:1", "semaprax-scalar-v1"),
            ]}],
        })
        return path

    def raw(self, root, plan, rows):
        path = root / "raw.json"
        write(path, {
            "schema": CAPTURE.RAW_SCHEMA,
            "plan_sha256": CAPTURE.digest(plan),
            "exporter": {"kind": "existing_agent_telemetry_export", "exported_at": "2026-10-04T00:00:00Z"},
            "trials": rows,
        })
        return path

    def test_completed_capture_retains_raw_telemetry_and_separate_phase_observations(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            plan = self.plan(root)
            raw = self.raw(root, plan, [observed("boolean:bend2:1"), observed("boolean:semaprax-scalar-v1:1")])
            document = CAPTURE.capture(plan, raw)
        self.assertEqual(document["status"], "completed")
        self.assertEqual(document["trial_counts"]["bend2"], {"captured": 1, "required": 1})
        self.assertEqual(document["captured_trials"][0]["telemetry"]["token_usage"]["value"], 123)
        self.assertEqual(set(document["captured_trials"][0]["phase_wall_ms"]), set(CAPTURE.PHASES))
        self.assertEqual(set(document["captured_trials"][0]["phase_measurements"]), set(CAPTURE.PHASES))
        self.assertEqual(document["missing_trial_ids"], [])

    def test_partial_capture_is_not_a_trial_result(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            plan = self.plan(root)
            raw = self.raw(root, plan, [observed("boolean:bend2:1")])
            document = CAPTURE.capture(plan, raw)
        self.assertEqual(document["status"], "partial")
        self.assertEqual(document["trial_counts"]["semaprax-scalar-v1"], {"captured": 0, "required": 1})
        self.assertEqual(document["missing_trial_ids"], ["boolean:semaprax-scalar-v1:1"])

    def test_accepted_attack_or_unbound_telemetry_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            plan = self.plan(root)
            bad = observed("boolean:bend2:1")
            bad["attacks"]["weakened-postcondition"][0]["outcome"] = "accepted"
            raw = self.raw(root, plan, [bad])
            with self.assertRaisesRegex(ValueError, "fixed criterion"):
                CAPTURE.capture(plan, raw)
            raw = self.raw(root, plan, [observed("boolean:bend2:1")])
            value = json.loads(raw.read_text())
            value["plan_sha256"] = sha("another plan")
            write(raw, value)
            with self.assertRaisesRegex(ValueError, "exact preregistration"):
                CAPTURE.capture(plan, raw)

    def test_phase_measurements_must_have_distinct_digest_bound_raw_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            plan = self.plan(root)
            row = observed("boolean:bend2:1")
            first = row["phases"][CAPTURE.PHASES[0]]["measurement_sha256"]
            row["phases"][CAPTURE.PHASES[1]]["measurement_sha256"] = first
            raw = self.raw(root, plan, [row])
            with self.assertRaisesRegex(ValueError, "reuses one measurement artifact"):
                CAPTURE.capture(plan, raw)


if __name__ == "__main__":
    unittest.main()
