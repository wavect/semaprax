"""A telemetry combiner register must saturate at the 32-bit signed boundary
rather than wrap or trap: two independent delta readings are summed into
one running total, and a reading that would carry the total past
`I32_MAX`/`I32_MIN` must clamp there instead of silently doing whatever the
underlying arithmetic happens to do at that magnitude. Unchanged between
the public and hidden phases.

Python integers are arbitrary precision, so there is no wraparound to
accidentally rely on; the explicit boundary constants below are what makes
this a genuine 32-bit-saturating combiner rather than a plain sum.
"""
I32_MAX = 2147483647
I32_MIN = -2147483648


def combine_telemetry(delta_a: int, delta_b: int) -> int:
    if delta_b > 0 and delta_a > I32_MAX - delta_b:
        return I32_MAX
    elif delta_b < 0 and delta_a < I32_MIN - delta_b:
        return I32_MIN
    else:
        return delta_a + delta_b
