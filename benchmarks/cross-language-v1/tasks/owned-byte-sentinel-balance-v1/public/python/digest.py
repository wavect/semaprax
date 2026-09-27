"""Public test entry: public sentinel vectors. See ../../EQUIVALENCE.md for
the exact contract.
"""
import sys

from candidate import sentinel_checksum

failures = 0


def check(actual: int, expected: int, label: str) -> None:
    global failures
    if actual != expected:
        sys.stderr.write(f"{label}: expected {expected}, got {actual}\n")
        failures += 1


check(sentinel_checksum(bytes([0xFF])), 0, "one ff")
check(sentinel_checksum(bytes([0xFF, 0x07, 0xFF])), 2, "two ff")
check(sentinel_checksum(bytes([0x01, 0x02, 0x7F])), 6, "other bytes")

sys.exit(0 if failures == 0 else 1)
