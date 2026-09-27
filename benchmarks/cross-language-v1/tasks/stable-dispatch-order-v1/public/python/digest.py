"""Public test entry: orders distinct priorities from each initial
position. See ../../EQUIVALENCE.md for the exact contract.
"""
import sys

from candidate import dispatch_order

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(dispatch_order(1, 2, 3), 123, "already ordered distinct priorities")
check(dispatch_order(3, 1, 2), 231, "middle arrival has smallest priority")
check(dispatch_order(2, 3, 1), 312, "last arrival has smallest priority")

sys.exit(0 if failures == 0 else 1)
