import os
SPX_DIR = "(unused)"

def t_spx_repair():
    core_ok = open("/Users/kevin/Documents/ChatGPT/hp-wt/hn-bench/crates/semaprax-harness/tests/fixtures/bench/projects/mixed/src/core.spx").read()
    app_ok = open("/Users/kevin/Documents/ChatGPT/hp-wt/hn-bench/crates/semaprax-harness/tests/fixtures/bench/projects/mixed/src/app.spx").read()
    tests_ok = open("/Users/kevin/Documents/ChatGPT/hp-wt/hn-bench/crates/semaprax-harness/tests/fixtures/bench/projects/mixed/src/tests.spx").read()
    toml = open("/Users/kevin/Documents/ChatGPT/hp-wt/hn-bench/crates/semaprax-harness/tests/fixtures/bench/projects/mixed/semaprax.toml").read()
    core_bad = core_ok.replace('@id("calculator.multiply")\nfn multiply', 'fn multiply')
    assert core_bad != core_ok
    app_bad = app_ok.replace('subtract(divide(4, 2), 2)', 'subtract(divide(4, 2), true)')
    assert app_bad != app_ok
    tests_bad = tests_ok.replace('is_negative(-1)', 'is_negative(-1, 0)')
    assert tests_bad != tests_ok
    project = {"semaprax.toml": toml, "src/core.spx": core_bad, "src/app.spx": app_bad, "src/tests.spx": tests_bad}
    hidden = {"_grader/check_ids.py": '''import re, pathlib
core = pathlib.Path("src/core.spx").read_text()
ids = set(re.findall(r'@id\\("(calculator\\.[a-z-]+)"\\)', core))
want = {"calculator.add","calculator.subtract","calculator.multiply","calculator.divide","calculator.is-negative","calculator.not"}
assert want <= ids, f"missing declarations: {sorted(want - ids)}"
t = pathlib.Path("src/tests.spx").read_text()
assert "is_negative(" in t and "divide(84, 2) == 42" in t, "tests module was weakened"
print("declarations and tests intact")
'''}
    ref = {"src/core.spx": core_ok, "src/app.spx": app_ok, "src/tests.spx": tests_ok}
    return dict(id="compile-repair-spx", cls="compile_repair", langs=["spx"], project=project, query="project does not check diagnostics",
      steps=[dict(id="s1", request="The SEMAPRAX project in this directory no longer passes `semaprax check`. Repair the .spx sources so that `semaprax check .` succeeds, `semaprax run .` prints 42 and `semaprax test .` passes. Do not delete declarations or weaken the tests module; do not edit semaprax.toml. SEMAPRAX syntax notes: every public fn needs a persistent `@id(\"...\")` line directly above it; imports look like `use function @id(\"calculator.add\") from calculator.core as add;`.",
        grade=[dict(cmd=["{compiler}","check","."]), dict(cmd=["{compiler}","run","."], expect_stdout="42"), dict(cmd=["{compiler}","test","."]), dict(cmd=["{python}","_grader/check_ids.py"])],
        hidden=hidden, ref=ref, feedback="grader", first_failure_only=True, max_attempts=3)])

def t_js_repair():
    project = {
    "package.json": '{"name":"csvlite","version":"1.0.0","private":true}\n',
    "src/parse.js": '''function parseRow(line) {
  return line.split(",").map((c) => c.trim();
}
function parseTable(text) {
  return text.trim().split("\\n").map(parseRow);
}
module.exports = { parseRow, parseTable };
''',
    "src/stats.js": '''const { sum } = require("./util");

function columnSum(table, idx) {
  return sum(table.map((r) => Number(r[idx])));
}
module.exports = { columnSum };
''',
    "src/utils.js": '''function sum(xs) {
  return xs.reduce((a, b) => a + b, 0);
}
module.exports = { sum };
''',
    "src/index.js": '''const { parseTable } = require("./parse");
const { columnSum } = require("./stats");

function totalOf(text, idx) {
  const rows = parseTable(text);
  return columnSum(rows.slice(1), idx);
}
module.exports = { totalOf, parseRow: require("./parse").parseRow };
''',
    "test/csv.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { totalOf, parseRow } = require("../src");

test("parseRow trims", () => {
  assert.deepStrictEqual(parseRow(" a, b ,c"), ["a", "b", "c"]);
});
test("totalOf sums a column and skips the header", () => {
  assert.strictEqual(totalOf("name,qty\\nx,2\\ny,5", 1), 7);
});
''',
    }
    hidden = {"_grader/syntax.js": '''const { execFileSync } = require("node:child_process");
for (const f of ["src/parse.js", "src/stats.js", "src/utils.js", "src/index.js"]) {
  execFileSync(process.execPath, ["--check", f], { stdio: "pipe" });
}
console.log("syntax ok");
''', "test/hidden.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { totalOf } = require("../src");
test("larger table", () => {
  assert.strictEqual(totalOf("a,b\\n1,10\\n2,20\\n3,30", 1), 60);
  assert.strictEqual(totalOf("a,b\\n1,10", 0), 1);
});
'''}
    ref = {"src/parse.js": '''function parseRow(line) {
  return line.split(",").map((c) => c.trim());
}
function parseTable(text) {
  return text.trim().split("\\n").map(parseRow);
}
module.exports = { parseRow, parseTable };
''', "src/stats.js": '''const { sum } = require("./utils");

function columnSum(table, idx) {
  return sum(table.map((r) => Number(r[idx])));
}
module.exports = { columnSum };
'''}
    return dict(id="compile-repair-js", cls="compile_repair", langs=["node"], project=project, query="syntax error missing module",
      steps=[dict(id="s1", request="The csvlite package no longer loads: tests fail before running. Repair the sources under src/ so that every file passes `node --check` and `node --test test/` passes. Do not edit files under test/. Only the first error is reported at a time; fix all that you can find.",
        grade=[dict(cmd=["{node}","_grader/syntax.js"]), dict(cmd=["{node}","--test"])], hidden=hidden, ref=ref, feedback="grader", first_failure_only=True, max_attempts=3)])

