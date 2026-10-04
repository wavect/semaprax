#!/usr/bin/env python3
"""Contracts for the retained two-input SEMAPRAX runtime-evidence evaluator."""
import hashlib
import importlib.util
import json
import pathlib
import tempfile
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("semaprax_boolean_runtime", ROOT / "semaprax_boolean_runtime_acceptance.py")
RUNTIME = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNTIME)
COMMIT = "d8c405fbd"


def reference(root, name, body):
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(body)
    return {"path": name, "sha256": RUNTIME.digest(body), "bytes": len(body)}


def plan(root):
    path = root / "plan.json"
    path.write_text(json.dumps({"schema": RUNTIME.PLAN.SCHEMA, "cells": [{"id": RUNTIME.TASK, "status": "preregistered", "trials": [
        {"id": RUNTIME.TASK + ":semaprax-scalar-v1:2", "language": RUNTIME.LANE, "status": "not_run"},
    ]}]}))
    return path


def evidence(root, plan_path, artifacts):
    source = (ROOT / "fixtures/semaprax-two-value-boolean-v1.spx").read_text().replace("module app;", "module app;\n// agent retained a nonsemantic mutant-control comment", 1)
    attack = (ROOT / "fixtures/semaprax-two-value-boolean-law-gaming-v1.spx").read_bytes()
    identity = {"commit": COMMIT, "executable_sha256": RUNTIME.digest(b"semaprax")}
    trial_id = RUNTIME.TASK + ":semaprax-scalar-v1:2"
    claim = {"source_sha256": RUNTIME.digest(attack), "decision": "reject", "reason": "the false result violates the contract"}
    response = {"schema": RUNTIME.EDIT_SCHEMA, "trial_id": trial_id, "language": RUNTIME.LANE,
                "final_source": source, "proof_explanation": "agent claim", "seeded_attack": claim}
    def route(prefix, body, exit_code, stdout, stderr):
        receipt = {"schema": RUNTIME.RECEIPT_SCHEMA, "argv": ["/pinned/semaprax", "run", prefix + ".spx"],
                   "exit_code": exit_code, "source_sha256": RUNTIME.digest(body), "compiler_identity": identity}
        return {"receipt": reference(artifacts, prefix + ".receipt.json", json.dumps(receipt, sort_keys=True).encode()),
                "stdout": reference(artifacts, prefix + ".stdout", stdout), "stderr": reference(artifacts, prefix + ".stderr", stderr)}
    value = {"schema": RUNTIME.INPUT_SCHEMA, "plan_sha256": RUNTIME.PLAN.digest(plan_path), "trial_id": trial_id,
             "compiler_identity": identity, "model_response": reference(artifacts, "response.json", json.dumps(response, sort_keys=True).encode()),
             "final_source": reference(artifacts, "candidate.spx", source.encode()), "seeded_attack_source": reference(artifacts, "attack.spx", attack),
             "candidate": route("candidate", source.encode(), 0, b"0\n", b""),
             "seeded_attack": route("attack", attack, 1, b"", b"language status: postcondition failed\n")}
    path = root / "evidence.json"
    path.write_text(json.dumps(value))
    return path, value


class SemapraxBooleanRuntimeAcceptanceTests(unittest.TestCase):
    def test_noncanonical_but_contract_preserving_source_needs_two_raw_runtime_routes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); artifacts = root / "artifacts"; artifacts.mkdir(); plan_path = plan(root)
            input_path, _ = evidence(root, plan_path, artifacts)
            result = RUNTIME.evaluate(plan_path, input_path, artifacts, COMMIT)
        self.assertEqual(result["status"], "two_input_runtime_authenticated")
        self.assertEqual(result["proof_phase"]["status"], "unavailable")
        self.assertEqual(result["cost_usage"]["status"], "unavailable")
        self.assertIn("not a successful law repair", result["nonclaims"][0])

    def test_successful_candidate_without_attack_rejection_or_contract_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); artifacts = root / "artifacts"; artifacts.mkdir(); plan_path = plan(root)
            input_path, value = evidence(root, plan_path, artifacts)
            value["seeded_attack"]["stderr"] = reference(artifacts, "attack.stderr", b"other error\n")
            input_path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, "required rejection"):
                RUNTIME.evaluate(plan_path, input_path, artifacts, COMMIT)
            input_path, value = evidence(root, plan_path, artifacts)
            bad = b"module app;\nfn negate(value: bool) -> bool { !value }\n"
            value["final_source"] = reference(artifacts, "candidate.spx", bad)
            response = json.loads((artifacts / "response.json").read_text()); response["final_source"] = bad.decode()
            value["model_response"] = reference(artifacts, "response.json", json.dumps(response, sort_keys=True).encode())
            receipt = json.loads((artifacts / "candidate.receipt.json").read_text()); receipt["source_sha256"] = RUNTIME.digest(bad)
            value["candidate"]["receipt"] = reference(artifacts, "candidate.receipt.json", json.dumps(receipt, sort_keys=True).encode())
            input_path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, "pinned Boolean module, identities, and contract"):
                RUNTIME.evaluate(plan_path, input_path, artifacts, COMMIT)


if __name__ == "__main__":
    unittest.main()
