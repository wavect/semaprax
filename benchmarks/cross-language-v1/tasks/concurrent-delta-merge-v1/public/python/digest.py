"""Public test entry: merges two deltas well inside the shared bound, and
negative deltas that never approach the floor. See ../../EQUIVALENCE.md.
"""
import sys

from candidate import merge_concurrent_deltas

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(merge_concurrent_deltas(100, 50, -30), 120, "well inside the bound")
check(merge_concurrent_deltas(500_000, 100, 100), 500_200, "midrange sum")
check(merge_concurrent_deltas(10, -5, -3), 2, "negative deltas away from the floor")
check(merge_concurrent_deltas(0, 0, 0), 0, "identity")

sys.exit(0 if failures == 0 else 1)
