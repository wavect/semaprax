import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';

const encode = text => new TextEncoder().encode(text);
const empty = new Uint8Array();
const unit = '\uFEFF\0€😀';
const unitBytes = Uint8Array.of(239,187,191,0,226,130,172,240,159,152,128);
assert.deepEqual(encode(unit), unitBytes);
assert.equal(encode(unit).length, 11);
const raw = Uint8Array.from({length:65537}, (_, index) => index % 251);
const unicode = unit.repeat(2000);
const euroExact = '€'.repeat(21845) + 'a';
const astralExact = '😀'.repeat(16384);
assert.equal(encode(unicode).length, 22000);
assert.equal(encode(euroExact).length, 65536);
assert.equal(encode(astralExact).length, 65536);

// Only wrap actual selected functions in an actual compiled module instance.
// Positive +1 assertions below calibrate every observed call; this is not an
// allocation observer, native-context observer, or substitute Wasm engine.
const originalInstantiate = WebAssembly.instantiate;
let entries = 0;
const wrapperCounts = [];
WebAssembly.instantiate = async (bytes, imports) => {
  const result = await originalInstantiate(bytes, imports);
  const exports = {...result.instance.exports};
  let count = 0;
  for (const [name, fn] of Object.entries(exports)) {
    if (!name.startsWith('spx_owned_v1_')) continue;
    assert.equal(typeof fn, 'function');
    count++;
    exports[name] = (...args) => { entries++; return fn(...args); };
  }
  wrapperCounts.push(count);
  return {...result, instance:{exports}};
};

function invoke(api, id, args, throughFunctions = false) {
  const before = entries;
  const result = throughFunctions ? api.functions[id](...args) : api.call(id, ...args);
  assert.equal(entries, before + 1, 'one real selected entry per accepted call');
  return result;
}

function output(api, flat, id, text, left, right, throughFunctions = false) {
  const result = invoke(api, id, [text,left,right], throughFunctions);
  if (!flat) return result;
  assert(Object.isFrozen(result));
  assert.equal(Object.getPrototypeOf(result), null);
  assert.deepEqual(Object.keys(result).sort(), [
    'spx_field_id_6279746573', 'spx_field_id_74657874',
    'spx_field_id_6c656674', 'spx_field_id_7269676874',
  ].sort());
  assert.equal(result.spx_field_id_74657874, BigInt(encode(text).length));
  assert.equal(result.spx_field_id_6c656674, BigInt(left.length));
  assert.equal(result.spx_field_id_7269676874, BigInt(right.length));
  return result.spx_field_id_6279746573;
}

function owned(actual, expected) {
  assert.equal(Object.getPrototypeOf(actual), Uint8Array.prototype);
  assert.deepEqual(actual, expected);
  assert.notEqual(actual.buffer, expected.buffer);
}

function accepted(api, flat, text, left, right) {
  assert(encode(text).length + left.length + right.length <= 65536);
  owned(output(api, flat, 'tuple.bytes', text,left,right), left);
  owned(output(api, flat, 'tuple.text', text,left,right, true), encode(text));
  if (!flat) {
    owned(invoke(api, 'tuple.maybe', [text,left,right,true]), left);
    assert.equal(invoke(api, 'tuple.maybe', [text,left,right,false]), null);
    const ok = invoke(api, 'tuple.result', [text,left,right,true]);
    assert(Object.isFrozen(ok));
    assert.deepEqual(Object.keys(ok).sort(), ['ok','value']);
    assert.equal(ok.ok, true);
    owned(ok.value, left);
    const error = invoke(api, 'tuple.result', [text,left,right,false]);
    assert(Object.isFrozen(error));
    assert.deepEqual(error, {ok:false,error:-7n});
  }
}

function recovery(api, flat) {
  accepted(api, flat, unit, Uint8Array.of(0,255,195,40), Uint8Array.of(128));
}

function rejected(api, flat, text, left, right) {
  assert(encode(text).length + left.length + right.length > 65536);
  const cases = [['tuple.bytes',[]],['tuple.text',[]]];
  if (!flat) for (const active of [false,true]) {
    cases.push(['tuple.maybe',[active]], ['tuple.result',[active]]);
  }
  for (const [id, tail] of cases) {
    const before = entries;
    assert.throws(() => api.call(id,text,left,right,...tail), error =>
      error instanceof RangeError && error.message === 'SEMAPRAX borrowed input capacity exceeded');
    assert.equal(entries, before, 'even unused and inactive inputs are charged before entry');
    recovery(api, flat);
  }
}

