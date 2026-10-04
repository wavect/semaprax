from part2 import EMPTY_ as EMPTY

def t_reuse_py():
    project = {
    "corelib/__init__.py": EMPTY,
    "corelib/text.py": '''import re


def slugify(text: str) -> str:
    """Lowercase, collapse non-alphanumerics to single dashes, trim dashes."""
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


def truncate(text: str, n: int) -> str:
    return text if len(text) <= n else text[: n - 1] + "…"


def title_case(text: str) -> str:
    return " ".join(w.capitalize() for w in text.split())
''',
    "corelib/dates.py": '''from datetime import date


def parse_iso(text: str) -> date:
    """Parse YYYY-MM-DD into a date; raises ValueError on bad input."""
    y, m, d = text.split("-")
    return date(int(y), int(m), int(d))


def days_between(a: date, b: date) -> int:
    return (b - a).days


def short_date(d: date) -> str:
    """Render as DD Mon YYYY, e.g. 05 Mar 2024."""
    months = "Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec".split()
    return f"{d.day:02d} {months[d.month - 1]} {d.year}"
''',
    "corelib/money.py": '''def format_cents(cents: int, symbol: str = "$") -> str:
    """Render integer cents as e.g. $12.30, with a leading minus for negatives."""
    sign = "-" if cents < 0 else ""
    cents = abs(cents)
    return f"{sign}{symbol}{cents // 100}.{cents % 100:02d}"


def parse_money(text: str) -> int:
    whole, _, frac = text.lstrip("$").partition(".")
    return int(whole) * 100 + int((frac + "00")[:2])
''',
    "corelib/ids.py": '''import hashlib


def short_id(text: str, n: int = 8) -> str:
    return hashlib.sha1(text.encode()).hexdigest()[:n]
''',
    "corelib/seq.py": '''def chunk(xs, n):
    return [xs[i : i + n] for i in range(0, len(xs), n)]


def group_by(xs, key):
    out = {}
    for x in xs:
        out.setdefault(key(x), []).append(x)
    return out
''',
    "corelib/mathx.py": '''def clamp(x, lo, hi):
    return max(lo, min(hi, x))


def pct(part, whole):
    return 0 if whole == 0 else round(100 * part / whole)
''',
    "corelib/tables.py": '''def pad_right(text: str, width: int) -> str:
    return text + " " * max(0, width - len(text))


def render_row(cells, widths):
    return " | ".join(pad_right(str(c), w) for c, w in zip(cells, widths))
''',
    "corelib/jsonx.py": '''import json


def dumps_stable(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"))
''',
    "reports/__init__.py": EMPTY,
    "reports/summary.py": '''from corelib.dates import parse_iso, days_between
from corelib.money import format_cents


def age_line(order: dict, today: str) -> str:
    age = days_between(parse_iso(order["placed"]), parse_iso(today))
    return f"{order['id']}: {format_cents(order['cents'])} ({age}d old)"
''',
    "reports/catalog.py": '''from corelib.text import slugify, truncate


def catalog_key(title: str) -> str:
    return truncate(slugify(title), 24)
''',
    "reports/order_row.py": '''def order_row(order: dict) -> str:
    """Return "<slug>|<DD Mon YYYY>|<money>" for an order dict:
    {"title": str, "placed": "YYYY-MM-DD", "cents": int}.
    TODO: not implemented yet."""
    raise NotImplementedError
''',
    "tests/__init__.py": EMPTY,
    "tests/test_reports.py": '''import unittest
from reports.summary import age_line
from reports.catalog import catalog_key


class T(unittest.TestCase):
    def test_age(self):
        self.assertEqual(age_line({"id": "o1", "placed": "2024-03-01", "cents": 1230}, "2024-03-11"), "o1: $12.30 (10d old)")

    def test_key(self):
        self.assertEqual(catalog_key("Hello, Big World!"), "hello-big-world")
''',
    }
    hidden = {"tests/test_order_row.py": '''import unittest
from reports.order_row import order_row


class T(unittest.TestCase):
    def test_row(self):
        self.assertEqual(order_row({"title": "Hello, Big World!", "placed": "2024-03-05", "cents": 1230}), "hello-big-world|05 Mar 2024|$12.30")

    def test_negative_and_trim(self):
        self.assertEqual(order_row({"title": "--Refund  Q--", "placed": "2023-12-31", "cents": -5}), "refund-q|31 Dec 2023|-$0.05")
''', "_grader/check_reuse.py": '''import pathlib, re
src = pathlib.Path("reports/order_row.py").read_text()
for need in ("slugify", "format_cents", "parse_iso", "short_date"):
    assert re.search(r"\\b%s\\b" % need, src), f"does not reuse existing {need}"
for forbid in ("import re", "strftime", "re.sub", "months", "// 100", "% 100"):
    assert forbid not in src, f"reimplements existing helper logic ({forbid})"
print("reuse ok")
'''}
    ref = {"reports/order_row.py": '''from corelib.text import slugify
from corelib.dates import parse_iso, short_date
from corelib.money import format_cents


def order_row(order: dict) -> str:
    return "|".join([slugify(order["title"]), short_date(parse_iso(order["placed"])), format_cents(order["cents"])])
'''}
    return dict(id="reuse-py-order-row", cls="index_reuse", langs=["python"], project=project, query="slugify short_date format_cents parse_iso",
      steps=[dict(id="s1", request="Implement reports/order_row.py:order_row(order) so that it returns \"<slug>|<DD Mon YYYY>|<money>\" for an order dict {\"title\", \"placed\" (YYYY-MM-DD), \"cents\"}: the title as a slug, the placed date as e.g. 05 Mar 2024, the amount as money with a $ sign. The corelib package already contains helpers for each of these; reuse them rather than reimplementing. Do not edit tests.",
        grade=[dict(cmd=["{python}","-m","unittest","discover","-s","tests","-t","."]), dict(cmd=["{python}","_grader/check_reuse.py"])], hidden=hidden, ref=ref, feedback="grader")])

