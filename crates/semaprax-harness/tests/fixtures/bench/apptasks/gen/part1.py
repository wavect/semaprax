# Task definitions part 1. Each task: dict(id, cls, langs, project{path:text}, steps[...])
# step: dict(id, request, grade[list of {cmd, expect_stdout?}], hidden{path:text}, ref{path:text}, feedback)
EMPTY = ""

def t_feature_py():
    project = {
    "shop/__init__.py": EMPTY,
    "shop/models.py": '''from dataclasses import dataclass


@dataclass(frozen=True)
class Item:
    sku: str
    name: str
    unit_cents: int
''',
    "shop/pricing.py": '''from .models import Item


def line_cents(item: Item, qty: int) -> int:
    if qty <= 0:
        raise ValueError("qty must be positive")
    return item.unit_cents * qty
''',
    "shop/cart.py": '''from .models import Item
from .pricing import line_cents


class Cart:
    def __init__(self):
        self.lines = []  # (Item, qty)

    def add(self, item: Item, qty: int = 1):
        self.lines.append((item, qty))

    def total_cents(self) -> int:
        return sum(line_cents(i, q) for i, q in self.lines)
''',
    "shop/report.py": '''from .pricing import line_cents


def fmt_money(cents: int) -> str:
    return f"${cents // 100}.{cents % 100:02d}"


def format_line(item, qty) -> str:
    return f"{item.name} x{qty} {fmt_money(line_cents(item, qty))}"
''',
    "tests/__init__.py": EMPTY,
    "tests/test_shop.py": '''import unittest
from shop.models import Item
from shop.cart import Cart
from shop.report import format_line


class ShopTests(unittest.TestCase):
    def test_total(self):
        c = Cart()
        c.add(Item("a", "Apple", 120), 2)
        c.add(Item("b", "Bread", 250))
        self.assertEqual(c.total_cents(), 490)

    def test_line(self):
        self.assertEqual(format_line(Item("a", "Apple", 120), 2), "Apple x2 $2.40")

    def test_bad_qty(self):
        from shop.pricing import line_cents
        with self.assertRaises(ValueError):
            line_cents(Item("a", "Apple", 1), 0)
''',
    }
    hidden = {"tests/test_bulk.py": '''import unittest
from shop.models import Item
from shop.cart import Cart
from shop.pricing import line_cents
from shop.report import format_line


class BulkTests(unittest.TestCase):
    def test_threshold(self):
        it = Item("a", "A", 1000)
        self.assertEqual(line_cents(it, 9), 9000)
        self.assertEqual(line_cents(it, 10), 9000)
        self.assertEqual(line_cents(it, 20), 18000)

    def test_rounding(self):
        self.assertEqual(line_cents(Item("a", "A", 105), 10), 945)

    def test_cart_total(self):
        c = Cart()
        c.add(Item("a", "A", 1000), 10)
        c.add(Item("b", "B", 300), 2)
        self.assertEqual(c.total_cents(), 9600)

    def test_report_marker(self):
        self.assertEqual(format_line(Item("a", "A", 1000), 10), "A x10 $90.00 (bulk)")
        self.assertEqual(format_line(Item("a", "A", 1000), 9), "A x9 $90.00")
'''}
    ref = {"shop/pricing.py": '''from .models import Item

BULK_QTY = 10


def is_bulk(qty: int) -> bool:
    return qty >= BULK_QTY


def line_cents(item: Item, qty: int) -> int:
    if qty <= 0:
        raise ValueError("qty must be positive")
    cents = item.unit_cents * qty
    if is_bulk(qty):
        cents -= cents // 10
    return cents
''', "shop/report.py": '''from .pricing import line_cents, is_bulk


def fmt_money(cents: int) -> str:
    return f"${cents // 100}.{cents % 100:02d}"


def format_line(item, qty) -> str:
    text = f"{item.name} x{qty} {fmt_money(line_cents(item, qty))}"
    return text + " (bulk)" if is_bulk(qty) else text
'''}
    return dict(id="feature-py-shop", cls="feature", langs=["python"], project=project, query="bulk discount line price cart report",
      steps=[dict(id="s1", request="Add a bulk discount to the shop package. When a line's quantity is 10 or more, that line costs 10% less: discounted cents = cents - cents // 10 (integer arithmetic). Cart totals must reflect the discount, and report.format_line must append \" (bulk)\" to the text of discounted lines. Existing tests must keep passing; do not edit files under tests/.",
        grade=[dict(cmd=["{python}","-m","unittest","discover","-s","tests","-t","."])], hidden=hidden, ref=ref, feedback="grader")])

