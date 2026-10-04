import os, tempfile, unittest
from notes.store import Store


class T(unittest.TestCase):
    def setUp(self):
        self.path = os.path.join(tempfile.mkdtemp(), "n.json")

    def test_add_and_reload(self):
        s = Store(self.path)
        s.add("Milk", "buy 2L")
        self.assertEqual([n.title for n in Store(self.path).notes], ["Milk"])

    def test_search(self):
        s = Store(self.path)
        s.add("Milk", "buy 2L")
        self.assertEqual(len(s.search("buy")), 1)
