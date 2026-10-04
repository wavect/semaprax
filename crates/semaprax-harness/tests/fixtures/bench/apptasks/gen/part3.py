from part2 import EMPTY_ as EMPTY

def t_failing_js():
    cases = "\n".join(f'test("slug case {n}", () => {{ assert.strictEqual(slug("Hello World {n}"), "hello-world-{n}"); }});' for n in range(1,9))
    cases2 = "\n".join(f'test("pad case {n}", () => {{ assert.strictEqual(iso({2020+n}, 1, {n}), "{2020+n}-01-0{n}"); }});' for n in range(1,8))
    project = {
    "package.json": '{"name":"textkit","version":"1.0.0","private":true}\n',
    "src/slug.js": '''function slug(text) {
  return text.toLowerCase().replace(/[^a-z0-9]+/g, "-");
}
module.exports = { slug };
''',
    "src/dates.js": '''function pad(n) {
  return n < 10 ? "0" + n : String(n);
}
// month is 1-based
function iso(year, month, day) {
  return `${year}-${pad(month - 1)}-${pad(day)}`;
}
module.exports = { iso };
''',
    "src/post.js": '''const { slug } = require("./slug");
const { iso } = require("./dates");

function permalink(title, y, m, d) {
  return `/${iso(y, m, d).replace(/-/g, "/")}/${slug(title)}`;
}
module.exports = { permalink };
''',
    "test/slug.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { slug } = require("../src/slug");

test("trailing punctuation", () => { assert.strictEqual(slug("Hi there!"), "hi-there"); });
test("leading punctuation", () => { assert.strictEqual(slug("  Hi"), "hi"); });
''' + cases + "\n",
    "test/dates.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { iso } = require("../src/dates");

test("december", () => { assert.strictEqual(iso(2021, 12, 31), "2021-12-31"); });
''' + cases2 + "\n",
    "test/post.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { permalink } = require("../src/post");

test("permalink", () => { assert.strictEqual(permalink("Hello, World!", 2024, 3, 9), "/2024/03/09/hello-world"); });
''',
    }
    ref = {"src/slug.js": '''function slug(text) {
  return text.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
}
module.exports = { slug };
''', "src/dates.js": '''function pad(n) {
  return n < 10 ? "0" + n : String(n);
}
// month is 1-based
function iso(year, month, day) {
  return `${year}-${pad(month)}-${pad(day)}`;
}
module.exports = { iso };
'''}
    hidden = {"test/hidden.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { slug } = require("../src/slug");
const { iso } = require("../src/dates");
const { permalink } = require("../src/post");
test("slug edges", () => {
  assert.strictEqual(slug("--A  b--"), "a-b");
  assert.strictEqual(slug("x"), "x");
});
test("iso edges", () => {
  assert.strictEqual(iso(1999, 10, 1), "1999-10-01");
});
test("permalink december", () => {
  assert.strictEqual(permalink("End of Year", 2023, 12, 31), "/2023/12/31/end-of-year");
});
'''}
    return dict(id="failing-tests-js-textkit", cls="failing_tests", langs=["node"], project=project, query="slug iso date failing tests",
      steps=[dict(id="s1", request="The test suite fails. Fix the code under src/ so that all tests pass. Do not edit anything under test/. The output of the test run is included below.",
        grade=[dict(cmd=["{node}","--test"])], hidden=hidden, ref=ref, feedback="grader",
        initial_command=dict(cmd=["{node}","--test"], rtk=["test","{node}","--test"]))])

