"""Candidate: a five-step account-ledger repair with a withdrawal fee, plus
a second, already-correct tier_label classifier left by a prior session
(out of scope for this repair). See ../../EQUIVALENCE.md for the full
iterative-repair narrative this task's public/hidden split is built
around. Unchanged between the public and hidden phases.
"""


def clamp_balance(value: int) -> int:
    if value < 0:
        return 0
    elif value > 500:
        return 500
    else:
        return value


def fee(adjustment: int) -> int:
    """A withdrawal (a negative adjustment) is charged a flat handling fee;
    a deposit (zero or positive) is not."""
    return 3 if adjustment < 0 else 0


def apply_step(balance: int, adjustment: int) -> int:
    """One step: fold the adjustment and its fee into the balance, THEN
    clamp the whole result. Subtracting the fee after clamping is the
    second, masked defect this task exists to catch."""
    return clamp_balance(balance + adjustment - fee(adjustment))


def process_batch(b0: int, a1: int, a2: int, a3: int, a4: int, a5: int) -> int:
    return apply_step(
        apply_step(apply_step(apply_step(apply_step(b0, a1), a2), a3), a4), a5
    )


def tier_label(balance: int) -> int:
    """prior-session: account tier classifier, unrelated to the ledger
    repair above; keep unchanged."""
    if balance < 100:
        return 0
    elif balance < 300:
        return 1
    else:
        return 2
