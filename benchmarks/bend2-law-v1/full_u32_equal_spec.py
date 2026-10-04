#!/usr/bin/env python3
"""Validate the versioned, full-range guarded-i64 equal-spec profile.

The profile has two precise universal checks: transfer behavior for every
U32-valued balance/amount triple, and insertion-sort sortedness plus exact
multiplicity for every four-element U32 input and queried value.  It binds the
candidate and seeded attacks used by the ordinary Bend, Bend verdict, and
SEMAPRAX native controls.  It deliberately does not call either model proof a
source-translation certificate or an unbounded-list theorem.
"""
from __future__ import annotations

import hashlib
import pathlib

ROOT = pathlib.Path(__file__).parent
FIXTURES = ROOT / "fixtures/full-u32-encoding-v1"
SCHEMA = "semaprax.bend2-law-benchmark.full-u32-equal-spec.v1"
PROFILE = "semaprax.checked-u32-value-encoding.v1"
U32_MAX = 2**32 - 1


def sha256(path: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition: bool, label: str) -> None:
    if not condition:
        raise ValueError(f"full-u32 equal-spec control failed: {label}")


def source_contracts() -> dict:
    balance_spx = (FIXTURES / "balance.spx").read_text()
    sort_spx = (FIXTURES / "sort.spx").read_text()
    balance_bend = (FIXTURES / "balance.bend").read_text()
    sort_bend = (FIXTURES / "sort.bend").read_text()
    for source, label in ((balance_spx, "balance.spx"), (sort_spx, "sort.spx")):
        require("4294967295" in source, f"{label} retains the full U32 upper bound")
        require("i64" in source, f"{label} is the guarded-i64 representation")
    require("ensures result.debit + result.credit == before.debit + before.credit" in balance_spx,
            "SEMAPRAX balance retains conservation")
    require("result.debit == before.debit - amount" in balance_spx,
            "SEMAPRAX balance retains the successful state change")
    require("insert(head, sort(tail))" in sort_spx,
            "SEMAPRAX sort retains recursive insertion")
    require("equal(checked_sort" in sort_spx and "list_cons(3, list_cons(3," in sort_spx,
            "SEMAPRAX sort retains duplicate-sensitive exact equality")
    require("U32.add(credit, amount)" in balance_bend and "U32.sub(debit, amount)" in balance_bend,
            "Bend balance retains guarded transfer arithmetic")
    require("insert(head, sort(tail))" in sort_bend and "sort([3, 1, 3, 2]) == [1, 2, 3, 3]" in sort_bend,
            "Bend sort retains duplicate-sensitive insertion witness")
    return {name: sha256(FIXTURES / name) for name in ("balance.spx", "balance.bend", "sort.spx", "sort.bend")}


def attacks() -> dict:
    # These exact substitutions are executed by full_u32_encoding_controls.py.
    import full_u32_encoding_controls as controls

    rows = {}
    for name, (old, new) in controls.MUTATIONS.items():
        candidate = (FIXTURES / name).read_text()
        attack = controls.mutation(name, candidate)
        require(old in candidate and new in attack and old not in attack,
                f"{name} has one executable seeded attack")
        if name.startswith("balance"):
            require("law case_0:" in attack or "ensures " in attack,
                    f"{name} no-op attack retains its observable law")
            rows[name] = "no-op-transfer"
        else:
            require("law case_0:" in attack or "ensures result == 0" in attack,
                    f"{name} empty-sort attack retains its observable law")
            rows[name] = "empty-sort"
    return rows


def profile() -> dict:
    return {
        "schema": SCHEMA,
        "profile": PROFILE,
        "numeric_domain": "all integers 0..4294967295 represented as guarded i64 in SEMAPRAX and U32 in Bend",
        "source_sha256": source_contracts(),
        "attacks": attacks(),
        "universal_model_checks": {
            "balance": {
                "source": "fixtures/full-u32-encoding-v1/representation.smt2",
                "expected": ["unsat", "sat", "sat"],
                "claim": "all U32 balance, credit, amount triples preserve matched guard decisions, outputs, conservation, order, equality, and reject a positive successful no-op",
            },
            "sort": {
                "source": "fixtures/full-u32-encoding-v1/sort-equal-spec.smt2",
                "expected": ["unsat", "sat"],
                "claim": "every U32 list of length four has sorted output and exact multiplicity for every queried U32 value; the second check exposes the empty-output loophole",
            },
        },
        "route_requirements": {
            "bend_normal": ["candidate accepted", "no-op-transfer rejected", "empty-sort rejected"],
            "bend_verdict": ["candidate accepted", "no-op-transfer rejected", "empty-sort rejected"],
            "semaprax_native": ["candidate returns zero", "no-op-transfer contract rejection", "empty-sort assertion rejection"],
        },
        "nonclaims": [
            "the SMT terms are model-level equal-spec checks, not source translation or lowering certificates",
            "the sort check is universal over full U32 values at fixed length four, not an unbounded-list theorem",
            "concrete Bend ordinary/verdict and SEMAPRAX native executions remain distinct assurance routes",
        ],
    }
