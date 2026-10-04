const test = require("node:test");
const assert = require("node:assert");
const { permalink } = require("../src/post");

test("permalink", () => { assert.strictEqual(permalink("Hello, World!", 2024, 3, 9), "/2024/03/09/hello-world"); });
