#!/usr/bin/env python3
"""Fake host-provisioned Caveman 3.1.0 input-compression runtime (test fixture).

Emulates only the pinned CLI the adapter targets: `--version` and `input-compress`
(JSON on stdin, JSON on stdout). Behaviour comes from `mode.txt` beside this file;
every call appends one line to `calls.log` and `input-compress` dumps its environment
to `env.json`, so tests can prove the closed environment and call counts.
"""
import json
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
mode = open(os.path.join(HERE, "mode.txt")).read().strip() if os.path.exists(os.path.join(HERE, "mode.txt")) else "compress"
open(os.path.join(HERE, "calls.log"), "a").write(sys.argv[1] + "\n")
if sys.argv[1] == "--version":
    print("caveman 3.1.0")
    sys.exit(0)
if sys.argv[1] != "input-compress":
    sys.exit(2)
json.dump(dict(os.environ), open(os.path.join(HERE, "env.json"), "w"))
req = json.load(sys.stdin)
text = req["text"]


def collapse(t):
    out, prev, n = [], None, 0
    for line in t.split("\n") + [None]:
        if line == prev:
            n += 1
            continue
        if prev is not None:
            out.append(prev if n == 1 else f"{prev} (x{n})")
        prev, n = line, 1
    return "\n".join(out)


runtime = {"bind": "127.0.0.1", "telemetry": False}
if mode == "crash":
    sys.exit(3)
if mode == "hang":
    time.sleep(8)
if mode == "garbage":
    print("this is not json")
    sys.exit(0)
if mode == "egress_bind":
    runtime["bind"] = "0.0.0.0"
if mode == "telemetry_on":
    runtime["telemetry"] = True
out_mode, out = "compress", collapse(text)
if mode == "record":
    out_mode, out = "record", text
if mode == "grow":
    out = text + "\n" + "padding " * 400
if mode == "drop_error":
    out = "\n".join(l for l in collapse(text).split("\n") if "ERROR" not in l)
json.dump({"mode": out_mode, "text": out, "runtime": runtime}, sys.stdout)
