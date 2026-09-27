"""Hidden overlay: replaces the public `digest.py` verbatim (same relative
path) for the scoring phase only, importing the unchanged public
`candidate.py`. A delta that would carry the total past the 32-bit signed
boundary must saturate instead of wrapping or trapping.
"""
import sys

from candidate import I32_MAX, I32_MIN, combine_telemetry

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(combine_telemetry(I32_MAX, 1), I32_MAX, "saturates at the positive boundary")
check(combine_telemetry(I32_MIN, -1), I32_MIN, "saturates at the negative boundary")

sys.exit(0 if failures == 0 else 1)
