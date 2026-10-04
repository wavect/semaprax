from corelib.text import slugify
from corelib.dates import parse_iso, short_date
from corelib.money import format_cents


def order_row(order: dict) -> str:
    return "|".join([slugify(order["title"]), short_date(parse_iso(order["placed"])), format_cents(order["cents"])])