# ---- failing tests (python) ----
def t_failing_py():
    cases_money = "\n".join(f'    def test_cents_{n}(self):\n        self.assertEqual(to_cents("{n}.00"), {n*100})\n' for n in range(1,9))
    cases_fmt = "\n".join(f'    def test_fmt_{n}(self):\n        self.assertEqual(fmt(to_cents("{n}.50")), "${n}.50")\n' for n in range(1,9))
    project = {
    "ledger/__init__.py": EMPTY_,
    "ledger/money.py": '''def to_cents(text: str) -> int:
    """Parse a decimal string such as "19.99" into integer cents."""
    return int(float(text) * 100)


def fmt(cents: int) -> str:
    return f"${cents // 100}.{cents % 100:02d}"
''',
    "ledger/tax.py": '''def tax_cents(net_cents: int, rate_percent: int) -> int:
    """Tax rounded half up to whole cents."""
    return net_cents * rate_percent // 100
''',
    "ledger/invoice.py": '''from .money import to_cents, fmt
from .tax import tax_cents


def invoice_total(lines, rate_percent):
    """lines: list of (decimal string, qty). Returns (net, tax, gross) in cents."""
    net = sum(to_cents(p) * q for p, q in lines)
    tax = tax_cents(net, rate_percent)
    return net, tax, net + tax


def invoice_text(lines, rate_percent):
    net, tax, gross = invoice_total(lines, rate_percent)
    return f"net {fmt(net)} tax {fmt(tax)} total {fmt(gross)}"
''',
    "tests/__init__.py": EMPTY_,
    "tests/test_money.py": '''import unittest
from ledger.money import to_cents, fmt


class MoneyTests(unittest.TestCase):
    def test_simple(self):
        self.assertEqual(to_cents("19.99"), 1999)

    def test_fmt_roundtrip(self):
        self.assertEqual(fmt(to_cents("4.05")), "$4.05")

''' + cases_money + "\n" + cases_fmt,
    "tests/test_tax.py": '''import unittest
from ledger.tax import tax_cents


class TaxTests(unittest.TestCase):
    def test_exact(self):
        self.assertEqual(tax_cents(1000, 20), 200)

    def test_half_up(self):
        self.assertEqual(tax_cents(1050, 10), 105)
        self.assertEqual(tax_cents(1005, 10), 101)

''' + "\n".join(f'    def test_zero_rate_{n}(self):\n        self.assertEqual(tax_cents({n*100}, 0), 0)\n' for n in range(1,7)),
    "tests/test_invoice.py": '''import unittest
from ledger.invoice import invoice_total, invoice_text


class InvoiceTests(unittest.TestCase):
    def test_total(self):
        self.assertEqual(invoice_total([("19.99", 3)], 10), (5997, 600, 6597))

    def test_text(self):
        self.assertEqual(invoice_text([("0.10", 3), ("0.20", 1)], 0), "net $0.50 tax $0.00 total $0.50")

''' + "\n".join(f'    def test_qty_{n}(self):\n        self.assertEqual(invoice_total([("1.00", {n})], 0)[0], {n*100})\n' for n in range(1,7)),
    }
    ref = {"ledger/money.py": '''from decimal import Decimal


def to_cents(text: str) -> int:
    """Parse a decimal string such as "19.99" into integer cents."""
    return int(Decimal(text) * 100)


def fmt(cents: int) -> str:
    return f"${cents // 100}.{cents % 100:02d}"
''', "ledger/tax.py": '''def tax_cents(net_cents: int, rate_percent: int) -> int:
    """Tax rounded half up to whole cents."""
    return (net_cents * rate_percent + 50) // 100
'''}
    hidden = {"tests/test_hidden_ledger.py": '''import unittest
from ledger.money import to_cents
from ledger.tax import tax_cents
from ledger.invoice import invoice_total


class Hidden(unittest.TestCase):
    def test_money_edges(self):
        self.assertEqual(to_cents("0.29"), 29)
        self.assertEqual(to_cents("1.15"), 115)
        self.assertEqual(to_cents("100"), 10000)

    def test_tax_edges(self):
        self.assertEqual(tax_cents(1, 50), 1)
        self.assertEqual(tax_cents(999, 7), 70)

    def test_invoice(self):
        self.assertEqual(invoice_total([("0.29", 100)], 0), (2900, 0, 2900))
'''}
    return dict(id="failing-tests-py-ledger", cls="failing_tests", langs=["python"], project=project, query="to_cents tax_cents rounding failing tests",
      steps=[dict(id="s1", request="The test suite fails. Fix the code under ledger/ so that all tests pass. Do not edit anything under tests/. The output of the test run is included below.",
        grade=[dict(cmd=["{python}","-m","unittest","discover","-s","tests","-t","."])], hidden=hidden, ref=ref, feedback="grader",
        initial_command=dict(cmd=["{python}","-m","unittest","discover","-v","-s","tests","-t","."], rtk=["test","{python}","-m","unittest","discover","-v","-s","tests","-t","."]))])

EMPTY_ = ""
