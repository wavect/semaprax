const test = require("node:test");
const assert = require("node:assert");
const { load, nextDelayMs } = require("../web/poller");
test("shared config is in milliseconds", () => {
  assert.deepStrictEqual(load(), { interval_ms: 30000, jitter_ms: 5000 });
});
test("delay stays in milliseconds without a second multiplication", () => {
  assert.strictEqual(nextDelayMs({ interval_ms: 30000, jitter_ms: 4000 }, 0.5), 32000);
});
