#!/usr/bin/env python3
"""Graft/Graphify-shaped fixture provider for HP-05 tests.

Differences from the source index, on purpose: provider-local float ranks on a
different scale, `inferred` provenance for documentation, unsupported-language
files reported in coverage, `no_references` only when the index is exhaustive,
and a false `compiler-verified` claim on `.spx` lines that the broker must refuse.
"""
import hashlib
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "..", "..", "..", "..", "packages", "semaprax-harness-adapters", "sdk", "python"))
from semaprax_harness_adapter import AdapterError, serve  # noqa: E402

LANGS = {".py": ("python", "structural"), ".rs": ("rust", "structural"), ".ts": ("typescript", "structural"),
         ".md": ("markdown", "inferred"), ".spx": ("semaprax", "compiler-verified")}
SKIP = {".git", "target", "node_modules"}
OPS = ["orient", "search", "skeleton", "references"]


def scan(root):
    files, skipped = {}, []
    for d, dirs, names in os.walk(root):
        dirs[:] = sorted(x for x in dirs if x not in SKIP)
        for n in sorted(names):
            full = os.path.join(d, n)
            rel = os.path.relpath(full, root).replace(os.sep, "/")
            ext = os.path.splitext(n)[1]
            if ext == "" or n.startswith(".") or n.endswith(".toml"):
                continue
            if ext not in LANGS:
                skipped.append({"path": rel, "reason": "unsupported-language"})
                continue
            try:
                files[rel] = open(full, "rb").read().decode("utf-8").split("\n")
            except UnicodeDecodeError:
                skipped.append({"path": rel, "reason": "not-utf8"})
    return files, skipped


def item(files, rel, no, rank):
    lang, prov = LANGS[os.path.splitext(rel)[1]]
    line = files[rel][no - 1]
    return {"path": rel, "span": {"start_line": no, "end_line": no}, "digest": "sha256:" + hashlib.sha256(line.encode()).hexdigest(),
            "provenance": prov, "language": lang, "rank": rank, "text": line}


def handler(op):
    def run(req):
        root = os.environ.get("SEMAPRAX_HARNESS_PROJECT_ROOT")
        if not root or not os.path.isdir(root):
            raise AdapterError("unavailable", "no-root", "no project root")
        files, skipped = scan(root)
        p = req.get("payload") or {}
        items = []
        if op in ("search", "references"):
            word = str(p.get("query") if op == "search" else p.get("symbol"))
            toks = re.findall(r"[A-Za-z_][A-Za-z0-9_]*", word)
            for rel in sorted(files):
                for i, line in enumerate(files[rel], 1):
                    hits = sum(len(re.findall(r"\b%s\b" % re.escape(t), line)) for t in toks)
                    if hits:
                        items.append(item(files, rel, i, round(0.25 + 0.5 / hits, 3)))
            items.sort(key=lambda x: (-x["rank"], x["path"], x["span"]["start_line"]))
            items = items[: int(p.get("max_items", 20))]
        complete = not skipped
        out = {"items": items, "coverage": {"complete": complete, "indexed_files": len(files), "skipped": skipped[:50], "exhaustive": complete}}
        if op == "references" and complete and not items:
            out["no_references"] = True
        return ("complete" if complete else "partial"), out, []
    return run


if __name__ == "__main__":
    serve([{"kind": "context.repository", "version": 1, "operations": OPS}], {("context.repository", o): handler(o) for o in OPS},
          {"provider_id": "org.example/graph-index", "adapter_version": "0.1.0", "upstream_version": None})
