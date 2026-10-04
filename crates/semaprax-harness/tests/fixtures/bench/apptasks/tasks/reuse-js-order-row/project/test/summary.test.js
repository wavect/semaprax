const test = require("node:test");
const assert = require("node:assert");
const { line } = require("../app/summary");
test("line", () => {
  assert.strictEqual(line({ id: "o1", cents: 1230, placed: "2024-03-05" }), "o1: $12.30 on 05 Mar 2024");
});
