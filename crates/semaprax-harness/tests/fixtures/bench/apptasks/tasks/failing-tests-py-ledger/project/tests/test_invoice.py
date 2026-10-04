import unittest
from ledger.invoice import invoice_total, invoice_text


class InvoiceTests(unittest.TestCase):
    def test_total(self):
        self.assertEqual(invoice_total([("19.99", 3)], 10), (5997, 600, 6597))

    def test_text(self):
        self.assertEqual(invoice_text([("0.10", 3), ("0.20", 1)], 0), "net $0.50 tax $0.00 total $0.50")

    def test_qty_1(self):
        self.assertEqual(invoice_total([("1.00", 1)], 0)[0], 100)

    def test_qty_2(self):
        self.assertEqual(invoice_total([("1.00", 2)], 0)[0], 200)

    def test_qty_3(self):
        self.assertEqual(invoice_total([("1.00", 3)], 0)[0], 300)

    def test_qty_4(self):
        self.assertEqual(invoice_total([("1.00", 4)], 0)[0], 400)

    def test_qty_5(self):
        self.assertEqual(invoice_total([("1.00", 5)], 0)[0], 500)

    def test_qty_6(self):
        self.assertEqual(invoice_total([("1.00", 6)], 0)[0], 600)
