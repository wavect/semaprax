import unittest
from reports.summary import age_line
from reports.catalog import catalog_key


class T(unittest.TestCase):
    def test_age(self):
        self.assertEqual(age_line({"id": "o1", "placed": "2024-03-01", "cents": 1230}, "2024-03-11"), "o1: $12.30 (10d old)")

    def test_key(self):
        self.assertEqual(catalog_key("Hello, Big World!"), "hello-big-world")
