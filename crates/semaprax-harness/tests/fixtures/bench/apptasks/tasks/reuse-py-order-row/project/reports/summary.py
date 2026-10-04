from corelib.dates import parse_iso, days_between
from corelib.money import format_cents


def age_line(order: dict, today: str) -> str:
    age = days_between(parse_iso(order["placed"]), parse_iso(today))
    return f"{order['id']}: {format_cents(order['cents'])} ({age}d old)"
