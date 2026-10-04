#!/usr/bin/env python3
"""skill.catalog/v1 conformance fixture: two in-memory skills."""
import hashlib
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, *[".."] * 6, "packages", "semaprax-harness-adapters", "sdk", "python"))
from semaprax_harness_adapter import AdapterError, serve  # noqa: E402

TEXTS = {"brevity": "Prefer short answers.\n", "review": "Review the diff before commit.\n"}
SKILLS = [{"id": k, "name": k, "description": f"{k} skill", "digest": "sha256:" + hashlib.sha256(v.encode()).hexdigest(),
           "bytes": len(v.encode())} for k, v in sorted(TEXTS.items())]


def lst(req):
    limit = req["payload"]["limit"]
    return "complete", {"skills": SKILLS[:limit], "truncated": len(SKILLS) > limit}, []


def load(req):
    want = req["payload"]["digest"]
    for s in SKILLS:
        if s["digest"] == want:
            return "complete", {"digest": want, "artifact_refs": [], "text": TEXTS[s["id"]]}, []
    raise AdapterError("refused", "unknown-digest", "no skill with that digest")


if __name__ == "__main__":
    serve([{"kind": "skill.catalog", "version": 1, "operations": ["list", "load"]}],
          {("skill.catalog", "list"): lst, ("skill.catalog", "load"): load},
          {"provider_id": "org.example/skill-fixture", "adapter_version": "0.1.0", "upstream_version": "builtin-0.1.0"})