def t_refactor_py():
    body = lambda extra: extra
    project = {
    "app/__init__.py": EMPTY,
    "app/users.py": '''def register(email: str, users: dict) -> str:
    e = email.strip().lower()
    if "@" not in e or e.startswith("@") or e.endswith("@"):
        raise ValueError("invalid email")
    if e in users:
        raise ValueError("duplicate")
    users[e] = {"email": e}
    return e
''',
    "app/invites.py": '''def invite(email: str, invited: set) -> str:
    e = email.strip().lower()
    if "@" not in e or e.startswith("@") or e.endswith("@"):
        raise ValueError("invalid email")
    invited.add(e)
    return e
''',
    "app/newsletter.py": '''def subscribe(email: str, subs: list) -> str:
    e = email.strip().lower()
    if "@" not in e or e.startswith("@") or e.endswith("@"):
        raise ValueError("invalid email")
    if e not in subs:
        subs.append(e)
    return e
''',
    "tests/__init__.py": EMPTY,
    "tests/test_app.py": '''import unittest
from app.users import register
from app.invites import invite
from app.newsletter import subscribe


class AppTests(unittest.TestCase):
    def test_register(self):
        users = {}
        self.assertEqual(register("  Bob@Example.COM ", users), "bob@example.com")
        with self.assertRaises(ValueError):
            register("bob@example.com", users)

    def test_invite(self):
        s = set()
        self.assertEqual(invite("A@b.c", s), "a@b.c")
        self.assertEqual(s, {"a@b.c"})

    def test_subscribe(self):
        subs = []
        subscribe("X@y.z", subs)
        subscribe("x@y.z", subs)
        self.assertEqual(subs, ["x@y.z"])
''',
    }
    hidden = {"tests/test_hidden_refactor.py": '''import unittest
from app.users import register
from app.invites import invite
from app.newsletter import subscribe


class Hidden(unittest.TestCase):
    def test_invalid_everywhere(self):
        for bad in ["nope", "@x", "x@", "   "]:
            with self.assertRaisesRegex(ValueError, "^invalid email$"):
                register(bad, {})
            with self.assertRaisesRegex(ValueError, "^invalid email$"):
                invite(bad, set())
            with self.assertRaisesRegex(ValueError, "^invalid email$"):
                subscribe(bad, [])

    def test_validators_module(self):
        from app.validators import normalize_email
        self.assertEqual(normalize_email("  Q@R.s "), "q@r.s")
        with self.assertRaisesRegex(ValueError, "^invalid email$"):
            normalize_email("zzz")
''', "_grader/check_refactor.py": '''import sys, pathlib
root = pathlib.Path(".")
v = (root / "app/validators.py").read_text()
assert "def normalize_email" in v, "validators.normalize_email missing"
for m in ("users", "invites", "newsletter"):
    src = (root / f"app/{m}.py").read_text()
    assert "normalize_email" in src, f"{m} does not use normalize_email"
    assert ".strip().lower()" not in src, f"{m} still duplicates normalization"
print("refactor structure ok")
'''}
    ref = {"app/validators.py": '''def normalize_email(raw: str) -> str:
    e = raw.strip().lower()
    if "@" not in e or e.startswith("@") or e.endswith("@"):
        raise ValueError("invalid email")
    return e
''', "app/users.py": '''from .validators import normalize_email


def register(email: str, users: dict) -> str:
    e = normalize_email(email)
    if e in users:
        raise ValueError("duplicate")
    users[e] = {"email": e}
    return e
''', "app/invites.py": '''from .validators import normalize_email


def invite(email: str, invited: set) -> str:
    e = normalize_email(email)
    invited.add(e)
    return e
''', "app/newsletter.py": '''from .validators import normalize_email


def subscribe(email: str, subs: list) -> str:
    e = normalize_email(email)
    if e not in subs:
        subs.append(e)
    return e
'''}
    return dict(id="refactor-py-validators", cls="refactor", langs=["python"], project=project, query="email validation normalize duplicated",
      steps=[dict(id="s1", request="The email normalization and validation logic is duplicated in app/users.py, app/invites.py and app/newsletter.py. Extract it into a new module app/validators.py exposing normalize_email(raw: str) -> str (strip, lowercase, raise ValueError(\"invalid email\") when invalid) and use it from all three modules. Observable behavior must not change. Do not edit files under tests/.",
        grade=[dict(cmd=["{python}","-m","unittest","discover","-s","tests","-t","."]), dict(cmd=["{python}","_grader/check_refactor.py"])], hidden=hidden, ref=ref, feedback="grader")])

