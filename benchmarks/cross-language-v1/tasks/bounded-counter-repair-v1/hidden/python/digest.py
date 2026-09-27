"""Hidden overlay: replaces the public `digest.py` verbatim (same relative
path) for the scoring phase only, adding hidden vectors that exercise the
classic off-by-one repair bug this task is about: clamping only the final
summed delta instead of clamping the running counter after every individual
step. Implementation functions are unchanged from the public file.
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

# Hidden cases: never shipped in the public directory tree. A single
# end-of-sequence clamp instead of a per-step clamp gets both of these wrong
# (100 and 35, respectively); the correct stepwise counter clamps after every
# delta and gets 70 and 50.
check(apply5(90, 50, -30, 0, 0, 0), 70, "apply5(90,50,-30,0,0,0) saturates upward then recovers downward")
check(apply5(5, -20, 50, 0, 0, 0), 50, "apply5(5,-20,50,0,0,0) floors downward then recovers upward")

sys.exit(0 if failures == 0 else 1)
