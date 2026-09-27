"""Hidden overlay: replaces the public `digest.py` verbatim (same relative
path) for the scoring phase only, importing the unchanged public
`candidate.py`. A corrupted percentage above full price must floor at
zero, and the preexisting `stale_note` helper must be unchanged.
"""
import sys

from candidate import apply_discount, stale_note

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(apply_discount(100, 150), 0, "corrupted percentage floors at zero")
check(apply_discount(40, 130), 0, "corrupted percentage floors at zero (2)")
check(stale_note(5), 17, "preexisting stale helper unchanged")
check(stale_note(0), 7, "preexisting stale helper unchanged at zero")

sys.exit(0 if failures == 0 else 1)
