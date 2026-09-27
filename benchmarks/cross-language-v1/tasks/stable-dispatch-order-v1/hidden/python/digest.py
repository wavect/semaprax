"""Hidden overlay: replaces the public `digest.py` verbatim (same relative
path) for the scoring phase only, importing the unchanged public
`candidate.py`. Ties must preserve arrival order, including a three-way tie.
"""
import sys

from candidate import dispatch_order

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(dispatch_order(1, 1, 2), 123, "first two arrivals tie")
check(dispatch_order(1, 2, 1), 132, "first and last arrivals tie")
check(dispatch_order(2, 1, 1), 231, "last two arrivals tie")
check(dispatch_order(7, 7, 7), 123, "all arrivals tie")

sys.exit(0 if failures == 0 else 1)
