#!/usr/bin/env python3
"""Small local source index. Independent of Semaprax: standard library only.

CLI: python3 index.py (orient|search|skeleton|references) <root> [arg]
"""
import hashlib
import json
import os
import re
import sys

SKIP_DIRS = {"node_modules", "target", "__pycache__"}
IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
DEFN = re.compile(r"^\s*(?:pub(?:\([a-z]+\))?\s+)?(?:async\s+)?(def|class|fn|function|struct|enum|trait)\s+([A-Za-z_]\w*)")
LANGS = {".py": "python", ".rs": "rust", ".js": "javascript", ".mjs": "javascript",
         ".ts": "typescript", ".go": "go", ".c": "c", ".h": "c", ".spx": "semaprax", ".md": "markdown"}


class Index:
    def __init__(self, root, max_files=2000, max_file_bytes=262144):
        self.root = os.path.realpath(root)
        self.files = {}  # rel path -> (language, [lines])
        self.skipped = []  # {"path","reason"}
        self.max_files, self.max_file_bytes = max_files, max_file_bytes
        self._walk()

    def _walk(self):
        for dirpath, dirs, names in os.walk(self.root):
            dirs[:] = sorted(d for d in dirs if not d.startswith(".") and d not in SKIP_DIRS)
            for name in sorted(names):
                full = os.path.join(dirpath, name)
                rel = os.path.relpath(full, self.root).replace(os.sep, "/")
                lang = LANGS.get(os.path.splitext(name)[1])
                if name.startswith(".") or lang is None or os.path.islink(full):
                    continue
                if len(self.files) >= self.max_files:
                    self.skipped.append({"path": rel, "reason": "file-count-limit"})
                    continue
                try:
                    if os.path.getsize(full) > self.max_file_bytes:
                        self.skipped.append({"path": rel, "reason": "file-too-large"})
                        continue
                    with open(full, "rb") as fh:
                        text = fh.read().decode("utf-8")
                except UnicodeDecodeError:
                    self.skipped.append({"path": rel, "reason": "not-utf8"})
                    continue
                except OSError:
                    self.skipped.append({"path": rel, "reason": "unreadable"})
                    continue
                self.files[rel] = (lang, text.split("\n"))

    def coverage(self):
        shown = self.skipped[:100]
        if len(self.skipped) > 100:  # bound the report itself; the sentinel keeps it honest
            shown = shown + [{"path": "*", "reason": f"{len(self.skipped) - 100} more skipped"}]
        return {"complete": not self.skipped, "indexed_files": len(self.files),
                "skipped": shown, "exhaustive": not self.skipped}

    def _item(self, rel, no, rank, text=None):
        lang, lines = self.files[rel]
        line = lines[no - 1]
        return {"path": rel, "span": {"start_line": no, "end_line": no},
                "digest": "sha256:" + hashlib.sha256(line.encode()).hexdigest(), "provenance": "structural",
                "language": lang, "rank": rank, "text": line if text is None else text}

    def orient(self):
        langs = {}
        for lang, _ in self.files.values():
            langs[lang] = langs.get(lang, 0) + 1
        return {"files": len(self.files), "languages": dict(sorted(langs.items())), "items": []}

    def occurrences(self, token):
        for rel in sorted(self.files):
            for i, line in enumerate(self.files[rel][1], 1):
                n = sum(1 for m in IDENT.findall(line) if m == token)
                if n:
                    yield rel, i, n

    def search(self, query, limit=20):
        toks = [t for t in IDENT.findall(query)]
        scored = {}
        for rel in sorted(self.files):
            for i, line in enumerate(self.files[rel][1], 1):
                idents = IDENT.findall(line)
                hits = sum(1 for t in toks if t in idents)
                if hits:
                    scored[(rel, i)] = hits * 100 + (50 if DEFN.match(line) else 0)
        order = sorted(scored, key=lambda k: (-scored[k], k))[:limit]
        return {"items": [self._item(r, n, scored[(r, n)]) for r, n in order]}

    def skeleton(self, path):
        if path not in self.files:
            return {"items": []}
        lines = self.files[path][1]
        return {"items": [self._item(path, i, 1) for i, l in enumerate(lines, 1) if DEFN.match(l)]}

    def references(self, symbol):
        return {"items": [self._item(r, i, n) for r, i, n in self.occurrences(symbol)]}


def main(argv):
    if len(argv) < 3 or argv[1] not in ("orient", "search", "skeleton", "references"):
        sys.stderr.write(__doc__)
        return 2
    idx = Index(argv[2])
    arg = argv[3] if len(argv) > 3 else ""
    out = {"orient": lambda: idx.orient(), "search": lambda: idx.search(arg),
           "skeleton": lambda: idx.skeleton(arg), "references": lambda: idx.references(arg)}[argv[1]]()
    out["coverage"] = idx.coverage()
    print(json.dumps(out, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
