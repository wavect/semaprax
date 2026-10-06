#!/usr/bin/env python3
"""Opt-in live smoke call through the real adapter: live_smoke.py clef|clef-flash.

Refuses to run unless the host supplies SEMAPRAX_HARNESS_CLOUDFLARE_ACCOUNT_ID,
SEMAPRAX_HARNESS_SECRET_CLOUDFLARE and SEMAPRAX_HARNESS_REMOTE_APPROVED=1.
Makes exactly one billable Workers AI request and prints a redacted summary.
"""

import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "sdk", "python"))
import decision_fixtures as fx  # noqa: E402

NEED = ("SEMAPRAX_HARNESS_CLOUDFLARE_ACCOUNT_ID", "SEMAPRAX_HARNESS_SECRET_CLOUDFLARE")


def main(argv):
    if len(argv) != 2 or argv[1] not in ("clef", "clef-flash"):
        print("usage: live_smoke.py clef|clef-flash", file=sys.stderr)
        return 2
    missing = [n for n in NEED if not os.environ.get(n)]
    if os.environ.get("SEMAPRAX_HARNESS_REMOTE_APPROVED") != "1":
        missing.append("SEMAPRAX_HARNESS_REMOTE_APPROVED=1")
    if missing:
        print("refusing live smoke; host must supply: " + ", ".join(missing), file=sys.stderr)
        return 2
    env = {k: os.environ[k] for k in (*NEED, "SEMAPRAX_HARNESS_REMOTE_APPROVED", "PATH") if k in os.environ}
    env["SEMAPRAX_HARNESS_MODEL"] = "@cf/cloudflare/" + argv[1]
    p = subprocess.Popen([sys.executable, os.path.join(HERE, "adapter.py")], stdin=subprocess.PIPE, stdout=subprocess.PIPE, env=env)

    def rpc(obj):
        p.stdin.write(json.dumps(obj).encode() + b"\n")
        p.stdin.flush()
        return json.loads(p.stdout.readline())

    rpc({"jsonrpc": "2.0", "id": 1, "method": "harness/initialize", "params": {
        "protocol": "semaprax.harness-rpc.v1", "offered": [{"kind": "decision.evaluate", "version": 2}]}})
    req = fx.v2_request(deadline_ms=30000)
    res = rpc({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": req})["result"]
    rpc({"jsonrpc": "2.0", "id": 9, "method": "harness/shutdown"})
    p.communicate(timeout=10)
    ok = res["status"] == "complete"
    if ok:
        fx.validate_v2_result(res["payload"], req["payload"])
    print(json.dumps({"variant": argv[1], "status": res["status"], "diagnostics": res["diagnostics"],
                      "call": (res["payload"] or {}).get("call")}, indent=2, sort_keys=True))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
