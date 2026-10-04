#!/usr/bin/env python3
"""Pilot evaluation: rules-only vs a SystemOne adapter on the seed corpus.

PILOT ONLY. Seed labels are author priors, not downstream outcomes, so the
numbers describe wiring and cost, never routing quality. See
docs/HARNESS-LAYA-JEV-V1.md "Evaluation design and default-enablement gate".

usage: eval_pilot.py <adapter.py> <corpus-dir>   (env: SEMAPRAX_HARNESS_ENDPOINT, ...)
"""

import json
import os
import subprocess
import sys
import time

OPTIONS = ["m-cheap", "m-mid", "m-strong"]
COST = {"m-cheap": 1, "m-mid": 3, "m-strong": 10}  # relative proxy units, not money
RANK = {"m-cheap": 0, "m-mid": 1, "m-strong": 2}
PROJECT = {"id": "p" * 64, "worktree": "w" * 64, "revision": "r" * 64}


def rules(f):
    return "m-strong" if f["task_family"] == "semantic_law" else "m-cheap"


def accepted_cost(route, best):
    """Proxy: a route below the prior-best fails, escalates one step up, and pays again."""
    cost, cur = 0, route
    while True:
        cost += COST[cur]
        if RANK[cur] >= RANK[best] or cur == "m-strong":
            return cost, RANK[route] >= RANK[best]
        cur = OPTIONS[RANK[cur] + 1]


class Client:
    def __init__(self, adapter):
        self.p = subprocess.Popen([sys.executable, adapter], stdin=subprocess.PIPE, stdout=subprocess.PIPE, env=os.environ)
        self.rpc({"jsonrpc": "2.0", "id": 1, "method": "harness/initialize", "params": {"protocol": "semaprax.harness-rpc.v1", "offered": [{"kind": "decision.evaluate", "version": 1}]}})

    def rpc(self, m):
        self.p.stdin.write(json.dumps(m).encode() + b"\n")
        self.p.stdin.flush()
        return json.loads(self.p.stdout.readline())

    def decide(self, n, features, deadline_ms):
        req = {"schema": "semaprax.harness-request.v1", "invocation_id": f"pilot-{n:04d}", "project": PROJECT, "lock_digest": "l" * 64,
               "capability": {"kind": "decision.evaluate", "version": 1}, "operation": "evaluate", "deadline_ms": deadline_ms,
               "budget": {"max_result_bytes": 65536, "remaining_calls": 1}, "lineage": [],
               "payload": {"task": "model-route/v1", "features": features, "options": OPTIONS}}
        t = time.monotonic()
        res = self.rpc({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": req})["result"]
        return res, (time.monotonic() - t) * 1000


def ece(pairs, bins=3):
    if not pairs:
        return None
    tot = 0.0
    for b in range(bins):
        lo, hi = b / bins, (b + 1) / bins
        s = [(c, ok) for c, ok in pairs if lo <= c < hi or (b == bins - 1 and c == 1.0)]
        if s:
            tot += len(s) / len(pairs) * abs(sum(c for c, _ in s) / len(s) - sum(ok for _, ok in s) / len(s))
    return round(tot, 4)


def main():
    adapter, corpus = sys.argv[1], sys.argv[2]
    items = [json.loads(l) for l in open(os.path.join(corpus, "seed.jsonl"))]
    ev = [i for i in items if i["split"] == "eval"]
    cl = Client(adapter)
    warm, wms = cl.decide(0, ev[0]["features"], 60000)  # cold load is measured separately
    rows, lat, pairs = [], [], []
    stats = {"rules": {"agree": 0, "cost": 0, "accepted": 0}, "adapter": {"agree": 0, "cost": 0, "accepted": 0, "abstain": 0, "refused": 0}}
    for n, it in enumerate(ev, 1):
        f, best = it["features"], it["label"]["best_route"]
        rc, racc = accepted_cost(rules(f), best)
        stats["rules"]["agree"] += rules(f) == best
        stats["rules"]["cost"] += rc
        stats["rules"]["accepted"] += racc
        res, ms = cl.decide(n, f, 60000)
        lat.append(ms)
        pay = res.get("payload")
        if res["status"] != "complete" or pay is None or pay["abstain"]:
            stats["adapter"]["abstain" if pay else "refused"] += 1
            route = rules(f)  # explicit rules fallback
        else:
            route = pay["choice"]
            pairs.append((pay["scores"][route], route == best))
        ac, aacc = accepted_cost(route, best)
        stats["adapter"]["agree"] += route == best
        stats["adapter"]["cost"] += ac
        stats["adapter"]["accepted"] += aacc
        rows.append({"id": it["id"], "family": it["family"], "prior_best": best, "rules": rules(f), "adapter": route})
    lat.sort()
    out = {"label": "PILOT ONLY: seed-author-prior labels, n=%d, not evidence" % len(ev),
           "first_call_ms": round(wms, 1), "stats": stats, "n": len(ev),
           "adapter_latency_ms": {"mean": round(sum(lat) / len(lat), 1), "p95": round(lat[int(0.95 * (len(lat) - 1))], 1), "max": round(lat[-1], 1)},
           "calibration": {"ece_3bin_vs_author_prior": ece(pairs), "n": len(pairs), "note": "n too small and labels unmeasured: uninformative"},
           "rows": rows}
    print(json.dumps(out, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
