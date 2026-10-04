"""Generic stdio host for semaprax.harness-bridge.v1: drives one scripted
session against `<bridge command...>` and prints the replies as one JSON list."""
import json
import subprocess
import sys

proto = "semaprax.harness-bridge.v1"
good = {"protocol": proto, "version": 1, "host": {"name": "fake-host", "version": "0"},
        "capabilities": {"semantic_query": True, "command_wrapper": True, "cancellation": True}}
script = [
    ("bridge/status", {}),  # before handshake: refused
    ("bridge/handshake", dict(good, protocol="semaprax.harness-bridge.v0")),
    ("bridge/handshake", good),
    ("bridge/status", {}),
    ("bridge/context", {"query": "add", "max_bytes": 8192}),
    ("bridge/command_view", {"argv": ["echo", "hello-bridge"]}),
    ("bridge/publish", {}),
    ("bridge/cancel", {"id": 1}),
]
p = subprocess.Popen(sys.argv[1:], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
replies = []
for i, (method, params) in enumerate(script, 1):
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i, "method": method, "params": params}) + "\n")
    p.stdin.flush()
    replies.append(json.loads(p.stdout.readline()))
p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 99, "method": "bridge/shutdown"}) + "\n")
p.stdin.flush()
p.stdin.close()
p.wait(timeout=10)
print(json.dumps({"replies": replies, "exit": p.returncode}))
