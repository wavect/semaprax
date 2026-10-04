def format_cents(cents: int, symbol: str = "$") -> str:
    """Render integer cents as e.g. $12.30, with a leading minus for negatives."""
    sign = "-" if cents < 0 else ""
    cents = abs(cents)
    return f"{sign}{symbol}{cents // 100}.{cents % 100:02d}"


def parse_money(text: str) -> int:
    whole, _, frac = text.lstrip("$").partition(".")
    return int(whole) * 100 + int((frac + "00")[:2])
