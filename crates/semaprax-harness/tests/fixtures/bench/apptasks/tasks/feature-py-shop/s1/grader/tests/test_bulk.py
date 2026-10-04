import unittest
from shop.models import Item
from shop.cart import Cart
from shop.pricing import line_cents
from shop.report import format_line


class BulkTests(unittest.TestCase):
    def test_threshold(self):
        it = Item("a", "A", 1000)
        self.assertEqual(line_cents(it, 9), 9000)
        self.assertEqual(line_cents(it, 10), 9000)
        self.assertEqual(line_cents(it, 20), 18000)

    def test_rounding(self):
        self.assertEqual(line_cents(Item("a", "A", 105), 10), 945)

    def test_cart_total(self):
        c = Cart()
        c.add(Item("a", "A", 1000), 10)
        c.add(Item("b", "B", 300), 2)
        self.assertEqual(c.total_cents(), 9600)

    def test_report_marker(self):
        self.assertEqual(format_line(Item("a", "A", 1000), 10), "A x10 $90.00 (bulk)")
        self.assertEqual(format_line(Item("a", "A", 1000), 9), "A x9 $90.00")
