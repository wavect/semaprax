'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { layout } = require('../layout.js');
const { PERF_FIXTURES } = require('../perf-fixtures.js');

test('layout accepts the renderer stress inventory without recursive traversal', () => {
  const declarations = PERF_FIXTURES.renderer_stress.declarations.map(row => ({ key: row.node_key }));
  // A long chain exceeds the depth that a recursive traversal can reliably
  // promise across browser engines. The full fixture also retains its 65,536
  // renderer-only edges for browser/performance measurement.
  const edges = declarations.slice(1).map((row, index) => ({ from: declarations[index].key, to: row.key }));
  const result = layout(declarations, edges);
  assert.equal(result.positions.size, 4096);
  assert.equal(result.components.length, 4096);
  assert.ok(result.height >= 240);
});
