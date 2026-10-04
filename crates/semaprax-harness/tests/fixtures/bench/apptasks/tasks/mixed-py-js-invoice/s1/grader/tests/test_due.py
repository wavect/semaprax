import unittest
from billing.models import Invoice
from billing.serialize import to_json


class Due(unittest.TestCase):
    def test_without_due(self):
        self.assertEqual(to_json(Invoice("A-1", 5)), {"number": "A-1", "totalCents": 5})

    def test_with_due(self):
        inv = Invoice("A-2", 7, due_date="2026-12-31")
        self.assertEqual(to_json(inv), {"number": "A-2", "totalCents": 7, "dueDate": "2026-12-31"})
