"""Bounded immutable-source reads shared by successor metadata tools."""
from collections import OrderedDict
from pathlib import Path
import hashlib
import subprocess

CACHE_BYTES = 16 * 1024 * 1024
CACHE_ENTRIES = 128


def sha(value):
    return hashlib.sha256(value).hexdigest()


def digest(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


class Git:
    def __init__(self, root):
        self.root = root
        self.process = subprocess.Popen(["git", "cat-file", "--batch"], cwd=root,
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        self.cache = OrderedDict()
        self.bytes = 0
        self.peak = 0

    def get(self, revision, source):
        key = (revision, source)
        if key in self.cache:
            self.cache.move_to_end(key)
            return self.cache[key]
        self.process.stdin.write((revision + ":" + source + "\n").encode())
        self.process.stdin.flush()
        header = self.process.stdout.readline().decode().strip()
        if header.endswith(" missing"):
            data = None
        else:
            _, kind, count = header.split()
            assert kind == "blob", (revision, source, kind)
            data = self.process.stdout.read(int(count))
            assert self.process.stdout.read(1) == b"\n"
        count = 0 if data is None else len(data)
        while self.cache and (self.bytes + count > CACHE_BYTES or len(self.cache) >= CACHE_ENTRIES):
            _, old = self.cache.popitem(last=False)
            self.bytes -= 0 if old is None else len(old)
        if count <= CACHE_BYTES:
            self.cache[key] = data
            self.bytes += count
            self.peak = max(self.peak, self.bytes)
        assert self.bytes <= CACHE_BYTES and len(self.cache) <= CACHE_ENTRIES
        return data

    def close(self):
        self.process.terminate()
        self.process.wait()
