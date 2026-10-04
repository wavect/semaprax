const test = require("node:test");
const assert = require("node:assert");
const { totalOf } = require("../src");
test("larger table", () => {
  assert.strictEqual(totalOf("a,b\n1,10\n2,20\n3,30", 1), 60);
  assert.strictEqual(totalOf("a,b\n1,10", 0), 1);
});
