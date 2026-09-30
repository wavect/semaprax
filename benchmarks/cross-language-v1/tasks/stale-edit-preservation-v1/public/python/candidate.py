"""prior-session: shipment tag helper; unrelated to this task's repair, keep
unchanged. A candidate that clobbers or deletes this while repairing
`apply_discount` below has not preserved a stale, already-completed edit.
Unchanged between the public and hidden phases.
"""


def stale_note(tag: int) -> int:
    return tag * 2 + 7


def apply_discount(price: int, pct: int) -> int:
    # A corrupted upstream feed can send `pct` above 100; the result must
    # floor at zero rather than go negative.
    raw = price - (price * pct) // 100
    if raw < 0:
        return 0
    return raw
