import unittest
from ledger.money import to_cents
from ledger.tax import tax_cents
from ledger.invoice import invoice_total


class Hidden(unittest.TestCase):
    def test_money_edges(self):
        self.assertEqual(to_cents("0.29"), 29)
        self.assertEqual(to_cents("1.15"), 115)
        self.assertEqual(to_cents("100"), 10000)

    def test_tax_edges(self):
        self.assertEqual(tax_cents(1, 50), 1)
        self.assertEqual(tax_cents(999, 7), 70)

    def test_invoice(self):
        self.assertEqual(invoice_total([("0.29", 100)], 0), (2900, 0, 2900))
