// Normalized value/status observer, not a Wasm String finalization model.
import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";

const minimum = -(1n << 63n), maximum = (1n << 63n) - 1n;
class Failure extends Error {
  constructor(domain, code) { super(`${domain}:${code}`); this.domain = domain; this.code = code; }
}
const arithmetic = code => { throw new Failure("semaprax.arithmetic.v1", code); };
const checked = (value, code) => value < minimum || value > maximum ? arithmetic(code) : value;
for (const entry of process.argv.slice(2)) {
  const [id, path] = entry.split("|");
  const module = new WebAssembly.Module(await readFile(path));
  const names = ["spx_add", "spx_sub", "spx_mul", "spx_div", "spx_rem", "spx_neg", "spx_contract_fail",
    "spx_string_new", "spx_string_eq", "spx_string_clone", "spx_string_len", "spx_string_concat",
    "spx_string_starts_with", "spx_string_contains", "spx_string_len_chars", "spx_string_from_char"];
  assert.deepEqual(WebAssembly.Module.imports(module), names.map(name => ({ module: "env", name, kind: "function" })));
  const strings = new Map();
  const encoder = new TextEncoder(), decoder = new TextDecoder("utf-8", { fatal: true });
  const get = handle => { assert.ok(strings.has(handle)); return strings.get(handle); };
  const add = value => {
    assert.equal(typeof value, "string");
    assert.ok(strings.size < 4096);
    const handle = BigInt(strings.size + 1); strings.set(handle, value); return handle;
  };
  let instance;
  instance = new WebAssembly.Instance(module, { env: {
    spx_add: (a, b) => checked(a + b, 1), spx_sub: (a, b) => checked(a - b, 2),
    spx_mul: (a, b) => checked(a * b, 3), spx_neg: a => checked(-a, 8),
    spx_div: (a, b) => b === 0n ? arithmetic(4) : a === minimum && b === -1n ? arithmetic(5) : a / b,
    spx_rem: (a, b) => b === 0n ? arithmetic(6) : a === minimum && b === -1n ? arithmetic(7) : a % b,
    spx_contract_fail: code => { throw new Failure("semaprax.contract.v1", Number(code)); },
    spx_string_new: (pointer, length) => add(decoder.decode(new Uint8Array(instance.exports.memory.buffer, Number(pointer), Number(length)))),
    spx_string_eq: (left, right) => get(left) === get(right) ? 1 : 0,
    spx_string_clone: handle => add(get(handle)),
    spx_string_len: handle => BigInt(encoder.encode(get(handle)).length),
    spx_string_concat: (left, right) => add(get(left) + get(right)),
    spx_string_starts_with: (value, prefix) => get(value).startsWith(get(prefix)) ? 1 : 0,
    spx_string_contains: (value, needle) => get(value).includes(get(needle)) ? 1 : 0,
    spx_string_len_chars: value => BigInt([...get(value)].length),
    spx_string_from_char: scalar => add(String.fromCodePoint(Number(scalar))),
  } });
  try {
    const value = instance.exports.semaprax_main();
    console.log(`${id}|ok|${value}`);
  } catch (error) {
    if (!(error instanceof Failure)) throw error;
    console.log(`${id}|${error.domain}|${error.code}`);
  }
}
