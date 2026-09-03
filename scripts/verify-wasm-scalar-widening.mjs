// Execute the generated facade of a widened Public Scalar Export Profile v1
// package. The package must export exactly the eight `widen.*` functions the
// Rust evidence builds, so this script is a known-answer test rather than a
// generic driver. It proves three things the Rust side cannot: the generated
// JavaScript admits and rejects exactly the widened ranges, the raw Wasm
// adapter traps on a host value its SEMAPRAX parameter type cannot contain,
// and both agree.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const directory = process.argv[2];
assert.ok(directory, "usage: node scripts/verify-wasm-scalar-widening.mjs <package>");

const bindings = await import(pathToFileURL(join(directory, "semaprax.bindings.js")));
const bytes = await readFile(join(directory, "app.wasm"));
const runtime = await bindings.instantiateBytes(bytes);

assert.deepEqual(bindings.exportIds, [
  "widen.bool",
  "widen.char",
  "widen.f32",
  "widen.f64",
  "widen.i32",
  "widen.i64",
  "widen.mixed",
  "widen.u8",
]);

// Every admitted scalar round-trips at both ends of its exact range.
const accepted = [
  ["widen.bool", [true], true],
  ["widen.bool", [false], false],
  ["widen.i64", [-(2n ** 63n)], -(2n ** 63n)],
  ["widen.i64", [2n ** 63n - 1n], 2n ** 63n - 1n],
  ["widen.i32", [-2147483648], -2147483648],
  ["widen.i32", [2147483647], 2147483647],
  ["widen.u8", [0], 0],
  ["widen.u8", [255], 255],
  ["widen.char", [0], 0],
  ["widen.char", [55295], 55295],
  ["widen.char", [57344], 57344],
  ["widen.char", [1114111], 1114111],
  ["widen.f32", [0], 0],
  ["widen.f32", [-0.5], -0.5],
  ["widen.f32", [Infinity], Infinity],
  ["widen.f64", [0.1], 0.1],
  ["widen.f64", [-Infinity], -Infinity],
  ["widen.mixed", [true, 1n, 2, 3, 4, 0.5], 2.5],
];
for (const [id, args, value] of accepted) {
  assert.deepEqual(runtime.call(id, ...args), { ok: true, value }, `${id}(${args})`);
  assert.deepEqual(runtime.functions[id](...args), { ok: true, value }, `${id}(${args})`);
}
assert.ok(Number.isNaN(runtime.call("widen.f32", NaN).value));
assert.ok(Number.isNaN(runtime.call("widen.f64", NaN).value));

// The facade rejects, with a TypeError, every host value outside the exact
// admitted range. It never truncates, rounds, or wraps one.
const rejected = [
  ["widen.u8", 256],
  ["widen.u8", -1],
  ["widen.u8", 1.5],
  ["widen.u8", 0n],
  ["widen.u8", "1"],
  ["widen.char", 1114112],
  ["widen.char", 0xd800],
  ["widen.char", 0xdbff],
  ["widen.char", 0xdc00],
  ["widen.char", 0xdfff],
  ["widen.char", -1],
  ["widen.char", 65.5],
  ["widen.i32", 2147483648],
  ["widen.i32", -2147483649],
  ["widen.i32", 1.5],
  ["widen.i32", 1n],
  ["widen.f32", 0.1],
  ["widen.f32", 1n],
  ["widen.f32", "0.5"],
  ["widen.f64", 1n],
  ["widen.f64", "0.1"],
  ["widen.i64", 1],
  ["widen.i64", 2n ** 63n],
  ["widen.bool", 1],
];
for (const [id, argument] of rejected) {
  assert.throws(() => runtime.call(id, argument), TypeError, `${id}(${String(argument)})`);
}

// The raw adapter fails closed on its own, without the generated facade. A
// caller that reaches `instance.exports` directly still cannot deliver a value
// the SEMAPRAX parameter type does not contain.
const rawImports = { env: {
  spx_add: (a, b) => a + b,
  spx_sub: (a, b) => a - b,
  spx_mul: (a, b) => a * b,
  spx_div: (a, b) => a / b,
  spx_rem: (a, b) => a % b,
  spx_neg: (value) => -value,
  spx_contract_fail: () => { throw new Error("unexpected contract failure"); },
} };
const raw = (await WebAssembly.instantiate(bytes, rawImports)).instance.exports;
const symbol = (id) => "spx_scalar_" + Buffer.from(id).toString("hex");

for (const value of [256, -1, 0x1_0000, 0x7fff_ffff]) {
  assert.throws(() => raw[symbol("widen.u8")](value), WebAssembly.RuntimeError, `u8 ${value}`);
}
for (const value of [1114112, 0xd800, 0xdbff, 0xdc00, 0xdfff, -1, 0x7fff_ffff]) {
  assert.throws(() => raw[symbol("widen.char")](value), WebAssembly.RuntimeError, `char ${value}`);
}
for (const value of [2, 255, -1]) {
  assert.throws(() => raw[symbol("widen.bool")](value), WebAssembly.RuntimeError, `bool ${value}`);
}
// `i32`, `f32`, and `f64` occupy their Wasm value type exactly, so the whole
// range is admissible and the adapter carries no check for them.
for (const value of [-2147483648, -1, 0, 2147483647]) {
  assert.equal(raw[symbol("widen.i32")](value), value);
}
for (const value of [0, 55295, 57344, 1114111]) {
  assert.equal(raw[symbol("widen.char")](value), value);
}
for (const value of [0, 255]) {
  assert.equal(raw[symbol("widen.u8")](value), value);
}

console.log("scalar-widening-v1-ok");
