const test = require("node:test");
const assert = require("node:assert");
const { slug } = require("../src/slug");

test("trailing punctuation", () => { assert.strictEqual(slug("Hi there!"), "hi-there"); });
test("leading punctuation", () => { assert.strictEqual(slug("  Hi"), "hi"); });
test("slug case 1", () => { assert.strictEqual(slug("Hello World 1"), "hello-world-1"); });
test("slug case 2", () => { assert.strictEqual(slug("Hello World 2"), "hello-world-2"); });
test("slug case 3", () => { assert.strictEqual(slug("Hello World 3"), "hello-world-3"); });
test("slug case 4", () => { assert.strictEqual(slug("Hello World 4"), "hello-world-4"); });
test("slug case 5", () => { assert.strictEqual(slug("Hello World 5"), "hello-world-5"); });
test("slug case 6", () => { assert.strictEqual(slug("Hello World 6"), "hello-world-6"); });
test("slug case 7", () => { assert.strictEqual(slug("Hello World 7"), "hello-world-7"); });
test("slug case 8", () => { assert.strictEqual(slug("Hello World 8"), "hello-world-8"); });
