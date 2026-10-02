'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const model = require('../model.js');
const { message, pageRequest } = require('../../../editors/vscode/explorer.js');
const { safePath } = require('../../../editors/vscode/review.js');

const digest = `sha256:${'a'.repeat(64)}`;
function summary() {
  return {
    schema: model.SCHEMA, kind: 'summary',
    subject: { kind: 'candidate', image_revision: digest, project_revision: digest, workspace_revision: digest, project_graph_digest: digest, candidate_revision: digest, side: 'candidate' },
    mode: 'overview', target: null,
    query: { direction: 'both', depth: 1, max_nodes: 256, max_bytes: 65536 }, artifact_digest: digest,
    truncation: { truncated: false }, coverage: { owner: 'workspace_graph', complete_within_retained_graph: true },
    inventories: model.VIEWS.map(view => ({ view, total_items: view === 'declarations' ? 1 : 0, handle: digest })),
    source_authority: false, execution: false, publication_authority: false, nonclaims: ['display data only']
  };
}
function page(selected, changes = {}) {
  return {
    schema: model.SCHEMA, kind: 'page', subject: selected.subject, mode: selected.mode, target: selected.target,
    query: selected.query, artifact_digest: selected.artifact_digest, truncation: selected.truncation,
    coverage: selected.coverage, view: 'declarations', handle: digest, cursor: null, offset: 0,
    total_items: 1, page_size: 32, max_bytes: 65536, next_cursor: null,
    items: [{ node_key: 'candidate:fn', id: 'fn', identity_origin: 'explicit', kind: 'function', display_name: 'fn', owner_id: null, module: 'm', path: 'm/main.spx', source_reference: { path: 'm/main.spx', source_revision: digest, source_digest: digest } }],
    source_authority: false, execution: false, publication_authority: false, nonclaims: selected.nonclaims,
    ...changes
  };
}

test('snapshot validator rejects prototype keys, deep payloads, unsafe counts, and false completeness', () => {
  // Create an actual own prototype key as hostile parsed JSON can contain.
  const withOwnPrototypeKey = JSON.parse(JSON.stringify(summary()).replace('{"schema"', '{"__proto__":{"polluted":true},"schema"'));
  assert.throws(() => model.summary(withOwnPrototypeKey), /unexpected fields/);
  assert.equal(Object.prototype.polluted, undefined);
  const nested = summary(); nested.truncation = { truncated: false }; let cursor = nested.truncation;
  for (let i = 0; i < 36; i++) cursor = cursor.next = {};
  assert.throws(() => model.summary(nested), /nested data/);
  const unsafe = summary(); unsafe.inventories[0].total_items = Number.MAX_SAFE_INTEGER + 1;
  assert.throws(() => model.summary(unsafe), /invalid count/);
  const falseComplete = summary(); falseComplete.coverage.complete_within_retained_graph = false;
  assert.throws(() => model.summary(falseComplete), /graph coverage/);
});

test('candidate pages bind to the selected subject and artifact digest', () => {
  const selected = model.summary(summary());
  assert.doesNotThrow(() => model.page(page(selected), selected));
  const foreign = page(selected); foreign.subject = { ...foreign.subject, candidate_revision: `sha256:${'b'.repeat(64)}` };
  assert.throws(() => model.page(foreign, selected), /foreign page/);
  const forgedDigest = page(selected, { artifact_digest: `sha256:${'c'.repeat(64)}` });
  assert.throws(() => model.page(forgedDigest, selected), /foreign page/);
});

test('editor boundary rejects stale generations, arbitrary RPC and forged cursors', () => {
  const request = { type: 'semaprax-explorer-request', generation: 8, requestId: 1, action: 'summary' };
  assert.equal(message(request, 8).action, 'summary');
  assert.equal(message({ ...request, generation: 7 }, 8), null);
  assert.equal(message({ ...request, action: 'tools/call' }, 8), null);
  const selected = { inventories: [{ view: 'modules', handle: digest }] };
  const cursors = new Map([['modules', 'page-2']]);
  const valid = { view: 'modules', handle: digest, cursor: 'page-2', page_size: 32, max_bytes: 65536 };
  assert.deepEqual(pageRequest(valid, selected, cursors), valid);
  assert.equal(pageRequest({ ...valid, cursor: 'page-1' }, selected, cursors), null);
});

test('source review paths reject absolute, traversal, UNC and drive-relative names', () => {
  for (const path of ['/etc/passwd', '../secret.spx', 'm/../secret.spx', '\\\\server\\share\\secret.spx', 'C:secret.spx']) {
    assert.equal(safePath(path), false, path);
  }
  assert.equal(safePath('src/main.spx'), true);
});
