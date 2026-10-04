const test = require("node:test");
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
  assert.strictEqual(receipt(c), "a x2 $10.00\nTotal $10.00");
});
