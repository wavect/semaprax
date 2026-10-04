const test = require("node:test");
const assert = require("node:assert");
const { orderRow } = require("../app/orderRow");
test("row", () => {
  assert.strictEqual(orderRow({ title: "Hello, Big World!", placed: "2024-03-05", cents: 1230 }), "hello-big-world|05 Mar 2024|$12.30");
});
test("negative", () => {
  assert.strictEqual(orderRow({ title: "--Refund  Q--", placed: "2023-12-31", cents: -5 }), "refund-q|31 Dec 2023|-$0.05");
});
