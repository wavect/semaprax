#!/usr/bin/env python3
"""WikiSkill evolution bridge for `skill.evolve/v1` (HN-15).

Protocol: one JSON request on stdin, one JSON result on stdout (see
docs/HARNESS-EVOLUTION-V1.md). Exit 69 with {"unavailable": reason} when the
evolution backend cannot run, which the host reports as outcome `unavailable`.

Status: the call path into the real `wikiskill` CLI (`maintain`, `propose`,
`run-task`) is NOT implemented. The audited candidate
(ashutoshsinghpr7/wikiskill 0.1.5, MIT) only drives hermes, claude, codex or
copilot agent CLIs; none runs against a local Ollama/OpenAI-compatible
endpoint, so the bridge could not be validated without remote model spend.
This file therefore only performs the truthful preflight.
"""
import json
import os
import shutil
import sys


def main() -> int:
    try:
        json.load(sys.stdin)
    except ValueError:
        print(json.dumps({"unavailable": "request is not JSON"}))
        return 69
    binary = os.environ.get("WIKISKILL_BIN") or shutil.which("wikiskill")
    backend = os.environ.get("WIKISKILL_BACKEND", "")
    if not binary:
        reason = "wikiskill executable not found (set WIKISKILL_BIN to a pinned install)"
    elif backend not in ("hermes", "claude", "codex", "copilot"):
        reason = "no supported agent backend configured (WIKISKILL_BACKEND must be hermes, claude, codex or copilot)"
    else:
        reason = "bridge into the wikiskill CLI is not implemented or validated; no local-model backend exists"
    print(json.dumps({"unavailable": reason}))
    return 69


if __name__ == "__main__":
    sys.exit(main())
