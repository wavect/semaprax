"""Hidden overlay: replaces the public `digest.py` verbatim (same relative
path) for the scoring phase only, adding two hidden vectors a solver never
sees. Implementation functions are unchanged from the public file.
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

# Hidden cases: never shipped in the public directory tree.
check(total(0, 0, 0, 0, 0), 0, "sum(0,0,0,0,0)")
check(count_even(0, 0, 0, 0, 0), 5, "count_even(0,0,0,0,0)")
check(count_negative(0, 0, 0, 0, 0), 0, "count_negative(0,0,0,0,0)")
check(max_of(0, 0, 0, 0, 0), 0, "max_of(0,0,0,0,0)")

check(total(-100, 7, 7, 7, 100), 21, "sum(-100,7,7,7,100)")
check(count_even(-100, 7, 7, 7, 100), 2, "count_even(-100,7,7,7,100)")
check(count_negative(-100, 7, 7, 7, 100), 1, "count_negative(-100,7,7,7,100)")
check(max_of(-100, 7, 7, 7, 100), 100, "max_of(-100,7,7,7,100)")

sys.exit(0 if failures == 0 else 1)
