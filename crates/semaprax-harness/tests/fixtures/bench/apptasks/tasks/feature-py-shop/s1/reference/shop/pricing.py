from .models import Item

BULK_QTY = 10


def is_bulk(qty: int) -> bool:
    return qty >= BULK_QTY


def line_cents(item: Item, qty: int) -> int:
    if qty <= 0:
        raise ValueError("qty must be positive")
    cents = item.unit_cents * qty
    if is_bulk(qty):
        cents -= cents // 10
    return cents
