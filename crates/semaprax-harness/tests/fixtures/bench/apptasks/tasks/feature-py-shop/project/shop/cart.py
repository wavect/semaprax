from .models import Item
from .pricing import line_cents


class Cart:
    def __init__(self):
        self.lines = []  # (Item, qty)

    def add(self, item: Item, qty: int = 1):
        self.lines.append((item, qty))

    def total_cents(self) -> int:
        return sum(line_cents(i, q) for i, q in self.lines)
