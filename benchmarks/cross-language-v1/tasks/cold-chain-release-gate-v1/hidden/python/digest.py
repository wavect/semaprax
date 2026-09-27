"""Hidden overlay: replaces the public `digest.py` verbatim (same relative
path) for the scoring phase only, importing the unchanged public
`candidate.py`. A good reading in one sensor must never override a bad
reading in the other, and the band edges are inclusive.
"""
import sys

from candidate import release_allowed

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(release_allowed(5, 106), 0, "good temperature cannot override bad pressure")
check(release_allowed(1, 100), 0, "good pressure cannot override bad temperature")
check(release_allowed(2, 95), 1, "lower inclusive edges")
check(release_allowed(8, 105), 1, "upper inclusive edges")

sys.exit(0 if failures == 0 else 1)
