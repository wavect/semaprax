"""Hidden overlay: replaces the public `digest.py` verbatim (same relative
path) for the scoring phase only, importing the unchanged public
`candidate.py`/`helper.py`. Vectors where the whole-subtotal tax must
precede shipping and round (floor) exactly once.
"""
import sys

from candidate import invoice_total

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(invoice_total(19, 3, 8, 10), 71, "whole subtotal tax before shipping")
check(invoice_total(7, 5, 13, 9), 48, "tax rounds once")
check(invoice_total(1, 64, 17, 3), 77, "bounded quantity")

sys.exit(0 if failures == 0 else 1)
