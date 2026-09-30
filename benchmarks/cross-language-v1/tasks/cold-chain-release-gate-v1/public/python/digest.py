"""Public test entry: accepts readings inside both operating bands; rejects
when both readings are outside their bands. See ../../EQUIVALENCE.md.
"""
import sys

from candidate import release_allowed

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(release_allowed(5, 100), 1, "both readings inside operating bands")
check(release_allowed(1, 94), 0, "both readings below operating bands")
check(release_allowed(9, 106), 0, "both readings above operating bands")

sys.exit(0 if failures == 0 else 1)
