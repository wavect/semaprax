import unittest
from ledger.money import to_cents, fmt


class MoneyTests(unittest.TestCase):
    def test_simple(self):
        self.assertEqual(to_cents("19.99"), 1999)

    def test_fmt_roundtrip(self):
        self.assertEqual(fmt(to_cents("4.05")), "$4.05")

    def test_cents_1(self):
        self.assertEqual(to_cents("1.00"), 100)

    def test_cents_2(self):
        self.assertEqual(to_cents("2.00"), 200)

    def test_cents_3(self):
        self.assertEqual(to_cents("3.00"), 300)

    def test_cents_4(self):
        self.assertEqual(to_cents("4.00"), 400)

    def test_cents_5(self):
        self.assertEqual(to_cents("5.00"), 500)

    def test_cents_6(self):
        self.assertEqual(to_cents("6.00"), 600)

    def test_cents_7(self):
        self.assertEqual(to_cents("7.00"), 700)

    def test_cents_8(self):
        self.assertEqual(to_cents("8.00"), 800)

    def test_fmt_1(self):
        self.assertEqual(fmt(to_cents("1.50")), "$1.50")

    def test_fmt_2(self):
        self.assertEqual(fmt(to_cents("2.50")), "$2.50")

    def test_fmt_3(self):
        self.assertEqual(fmt(to_cents("3.50")), "$3.50")

    def test_fmt_4(self):
        self.assertEqual(fmt(to_cents("4.50")), "$4.50")

    def test_fmt_5(self):
        self.assertEqual(fmt(to_cents("5.50")), "$5.50")

    def test_fmt_6(self):
        self.assertEqual(fmt(to_cents("6.50")), "$6.50")

    def test_fmt_7(self):
        self.assertEqual(fmt(to_cents("7.50")), "$7.50")

    def test_fmt_8(self):
        self.assertEqual(fmt(to_cents("8.50")), "$8.50")
