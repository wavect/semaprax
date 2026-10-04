def to_cents(text: str) -> int:
    """Parse a decimal string such as "19.99" into integer cents."""
    return int(float(text) * 100)


def fmt(cents: int) -> str:
    return f"${cents // 100}.{cents % 100:02d}"
