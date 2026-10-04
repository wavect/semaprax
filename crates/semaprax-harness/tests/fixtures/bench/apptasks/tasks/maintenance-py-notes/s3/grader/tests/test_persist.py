import os, tempfile, unittest
from notes.persist import load_notes, save_notes
from notes.model import Note


class Persist(unittest.TestCase):
    def test_roundtrip(self):
        p = os.path.join(tempfile.mkdtemp(), "n.json")
        save_notes(p, [Note(1, "a", "b", ["t"])])
        self.assertEqual(load_notes(p), [Note(1, "a", "b", ["t"])])
        self.assertEqual(load_notes(os.path.join(tempfile.mkdtemp(), "missing.json")), [])
