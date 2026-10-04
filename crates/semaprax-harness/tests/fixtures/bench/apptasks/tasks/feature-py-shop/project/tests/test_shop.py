import unittest
from shop.models import Item
from shop.cart import Cart
from shop.report import format_line


class ShopTests(unittest.TestCase):
    def test_total(self):
        c = Cart()
        c.add(Item("a", "Apple", 120), 2)
        c.add(Item("b", "Bread", 250))
        self.assertEqual(c.total_cents(), 490)

    def test_line(self):
        self.assertEqual(format_line(Item("a", "Apple", 120), 2), "Apple x2 $2.40")

    def test_bad_qty(self):
        from shop.pricing import line_cents
        with self.assertRaises(ValueError):
            line_cents(Item("a", "Apple", 1), 0)
