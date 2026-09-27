"""Public implementation: a saturating counter (bounded to [0, 100]) that
applies five signed 64-bit deltas in sequence, clamping after every
individual step rather than only once at the end. See ../../EQUIVALENCE.md
for the exact input/output/boundary contract every language implementation
of this task must meet.

Run directly with `python3 digest.py`, exactly as sequence-digest-v1's own
Python port already establishes for this suite's Python lane.
"""
import sys


def clamp(value: int) -> int:
    if value > 100:
        return 100
    elif value < 0:
        return 0
    else:
        return value


def step(counter: int, delta: int) -> int:
    return clamp(counter + delta)


def apply5(c0: int, d1: int, d2: int, d3: int, d4: int, d5: int) -> int:
    return step(step(step(step(step(c0, d1), d2), d3), d4), d5)


failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(apply5(0, 10, 10, 10, 10, 10), 50, "apply5(0,10,10,10,10,10)")
check(apply5(50, 10, -5, 10, -5, 10), 70, "apply5(50,10,-5,10,-5,10)")
check(apply5(95, 10, 0, 0, 0, 0), 100, "apply5(95,10,0,0,0,0)")
check(apply5(5, -10, 0, 0, 0, 0), 0, "apply5(5,-10,0,0,0,0)")

sys.exit(0 if failures == 0 else 1)
