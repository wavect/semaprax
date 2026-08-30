import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';

const cases = JSON.parse(readFileSync(new URL('./cases.json', import.meta.url), 'utf8'));
assert.equal(cases.length, 12);
assert.equal(cases.filter(row => row[2] !== 0).length, 3);
const expected = Uint8Array.of(7, 0, 255);
const wasm = new Uint8Array(readFileSync(new URL('./package/app.wasm', import.meta.url)));
const metadata = JSON.parse(readFileSync(new URL('./package/semaprax.api.json', import.meta.url), 'utf8'));
const digest = createHash('sha256').update(wasm).digest('hex');
assert.deepEqual(metadata.wasm, {path:'app.wasm', sha256:digest});
const ids = ['mul.forward', 'mul.precedence', 'mul.reverse'];
assert.deepEqual(JSON.parse(metadata.descriptor).exports.map(row => row.stable_id), ids);

// Pass through the real generated runtime, real imports, and real Wasm body.
// Count exact arena entry deletions, not JS heap reclamation. Observers are
// installed only for this isolated fixture and restored even on assertion error.
const instantiate = WebAssembly.instantiate;
const mapDelete = Map.prototype.delete, mapGet = Map.prototype.get;
let active = false, carrier = null, mints = 0, drops = 0, deletions = 0, calls = 0;
let expectedStatus = null, memory;
Map.prototype.delete = function(key) {
  if (active && carrier !== null && key === Number((BigInt.asUintN(64, carrier) >> 32n) & 0x7fffffffn)) {
    const value = mapGet.call(this, key);
    if (value instanceof Uint8Array) {
      assert.deepEqual(value, expected);
      deletions++;
    }
  }
  return mapDelete.call(this, key);
};
WebAssembly.instantiate = async (bytes, supplied) => {
  const env = {...supplied.env};
  for (const name of ['spx_bytes_copy', 'spx_bytes_drop']) {
    const operation = env[name];
    assert.equal(typeof operation, 'function');
    env[name] = (...args) => {
      const result = operation(...args);
      if (name === 'spx_bytes_copy') {
        assert.equal(active, true); mints++; carrier = result;
      } else {
        assert.equal(args[0], carrier); drops++;
      }
      return result;
    };
  }
  const result = await instantiate.call(WebAssembly, bytes, {env});
  const exports = {...result.instance.exports};
  memory = exports.memory;
  let selected = 0;
  for (const [name, operation] of Object.entries(exports)) {
    if (!name.startsWith('spx_owned_v1_')) continue;
    selected++;
    exports[name] = (...args) => {
      calls++;
      const out = args.at(-1);
      assert.equal(out, 65536);
      const before = new Uint8Array(memory.buffer, out, 8).slice();
      assert(before.every(byte => byte === 0xa5));
      const status = operation(...args);
      assert.equal(status, expectedStatus, `${name}: genuine raw arithmetic status`);
      if (status !== 0) assert.deepEqual(new Uint8Array(memory.buffer, out, 8), before);
      return status;
    };
  }
  assert.equal(selected, 3);
  return {...result, instance:{exports}};
};
try {
  const bindings = await import('./package/semaprax.bindings.js');
  assert.equal(bindings.wasmSha256, digest);
  const api = await bindings.instantiate(wasm);
  assert.deepEqual(Object.keys(api.functions), ids);
  const retained = [];
  for (let round = 0; round < 4; round++) {
    for (const [name, length, status] of cases) {
      const before = {mints, drops, deletions, calls};
      carrier = null; expectedStatus = status; active = true;
      try {
        const input = Uint8Array.of(19, 23, 29).slice(0, length);
        if (status === 0) {
          const output = api.call(`mul.${name}`, input);
          assert.equal(Object.getPrototypeOf(output), Uint8Array.prototype);
          assert.deepEqual(output, expected);
          assert.notEqual(output.buffer, input.buffer);
          retained.push(output);
        } else {
          assert.throws(() => api.call(`mul.${name}`, input), error => {
            assert.equal(error.constructor, Error);
            assert.equal(error.status, status);
            assert.equal(error.message, `SEMAPRAX semantic failure ${status}`);
            return true;
          });
        }
      } finally { active = false; }
      assert.equal(calls - before.calls, 1);
      assert.equal(mints - before.mints, 1, 'owner really staged before arithmetic');
      assert.equal(drops - before.drops, Number(status !== 0));
      assert.equal(deletions - before.deletions, 1, 'failure drop or successful consume exactly once');
      assert(new Uint8Array(memory.buffer, 65536, 8).every(byte => byte === 0xa5));
      assert(new Uint8Array(memory.buffer, 0, length).every(byte => byte === 0));
    }
  }
  assert.equal(calls, 48); assert.equal(mints, 48); assert.equal(drops, 12); assert.equal(deletions, 48);
  assert.equal(retained.length, 36);
  for (const output of retained) assert.deepEqual(output, expected);
  assert.equal(new Set(retained.map(output => output.buffer)).size, retained.length);
} finally {
  WebAssembly.instantiate = instantiate;
  Map.prototype.delete = mapDelete;
}
console.log('usize-owned-mul-ok');
