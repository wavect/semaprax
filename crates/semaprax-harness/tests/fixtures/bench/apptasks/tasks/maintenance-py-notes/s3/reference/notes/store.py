from .model import Note
from .persist import load_notes, save_notes


class Store:
    def __init__(self, path):
        self.path = path
        self.notes = load_notes(path)

    def add(self, title, body="", tags=None):
        n = Note(len(self.notes) + 1, title, body, list(tags or []))
        self.notes.append(n)
        self.save()
        return n

    def find_by_tag(self, tag):
        return [n for n in self.notes if tag in n.tags]

    def search(self, text):
        t = text.lower()
        return [n for n in self.notes if t in n.title.lower() or t in (n.body or "").lower()]

    def save(self):
        save_notes(self.path, self.notes)