def t_reuse_js():
    project = {
    "package.json": '{"name":"kit","version":"1.0.0","private":true}\n',
    "lib/strings.js": '''// kebab("Hello, World!") -> "hello-world"
function kebab(s) {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
function ellipsize(s, n) {
  return s.length <= n ? s : s.slice(0, n - 1) + "…";
}
function upperFirst(s) {
  return s.charAt(0).toUpperCase() + s.slice(1);
}
module.exports = { kebab, ellipsize, upperFirst };
''',
    "lib/when.js": '''const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
// readDay("2024-03-05") -> {y, m, d}
function readDay(text) {
  const [y, m, d] = text.split("-").map(Number);
  return { y, m, d };
}
// dayLabel({y,m,d}) -> "05 Mar 2024"
function dayLabel({ y, m, d }) {
  return `${String(d).padStart(2, "0")} ${MONTHS[m - 1]} ${y}`;
}
module.exports = { readDay, dayLabel };
''',
    "lib/cash.js": '''// cashLabel(-5) -> "-$0.05"
function cashLabel(cents, symbol = "$") {
  const abs = Math.abs(cents);
  return `${cents < 0 ? "-" : ""}${symbol}${Math.floor(abs / 100)}.${String(abs % 100).padStart(2, "0")}`;
}
module.exports = { cashLabel };
''',
    "lib/lists.js": '''function chunk(xs, n) {
  const out = [];
  for (let i = 0; i < xs.length; i += n) out.push(xs.slice(i, i + n));
  return out;
}
function uniq(xs) {
  return [...new Set(xs)];
}
module.exports = { chunk, uniq };
''',
    "lib/num.js": '''function clamp(x, lo, hi) {
  return Math.max(lo, Math.min(hi, x));
}
module.exports = { clamp };
''',
    "app/summary.js": '''const { readDay, dayLabel } = require("../lib/when");
const { cashLabel } = require("../lib/cash");
function line(order) {
  return `${order.id}: ${cashLabel(order.cents)} on ${dayLabel(readDay(order.placed))}`;
}
module.exports = { line };
''',
    "app/orderRow.js": '''// orderRow({title, placed: "YYYY-MM-DD", cents}) -> "<slug>|<DD Mon YYYY>|<money>"
function orderRow(order) {
  throw new Error("not implemented");
}
module.exports = { orderRow };
''',
    "test/summary.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { line } = require("../app/summary");
test("line", () => {
  assert.strictEqual(line({ id: "o1", cents: 1230, placed: "2024-03-05" }), "o1: $12.30 on 05 Mar 2024");
});
''',
    }
    hidden = {"test/orderRow.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { orderRow } = require("../app/orderRow");
test("row", () => {
  assert.strictEqual(orderRow({ title: "Hello, Big World!", placed: "2024-03-05", cents: 1230 }), "hello-big-world|05 Mar 2024|$12.30");
});
test("negative", () => {
  assert.strictEqual(orderRow({ title: "--Refund  Q--", placed: "2023-12-31", cents: -5 }), "refund-q|31 Dec 2023|-$0.05");
});
''', "_grader/check_reuse.py": '''import pathlib, re
src = pathlib.Path("app/orderRow.js").read_text()
for need in ("kebab", "readDay", "dayLabel", "cashLabel"):
    assert re.search(r"\\b%s\\b" % need, src), f"does not reuse existing {need}"
for forbid in ("toLowerCase", "padStart", "MONTHS", "Math.floor", ".replace("):
    assert forbid not in src, f"reimplements existing helper logic ({forbid})"
print("reuse ok")
'''}
    ref = {"app/orderRow.js": '''const { kebab } = require("../lib/strings");
const { readDay, dayLabel } = require("../lib/when");
const { cashLabel } = require("../lib/cash");

function orderRow(order) {
  return [kebab(order.title), dayLabel(readDay(order.placed)), cashLabel(order.cents)].join("|");
}
module.exports = { orderRow };
'''}
    return dict(id="reuse-js-order-row", cls="index_reuse", langs=["node","python"], project=project, query="kebab dayLabel cashLabel readDay",
      steps=[dict(id="s1", request="Implement app/orderRow.js:orderRow(order) so that it returns \"<slug>|<DD Mon YYYY>|<money>\" for {title, placed (YYYY-MM-DD), cents}: the title as a slug, the date like 05 Mar 2024, the amount as money with a $ sign. The lib/ directory already has helpers for each of these (names do not say \"slug\", \"format\" or \"parse\"); reuse them instead of reimplementing. Do not edit tests.",
        grade=[dict(cmd=["{node}","--test"]), dict(cmd=["{python}","_grader/check_reuse.py"])], hidden=hidden, ref=ref, feedback="grader")])

