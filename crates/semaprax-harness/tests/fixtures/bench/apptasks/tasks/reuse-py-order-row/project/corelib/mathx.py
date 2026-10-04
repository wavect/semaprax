def clamp(x, lo, hi):
    return max(lo, min(hi, x))


def pct(part, whole):
    return 0 if whole == 0 else round(100 * part / whole)
