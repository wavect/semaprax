#!/usr/bin/env python3
"""Quick protocol driver: initialize + one invoke + shutdown, printing each frame.

usage: drive.py --kind K --op OP [--payload JSON] [--root DIR] [--version N]
                [--env NAME[=VALUE] ...] -- <adapter argv...>
Exit 0 when every response is a JSON-RPC result, 1 otherwise.
"""
import argparse
import json
import os
import subprocess
import sys


def frame(obj):
    return json.dumps(obj, separators=(",", ":"), sort_keys=True).encode() + b"\n"


def drive(argv, kind, op, payload, root=None, version=1, env_extra=(), timeout=20):
    env = dict(os.environ)
    if root:
        env["SEMAPRAX_HARNESS_PROJECT_ROOT"] = root
    for item in env_extra:
        k, eq, v = item.partition("=")
        if eq:
            env[k] = v
        else:  # bare NAME unsets the variable
            env.pop(k, None)
    project = {"id": "p" * 64, "worktree": "w" * 64, "revision": "r" * 64}
    cap = {"kind": kind, "version": version}
    msgs = [
        {"jsonrpc": "2.0", "id": 1, "method": "harness/initialize", "params": {
            "protocol": "semaprax.harness-rpc.v1", "host_version": "drive-0", "descriptor_digest": "d" * 64,
            "offered": [cap], "project": project}},
        {"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": {
            "schema": "semaprax.harness-request.v1", "invocation_id": "inv-000001", "project": project,
            "lock_digest": "l" * 64, "capability": cap, "operation": op, "deadline_ms": 30000,
            "budget": {"max_result_bytes": 65536, "remaining_calls": 8}, "lineage": [], "payload": payload}},
        {"jsonrpc": "2.0", "id": 3, "method": "harness/shutdown", "params": {}},
    ]
    proc = subprocess.run(argv, input=b"".join(frame(m) for m in msgs), stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, env=env, timeout=timeout)
    replies = [json.loads(l) for l in proc.stdout.splitlines() if l.strip()]
    return replies, proc.stderr.decode(errors="replace"), proc.returncode


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--kind", required=True)
    ap.add_argument("--op", required=True)
    ap.add_argument("--payload", default="{}")
    ap.add_argument("--root")
    ap.add_argument("--version", type=int, default=1)
    ap.add_argument("--env", action="append", default=[])
    ap.add_argument("argv", nargs=argparse.REMAINDER)
    a = ap.parse_args()
    argv = a.argv[1:] if a.argv[:1] == ["--"] else a.argv
    replies, err, code = drive(argv, a.kind, a.op, json.loads(a.payload), a.root, a.version, a.env)
    for r in replies:
        print(json.dumps(r, indent=2, sort_keys=True))
    if err:
        sys.stderr.write(err)
    return 0 if len(replies) == 3 and all("result" in r for r in replies) and code == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