def t_mixed_invoice():
    project = {
    "billing/__init__.py": EMPTY,
    "billing/models.py": '''from dataclasses import dataclass


@dataclass
class Invoice:
    number: str
    total_cents: int
''',
    "billing/serialize.py": '''from .models import Invoice


def to_json(inv: Invoice) -> dict:
    return {"number": inv.number, "totalCents": inv.total_cents}
''',
    "web/render.js": '''function renderInvoice(obj) {
  const total = (obj.totalCents / 100).toFixed(2);
  return `<article class="invoice"><h2>${obj.number}</h2><span class="total">${total}</span></article>`;
}
module.exports = { renderInvoice };
''',
    "docs/contract.md": '''# Invoice JSON contract

Python `billing.serialize.to_json` produces the object that `web/render.js` renders.

| key | type | notes |
| --- | --- | --- |
| number | string | invoice number |
| totalCents | integer | total in cents |
''',
    "tests/__init__.py": EMPTY,
    "tests/test_serialize.py": '''import unittest
from billing.models import Invoice
from billing.serialize import to_json


class SerializeTests(unittest.TestCase):
    def test_basic(self):
        self.assertEqual(to_json(Invoice("A-1", 1250)), {"number": "A-1", "totalCents": 1250})
''',
    "web/render.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { renderInvoice } = require("./render");
test("render basic", () => {
  assert.strictEqual(renderInvoice({ number: "A-1", totalCents: 1250 }),
    '<article class="invoice"><h2>A-1</h2><span class="total">12.50</span></article>');
});
''',
    }
    hidden = {"tests/test_due.py": '''import unittest
from billing.models import Invoice
from billing.serialize import to_json


class Due(unittest.TestCase):
    def test_without_due(self):
        self.assertEqual(to_json(Invoice("A-1", 5)), {"number": "A-1", "totalCents": 5})

    def test_with_due(self):
        inv = Invoice("A-2", 7, due_date="2026-12-31")
        self.assertEqual(to_json(inv), {"number": "A-2", "totalCents": 7, "dueDate": "2026-12-31"})
''', "_grader/emit.py": '''import json, sys
sys.path.insert(0, ".")
from billing.models import Invoice
from billing.serialize import to_json
json.dump([to_json(Invoice("Z-9", 4200, due_date="2027-01-15")), to_json(Invoice("Z-10", 100))], open("_cross.json", "w"))
''', "_grader/cross.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const { renderInvoice } = require("../web/render");
const [withDue, without] = JSON.parse(fs.readFileSync("_cross.json", "utf8"));
test("python output renders the due date", () => {
  assert.strictEqual(renderInvoice(withDue),
    '<article class="invoice"><h2>Z-9</h2><span class="total">42.00</span><time class="due">2027-01-15</time></article>');
});
test("no due date renders as before", () => {
  assert.strictEqual(renderInvoice(without),
    '<article class="invoice"><h2>Z-10</h2><span class="total">1.00</span></article>');
});
''', "_grader/check_doc.py": '''import pathlib
d = pathlib.Path("docs/contract.md").read_text()
assert "dueDate" in d, "contract doc does not mention dueDate"
print("doc ok")
'''}
    ref = {"billing/models.py": '''from dataclasses import dataclass
from typing import Optional


@dataclass
class Invoice:
    number: str
    total_cents: int
    due_date: Optional[str] = None
''', "billing/serialize.py": '''from .models import Invoice


def to_json(inv: Invoice) -> dict:
    out = {"number": inv.number, "totalCents": inv.total_cents}
    if inv.due_date is not None:
        out["dueDate"] = inv.due_date
    return out
''', "web/render.js": '''function renderInvoice(obj) {
  const total = (obj.totalCents / 100).toFixed(2);
  const due = obj.dueDate ? `<time class="due">${obj.dueDate}</time>` : "";
  return `<article class="invoice"><h2>${obj.number}</h2><span class="total">${total}</span>${due}</article>`;
}
module.exports = { renderInvoice };
''', "docs/contract.md": '''# Invoice JSON contract

Python `billing.serialize.to_json` produces the object that `web/render.js` renders.

| key | type | notes |
| --- | --- | --- |
| number | string | invoice number |
| totalCents | integer | total in cents |
| dueDate | string | optional ISO date, omitted when unset |
'''}
    return dict(id="mixed-py-js-invoice", cls="mixed_language", langs=["python","node"], project=project, query="invoice json contract due date render",
      steps=[dict(id="s1", request="Add an optional due date to invoices end to end. Python: Invoice gets an optional due_date (ISO string, default None) and billing.serialize.to_json emits it as the camelCase key \"dueDate\" only when set. JavaScript: web/render.js renders `<time class=\"due\">DATE</time>` right after the total span when dueDate is present, and renders exactly as before when it is absent. Update docs/contract.md to document the new key. Do not edit tests.",
        grade=[dict(cmd=["{python}","-m","unittest","discover","-s","tests","-t","."]), dict(cmd=["{python}","_grader/emit.py"]), dict(cmd=["{node}","--test"]), dict(cmd=["{python}","_grader/check_doc.py"])], hidden=hidden, ref=ref, feedback="grader")])