def t_feature_js():
    project = {
    "package.json": '{"name":"cart","version":"1.0.0","private":true}\n',
    "src/cart.js": '''class Cart {
  constructor() {
    this.items = [];
  }
  add(sku, cents, qty = 1) {
    this.items.push({ sku, cents, qty });
  }
  subtotal() {
    return this.items.reduce((n, i) => n + i.cents * i.qty, 0);
  }
  total(rate = 0) {
    return Math.round(this.subtotal() * (1 + rate));
  }
}
module.exports = { Cart };
''',
    "src/format.js": '''function money(cents) {
  return "$" + Math.floor(cents / 100) + "." + String(cents % 100).padStart(2, "0");
}
function receipt(cart) {
  const lines = cart.items.map((i) => `${i.sku} x${i.qty} ${money(i.cents * i.qty)}`);
  lines.push(`Total ${money(cart.total())}`);
  return lines.join("\\n");
}
module.exports = { money, receipt };
''',
    "src/index.js": '''const { Cart } = require("./cart");
const { money, receipt } = require("./format");
module.exports = { Cart, money, receipt };
''',
    "test/cart.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { Cart, receipt } = require("../src");

test("subtotal and total", () => {
  const c = new Cart();
  c.add("a", 1000, 2);
  c.add("b", 250);
  assert.strictEqual(c.subtotal(), 2250);
  assert.strictEqual(c.total(0.2), 2700);
});
test("receipt", () => {
  const c = new Cart();
  c.add("a", 500, 2);
  assert.strictEqual(receipt(c), "a x2 $10.00\\nTotal $10.00");
});
''',
    }
    hidden = {"test/coupon.test.js": '''const test = require("node:test");
const assert = require("node:assert");
const { Cart, receipt } = require("../src");

test("SAVE10 takes 10 percent off, floored", () => {
  const c = new Cart();
  c.add("a", 1005, 1);
  c.applyCoupon("SAVE10");
  assert.strictEqual(c.total(), 905);
});
test("FIVE takes 500 off but never below zero", () => {
  const c = new Cart();
  c.add("a", 300, 1);
  c.applyCoupon("FIVE");
  assert.strictEqual(c.total(), 0);
  const d = new Cart();
  d.add("a", 2000, 1);
  d.applyCoupon("FIVE");
  assert.strictEqual(d.total(), 1500);
});
test("tax applies after coupon", () => {
  const c = new Cart();
  c.add("a", 2000, 1);
  c.applyCoupon("FIVE");
  assert.strictEqual(c.total(0.2), 1800);
});
test("unknown coupon throws", () => {
  assert.throws(() => new Cart().applyCoupon("NOPE"), /unknown coupon/);
});
test("receipt shows the coupon line", () => {
  const c = new Cart();
  c.add("a", 2000, 1);
  c.applyCoupon("FIVE");
  assert.strictEqual(receipt(c), "a x1 $20.00\\nCoupon FIVE -$5.00\\nTotal $15.00");
});
test("receipt without coupon is unchanged", () => {
  const c = new Cart();
  c.add("a", 500, 2);
  assert.strictEqual(receipt(c), "a x2 $10.00\\nTotal $10.00");
});
'''}
    ref = {"src/coupons.js": '''const COUPONS = {
  SAVE10: (subtotal) => Math.floor(subtotal / 10),
  FIVE: (subtotal) => Math.min(500, subtotal),
};
function discount(code, subtotal) {
  const rule = COUPONS[code];
  if (!rule) throw new Error("unknown coupon");
  return rule(subtotal);
}
module.exports = { discount };
''', "src/cart.js": '''const { discount } = require("./coupons");

class Cart {
  constructor() {
    this.items = [];
    this.coupon = null;
  }
  add(sku, cents, qty = 1) {
    this.items.push({ sku, cents, qty });
  }
  applyCoupon(code) {
    discount(code, 0);
    this.coupon = code;
  }
  subtotal() {
    return this.items.reduce((n, i) => n + i.cents * i.qty, 0);
  }
  discountCents() {
    return this.coupon ? discount(this.coupon, this.subtotal()) : 0;
  }
  total(rate = 0) {
    return Math.round((this.subtotal() - this.discountCents()) * (1 + rate));
  }
}
module.exports = { Cart };
''', "src/format.js": '''function money(cents) {
  return "$" + Math.floor(cents / 100) + "." + String(cents % 100).padStart(2, "0");
}
function receipt(cart) {
  const lines = cart.items.map((i) => `${i.sku} x${i.qty} ${money(i.cents * i.qty)}`);
  if (cart.coupon) lines.push(`Coupon ${cart.coupon} -${money(cart.discountCents())}`);
  lines.push(`Total ${money(cart.total())}`);
  return lines.join("\\n");
}
module.exports = { money, receipt };
'''}
    return dict(id="feature-js-cart", cls="feature", langs=["node"], project=project, query="coupon discount cart receipt total",
      steps=[dict(id="s1", request="Add coupon support to the cart. cart.applyCoupon(code) accepts \"SAVE10\" (10% off the subtotal, floored: Math.floor(subtotal/10)) and \"FIVE\" (500 cents off, never more than the subtotal); any other code throws Error(\"unknown coupon\"). total(rate) applies the coupon first and tax after. format.receipt adds a line `Coupon <CODE> -<money>` before the Total line when a coupon is applied. Put the coupon rules in a new module src/coupons.js. Do not edit files under test/.",
        grade=[dict(cmd=["{node}","--test"])], hidden=hidden, ref=ref, feedback="grader")])
