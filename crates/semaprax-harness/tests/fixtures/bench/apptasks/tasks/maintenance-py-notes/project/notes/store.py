import json
import os

from .model import Note


class Store:
    def __init__(self, path):
        self.path = path
        self.notes = []
        if os.path.exists(path):
            with open(path) as f:
                self.notes = [Note(**d) for d in json.load(f)]

    def add(self, title, body=""):
        n = Note(len(self.notes) + 1, title, body)
        self.notes.append(n)
        self.save()
        return n

    def search(self, text):
        return [n for n in self.notes if text in n.title or text in n.body]

    def save(self):
        with open(self.path, "w") as f:
            json.dump([n.__dict__ for n in self.notes], f)
