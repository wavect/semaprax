import unittest
from ledger.tax import tax_cents


class TaxTests(unittest.TestCase):
    def test_exact(self):
        self.assertEqual(tax_cents(1000, 20), 200)

    def test_half_up(self):
        self.assertEqual(tax_cents(1050, 10), 105)
        self.assertEqual(tax_cents(1005, 10), 101)

    def test_zero_rate_1(self):
        self.assertEqual(tax_cents(100, 0), 0)

    def test_zero_rate_2(self):
        self.assertEqual(tax_cents(200, 0), 0)

    def test_zero_rate_3(self):
        self.assertEqual(tax_cents(300, 0), 0)

    def test_zero_rate_4(self):
        self.assertEqual(tax_cents(400, 0), 0)

    def test_zero_rate_5(self):
        self.assertEqual(tax_cents(500, 0), 0)

    def test_zero_rate_6(self):
        self.assertEqual(tax_cents(600, 0), 0)