def t_maintenance():
    project = {
    "notes/__init__.py": EMPTY,
    "notes/model.py": '''from dataclasses import dataclass


@dataclass
class Note:
    id: int
    title: str
    body: str = ""
''',
    "notes/store.py": '''import json
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
''',
    "tests/__init__.py": EMPTY,
    "tests/test_store.py": '''import os, tempfile, unittest
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
''',
    }
    h1 = {"tests/test_tags.py": '''import os, tempfile, unittest
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
'''}
    r1 = {"notes/model.py": '''from dataclasses import dataclass, field


@dataclass
class Note:
    id: int
    title: str
    body: str = ""
    tags: list = field(default_factory=list)
''', "notes/store.py": '''import json
import os

from .model import Note


class Store:
    def __init__(self, path):
        self.path = path
        self.notes = []
        if os.path.exists(path):
            with open(path) as f:
                self.notes = [Note(**d) for d in json.load(f)]

    def add(self, title, body="", tags=None):
        n = Note(len(self.notes) + 1, title, body, list(tags or []))
        self.notes.append(n)
        self.save()
        return n

    def find_by_tag(self, tag):
        return [n for n in self.notes if tag in n.tags]

    def search(self, text):
        return [n for n in self.notes if text in n.title or text in n.body]

    def save(self):
        with open(self.path, "w") as f:
            json.dump([n.__dict__ for n in self.notes], f)
'''}
    h2 = {"tests/test_search.py": '''import os, tempfile, unittest
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
'''}
    r2 = {"notes/store.py": r1["notes/store.py"].replace('''        return [n for n in self.notes if text in n.title or text in n.body]''','''        t = text.lower()
        return [n for n in self.notes if t in n.title.lower() or t in (n.body or "").lower()]''')}
    h3 = {"tests/test_persist.py": '''import os, tempfile, unittest
from notes.persist import load_notes, save_notes
from notes.model import Note


class Persist(unittest.TestCase):
    def test_roundtrip(self):
        p = os.path.join(tempfile.mkdtemp(), "n.json")
        save_notes(p, [Note(1, "a", "b", ["t"])])
        self.assertEqual(load_notes(p), [Note(1, "a", "b", ["t"])])
        self.assertEqual(load_notes(os.path.join(tempfile.mkdtemp(), "missing.json")), [])
''', "_grader/check_persist.py": '''import pathlib
s = pathlib.Path("notes/store.py").read_text()
assert "json" not in s, "store.py still touches json directly"
assert "load_notes" in s and "save_notes" in s, "store.py does not use the persist module"
print("persist structure ok")
'''}
    r3 = {"notes/persist.py": '''import json
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
''', "notes/store.py": '''from .model import Note
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
'''}
    cmd = [dict(cmd=["{python}","-m","unittest","discover","-s","tests","-t","."])]
    # hidden tests accumulate across steps (each step's grader overlays all earlier hidden tests)
    return dict(id="maintenance-py-notes", cls="maintenance", langs=["python"], project=project, query="notes store search tags persistence",
      steps=[
        dict(id="s1", request="Maintenance session 1 of 3. Add tags to notes: Note gets a `tags` list (default empty), Store.add accepts an optional tags list, tags are persisted to the JSON file and reloaded, and Store.find_by_tag(tag) returns matching notes. Do not edit tests.", grade=cmd, hidden=h1, ref=r1, feedback="grader"),
        dict(id="s2", request="Maintenance session 2 of 3. Bug report: searching for \"MILK\" does not find a note titled \"Milk\", and Store.search raises TypeError when a note's body is None. Make search case-insensitive and tolerate a None body. Do not edit tests.", grade=cmd, hidden=h2, ref=r2, feedback="grader"),
        dict(id="s3", request="Maintenance session 3 of 3. Refactor: move JSON file loading and saving out of notes/store.py into a new notes/persist.py with load_notes(path) -> list[Note] (empty list when the file is missing) and save_notes(path, notes). Store must use them and must no longer import or use json. Behavior must not change. Do not edit tests.", grade=cmd+[dict(cmd=["{python}","_grader/check_persist.py"])], hidden=h3, ref=r3, feedback="grader"),
      ])

ALL = lambda: [t_feature_py(), t_refactor_py(), t_feature_js(), t_spx_repair(), t_js_repair(), t_failing_py(), t_failing_js(), t_mixed_invoice(), t_mixed_units(), t_reuse_py(), t_reuse_js(), t_maintenance()]
