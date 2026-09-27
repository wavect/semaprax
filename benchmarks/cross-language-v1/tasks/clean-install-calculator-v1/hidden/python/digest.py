"""Hidden overlay: replaces the public `digest.py` verbatim (same relative
path) for the scoring phase only, importing the unchanged public
`candidate.py`. Subtraction may produce a negative result, unlike a bounded
counter elsewhere in this corpus.
"""
import sys

from candidate import add, subtract

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(subtract(8, 50), -42, "subtraction may go negative, unlike a bounded counter")
check(subtract(-5, -5), 0, "subtracting equal negatives")
check(add(19, 23), 42, "the starter operation is unchanged")

sys.exit(0 if failures == 0 else 1)
