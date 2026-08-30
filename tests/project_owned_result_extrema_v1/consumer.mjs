import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';

const inputBytes = Uint8Array.of(0, 255, 128, 65, 0);
// Decimal strings never cross a lossy JSON Number representation.
const errors = ['0', '-9223372036854775808', '9223372036854775807'].map(BigInt);
const bits = [0n, 0x8000000000000000n, 0x7fffffffffffffffn];
const wasm = new Uint8Array(readFileSync(new URL('./package/app.wasm', import.meta.url)));
const metadata = JSON.parse(readFileSync(new URL('./package/semaprax.api.json', import.meta.url), 'utf8'));
const digest = createHash('sha256').update(wasm).digest('hex');
assert.deepEqual(metadata.wasm, {path:'app.wasm', sha256:digest});
assert.deepEqual(JSON.parse(metadata.descriptor).exports.map(row => row.stable_id), ['result.value']);
const instantiate = WebAssembly.instantiate;
const mapDelete = Map.prototype.delete, mapGet = Map.prototype.get;
let active = false, length = -1, carrier = null, memory;
let calls = 0, mints = 0, drops = 0, deletions = 0;
Map.prototype.delete = function(key) {
  const bytes = mapGet.call(this, key);
  const ours = active && carrier !== null && key === Number((BigInt.asUintN(64, carrier) >> 32n) & 0x7fffffffn) && bytes instanceof Uint8Array;
  if (ours) assert.deepEqual(bytes, inputBytes.slice(0, length));
  const removed = mapDelete.call(this, key);
  if (ours && removed) deletions++;
  return removed;
};
WebAssembly.instantiate = async (bytes, supplied) => {
  const env = {...supplied.env};
  for (const name of ['spx_bytes_copy', 'spx_bytes_drop']) {
    const operation = env[name];
    assert.equal(typeof operation, 'function');
    env[name] = (...args) => {
      assert.equal(active, true);
      const result = operation(...args);
      if (name === 'spx_bytes_copy') { carrier = result; mints++; }
      else { assert.equal(args[0], carrier); drops++; }
      return result;
    };
  }
  const result = await instantiate.call(WebAssembly, bytes, {env});
  const exports = {...result.instance.exports};
  memory = exports.memory;
  const selected = Object.keys(exports).filter(name => name.startsWith('spx_owned_v1_'));
  assert.deepEqual(selected, ['spx_owned_v1_726573756c742e76616c7565']);
  const operation = exports[selected[0]];
  exports[selected[0]] = (...args) => {
    assert.equal(active, true); calls++;
    const out = args.at(-1);
    assert.equal(out, 65536);
    const before = new Uint8Array(memory.buffer, out, 16).slice();
    assert(before.every(byte => byte === 0xa5));
    const status = operation(...args);
    assert.equal(status, length === 4 ? 4 : 0);
    const view = new DataView(memory.buffer);
    if (length === 4) assert.deepEqual(new Uint8Array(memory.buffer, out, 16), before);
    else if (length < 3) {
      assert.equal(view.getUint32(out, true), 1);
      assert.equal(view.getBigUint64(out + 8, true), bits[length], 'raw signed payload bits before JS conversion');
      assert.equal(view.getBigInt64(out + 8, true), errors[length]);
    } else {
      assert.equal(view.getUint32(out, true), 0);
      assert.equal(view.getBigInt64(out + 8, true), carrier);
    }
    return status;
  };
  return {...result, instance:{exports}};
};
try {
  const bindings = await import('./package/semaprax.bindings.js');
  assert.equal(bindings.wasmSha256, digest);
  const api = await bindings.instantiate(wasm);
  assert.deepEqual(Object.keys(api.functions), ['result.value']);
  const retained = [];
  for (let round = 0; round < 8; round++) {
    for (length of [0, 1, 2, 3, 4, 5, 2, 1, 0, 3]) {
      const before = {calls, mints, drops, deletions};
      const input = inputBytes.slice(0, length);
      carrier = null; active = true;
      try {
        if (length === 4) {
          assert.throws(() => api.call('result.value', input), error => {
            assert.equal(error.constructor, Error);
            assert.equal(error.status, 4);
            assert.equal(error.message, 'SEMAPRAX semantic failure 4');
            return true;
          });
        } else {
          const result = api.call('result.value', input);
          assert(Object.isFrozen(result));
          if (length < 3) assert.deepEqual(result, {ok:false, error:errors[length]});
          else {
            assert.deepEqual(result, {ok:true, value:inputBytes.slice(0, length)});
            assert.notEqual(result.value.buffer, input.buffer);
            input.fill(17);
            retained.push([result.value, inputBytes.slice(0, length)]);
          }
        }
      } finally { active = false; }
      assert.equal(calls - before.calls, 1);
      assert.equal(mints - before.mints, Number(length >= 3));
      assert.equal(drops - before.drops, Number(length === 4));
      assert.equal(deletions - before.deletions, Number(length >= 3));
      assert(new Uint8Array(memory.buffer, 65536, 16).every(byte => byte === 0xa5));
      assert(new Uint8Array(memory.buffer, 0, length).every(byte => byte === 0));
    }
  }
  assert.equal(calls, 80); assert.equal(mints, 32); assert.equal(drops, 8); assert.equal(deletions, 32);
  assert.equal(retained.length, 24);
  for (const [value, expected] of retained) assert.deepEqual(value, expected);
  assert.equal(new Set(retained.map(([value]) => value.buffer)).size, 24);
} finally {
  WebAssembly.instantiate = instantiate;
  Map.prototype.delete = mapDelete;
}
console.log('result-extrema-ok');
