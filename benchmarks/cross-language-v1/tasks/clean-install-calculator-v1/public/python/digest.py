"""Public test entry: the starter operation and its new sibling compute
correctly. See ../../EQUIVALENCE.md for the exact contract.
"""
import sys

from candidate import add, subtract

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(add(19, 23), 42, "the starter operation still works")
check(subtract(50, 8), 42, "the new sibling operation")

sys.exit(0 if failures == 0 else 1)
