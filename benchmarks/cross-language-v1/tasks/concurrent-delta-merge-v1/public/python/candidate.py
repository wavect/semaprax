"""Candidate: merge two independently-arriving deltas against a shared
[0, 1_000_000]-bounded counter from one common base value, treating both
deltas as arriving concurrently (summed against the same base, then
clamped once) rather than sequentially. Unchanged between the public and
hidden phases.
"""


def merge_concurrent_deltas(base: int, delta_a: int, delta_b: int) -> int:
    total = base + delta_a + delta_b
    if total < 0:
        return 0
    elif total > 1_000_000:
        return 1_000_000
    else:
        return total
