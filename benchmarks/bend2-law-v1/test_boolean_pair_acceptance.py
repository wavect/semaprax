#!/usr/bin/env python3
"""Contracts for the bounded Boolean final-source pair evaluator."""
import hashlib
import importlib.util
import json
import pathlib
import tempfile
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("bend2_boolean_pair", ROOT / "boolean_pair_acceptance.py")
PAIR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PAIR)


def reference(root, name, body):
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(body)
    return {"path": name, "sha256": "sha256:" + hashlib.sha256(body).hexdigest(), "bytes": len(body)}


def plan(root):
    path = root / "plan.json"
    path.write_text(json.dumps({"schema": PAIR.PLAN.SCHEMA, "cells": [{"id": PAIR.TASK, "status": "preregistered", "trials": [
        {"id": PAIR.TASK + ":bend2:1", "language": "bend2", "ordinal": 1, "status": "not_run"},
        {"id": PAIR.TASK + ":semaprax-scalar-v1:1", "language": "semaprax-scalar-v1", "ordinal": 1, "status": "not_run"},
    ]}]}))
    return path


def input_document(root, plan_path, artifacts):
    bend = (ROOT / "fixtures/bend-two-value-boolean-v1.bend").read_bytes()
    semaprax = (ROOT / "fixtures/semaprax-two-value-boolean-v1.spx").read_bytes()
    value = {"schema": PAIR.INPUT_SCHEMA, "plan_sha256": PAIR.PLAN.digest(plan_path), "task": PAIR.TASK, "ordinal": 1, "lanes": {
        "bend2": {"trial_id": PAIR.TASK + ":bend2:1", "final_source": reference(artifacts, "bend/final.bend", bend), "verification_artifact": reference(artifacts, "bend/verdict.stdout", b"ALL PROOFS CHECK\n")},
        "semaprax-scalar-v1": {"trial_id": PAIR.TASK + ":semaprax-scalar-v1:1", "final_source": reference(artifacts, "semaprax/final.spx", semaprax), "verification_artifact": reference(artifacts, "semaprax/runtime.stdout", b"0\n")},
    }}
    path = root / "final-artifacts.json"
    path.write_text(json.dumps(value))
    return path, value


class BooleanPairAcceptanceTests(unittest.TestCase):
    def test_exact_sources_and_raw_artifacts_authenticate_without_a_repair_claim(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); artifacts = root / "artifacts"; artifacts.mkdir(); plan_path = plan(root)
            input_path, _ = input_document(root, plan_path, artifacts)
            result = PAIR.evaluate(plan_path, input_path, artifacts)
        self.assertEqual(result["status"], "source_pair_authenticated")
        self.assertEqual(result["proof_phases"]["semaprax_scalar"]["status"], "unavailable")
        self.assertEqual(result["cost_usage"]["status"], "unavailable")
        self.assertIn("not a successful law repair", result["nonclaims"][0])

    def test_mutated_final_source_and_missing_kernel_marker_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); artifacts = root / "artifacts"; artifacts.mkdir(); plan_path = plan(root)
            input_path, value = input_document(root, plan_path, artifacts)
            (artifacts / "bend/final.bend").write_bytes(b"mutant")
            with self.assertRaisesRegex(ValueError, "bytes disagree"):
                PAIR.evaluate(plan_path, input_path, artifacts)
            input_path, value = input_document(root, plan_path, artifacts)
            proof = artifacts / "bend/verdict.stdout"; proof.write_bytes(b"other proof output\n")
            value["lanes"]["bend2"]["verification_artifact"] = reference(artifacts, "bend/verdict.stdout", proof.read_bytes())
            input_path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, "proof-kernel success marker"):
                PAIR.evaluate(plan_path, input_path, artifacts)

    def test_linked_artifact_and_unbound_trial_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); artifacts = root / "artifacts"; artifacts.mkdir(); plan_path = plan(root)
            input_path, value = input_document(root, plan_path, artifacts)
            (artifacts / "linked.spx").symlink_to(artifacts / "semaprax/final.spx")
            value["lanes"]["semaprax-scalar-v1"]["final_source"]["path"] = "linked.spx"
            input_path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, "symbolic link"):
                PAIR.evaluate(plan_path, input_path, artifacts)
            input_path, value = input_document(root, plan_path, artifacts)
            value["lanes"]["bend2"]["trial_id"] = "different"
            input_path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, "selected preregistered trial"):
                PAIR.evaluate(plan_path, input_path, artifacts)


if __name__ == "__main__":
    unittest.main()
