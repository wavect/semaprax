const test = require("node:test");
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
