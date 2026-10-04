const test = require("node:test");
const assert = require("node:assert");
const { nextDelayMs } = require("./poller");
test("delay", () => { assert.strictEqual(nextDelayMs({ interval_ms: 30000, jitter_ms: 4000 }, 0.5), 32000); });
