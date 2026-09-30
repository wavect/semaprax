"""Public test entry: valid boundaries and single-field errors. See
../../EQUIVALENCE.md for the exact contract.
"""
import sys

from candidate import validate

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(validate(7, 1, 1), 0, "minimum valid envelope")
check(validate(7, 1, 64), 0, "maximum valid envelope")
check(validate(6, 1, 10), 1, "unknown kind")
check(validate(7, 2, 10), 2, "unsupported version")
check(validate(7, 1, 0), 3, "short payload")
check(validate(7, 1, 65), 3, "long payload")

sys.exit(0 if failures == 0 else 1)
