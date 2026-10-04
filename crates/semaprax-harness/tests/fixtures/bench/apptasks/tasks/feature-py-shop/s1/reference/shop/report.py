from .pricing import line_cents, is_bulk


def fmt_money(cents: int) -> str:
    return f"${cents // 100}.{cents % 100:02d}"


def format_line(item, qty) -> str:
    text = f"{item.name} x{qty} {fmt_money(line_cents(item, qty))}"
    return text + " (bulk)" if is_bulk(qty) else text
