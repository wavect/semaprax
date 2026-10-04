const test = require("node:test");
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
  assert.strictEqual(receipt(c), "a x1 $20.00\nCoupon FIVE -$5.00\nTotal $15.00");
});
test("receipt without coupon is unchanged", () => {
  const c = new Cart();
  c.add("a", 500, 2);
  assert.strictEqual(receipt(c), "a x2 $10.00\nTotal $10.00");
});
