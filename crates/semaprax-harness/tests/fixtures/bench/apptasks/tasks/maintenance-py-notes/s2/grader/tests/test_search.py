import os, tempfile, unittest
from notes.store import Store


class Search(unittest.TestCase):
    def setUp(self):
        self.path = os.path.join(tempfile.mkdtemp(), "n.json")

    def test_case_insensitive(self):
        s = Store(self.path)
        s.add("Milk", "Buy Two")
        self.assertEqual(len(s.search("MILK")), 1)
        self.assertEqual(len(s.search("two")), 1)

    def test_none_body(self):
        s = Store(self.path)
        n = s.add("Plan")
        n.body = None
        self.assertEqual(len(s.search("plan")), 1)
        self.assertEqual(s.search("zzz"), [])
