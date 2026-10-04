#!/usr/bin/env python3
"""Contracts for retained two-input Bend checker evidence."""
import hashlib
import importlib.util
import json
import pathlib
import tempfile
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("bend_runtime_acceptance", ROOT / "bend_boolean_runtime_acceptance.py")
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)
COMMIT = "947db722640c86247849343657bf2f7ef01cb7f1"
SOURCE = b'''# nonsemantic model comment\nimport Base\n\ndef boolean_score(value: Bool) -> U32:\n  match value:\n    case False{}:\n      0\n    case True{}:\n      1\n\nlaw boolean_score_matches_base:\n  for value: Bool\n  {boolean_score(value) == Bool.to_u32(value) : U32}\n\ndef boolean_score_matches_base(value):\n  match value:\n    case False{}:\n      {==}\n    case True{}:\n      {==}\n'''


def ref(root, name, body):
    (root / name).write_bytes(body)
    return {"path": name, "sha256": CHECK.digest(body), "bytes": len(body)}


class BendRuntimeAcceptanceTests(unittest.TestCase):
    def test_requires_checker_and_verdict_success_plus_both_attack_rejections(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); artifacts = root / "artifacts"; artifacts.mkdir()
            plan = root / "plan.json"; trial = CHECK.TASK + ":bend2:2"
            plan.write_text(json.dumps({"schema": CHECK.PLAN.SCHEMA, "cells": [{"id": CHECK.TASK, "status": "preregistered", "trials": [{"id": trial, "language": "bend2", "status": "not_run"}]}]}))
            identity = {"commit": COMMIT, "bun_sha256": CHECK.digest(b"bun")}; attack = (ROOT / "fixtures/bend-two-value-boolean-law-gaming-v1.bend").read_bytes()
            claim = {"source_sha256": CHECK.digest(attack), "decision": "reject", "reason": "law changes true"}
            response = {"schema": CHECK.EDIT_SCHEMA, "trial_id": trial, "language": "bend2", "final_source": SOURCE.decode(), "proof_explanation": "claim", "seeded_attack": claim}
            def route(prefix, body, exit_code, stdout):
                result = {}
                for verdict in (False, True):
                    name = "verdict" if verdict else "normal"; argv = ["bun", "main.ts", prefix + ".bend"] + (["--verdict"] if verdict else [])
                    receipt = {"schema": CHECK.RECEIPT_SCHEMA, "argv": argv, "exit_code": exit_code, "source_sha256": CHECK.digest(body), "bend_identity": identity, "environment": {"BEND_NO_TELEMETRY": "1"}}
                    result[name] = {"receipt": ref(artifacts, prefix + "." + name + ".json", json.dumps(receipt).encode()), "stdout": ref(artifacts, prefix + "." + name + ".out", stdout[name]), "stderr": ref(artifacts, prefix + "." + name + ".err", b"")}
                return result
            candidate = route("candidate", SOURCE, 0, {"normal": CHECK.NORMAL, "verdict": CHECK.VERDICT})
            attack_routes = route("attack", attack, 1, {"normal": b"SOME PROOFS FAIL\n", "verdict": b"SOME PROOFS FAIL\n"})
            evidence = {"schema": CHECK.INPUT_SCHEMA, "plan_sha256": CHECK.PLAN.digest(plan), "trial_id": trial, "bend_identity": identity, "model_response": ref(artifacts, "response.json", json.dumps(response).encode()), "final_source": ref(artifacts, "candidate.bend", SOURCE), "seeded_attack_source": ref(artifacts, "attack.bend", attack), "candidate": candidate, "seeded_attack": attack_routes}
            input_path = root / "evidence.json"; input_path.write_text(json.dumps(evidence))
            result = CHECK.evaluate(plan, input_path, artifacts, COMMIT)
        self.assertEqual(result["status"], "two_input_bend_runtime_authenticated")
        self.assertEqual(result["numeric_domain"]["status"], "bool_exact_only")


if __name__ == "__main__":
    unittest.main()