def t_mixed_units():
    project = {
    "config/schedule.json": '{\n  "interval": 30,\n  "jitter": 5\n}\n',
    "worker/__init__.py": EMPTY,
    "worker/schedule.py": '''import json


def load(path="config/schedule.json"):
    with open(path) as f:
        return json.load(f)


def next_delay(cfg, rand01):
    """Seconds to wait before the next run; rand01 in [0, 1)."""
    return cfg["interval"] + cfg["jitter"] * rand01
''',
    "web/poller.js": '''const fs = require("node:fs");

function load(path = "config/schedule.json") {
  return JSON.parse(fs.readFileSync(path, "utf8"));
}
// milliseconds until the next poll; rand01 in [0, 1)
function nextDelayMs(cfg, rand01) {
  return (cfg.interval + cfg.jitter * rand01) * 1000;
}
module.exports = { load, nextDelayMs };
''',
    "docs/schedule.md": "# Schedule\n\n`config/schedule.json` has `interval` and `jitter`, both in seconds.\n",
    "tests/__init__.py": EMPTY,
    "tests/test_schedule.py": '''import unittest
from worker.schedule import load, next_delay


class T(unittest.TestCase):
    def test_delay(self):
        self.assertEqual(next_delay({"interval": 30, "jitter": 4}, 0.5), 32)
''',
    "web/poller.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { nextDelayMs } = require("./poller");
test("delay", () => { assert.strictEqual(nextDelayMs({ interval: 30, jitter: 4 }, 0.5), 32000); });
''',
    }
    hidden = {"tests/test_schedule.py": '''import unittest
from worker.schedule import next_delay


class T(unittest.TestCase):
    def test_delay(self):
        self.assertEqual(next_delay({"interval_ms": 30000, "jitter_ms": 4000}, 0.5), 32000)
''', "web/poller.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { nextDelayMs } = require("./poller");
test("delay", () => { assert.strictEqual(nextDelayMs({ interval_ms: 30000, jitter_ms: 4000 }, 0.5), 32000); });
''', "tests/test_units.py": '''import unittest, json
from worker.schedule import load, next_delay


class Units(unittest.TestCase):
    def test_config_keys(self):
        cfg = load()
        self.assertEqual(cfg, {"interval_ms": 30000, "jitter_ms": 5000})

    def test_delay_is_milliseconds(self):
        self.assertEqual(next_delay({"interval_ms": 30000, "jitter_ms": 4000}, 0.5), 32000)
''', "_grader/units.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { load, nextDelayMs } = require("../web/poller");
test("shared config is in milliseconds", () => {
  assert.deepStrictEqual(load(), { interval_ms: 30000, jitter_ms: 5000 });
});
test("delay stays in milliseconds without a second multiplication", () => {
  assert.strictEqual(nextDelayMs({ interval_ms: 30000, jitter_ms: 4000 }, 0.5), 32000);
});
''', "_grader/check_doc.py": '''import pathlib
d = pathlib.Path("docs/schedule.md").read_text()
assert "interval_ms" in d and "jitter_ms" in d and "milliseconds" in d, "docs/schedule.md not updated"
print("doc ok")
'''}
    ref = {"config/schedule.json": '{\n  "interval_ms": 30000,\n  "jitter_ms": 5000\n}\n', "worker/schedule.py": '''import json


def load(path="config/schedule.json"):
    with open(path) as f:
        return json.load(f)


def next_delay(cfg, rand01):
    """Milliseconds to wait before the next run; rand01 in [0, 1)."""
    return cfg["interval_ms"] + cfg["jitter_ms"] * rand01
''', "web/poller.js": '''const fs = require("node:fs");

function load(path = "config/schedule.json") {
  return JSON.parse(fs.readFileSync(path, "utf8"));
}
// milliseconds until the next poll; rand01 in [0, 1)
function nextDelayMs(cfg, rand01) {
  return cfg.interval_ms + cfg.jitter_ms * rand01;
}
module.exports = { load, nextDelayMs };
''', "docs/schedule.md": "# Schedule\n\n`config/schedule.json` has `interval_ms` and `jitter_ms`, both in milliseconds.\n"}
    return dict(id="mixed-config-units", cls="mixed_language", langs=["python","node"], project=project, query="schedule interval jitter units config",
      steps=[dict(id="s1", request="Change the shared schedule configuration from seconds to milliseconds everywhere. config/schedule.json keys become `interval_ms` and `jitter_ms` (30000 and 5000). The Python worker (worker/schedule.py: next_delay) and the JavaScript poller (web/poller.js: nextDelayMs) must read the new keys and return milliseconds, and docs/schedule.md must describe the new keys and unit. The existing visible tests encode the old unit; the graders encode the new one. Do not rely on editing tests.",
        grade=[dict(cmd=["{python}","-m","unittest","discover","-s","tests","-t","."]), dict(cmd=["{node}","--test"]), dict(cmd=["{python}","_grader/check_doc.py"])], hidden=hidden, ref=ref, feedback="grader")])
