'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const evidence = require('../evidence.js');

const subject = side => ({ kind: side === 'current' ? 'image' : 'candidate', image_revision: 'image-1', project_revision: `${side}-project`, workspace_revision: `${side}-workspace`, project_graph_digest: `sha256:${'a'.repeat(64)}`, candidate_revision: side === 'current' ? null : 'candidate-1', side });
const functionDeclaration = { id: 'f', kind: 'function' };
const fieldDeclaration = { id: 'record.field', kind: 'field' };
function available(request) {
  const compact = { stable_id: request.target || 'candidate', revision: request.subject.project_revision, count: 1 };
  if (request.method.endsWith('function-summary')) compact.effects = ['declared:filesystem'];
  return { schema: evidence.SCHEMA, subject: request.subject, method: request.method, target: request.target, facet: request.facet, state: 'available', compact, omitted: ['source bodies'], nonclaims: ['tests not run'], source_authority: false, execution: false };
}

test('each selected tab reads only documented immutable evidence, including selected candidate facts', async () => {
  const calls = [];
  const host = { async readEvidence(request) { calls.push(request); return available(request); } };
  const inspector = evidence.createEvidenceInspector(host, subject('candidate'), functionDeclaration);
  const contracts = await inspector.inspect('contracts_effects');
  const ownership = await inspector.inspect('ownership_cleanup');
  const dependencies = await inspector.inspect('dependencies');
  const coverage = await inspector.inspect('evidence_limits');
  await inspector.inspect('contracts_effects');
  assert.deepEqual(calls.map(call => call.method), [
    'candidate/contract-delta', 'candidate/function-summary', 'candidate/function-facet',
    'candidate/ownership-delta', 'candidate/function-facet', 'candidate/function-facet', 'candidate/function-facet',
    'candidate/dependency-summary', 'candidate/analysis-coverage'
  ]);
  assert.deepEqual(calls.filter(call => call.method === 'candidate/function-facet').map(call => call.facet), ['contracts', 'ownership', 'loans', 'cleanup']);
  assert.equal(contracts.state, 'available'); assert.equal(ownership.state, 'available'); assert.equal(dependencies.state, 'available'); assert.equal(coverage.state, 'available');
  assert.ok(calls.every(call => !/run|apply|test|publish|build|commit|agent/.test(call.method)));
});

