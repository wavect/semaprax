const test = require("node:test");
const assert = require("node:assert");
const { iso } = require("../src/dates");

test("december", () => { assert.strictEqual(iso(2021, 12, 31), "2021-12-31"); });
test("pad case 1", () => { assert.strictEqual(iso(2021, 1, 1), "2021-01-01"); });
test("pad case 2", () => { assert.strictEqual(iso(2022, 1, 2), "2022-01-02"); });
test("pad case 3", () => { assert.strictEqual(iso(2023, 1, 3), "2023-01-03"); });
test("pad case 4", () => { assert.strictEqual(iso(2024, 1, 4), "2024-01-04"); });
test("pad case 5", () => { assert.strictEqual(iso(2025, 1, 5), "2025-01-05"); });
test("pad case 6", () => { assert.strictEqual(iso(2026, 1, 6), "2026-01-06"); });
test("pad case 7", () => { assert.strictEqual(iso(2027, 1, 7), "2027-01-07"); });
