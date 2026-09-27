"""Public test entry: ordinary deltas combine by plain addition. See
../../EQUIVALENCE.md for the exact contract.
"""
import sys

from candidate import combine_telemetry

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(combine_telemetry(10, 20), 30, "ordinary positive deltas")
check(combine_telemetry(-5, 5), 0, "ordinary mixed deltas")

sys.exit(0 if failures == 0 else 1)
