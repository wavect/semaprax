#!/usr/bin/env python3
"""Regenerates corpus.json (pins included) next to this script.

The corpus is data; this generator exists so the pins are recomputed, never
hand-edited. Usage: python3 gen_corpus.py   (run inside a git checkout).
"""
import hashlib
import json
import os
import subprocess

B = os.path.dirname(os.path.abspath(__file__))


def tree_digest(d):
    rows = []

    def walk(dd):
        for n in sorted(os.listdir(dd), key=lambda x: x.encode()):
            p = os.path.join(dd, n)
            if os.path.isdir(p):
                walk(p)
            else:
                rows.append((os.path.relpath(p, d).replace(os.sep, "/"), "sha256:" + hashlib.sha256(open(p, "rb").read()).hexdigest()))

    walk(d)
    return "sha256:" + hashlib.sha256("".join(f"{a}\0{b}\n" for a, b in rows).encode()).hexdigest()


projects = {k: {"path": f"projects/{k}"} for k in ["downstream", "mixed", "harness-src", "scripts"]}
pins = {k: tree_digest(os.path.join(B, v["path"])) for k, v in projects.items()}
rw = lambda: {"read": ["project"], "write": "none", "publish": False}
rc = lambda: {"read": ["project"], "write": "candidate", "publish": False}
bud = lambda b=16384, c=6: {"max_visible_bytes": b, "max_calls": c}
noisy = ["{python}", "{projects}/scripts/noisy_test_run.py", "--counter", "{counter}"]
crit = ["case_173 ... FAILED", "assertion failed: ledger.line_total(3, 4) == 12"]


def plan(i, cost, rank):
    return {"id": i, "destination": {"kind": "local"}, "capabilities": ["structured_output", "tools"], "max_context": 200000,
            "est_cost_micros": cost, "est_latency_ms": 1000, "strength_rank": rank}


def route(fam):
    return {"task": "model-route/v1",
            "features": {"task_family": fam, "estimated_context_tokens": 6000, "requires_structured_output": False,
                         "requires_tools": True, "confidentiality": "project", "latency_class": "interactive"},
            "budget": {"max_cost_micros": 1000000, "max_latency_ms": 60000, "max_router_calls": 1},
            "catalog": [plan("m-cheap", 10, 1), plan("m-mid", 30, 2), plan("m-strong", 100, 3)]}


