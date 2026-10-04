import unittest
from reports.order_row import order_row


class T(unittest.TestCase):
    def test_row(self):
        self.assertEqual(order_row({"title": "Hello, Big World!", "placed": "2024-03-05", "cents": 1230}), "hello-big-world|05 Mar 2024|$12.30")

    def test_negative_and_trim(self):
        self.assertEqual(order_row({"title": "--Refund  Q--", "placed": "2023-12-31", "cents": -5}), "refund-q|31 Dec 2023|-$0.05")
