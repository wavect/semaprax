"""Public implementation: four independent scalar digests over five signed
64-bit inputs. See ../../EQUIVALENCE.md for the exact input/output/boundary
contract every language implementation of this task must meet.

Run directly with `python3 digest.py`. A failed check calls `sys.exit(1)`
rather than relying on a bare `assert`, which some interpreters strip under
`-O`; this mirrors the same "an uncaught failure is the process's own
nonzero-exit signal" convention TypeScript's port already uses.
"""
import sys


def is_even(value: int) -> int:
    return 1 if value % 2 == 0 else 0


def is_negative(value: int) -> int:
    return 1 if value < 0 else 0


def max2(left: int, right: int) -> int:
    return left if left > right else right


def total(a: int, b: int, c: int, d: int, e: int) -> int:
    return a + b + c + d + e


def count_even(a: int, b: int, c: int, d: int, e: int) -> int:
    return is_even(a) + is_even(b) + is_even(c) + is_even(d) + is_even(e)


def count_negative(a: int, b: int, c: int, d: int, e: int) -> int:
    return is_negative(a) + is_negative(b) + is_negative(c) + is_negative(d) + is_negative(e)


def max_of(a: int, b: int, c: int, d: int, e: int) -> int:
    return max2(max2(max2(a, b), max2(c, d)), e)


failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(total(1, 2, 3, 4, 5), 15, "sum(1,2,3,4,5)")
check(count_even(1, 2, 3, 4, 5), 2, "count_even(1,2,3,4,5)")
check(count_negative(1, 2, 3, 4, 5), 0, "count_negative(1,2,3,4,5)")
check(max_of(1, 2, 3, 4, 5), 5, "max_of(1,2,3,4,5)")

check(total(-1, -2, -3, -4, -5), -15, "sum(-1,-2,-3,-4,-5)")
check(count_even(-1, -2, -3, -4, -5), 2, "count_even(-1,-2,-3,-4,-5)")
check(count_negative(-1, -2, -3, -4, -5), 5, "count_negative(-1,-2,-3,-4,-5)")
check(max_of(-1, -2, -3, -4, -5), -1, "max_of(-1,-2,-3,-4,-5)")

sys.exit(0 if failures == 0 else 1)
