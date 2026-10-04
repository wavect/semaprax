from .models import Item


def line_cents(item: Item, qty: int) -> int:
    if qty <= 0:
        raise ValueError("qty must be positive")
    return item.unit_cents * qty
