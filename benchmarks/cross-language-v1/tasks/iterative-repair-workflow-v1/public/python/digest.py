"""Public test entry: four vectors that never let a withdrawal's fee
interact with the balance floor. See ../../EQUIVALENCE.md's
iterative-repair narrative for why a candidate that passes every one of
these is not yet done.
"""
import sys

from candidate import process_batch

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(process_batch(0, 50, 50, 50, 50, 50), 250, "deposit-only sequence never charges a fee")
check(process_batch(400, 50, 0, 0, 0, 0), 450, "single deposit baseline")
check(process_batch(200, -10, -10, -10, -10, -10), 135, "withdrawals mid-range, never approach the floor")
check(process_batch(480, 50, 0, 0, 0, 0), 500, "a deposit clamps at the ceiling with no fee involved")

sys.exit(0 if failures == 0 else 1)
