import json
import os

from .model import Note


def load_notes(path):
    if not os.path.exists(path):
        return []
    with open(path) as f:
        return [Note(**d) for d in json.load(f)]


def save_notes(path, notes):
    with open(path, "w") as f:
        json.dump([n.__dict__ for n in notes], f)
