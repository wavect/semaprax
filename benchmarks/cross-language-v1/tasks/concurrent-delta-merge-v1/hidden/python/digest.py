"""Hidden overlay: replaces the public `digest.py` verbatim (same relative
path) for the scoring phase only, importing the unchanged public
`candidate.py`. A ceiling- or floor-side delta must not clamp before the
other concurrent delta lands.
"""
import sys

from candidate import merge_concurrent_deltas

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(merge_concurrent_deltas(100, 50, -30), 120, "well inside the bound")
check(merge_concurrent_deltas(500_000, 100, 100), 500_200, "midrange sum")
check(merge_concurrent_deltas(10, -5, -3), 2, "negative deltas away from the floor")
check(merge_concurrent_deltas(0, 0, 0), 0, "identity")

# Hidden boundary vectors: never shipped in the public directory tree. A
# candidate that clamps delta_a against base before adding delta_b --
# treating the two concurrent deltas as a sequential edit -- diverges from
# the correct concurrent merge exactly here.
check(
    merge_concurrent_deltas(999_990, 20, -50),
    999_960,
    "a ceiling-side delta must not clamp before the other concurrent delta lands",
)
check(
    merge_concurrent_deltas(10, -20, 15),
    5,
    "a floor-side delta must not clamp before the other concurrent delta lands",
)

sys.exit(0 if failures == 0 else 1)
