import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import * as bindings from './package/semaprax.bindings.js';

const ids = Array.from({length:9}, (_, arity) => `mixed.arity${arity}`);
assert.deepEqual(bindings.exportIds, ids);
const wasm = Uint8Array.from(readFileSync(new URL('./package/app.wasm', import.meta.url)));
assert.equal(bindings.wasmSha256, createHash('sha256').update(wasm).digest('hex'));
const metadata = JSON.parse(readFileSync(new URL('./package/semaprax.api.json', import.meta.url), 'utf8'));
const descriptor = JSON.parse(metadata.descriptor);
assert.equal(descriptor.schema, 'semaprax.public-owned-data-api.v1');
assert.deepEqual(descriptor.exports.map(row => row.stable_id), ids);
assert.deepEqual(descriptor.exports.map(row => row.parameters.length), [0,1,2,3,4,5,6,7,8]);

function healthy() {
  return [-13n, true, 'é\0A', Uint8Array.of(0,255,128), 29n, false, 'Z\0λ!', Uint8Array.of(65,0,255,127,128,42)];
}
assert.deepEqual(new TextEncoder().encode(healthy()[2]), Uint8Array.of(195,169,0,65));
assert.deepEqual(new TextEncoder().encode(healthy()[6]), Uint8Array.of(90,0,206,187,33));
const ok = Uint8Array.of(111,107), bad = Uint8Array.of(98,97,100);
function observe(api, arity, args, good, direct) {
  const prefix = args.slice(0, arity);
  const result = direct ? api.functions[ids[arity]](...prefix) : api.call(ids[arity], ...prefix);
  assert.equal(Object.getPrototypeOf(result), Uint8Array.prototype);
  assert.deepEqual(result, good ? ok : bad);
  for (const value of prefix) if (value instanceof Uint8Array) assert.notEqual(result.buffer, value.buffer);
  return result;
}
const first = await bindings.instantiate(wasm), second = await bindings.instantiate(wasm);
for (const api of [first, second]) {
  assert.deepEqual(Object.keys(api.functions), ids);
  for (let round = 0; round < 2; ++round) {
    for (let arity = 0; arity <= 8; ++arity) {
      observe(api, arity, healthy(), true, true);
      for (let position = 0; position < arity; ++position) {
        const args = healthy();
        args[position] = [29n, false, '', new Uint8Array(), -13n, true, '', new Uint8Array()][position];
        observe(api, arity, args, false, false);
        observe(api, arity, healthy(), true, true);
      }
    }
    for (const [left, right] of [[0,4],[1,5],[2,6],[3,7]]) {
      const args = healthy();
      [args[left], args[right]] = [args[right], args[left]];
      observe(api, 8, args, false, false);
      observe(api, 8, healthy(), true, true);
    }
  }
}
const input = healthy();
const kept = observe(first, 8, input, true, true);
const other = observe(second, 8, input, true, false);
assert.notEqual(kept.buffer, other.buffer);
input[3].fill(17); input[7].fill(23);
for (let arity = 0; arity <= 8; ++arity) {
  observe(first, arity, healthy(), true, true);
  observe(second, arity, healthy(), true, false);
}
assert.deepEqual(kept, ok); assert.deepEqual(other, ok);
kept[0] = 0;
assert.deepEqual(other, ok);
assert.deepEqual(observe(first, 8, healthy(), true, true), ok);
console.log('mixed-arity-npm-ok');
