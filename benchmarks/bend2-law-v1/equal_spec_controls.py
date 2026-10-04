#!/usr/bin/env python3
"""Validate LAW-16's matched-task witnesses and law-gaming controls.

These checks validate the committed, language-neutral fixture corpus before a
driver can time it. They neither execute Bend nor SEMAPRAX nor treat the
checked-u32 cells as supported by SEMAPRAX.
"""
from __future__ import annotations


U32_MAX = 2**32 - 1

EXPECTED = {
    "scalar-contract-bug-v1": {"numeric_domain": "bool exact", "laws": ["postcondition", "total-negation"], "attacks": ["weakened-postcondition"]},
    "structured-balance-transfer-v1": {"numeric_domain": "u32 checked", "laws": ["conservation", "intended-state-change", "nonnegative-balances"], "attacks": ["no-op-transfer"]},
    "supported-list-theorem-v1": {"numeric_domain": "u32 checked", "laws": ["permutation-and-multiplicity", "sortedness"], "attacks": ["empty-sort"]},
    "law-preserving-refactor-v1": {"numeric_domain": "u32 checked", "laws": ["before-after-observational-equivalence"], "attacks": ["law-dropped-during-refactor"]},
    "law-breaking-agent-edit-v1": {"numeric_domain": "u32 checked", "laws": ["declared-law-inventory"], "attacks": ["agent-law-gaming"]},
    "project-incremental-edit-v1": {"numeric_domain": "u32 checked", "laws": ["unchanged-module-reuse", "changed-module-recheck"], "attacks": ["stale-cache-reuse"]},
}


def require(condition: bool, label: str) -> None:
    if not condition:
        raise ValueError(f"equal-spec control failed: {label}")


def u32(value: object, label: str) -> int:
    require(isinstance(value, int) and 0 <= value <= U32_MAX, label)
    return value


def one(rows: object, label: str) -> dict:
    require(isinstance(rows, list) and len(rows) == 1 and isinstance(rows[0], dict), label)
    return rows[0]


def scalar(fixture: dict) -> None:
    success = one(fixture["success"], "scalar success witness")
    attack = one(fixture["attacks"]["weakened-postcondition"], "scalar attack witness")
    require(isinstance(success.get("input"), bool) and success.get("output") is not success["input"], "scalar success is total negation")
    require(attack.get("input") is False and attack.get("output") is False, "scalar attack weakens false postcondition")


def balance(fixture: dict) -> None:
    success = one(fixture["success"], "balance success witness")
    attack = one(fixture["attacks"]["no-op-transfer"], "balance attack witness")
    before, after = success.get("before"), success.get("after")
    require(isinstance(before, list) and isinstance(after, list) and len(before) == len(after) == 2, "balance has two accounts")
    before = [u32(value, "balance input is checked u32") for value in before]
    after = [u32(value, "balance output is checked u32") for value in after]
    amount = u32(success.get("amount"), "balance amount is checked u32")
    require(amount > 0 and before[0] >= amount, "balance transfer is admitted")
    require(sum(before) == sum(after), "balance conservation")
    require(after == [before[0] - amount, before[1] + amount], "balance intended state change")
    require(attack.get("before") == before and attack.get("amount") == amount and attack.get("after") == before, "balance attack is a no-op transfer")


def list_theorem(fixture: dict) -> None:
    success = one(fixture["success"], "list success witness")
    attack = one(fixture["attacks"]["empty-sort"], "list attack witness")
    values, result = success.get("input"), success.get("output")
    require(isinstance(values, list) and values and isinstance(result, list), "list witness has a nonempty list")
    values = [u32(value, "list input is checked u32") for value in values]
    result = [u32(value, "list output is checked u32") for value in result]
    require(result == sorted(values), "list sortedness and permutation/multiplicity")
    require(attack.get("input") == values and attack.get("output") == [], "list attack is empty sort")


def refactor(fixture: dict) -> None:
    success = one(fixture["success"], "refactor success witness")
    attack = one(fixture["attacks"]["law-dropped-during-refactor"], "refactor attack witness")
    value = u32(success.get("input"), "refactor input is checked u32")
    before, after = u32(success.get("before"), "refactor before is checked u32"), u32(success.get("after"), "refactor after is checked u32")
    require(before == after, "refactor preserves observed result")
    require(attack.get("input") == value and attack.get("before") == before and attack.get("after") != before, "refactor attack drops observational equivalence")


def agent_edit(fixture: dict) -> None:
    success = one(fixture["success"], "agent-edit success witness")
    attack = one(fixture["attacks"]["agent-law-gaming"], "agent-edit attack witness")
    value, output = u32(success.get("input"), "agent input is checked u32"), u32(success.get("output"), "agent output is checked u32")
    require(success.get("law_inventory") == ["double"] and output == value * 2, "agent success retains double law")
    require(attack.get("input") == value and attack.get("output") != output and attack.get("law_inventory") == [], "agent attack removes declared law")


def incremental(fixture: dict) -> None:
    success = one(fixture["success"], "incremental success witness")
    attack = one(fixture["attacks"]["stale-cache-reuse"], "incremental attack witness")
    require(success.get("changed") == "core" and success.get("unchanged") == "api", "incremental modules are named")
    require(success.get("rechecked") == ["core"] and success.get("reused") == ["api"], "incremental success rechecks only changed module")
    require(u32(success.get("output"), "incremental success output is checked u32") == 9, "incremental success produces changed result")
    require(attack.get("changed") == "core" and attack.get("unchanged") == "api", "incremental attack uses same edit")
    require(attack.get("rechecked") == [] and attack.get("reused") == ["core", "api"], "incremental attack is stale cache reuse")
    require(attack.get("output") != success["output"], "incremental stale cache retains old result")


VALIDATORS = {"scalar-contract-bug-v1": scalar, "structured-balance-transfer-v1": balance, "supported-list-theorem-v1": list_theorem, "law-preserving-refactor-v1": refactor, "law-breaking-agent-edit-v1": agent_edit, "project-incremental-edit-v1": incremental}


def validate(cell: dict, fixture: dict) -> None:
    """Require canonical equal-spec semantics for a known LAW-16 cell."""
    expected = EXPECTED.get(cell["id"])
    if expected is None:
        return
    require(cell["numeric_domain"] == expected["numeric_domain"], "manifest numeric domain")
    require(cell["laws"] == expected["laws"] and cell["attacks"] == expected["attacks"], "manifest law inventory")
    VALIDATORS[cell["id"]](fixture)
