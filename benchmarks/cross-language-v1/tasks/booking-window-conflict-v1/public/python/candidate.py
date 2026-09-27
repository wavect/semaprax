"""Candidate: two half-open booking windows conflict exactly when they
share at least one instant. Unchanged between the public and hidden phases.
"""


def conflicts(a_start: int, a_end: int, b_start: int, b_end: int) -> int:
    if a_start < b_end and b_start < a_end:
        return 1
    return 0
