import os, tempfile, unittest
from notes.store import Store


class Tags(unittest.TestCase):
    def setUp(self):
        self.path = os.path.join(tempfile.mkdtemp(), "n.json")

    def test_tags_persist(self):
        s = Store(self.path)
        s.add("Milk", "buy", tags=["home", "food"])
        s.add("Plan", "x")
        s2 = Store(self.path)
        self.assertEqual(s2.notes[0].tags, ["home", "food"])
        self.assertEqual(s2.notes[1].tags, [])

    def test_find_by_tag(self):
        s = Store(self.path)
        s.add("Milk", "buy", tags=["home"])
        s.add("Plan", "x", tags=["work"])
        self.assertEqual([n.title for n in s.find_by_tag("home")], ["Milk"])
        self.assertEqual(s.find_by_tag("none"), [])
