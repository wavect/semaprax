import unittest
from billing.models import Invoice
from billing.serialize import to_json


class SerializeTests(unittest.TestCase):
    def test_basic(self):
        self.assertEqual(to_json(Invoice("A-1", 1250)), {"number": "A-1", "totalCents": 1250})
