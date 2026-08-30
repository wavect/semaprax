// Offline consumer of three already-published files. No package manager,
// downloads, network operations, or ambient imports. This is not an OS sandbox.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const directory = process.argv[2];
assert.equal(process.argv.length, 3);
const bytes = readFileSync(join(directory, 'module.wasm'));
const manifest = JSON.parse(readFileSync(join(directory, 'semaprax.package-build.json'), 'utf8'));
const module = new WebAssembly.Module(bytes);
const runtimeNames = ['spx_add', 'spx_sub', 'spx_mul', 'spx_div', 'spx_rem', 'spx_neg', 'spx_contract_fail'];
const expectedImports = runtimeNames.map(name => ({module: 'env', name, kind: 'function'}));
assert.deepEqual(WebAssembly.Module.imports(module), expectedImports);
assert.deepEqual(manifest.runtime_imports, expectedImports);
const ids = ['app.main.add', 'app.main.invert', 'app.main.run'];
const symbol = id => `spx_scalar_${Buffer.from(id, 'utf8').toString('hex')}`;
assert.deepEqual(manifest.exports.map(row => row.stable_id), ids);
assert.deepEqual(manifest.exports.map(row => row.wasm_export), ids.map(symbol));
assert.deepEqual(WebAssembly.Module.exports(module), ids.map(id => ({name: symbol(id), kind: 'function'})));
const minimum = -(1n << 63n), maximum = (1n << 63n) - 1n;
let additions = 0;
const checked = value => {
  if (value < minimum || value > maximum) throw new RangeError('checked-i64-overflow');
  return value;
};
const env = {
  spx_add: (a, b) => { additions++; return checked(a + b); },
  spx_sub: (a, b) => checked(a - b),
  spx_mul: (a, b) => checked(a * b),
  spx_div: (a, b) => checked(a / b),
  spx_rem: (a, b) => a % b,
  spx_neg: a => checked(-a),
  spx_contract_fail: () => { throw new Error('checked-contract-failure'); },
};
const { exports } = new WebAssembly.Instance(module, {env});
for (const id of ['lib.math.answer', 'lib.math.invert', 'lib.math.sum']) {
  assert.equal(Object.hasOwn(exports, symbol(id)), false, 'provider functions remain private');
}
// The interface Report's root placeholder returns zero; only linked source
// execution can produce 42, including the selected provider's actual 41.
assert.equal(exports[symbol('app.main.run')](), 42n);
assert.equal(additions, 1);
assert.equal(exports[symbol('app.main.invert')](0), 1);
assert.equal(exports[symbol('app.main.invert')](1), 0);
assert.throws(() => exports[symbol('app.main.invert')](2), WebAssembly.RuntimeError);
assert.throws(() => exports[symbol('app.main.invert')](-1), WebAssembly.RuntimeError);
assert.equal(exports[symbol('app.main.add')](maximum, 0n), maximum);
const beforeOverflow = additions;
assert.throws(() => exports[symbol('app.main.add')](maximum, 1n), {name: 'RangeError', message: 'checked-i64-overflow'});
assert.equal(additions, beforeOverflow + 1, 'dependency addition reaches the checked host import');
assert.equal(exports[symbol('app.main.add')](19n, 23n), 42n);
assert.equal(exports[symbol('app.main.run')](), 42n);
process.stdout.write('linked-package-consumer-ok\n');
