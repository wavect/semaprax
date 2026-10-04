const test = require("node:test");
const assert = require("node:assert");
const { nextDelayMs } = require("./poller");
test("delay", () => { assert.strictEqual(nextDelayMs({ interval: 30, jitter: 4 }, 0.5), 32000); });
