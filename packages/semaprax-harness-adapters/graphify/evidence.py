#!/usr/bin/env python3
"""Reproduces the EVIDENCE.md numbers: cold extract vs warm query, graph vs source bytes.

usage: evidence.py <project-root> <scratch-cache-dir> <query> <symbol>
Runs the real adapter (and thus real graphify) against the project; writes only under the cache dir.
"""
import json, os, sys, time
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "test"))
import test_adapter as t  # noqa: E402

root, cache, query, symbol = sys.argv[1:5]
os.makedirs(cache, exist_ok=True)
c = t.Client(os.path.realpath(root), cache)
c.init()
def timed(op, payload):
    t0 = time.perf_counter(); r = c.call(op, payload); return r, time.perf_counter() - t0
r, cold = timed("search", {"query": query})
r2, warm = timed("search", {"query": query})
r3, refs = timed("references", {"symbol": symbol})
c.close()
cov = r["payload"]["coverage"]
out = os.path.join(cache, "graphify-index", "graphify-out")
manifest = json.load(open(os.path.join(out, "manifest.json")))
src = sum(os.path.getsize(os.path.join(root, k)) for k in manifest)
print(json.dumps({
    "cold_search_s": round(cold, 3), "warm_search_s": round(warm, 3), "warm_references_s": round(refs, 3),
    "search_status": r["status"], "indexed_files": cov["indexed_files"], "skipped_files": len(cov["skipped"]),
    "extraction_errors": cov["extraction_errors"], "search_items": len(r["payload"]["items"]),
    "search_result_bytes": len(json.dumps(r)), "references_items": len(r3["payload"]["items"]),
    "references_inferred": sum(i["provenance"] == "inferred" for i in r3["payload"]["items"]),
    "references_status": r3["status"], "references_exhaustive": r3["payload"]["coverage"]["exhaustive"],
    "graph_json_bytes": os.path.getsize(os.path.join(out, "graph.json")), "indexed_source_bytes": src,
    "cache_dir_total_bytes": sum(os.path.getsize(os.path.join(b, f)) for b, _, fs in os.walk(cache) for f in fs),
}, indent=1, sort_keys=True))