function exercise(api, flat) {
  // Same independently spelled corpus as the published native SDK consumer.
  for (let round = 0; round < 8; round++) {
    accepted(api, flat, '', empty, empty);
    recovery(api, flat);
    for (const length of [65535,65536]) {
      accepted(api, flat, '', raw.subarray(0,length), empty);
      accepted(api, flat, '', empty, raw.subarray(0,length));
      accepted(api, flat, '', raw.subarray(0,32768), raw.subarray(0,length-32768));
      accepted(api, flat, unicode, raw.subarray(0,20000), raw.subarray(0,length-42000));
    }
    accepted(api, flat, euroExact, empty, empty);
    accepted(api, flat, astralExact, empty, empty);
    rejected(api, flat, '', raw, empty);
    rejected(api, flat, '', empty, raw);
    rejected(api, flat, '', raw.subarray(0,32768), raw.subarray(0,32769));
    rejected(api, flat, unicode, raw.subarray(0,20000), raw.subarray(0,23537));
    rejected(api, flat, 'a', raw.subarray(0,32768), raw.subarray(0,32768));
    rejected(api, flat, euroExact, Uint8Array.of(0), empty);
    rejected(api, flat, astralExact, empty, Uint8Array.of(255));
    rejected(api, flat, euroExact+'a', empty, empty);
  }
}

try {
  for (const flat of [false,true]) {
    const directory = new URL(`./${flat?'v9':'v8'}/package/`, import.meta.url);
    const bindings = await import(new URL('semaprax.bindings.js', directory));
    const ids = flat ? ['tuple.bytes','tuple.text'] : ['tuple.bytes','tuple.maybe','tuple.result','tuple.text'];
    assert.deepEqual(bindings.exportIds, ids);
    assert(Object.isFrozen(bindings.exportIds));
    const metadata = JSON.parse(readFileSync(new URL('semaprax.api.json', directory), 'utf8'));
    const descriptor = JSON.parse(metadata.descriptor);
    assert.equal(descriptor.schema, flat?'semaprax.public-flat-owned-record-api.v1':'semaprax.public-owned-data-api.v1');
    assert.deepEqual(descriptor.exports.map(row => row.stable_id), ids);
    assert.equal(descriptor.limits.max_borrowed_input_bytes, 65536);
    assert.equal(descriptor.limits.max_owned_output_bytes, 65536);
    const wasm = Uint8Array.from(readFileSync(new URL('app.wasm', directory)));
    const wasmDigest = createHash('sha256').update(wasm).digest('hex');
    assert.equal(metadata.schema, flat?'semaprax.flat-owned-record-api.v1':'semaprax.owned-data-api.v1');
    if (flat) assert.equal(metadata.wasm_sha256, 'sha256:'+wasmDigest);
    else assert.deepEqual(metadata.wasm, {path:'app.wasm',sha256:wasmDigest});
    assert.equal(bindings.wasmSha256, wasmDigest);
    const before = wrapperCounts.length;
    const first = await bindings.instantiate(wasm), second = await bindings.instantiate(wasm);
    assert.deepEqual(wrapperCounts.slice(before), [ids.length,ids.length]);
    for (const api of [first,second]) {
      assert(Object.isFrozen(api));
      assert(Object.isFrozen(api.functions));
      assert.equal(Object.getPrototypeOf(api.functions), null);
      assert.deepEqual(Object.keys(api.functions), ids);
      exercise(api, flat);
    }
    const input = Uint8Array.of(0,255,195,40,128);
    const kept = output(first, flat, 'tuple.bytes', '',input,empty);
    const other = output(second, flat, 'tuple.bytes', '',input,empty, true);
    const keptText = output(second, flat, 'tuple.text', unit,empty,empty);
    assert.notEqual(kept.buffer, input.buffer);
    assert.notEqual(other.buffer, input.buffer);
    assert.notEqual(kept.buffer, other.buffer);
    input.fill(7);
    recovery(first, flat); recovery(second, flat);
    assert.deepEqual(kept, Uint8Array.of(0,255,195,40,128));
    assert.deepEqual(other, Uint8Array.of(0,255,195,40,128));
    assert.deepEqual(keptText, unitBytes);
    kept[0] = 9;
    assert.equal(other[0], 0);
    assert.deepEqual(input, Uint8Array.of(7,7,7,7,7));
  }
} finally {
  WebAssembly.instantiate = originalInstantiate;
}
console.log('project-owned-tuple-npm-ok');
