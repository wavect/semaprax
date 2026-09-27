"""Hidden overlay: replaces the public `digest.py` verbatim (same relative
path) for the scoring phase only, importing the unchanged public
`candidate.py`. Vectors mixing zero and 0xff sentinels at several
positions.
"""
import sys

from candidate import sentinel_checksum

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(sentinel_checksum(bytes([0x00, 0xFF, 0x00, 0x09])), 1024, "zero and ff")
check(sentinel_checksum(bytes([0x09, 0x00, 0xFF, 0x00, 0x09])), 1536, "two zeroes")

sys.exit(0 if failures == 0 else 1)
