"""Hidden overlay: replaces the public digest.py verbatim (same relative
path) for the scoring phase only, importing the unchanged public
candidate.py. Adds the two floor-interaction vectors that only a
correctly-ordered ("clamp last") repair passes, plus three checks on the
unrelated tier_label classifier the public suite never calls at all.
"""
import sys

from candidate import process_batch, tier_label

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

# A withdrawal fee subtracted after the balance is clamped, instead of
# before, can return a balance below the declared floor whenever the
# violating step is the sequence's last one. attempt_1 in EQUIVALENCE.md's
# narrative fixes the deposit-fee defect but keeps this one, so it passes
# every public vector above and fails both of these.
check(
    process_batch(3, 0, 0, 0, 0, -3),
    0,
    "a final withdrawal that would cross the floor must fold its fee before clamping",
)
check(
    process_batch(53, 0, 0, 0, 0, -51),
    0,
    "a final withdrawal whose pre-fee sum is still in range must still fold its fee before clamping",
)

check(tier_label(50), 0, "tier classifier: low balance")
check(tier_label(150), 1, "tier classifier: mid balance")
check(tier_label(350), 2, "tier classifier: high balance")

sys.exit(0 if failures == 0 else 1)
