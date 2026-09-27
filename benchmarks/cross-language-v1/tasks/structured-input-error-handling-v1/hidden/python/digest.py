"""Hidden overlay: replaces the public `digest.py` verbatim (same relative
path) for the scoring phase only, importing the unchanged public
`candidate.py`. Compound-invalid envelopes make the first-error precedence
independently observable: a candidate that checks version before kind, or
accepts an out-of-range length, fails here.
"""
import sys

from candidate import validate

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(validate(6, 2, 0), 1, "kind precedes version and length")
check(validate(7, 2, 0), 2, "version precedes length")
check(validate(7, 1, -1), 3, "negative payload")
check(validate(7, 1, 65), 3, "large payload")
check(validate(7, 1, 64), 0, "maximum valid payload")
check(validate(7, 1, 1), 0, "minimum valid payload")

sys.exit(0 if failures == 0 else 1)