test('function and non-function applicability stays explicit', async () => {
  const calls = [];
  const host = { async readEvidence(request) { calls.push(request); return available(request); } };
  const image = evidence.createEvidenceInspector(host, subject('current'), functionDeclaration);
  assert.equal((await image.inspect('declaration')).method, 'image/function-summary');
  const contracts = await image.inspect('contracts_effects');
  assert.deepEqual(contracts.compact.declared_effects.effects, ['declared:filesystem']);
  assert.deepEqual(calls.slice(1, 3).map(call => [call.method, call.facet]), [['image/function-summary', null], ['image/facet', 'contracts']]);
  const ownership = await image.inspect('ownership_cleanup', { detail: 'cleanup' });
  assert.deepEqual(Object.keys(ownership.compact), ['ownership', 'loan_plan', 'cleanup_plan']);
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

test('candidate contract delta retains a changed helper under an unchanged predicate beside selected facts', async () => {
  const calls = [];
  const host = { async readEvidence(request) {
    calls.push(request);
    const row = available(request);
    if (request.method === 'candidate/contract-delta') row.compact = {
      inventory: { affected_functions: 1 }, changed: [{ id: 'helper', change: 'modified', predicate_change: 'unchanged' }]
    };
    return row;
  } };
  const result = await evidence.createEvidenceInspector(host, subject('candidate'), functionDeclaration).inspect('contracts_effects');
  assert.deepEqual(result.compact.contract_changes.changed, [{ id: 'helper', change: 'modified', predicate_change: 'unchanged' }]);
  assert.deepEqual(calls.map(call => [call.method, call.target, call.facet]), [
    ['candidate/contract-delta', null, null],
    ['candidate/function-summary', 'f', null],
    ['candidate/function-facet', 'f', 'contracts']
  ]);
});

test('offline evidence index binds a selected subject and exposes only its compact states', async () => {
  const selected = subject('candidate');
  const index = evidence.offlineIndex({ schema: evidence.INDEX_SCHEMA, entries: [{
    subject: selected, target: 'f',
    states: { function_summary: 'available', dependency_summary: 'available', analysis_coverage: 'available', contract_delta: 'available', ownership_delta: 'not applicable' },
    compact: {
      function_summary: { id: 'f', parameter_count: 1, return_type_id: 'Int', effects: [], requires_count: 0, ensures_count: 0, facets: [] },
      dependency_summary: { target: 'f', kind: 'function', facets: [{ view: 'callers', total_items: 1 }], test_reachable: false },
      analysis_coverage: { inventory: { functions: 1 }, areas: [{ area: 'workspace', status: 'not_inspected' }] },
      contract_delta: { inventory: { affected_functions: 1 }, changed: [{ id: 'helper', change: 'modified' }] }
    }, omitted: ['source bodies', 'raw facet items']
  }] }, [selected]);
  const host = { offline: true, async readEvidence(request) {
    const value = evidence.offlineEnvelope(index, request);
    if (!value || value.state !== 'available') { const error = new Error(value?.reason || 'not bundled'); error.code = value?.state || 'not_bundled'; throw error; }
    return value;
  } };
  const inspector = evidence.createEvidenceInspector(host, selected, functionDeclaration);
  assert.equal((await inspector.inspect('declaration')).state, 'available');
  assert.equal((await inspector.inspect('evidence_limits')).state, 'available');
  assert.equal((await inspector.inspect('contracts_effects')).state, 'not_bundled');
  assert.equal((await inspector.inspect('ownership_cleanup')).state, 'not_bundled');
  const deltaIndex = evidence.offlineIndex({ schema: evidence.INDEX_SCHEMA, entries: [{
    subject: selected, target: null,
    states: { function_summary: 'error', dependency_summary: 'error', analysis_coverage: 'error', contract_delta: 'available', ownership_delta: 'not applicable' },
    compact: { contract_delta: { inventory: { affected_functions: 1 }, changed: [{ id: 'helper', change: 'modified' }] } }, omitted: ['source bodies']
  }] }, [selected]);
  const offlineDelta = { offline: true, async readEvidence(request) {
    const value = evidence.offlineEnvelope(deltaIndex, request);
    if (!value || value.state !== 'available') { const error = new Error(value?.reason || 'not bundled'); error.code = value?.state || 'not_bundled'; throw error; }
    return value;
  } };
  const deltaInspector = evidence.createEvidenceInspector(offlineDelta, selected, functionDeclaration);
  assert.equal((await deltaInspector.inspect('contracts_effects')).state, 'available');
  assert.equal((await deltaInspector.inspect('ownership_cleanup')).state, 'not_applicable');
  const sourceBearing = { schema: evidence.INDEX_SCHEMA, entries: [{
    subject: selected, target: 'f',
    states: { function_summary: 'available', dependency_summary: 'error', analysis_coverage: 'error' },
    compact: { function_summary: { id: 'f', parameter_count: 0, return_type_id: 'Int', effects: [], requires_count: 0, ensures_count: 0, facets: [], predicate: 'literal source text' } }, omitted: []
  }] };
  assert.throws(() => evidence.offlineIndex(sourceBearing, [selected]), /function_summary/);
  const foreign = { ...selected, project_revision: 'other-project' };
  assert.throws(() => evidence.offlineIndex({ schema: evidence.INDEX_SCHEMA, entries: [{ subject: foreign, target: 'f', states: { function_summary: 'error', dependency_summary: 'error', analysis_coverage: 'error' }, compact: {}, omitted: [] }] }, [selected]), /offline binding/);
});
