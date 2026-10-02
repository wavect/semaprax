'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const evidence = require('../evidence.js');

const subject = side => ({ kind: side === 'current' ? 'image' : 'candidate', image_revision: 'image-1', project_revision: `${side}-project`, workspace_revision: `${side}-workspace`, project_graph_digest: `sha256:${'a'.repeat(64)}`, candidate_revision: side === 'current' ? null : 'candidate-1', side });
const functionDeclaration = { id: 'f', kind: 'function' };
const fieldDeclaration = { id: 'record.field', kind: 'field' };
function available(request) { return { schema: evidence.SCHEMA, subject: request.subject, method: request.method, target: request.target, facet: request.facet, state: 'available', compact: { stable_id: request.target || 'candidate', revision: request.subject.project_revision, count: 1 }, omitted: ['source bodies'], nonclaims: ['tests not run'], source_authority: false, execution: false }; }

test('each selected tab uses only its documented immutable method and caches one read', async () => {
  const calls = [];
  const host = { async readEvidence(request) { calls.push(request); return available(request); } };
  const inspector = evidence.createEvidenceInspector(host, subject('candidate'), functionDeclaration);
  const contracts = await inspector.inspect('contracts_effects');
  const ownership = await inspector.inspect('ownership_cleanup');
  const dependencies = await inspector.inspect('dependencies');
  const coverage = await inspector.inspect('evidence_limits');
  await inspector.inspect('contracts_effects');
  assert.deepEqual(calls.map(call => call.method), ['candidate/contract-delta', 'candidate/ownership-delta', 'candidate/dependency-summary', 'candidate/analysis-coverage']);
  assert.equal(contracts.state, 'available'); assert.equal(ownership.state, 'available'); assert.equal(dependencies.state, 'available'); assert.equal(coverage.state, 'available');
  assert.ok(calls.every(call => !/run|apply|test|publish|build/.test(call.method)));
});

test('function and non-function applicability stays explicit', async () => {
  const calls = [];
  const host = { async readEvidence(request) { calls.push(request); return available(request); } };
  const image = evidence.createEvidenceInspector(host, subject('current'), functionDeclaration);
  assert.equal((await image.inspect('declaration')).method, 'image/function-summary');
  assert.equal((await image.inspect('contracts_effects')).facet, 'contracts');
  assert.equal((await image.inspect('ownership_cleanup', { detail: 'cleanup' })).facet, 'cleanup');
  const field = evidence.createEvidenceInspector(host, subject('current'), fieldDeclaration);
  assert.equal((await field.inspect('declaration')).state, 'not_applicable');
  assert.equal((await field.inspect('contracts_effects')).state, 'not_applicable');
  assert.equal((await field.inspect('ownership_cleanup')).state, 'not_applicable');
  assert.equal((await field.inspect('dependencies')).method, 'image/dependency-summary');
});

test('offline, stale, unsupported, and error evidence states remain distinct', async () => {
  for (const code of ['not_bundled', 'not_requested', 'unsupported', 'stale', 'error']) {
    const host = { async readEvidence() { const error = new Error(code); error.code = code; throw error; } };
    const result = await evidence.createEvidenceInspector(host, subject('current'), functionDeclaration).inspect('declaration');
    assert.equal(result.state, code);
  }
});

test('candidate identity is part of the cache binding and source-bearing compact fields are refused', async () => {
  const calls = [];
  const host = { async readEvidence(request) { calls.push(request); return available(request); } };
  await evidence.createEvidenceInspector(host, subject('candidate'), functionDeclaration).inspect('declaration');
  const other = subject('candidate'); other.candidate_revision = 'candidate-2';
  await evidence.createEvidenceInspector(host, other, functionDeclaration).inspect('declaration');
  assert.equal(calls.length, 2); assert.notEqual(calls[0].subject.candidate_revision, calls[1].subject.candidate_revision);
  const unsafe = { async readEvidence(request) { const row = available(request); row.compact.body = 'hidden source'; return row; } };
  const result = await evidence.createEvidenceInspector(unsafe, subject('current'), functionDeclaration).inspect('declaration');
  assert.equal(result.state, 'error');
});
