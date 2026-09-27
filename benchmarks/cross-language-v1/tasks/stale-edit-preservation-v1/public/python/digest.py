"""Public test entry: ordinary discounts reduce price. See
../../EQUIVALENCE.md for the exact contract.
"""
import sys

from candidate import apply_discount

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(apply_discount(100, 10), 90, "ten percent off")
check(apply_discount(200, 25), 150, "twenty five percent off")

sys.exit(0 if failures == 0 else 1)
