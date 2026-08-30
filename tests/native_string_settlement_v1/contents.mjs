// Value-conformance host only. Ordinary Wasm has no String drop import, so
// retaining these bounded handles is explicitly not physical cleanup proof.
import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";

const group = process.argv[3];
assert.ok(["base", "v1", "v2", "owned", "generic"].includes(group));
const module = new WebAssembly.Module(await readFile(process.argv[2]));
const scalar = ["spx_add", "spx_sub", "spx_mul", "spx_div", "spx_rem", "spx_neg", "spx_contract_fail"];
const base = ["spx_string_new", "spx_string_eq", "spx_string_clone"];
const v1 = ["spx_string_len", "spx_string_concat"];
const v2 = ["spx_string_starts_with", "spx_string_contains", "spx_string_len_chars", "spx_string_from_char"];
const expected = [...scalar, ...base, ...(["v1", "v2"].includes(group) ? v1 : []), ...(group === "v2" ? v2 : [])];
assert.deepEqual(WebAssembly.Module.imports(module), expected.map(name => ({ module: "env", name, kind: "function" })));
const strings = new Map();
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const get = handle => {
  assert.equal(typeof handle, "bigint");
  assert.ok(strings.has(handle), `unknown handle ${handle}`);
  return strings.get(handle);
};
const add = value => {
  assert.equal(typeof value, "string");
  assert.ok(strings.size < 4096, "bounded value observer exhausted");
  const handle = BigInt(strings.size + 1);
  strings.set(handle, value);
  return handle;
};
const unexpected = name => () => { throw new Error(`unexpected failure import ${name}`); };
let instance;
const host = {
  ...Object.fromEntries(scalar.map(name => [name, unexpected(name)])),
  spx_string_new(pointer, length) {
    assert.ok(Number.isInteger(pointer) && pointer >= 0 && Number.isInteger(length) && length >= 0);
    return add(decoder.decode(new Uint8Array(instance.exports.memory.buffer, pointer, length)));
  },
  spx_string_eq: (left, right) => get(left) === get(right) ? 1 : 0,
  spx_string_clone: handle => add(get(handle)),
  spx_string_len: handle => BigInt(encoder.encode(get(handle)).length),
  spx_string_concat: (left, right) => add(get(left) + get(right)),
  spx_string_starts_with: (value, prefix) => get(value).startsWith(get(prefix)) ? 1 : 0,
  spx_string_contains: (value, needle) => get(value).includes(get(needle)) ? 1 : 0,
  spx_string_len_chars: value => BigInt([...get(value)].length),
  spx_string_from_char: scalar => add(String.fromCodePoint(Number(scalar))),
};
instance = new WebAssembly.Instance(module, { env: host });
for (let repetition = 0; repetition < 4; ++repetition) {
  assert.equal(instance.exports.semaprax_main(), 42n);
}
assert.ok(strings.size > 0);
console.log("string-contents-wasm-ok");
