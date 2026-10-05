"""Serve one loopback fake for a cross-language test (tests only).

An optional second argument selects a fake mode (for example `abstain`).
Prints the bound port, then answers each `posts` line on stdin with the number
of POST requests the fake has seen; exits when stdin closes.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fake_servers as fs  # noqa: E402

srv = fs.start(sys.argv[1] if len(sys.argv) > 1 else "laya", sys.argv[2] if len(sys.argv) > 2 else "ok")
print(srv.server_port, flush=True)
for line in sys.stdin:
    if line.strip() == "posts":
        print(sum(1 for x in srv.log if x[0] == "POST"), flush=True)
srv.stop()
