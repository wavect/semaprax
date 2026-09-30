"""Public test entry: overlapping and contained bookings conflict; a real
gap does not. See ../../EQUIVALENCE.md for the exact contract.
"""
import sys

from candidate import conflicts

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(conflicts(10, 20, 15, 25), 1, "proper overlap")
check(conflicts(10, 30, 12, 18), 1, "contained booking")
check(conflicts(10, 20, 25, 30), 0, "strictly separated bookings")

sys.exit(0 if failures == 0 else 1)