ledger_files = ["src/lib.spx", "src/report.spx", "src/tests.spx", "src/examples.spx"]
wf = lambda: {"kind": "workflow", "proposal": "proposals/valid.json", "task": "task-ledger.json", "expect_status": "approved-candidate-ready"}
tasks = [
    {"id": "orient-ledger", "family": "orientation", "project": "downstream", "request": "Orient in the ledger package: which declarations take part in invoice_total?", "authority": rw(),
     "query": "ledger.invoice_total", "max_bytes": 4096, "baseline_files": ledger_files,
     "required_facts": ["ledger.invoice_total", "ledger.line_total", "ledger.tests.main"], "acceptance": [{"kind": "facts"}], "budget": bud(),
     "question": {"ask": "Which function does ledger.invoice_total call?", "expect": ["line_total"]}},
    {"id": "reuse-line-total-contract", "family": "api_reuse", "project": "downstream", "request": "Reuse ledger.line_total: state its postcondition before calling it.", "authority": rw(),
     "query": "ledger.line_total", "max_bytes": 4096, "baseline_files": ["src/lib.spx"],
     "required_facts": ["ledger.line_total", "ensures result == price * qty"], "acceptance": [{"kind": "facts"}], "budget": bud(),
     "question": {"ask": "What does ledger.line_total ensure about its result?", "expect": ["price * qty"]}},
    {"id": "diagnose-ledger-failure", "family": "failing_test_diagnosis", "project": "downstream",
     "request": "The ledger test run fails: find the failing case and the declaration under test, then propose a repair the compiler accepts.", "authority": rc(),
     "query": "ledger.line_total", "max_bytes": 4096, "baseline_files": ledger_files, "required_facts": ["ledger.line_total", "ledger.invoice_total"],
     "acceptance": [{"kind": "facts"}, {"kind": "command", "argv": noisy, "critical": crit}, wf()], "budget": bud(32768, 8)},
    {"id": "diagnose-noisy-run", "family": "failing_test_diagnosis", "project": "downstream", "request": "Read a 320-test run and name the one failing case.", "authority": rw(),
     "baseline_files": [], "required_facts": [], "acceptance": [{"kind": "command", "argv": noisy, "critical": crit}], "budget": bud(16384, 3)},
    {"id": "repair-ledger-two-files", "family": "multi_file_repair", "project": "downstream",
     "request": "invoice_total in report.spx is wrong because line_total in lib.spx is wrong: repair it so the contract holds.", "authority": rc(),
     "query": "ledger.invoice_total", "max_bytes": 4096, "baseline_files": ledger_files, "required_facts": ["src/lib.spx", "src/report.spx", "ledger.line_total"],
     "acceptance": [{"kind": "facts"}, wf()], "budget": bud(24576, 6)},
    {"id": "law-sound-fix", "family": "law_effect_change", "project": "downstream", "request": "Fix line_total without touching its contract or effects.", "authority": rc(),
     "query": "ledger.line_total", "max_bytes": 4096, "baseline_files": ["src/lib.spx"], "required_facts": ["ensures result == price * qty"],
     "acceptance": [{"kind": "facts"}, wf()], "budget": bud(24576, 6)},
    {"id": "law-weakening-refused", "family": "law_effect_change", "project": "downstream",
     "request": "A proposal tries to pass by weakening requirements: the host must refuse it.", "authority": rc(),
     "baseline_files": [], "required_facts": [],
     "acceptance": [{"kind": "workflow", "proposal": "proposals/weak-requirements.json", "task": "task-ledger.json", "expect_status": "refused"}], "budget": bud(8192, 2)},
    {"id": "orient-mixed", "family": "orientation", "project": "mixed", "request": "Orient in the calculator project: what does main use?", "authority": rw(),
     "query": "calculator.app.main", "max_bytes": 4096, "baseline_files": ["src/app.spx", "src/core.spx", "src/tests.spx"],
     "required_facts": ["calculator.app.main", "calculator.multiply", "calculator.add"], "acceptance": [{"kind": "facts"}], "budget": bud()},
    {"id": "refactor-multiply-callers", "family": "mechanical_refactor", "project": "mixed", "request": "Rename multiply safely: list every caller first.", "authority": rw(),
     "query": "calculator.multiply", "max_bytes": 4096, "baseline_files": ["src/app.spx", "src/core.spx", "src/tests.spx"], "required_facts": ["calculator.multiply"],
     "acceptance": [{"kind": "facts"}, {"kind": "references", "symbol": "multiply", "callers": ["src/app.spx", "src/tests.spx"]}], "budget": bud()},
    {"id": "reuse-cross-language-add", "family": "api_reuse", "project": "mixed", "request": "Find the Rust and TypeScript wrappers of the add export before writing a third.", "authority": rw(),
     "query": "add", "max_bytes": 4096, "baseline_files": ["src/host.rs", "web/app.ts", "src/core.spx"],
     "required_facts": ["pub fn run_add", "export function renderAdd"], "acceptance": [{"kind": "facts"}], "budget": bud(),
     "question": {"ask": "Which Rust function wraps the add export?", "expect": ["run_add"]}},
    {"id": "orient-observer", "family": "orientation", "project": "harness-src", "request": "Orient in the observation module: where are events recorded and what are the limits?", "authority": rw(),
     "query": "Observer", "max_bytes": 8192, "baseline_files": ["observe/sink.rs", "observe/event.rs", "observe/aggregate.rs", "observe/mod.rs"],
     "required_facts": ["pub struct Observer", "pub fn record"], "acceptance": [{"kind": "facts"}], "budget": bud(),
     "question": {"ask": "Which method of Observer records an event?", "expect": ["record"]}},
    {"id": "reuse-byte-tokenizer", "family": "api_reuse", "project": "harness-src", "request": "Reuse the byte measurement instead of writing a new counter.", "authority": rw(),
     "query": "ByteTokenizer", "max_bytes": 8192, "baseline_files": ["observe/tokenizer.rs"], "required_facts": ["pub struct ByteTokenizer", "pub fn measure"],
     "acceptance": [{"kind": "facts"}], "budget": bud(),
     "question": {"ask": "Which function counts the exact text given with a tokenizer?", "expect": ["measure"]}},
    {"id": "refactor-host-traffic-callers", "family": "mechanical_refactor", "project": "harness-src", "request": "Rename HostTraffic: list every file that uses it.", "authority": rw(),
     "query": "HostTraffic", "max_bytes": 8192, "baseline_files": ["observe/sink.rs", "observe/aggregate.rs", "observe/report.rs", "observe/mod.rs"],
     "required_facts": ["pub struct HostTraffic"],
     "acceptance": [{"kind": "facts"}, {"kind": "references", "symbol": "HostTraffic", "callers": ["observe/aggregate.rs", "observe/report.rs", "observe/mod.rs"]}], "budget": bud()},
    {"id": "route-mechanical", "family": "mechanical_refactor", "project": "mixed", "request": "Route a mechanical rename to the cheapest admissible model.", "authority": rw(),
     "baseline_files": [], "required_facts": [], "acceptance": [{"kind": "route", "request": route("mechanical"), "allowed": ["m-cheap"], "forbidden": []}], "budget": bud(1024, 2)},
    {"id": "route-semantic-law", "family": "law_effect_change", "project": "mixed", "request": "Route a law-sensitive change: the strongest admissible model, never a cheaper one.", "authority": rw(),
     "baseline_files": [], "required_facts": [], "acceptance": [{"kind": "route", "request": route("semantic_law"), "allowed": ["m-strong"], "forbidden": ["m-cheap"]}], "budget": bud(1024, 2)},
]
inst = lambda desc, prov, cap, **k: dict({"descriptor": desc, "provider": prov, "capability": cap}, **k)
A = "repo:packages/semaprax-harness-adapters/"
profiles = [
    {"id": "native-only", "baseline": True, "description": "Compiler-native context, host raw command view, rules decision. No external provider."},
    {"id": "native+source-index", "description": "Third-party example adapter (examples/source-index-python) through the same contract; proves no vendor special cases.",
     "installs": [inst(A + "examples/source-index-python/harness-provider.json", "org.example/source-index", "context.repository", strip_upstream=True)], "requires_env": ["HARNESS_PYTHON"]},
    {"id": "native+graft", "description": "Native plus the Graft repository-context adapter.",
     "installs": [inst(A + "graft/harness-provider.json", "org.nanonets/graft-context", "context.repository", upstream_env="HARNESS_GRAFT")], "requires_env": ["HARNESS_GRAFT", "HARNESS_NODE"]},
    {"id": "native+graphify", "description": "Native plus the Graphify repository-context adapter.",
     "installs": [inst(A + "graphify/harness-provider.json", "com.graphify-labs/graphify-context", "context.repository", upstream_env="HARNESS_GRAPHIFY")], "requires_env": ["HARNESS_GRAPHIFY", "HARNESS_PYTHON"]},
    {"id": "rtk", "description": "Raw command view versus the RTK command-view adapter.",
     "installs": [inst(A + "rtk/harness-provider.json", "ai.rtk/rtk-command-view", "command.view", upstream_env="HARNESS_RTK")], "requires_env": ["HARNESS_RTK", "HARNESS_PYTHON"]},
    {"id": "laya", "description": "Rules decision versus the Laya learned router (decision.evaluate adapter).",
     "router": {"descriptor": A + "systemone/laya-local/harness-provider.json", "runtime_env": "HARNESS_PYTHON",
                "env": {"SEMAPRAX_HARNESS_ENDPOINT": "http://127.0.0.1:18427"}, "provider_id": "ai.convai/laya-decision",
                "model_id": "laya-multilingual", "checkpoint": "convaiinnovations/laya@7b928d82:multilingual"},
     "requires_env": ["HARNESS_PYTHON"],
     "skip": "untested: resource (disk); real local Laya evidence exists in docs/HARNESS-LAYA-JEV-V1.md (HP-11), not repeated here"},
    {"id": "local-efficient", "description": "Combined: native + Graft context + RTK command view + rules decision.",
     "installs": [inst(A + "graft/harness-provider.json", "org.nanonets/graft-context", "context.repository", upstream_env="HARNESS_GRAFT"),
                  inst(A + "rtk/harness-provider.json", "ai.rtk/rtk-command-view", "command.view", upstream_env="HARNESS_RTK")],
     "requires_env": ["HARNESS_GRAFT", "HARNESS_NODE", "HARNESS_RTK", "HARNESS_PYTHON"]},
]
adv = [{"id": "seed-missing-callers", "kind": "missing_callers", "project": "mixed"},
       {"id": "seed-hidden-critical-error", "kind": "hidden_critical_error", "project": "scripts"},
       {"id": "seed-stale-graph", "kind": "stale_graph", "project": "mixed"},
       {"id": "seed-wrong-router-choice", "kind": "wrong_router_choice", "project": "mixed"},
       {"id": "seed-double-execution", "kind": "double_execution", "project": "scripts"},
       {"id": "seed-law-weakening", "kind": "law_weakening", "project": "downstream"},
       {"id": "seed-permission-widening", "kind": "permission_widening", "project": "mixed"}]
commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=B).decode().strip()
c = {"schema": "semaprax.harness-benchmark-corpus.v1", "id": "hp17-local-v1", "seed": 17, "trials": {"cold": 1, "warm": 9},
     "pin": {"source_commit": commit, "projects": pins}, "projects": projects, "tasks": tasks, "profiles": profiles, "adversarial": adv}
open(os.path.join(B, "corpus.json"), "w").write(json.dumps(c, indent=1) + "\n")
print(pins)
