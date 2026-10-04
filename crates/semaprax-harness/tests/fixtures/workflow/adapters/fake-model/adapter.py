#!/usr/bin/env python3
"""Fixture model.generate adapter. The task goal selects the behaviour:
MODE:good | MODE:bad | MODE:publish (adds a forbidden `publish` member) |
MODE:claims (adds a fabricated passing-test claim). The SDK file is copied
next to this adapter by the test."""
import base64
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from semaprax_harness_adapter import serve  # noqa: E402


def body(right):
    return {"kind": "binary", "op": "*", "left": {"kind": "place", "name": "price"}, "right": right}


def generate(req):
    prompt = json.loads(base64.b64decode(req["payload"]["input_base64"]))
    goal = prompt.get("goal", "")
    right = {"kind": "i64", "value": 2} if "MODE:bad" in goal else {"kind": "place", "name": "qty"}
    out = {"schema": "semaprax.harness-proposal.v1",
           "intent": {"kind": "replace_function_body", "target": prompt.get("seed") or "ledger.line_total", "body": body(right)}}
    if "MODE:claims" in goal:
        out["claims"] = {"tests_passed": True}
    raw = json.dumps(out).encode()
    payload = {"model": req["payload"]["model"], "output_base64": base64.b64encode(raw).decode(),
               "usage": {"input_bytes": len(req["payload"]["input_base64"]), "output_bytes": len(raw)}}
    if "MODE:publish" in goal:
        payload["publish"] = True
    return "complete", payload, []


if __name__ == "__main__":
    serve([{"kind": "model.generate", "version": 1, "operations": ["generate"]}],
          {("model.generate", "generate"): generate},
          {"provider_id": "org.example/fake-model", "adapter_version": "0.1.0", "upstream_version": "builtin-0.1.0"})
