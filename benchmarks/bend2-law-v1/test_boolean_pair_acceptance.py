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
    bend_attack = (ROOT / "fixtures/bend-two-value-boolean-law-gaming-v1.bend").read_bytes()
    semaprax_attack = (ROOT / "fixtures/semaprax-two-value-boolean-law-gaming-v1.spx").read_bytes()
    def lane(language, trial_id, source, attack, prefix, verification):
        claim = {"source_sha256": "sha256:" + hashlib.sha256(attack).hexdigest(), "decision": "reject", "reason": "seeded control changes the required Boolean result"}
        response = {"schema": "semaprax.bend2-law-benchmark.boolean-edit-response.v1", "trial_id": trial_id, "language": language,
                    "final_source": source.decode(), "proof_explanation": "restores the required source", "seeded_attack": claim}
        return {"trial_id": trial_id, "final_source": reference(artifacts, prefix + "/final", source),
                "verification_artifact": reference(artifacts, prefix + "/verification", verification),
                "model_response": reference(artifacts, prefix + "/response.json", json.dumps(response, sort_keys=True).encode()),
                "seeded_attack_source": reference(artifacts, prefix + "/attack", attack),
                "seeded_attack_claim": reference(artifacts, prefix + "/claim.json", json.dumps(claim, sort_keys=True).encode())}
    value = {"schema": PAIR.INPUT_SCHEMA, "plan_sha256": PAIR.PLAN.digest(plan_path), "task": PAIR.TASK, "ordinal": 1, "lanes": {
        "bend2": lane("bend2", PAIR.TASK + ":bend2:1", bend, bend_attack, "bend", b"ALL PROOFS CHECK\n"),
        "semaprax-scalar-v1": lane("semaprax-scalar-v1", PAIR.TASK + ":semaprax-scalar-v1:1", semaprax, semaprax_attack, "semaprax", b"0\n"),
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
            (artifacts / "bend/final").write_bytes(b"mutant")
            with self.assertRaisesRegex(ValueError, "bytes disagree"):
                PAIR.evaluate(plan_path, input_path, artifacts)
            input_path, value = input_document(root, plan_path, artifacts)
            proof = artifacts / "bend/verification"; proof.write_bytes(b"other proof output\n")
            value["lanes"]["bend2"]["verification_artifact"] = reference(artifacts, "bend/verification", proof.read_bytes())
            input_path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, "proof-kernel success marker"):
                PAIR.evaluate(plan_path, input_path, artifacts)

    def test_linked_artifact_and_unbound_trial_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); artifacts = root / "artifacts"; artifacts.mkdir(); plan_path = plan(root)
            input_path, value = input_document(root, plan_path, artifacts)
            (artifacts / "linked.spx").symlink_to(artifacts / "semaprax/final")
            value["lanes"]["semaprax-scalar-v1"]["final_source"]["path"] = "linked.spx"
            input_path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, "symbolic link"):
                PAIR.evaluate(plan_path, input_path, artifacts)
            input_path, value = input_document(root, plan_path, artifacts)
            value["lanes"]["bend2"]["trial_id"] = "different"
            response = json.loads((artifacts / "bend/response.json").read_text())
            response["trial_id"] = "different"
            value["lanes"]["bend2"]["model_response"] = reference(artifacts, "bend/response.json", json.dumps(response, sort_keys=True).encode())
            input_path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, "selected preregistered trial"):
                PAIR.evaluate(plan_path, input_path, artifacts)


if __name__ == "__main__":
    unittest.main()
