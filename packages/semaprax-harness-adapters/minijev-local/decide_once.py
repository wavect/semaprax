#!/usr/bin/env python3
"""One decision through the real adapter over the SDK frame protocol (EXPERIMENTAL demo).

Needs a running worker (see README.md) and the same two environment variables the
host would pass: SEMAPRAX_HARNESS_MINIJEV_ADDR and SEMAPRAX_HARNESS_SECRET_MINIJEV.
Chooses between two candidate models for a fixed mechanical-task fixture and prints
the full result envelope. An uncalibrated demonstration, not routing-accuracy evidence.
"""

import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "sdk", "python"))
import decision_fixtures as fx  # noqa: E402


def main():
    req = fx.v2_request(n=2, deadline_ms=int(os.environ.get("DECIDE_DEADLINE_MS", "30000")))
    env = {k: v for k, v in os.environ.items() if k.startswith("SEMAPRAX_HARNESS_") or k in ("PATH", "HOME")}
    p = subprocess.Popen([sys.executable, os.path.join(HERE, "adapter.py")], stdin=subprocess.PIPE, stdout=subprocess.PIPE, env=env)

    def rpc(obj):
        p.stdin.write(json.dumps(obj).encode() + b"\n")
        p.stdin.flush()
        return json.loads(p.stdout.readline())
    rpc({"jsonrpc": "2.0", "id": 1, "method": "harness/initialize", "params": {
        "protocol": "semaprax.harness-rpc.v1", "offered": [{"kind": "decision.evaluate", "version": 2}]}})
    res = rpc({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": req})["result"]
    rpc({"jsonrpc": "2.0", "id": 3, "method": "harness/shutdown"})
    p.wait(timeout=5)
    print(json.dumps(res, indent=2, sort_keys=True))
    return 0 if res["status"] == "complete" else 1


if __name__ == "__main__":
    sys.exit(main())
