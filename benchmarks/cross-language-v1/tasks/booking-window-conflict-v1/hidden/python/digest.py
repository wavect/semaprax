"""Hidden overlay: replaces the public `digest.py` verbatim (same relative
path) for the scoring phase only, importing the unchanged public
`candidate.py`. Forward and reverse adjacency (a shared boundary instant,
not a shared interior instant) must not conflict.
"""
import sys

from candidate import conflicts

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(conflicts(10, 20, 20, 25), 0, "forward adjacency")
check(conflicts(20, 25, 10, 20), 0, "reverse adjacency")

sys.exit(0 if failures == 0 else 1)
