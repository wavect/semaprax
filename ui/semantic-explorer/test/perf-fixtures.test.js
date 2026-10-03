'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { PERF_FIXTURES } = require('../perf-fixtures.js');

test('renderer fixtures have contracted deterministic cardinalities', () => {
  assert.equal(PERF_FIXTURES.small.declarations.length, 30); assert.equal(PERF_FIXTURES.small.relations.length, 60);
  assert.equal(PERF_FIXTURES.medium.declarations.length, 500); assert.equal(PERF_FIXTURES.medium.relations.length, 2000);
  assert.equal(PERF_FIXTURES.renderer_stress.declarations.length, 4096); assert.equal(PERF_FIXTURES.renderer_stress.relations.length, 65536);
  assert.equal(PERF_FIXTURES.renderer_stress.renderer_only, true);
  assert.deepEqual(PERF_FIXTURES.pathological.duplicateSite, [[0, 1], [0, 1]]);
  assert.equal(PERF_FIXTURES.pathological.star.length, 255);
  assert.equal(PERF_FIXTURES.pathological.chain.length, 255);
  assert.deepEqual(PERF_FIXTURES.pathological.disconnected, [[254, 255]]);
});
