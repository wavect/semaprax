"""Candidate: order three jobs by increasing priority while preserving
arrival order for ties. Unchanged between the public and hidden phases.
"""


def dispatch_order(a_priority: int, b_priority: int, c_priority: int) -> int:
    if a_priority <= b_priority and a_priority <= c_priority:
        if b_priority <= c_priority:
            return 123
        else:
            return 132
    elif b_priority <= a_priority and b_priority <= c_priority:
        if a_priority <= c_priority:
            return 213
        else:
            return 231
    elif a_priority <= b_priority:
        return 312
    else:
        return 321
