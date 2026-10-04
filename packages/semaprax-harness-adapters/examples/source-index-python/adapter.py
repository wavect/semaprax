#!/usr/bin/env python3
"""semaprax.harness-rpc.v1 adapter for the source index (context.repository/v1)."""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "sdk", "python"))
sys.path.insert(0, HERE)

from index import Index  # noqa: E402
from semaprax_harness_adapter import AdapterError, serve  # noqa: E402

OPS = ["orient", "search", "skeleton", "references"]
_cache = {}


def _index():
    root = os.environ.get("SEMAPRAX_HARNESS_PROJECT_ROOT")
    if not root or not os.path.isdir(root):
        raise AdapterError("unavailable", "no-project-root", "SEMAPRAX_HARNESS_PROJECT_ROOT is not set to a directory")
    if root not in _cache:
        _cache.clear()
        _cache[root] = Index(root)
    return _cache[root]


def handler(op):
    def run(req):
        idx = _index()
        p = req.get("payload") or {}
        if op == "orient":
            out = idx.orient()
        elif op == "search":
            out = idx.search(str(p.get("query", "")), int(p.get("limit", 20)))
        elif op == "skeleton":
            out = idx.skeleton(str(p.get("path", "")))
        else:
            out = idx.references(str(p.get("symbol", "")))
        out["coverage"] = idx.coverage()
        return ("complete" if out["coverage"]["complete"] else "partial"), out, []
    return run


if __name__ == "__main__":
    serve([{"kind": "context.repository", "version": 1, "operations": OPS}],
          {("context.repository", op): handler(op) for op in OPS},
          {"provider_id": "org.example/source-index", "adapter_version": "0.1.0", "upstream_version": "builtin-0.1.0"})
