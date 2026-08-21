import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const directory = process.argv[2];
assert.ok(directory, "usage: node scripts/verify-wasm-scalar-exports.mjs <package>");

const bindings = await import(pathToFileURL(join(directory, "semaprax.bindings.js")));
const bytes = await readFile(join(directory, "app.wasm"));
const runtime = await bindings.instantiateBytes(bytes);

assert.deepEqual(bindings.exportIds, [
  "calculator.add",
  "calculator.divide",
  "calculator.is-negative",
  "calculator.multiply",
  "calculator.not",
  "calculator.subtract",
]);
assert.deepEqual(runtime.call("calculator.add", 19n, 23n), { ok: true, value: 42n });
assert.deepEqual(runtime.functions["calculator.divide"](84n, 2n), { ok: true, value: 42n });
assert.deepEqual(runtime.call("calculator.is-negative", -1n), { ok: true, value: true });
assert.deepEqual(runtime.call("calculator.is-negative", 0n), { ok: true, value: false });
assert.deepEqual(runtime.call("calculator.not", true), { ok: true, value: false });
assert.deepEqual(runtime.call("calculator.add", (1n << 63n) - 1n, 1n), {
  ok: false,
  status: { schema: "semaprax.status.v1", domain_id: "semaprax.arithmetic.v1", code: 1 },
});
assert.deepEqual(runtime.call("calculator.divide", 42n, 0n), {
  ok: false,
  status: { schema: "semaprax.status.v1", domain_id: "semaprax.contract.v1", code: 1 },
});
assert.throws(() => runtime.call("calculator.add", 1, 2n), TypeError);
assert.throws(() => runtime.call("unknown", 1n), RangeError);

const tampered = Buffer.from(bytes);
tampered[tampered.length - 1] ^= 1;
await assert.rejects(
  bindings.instantiateBytes(tampered),
  /WebAssembly artifact authentication failed/,
);

const rawImports = { env: {
  spx_add: (a, b) => a + b,
  spx_sub: (a, b) => a - b,
  spx_mul: (a, b) => a * b,
  spx_div: (a, b) => a / b,
  spx_rem: (a, b) => a % b,
  spx_neg: (value) => -value,
  spx_contract_fail: () => { throw new Error("unexpected contract failure"); },
} };
const raw = await WebAssembly.instantiate(bytes, rawImports);
const boolExport = "spx_scalar_" + Buffer.from("calculator.not").toString("hex");
assert.throws(() => raw.instance.exports[boolExport](2), WebAssembly.RuntimeError);

console.log("scalar-exports-v1-ok");
