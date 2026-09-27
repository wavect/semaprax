"""Public test entry: exact tax and zero-shipping cases. See
../../EQUIVALENCE.md for the exact contract.
"""
import sys

from candidate import invoice_total

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(invoice_total(10, 2, 10, 5), 27, "exact tax with shipping")
check(invoice_total(40, 2, 25, 0), 100, "exact tax without shipping")
check(invoice_total(25, 4, 20, 10), 130, "larger exact subtotal")

sys.exit(0 if failures == 0 else 1)
