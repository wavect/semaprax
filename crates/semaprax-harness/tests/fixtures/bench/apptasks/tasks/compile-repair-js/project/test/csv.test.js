const test = require("node:test");
const assert = require("node:assert");
const { totalOf, parseRow } = require("../src");

test("parseRow trims", () => {
  assert.deepStrictEqual(parseRow(" a, b ,c"), ["a", "b", "c"]);
});
test("totalOf sums a column and skips the header", () => {
  assert.strictEqual(totalOf("name,qty\nx,2\ny,5", 1), 7);
});
