from .pricing import line_cents


def fmt_money(cents: int) -> str:
    return f"${cents // 100}.{cents % 100:02d}"


def format_line(item, qty) -> str:
    return f"{item.name} x{qty} {fmt_money(line_cents(item, qty))}"
