'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const model = require('../model.js');
const { layout } = require('../layout.js');

const hex = 'a'.repeat(64);
function summary() {
  return {
    schema: model.SCHEMA,
    kind: 'summary',
    subject: { kind: 'image', image_revision: 'image-1', project_revision: 'project-1', workspace_revision: 'workspace-1', project_graph_digest: `sha256:${hex}`, candidate_revision: null, side: 'current' },
    mode: 'overview', target: null, query: { direction: 'both', depth: 1, max_nodes: 256, max_bytes: 262144 }, artifact_digest: `sha256:${hex}`,
    truncation: { truncated: false, reason: null }, coverage: { owner: 'workspace_graph', complete_within_retained_graph: true },
    inventories: model.VIEWS.map((view, index) => ({ view, total_items: index === 2 ? 2 : 1, handle: `sha256:${hex}` })),
    source_authority: false, execution: false, publication_authority: false, nonclaims: ['external callers not analyzed']
  };
}
function page(selected, view, items) {
  return {
    schema: selected.schema, kind: 'page', subject: selected.subject, mode: selected.mode, target: selected.target, query: selected.query,
    artifact_digest: selected.artifact_digest, truncation: selected.truncation, coverage: selected.coverage,
    view, handle: `sha256:${hex}`, cursor: null, offset: 0,
    total_items: selected.inventories.find(row => row.view === view).total_items,
    page_size: 32, max_bytes: 65536, next_cursor: null, items
    , source_authority: false, execution: false, publication_authority: false, nonclaims: selected.nonclaims
  };
}
const decl = { node_key: 'image-1:auth', id: 'auth', identity_origin: 'explicit', kind: 'function', display_name: '<script>alert(1)</script>', owner_id: null, module: 'auth', path: 'auth/main.spx', source_reference: { path: 'auth/main.spx', source_revision: 'r1', source_digest: 'd1' } };
const rel = site_id => ({ family: 'call', from: 'image-1:auth', to: 'image-1:pay', direction: 'forward', site_id, provenance: { path: 'auth/main.spx', site_id } });

test('closed summary and bound page preserve compiler identities', () => {
  const selected = model.summary(summary());
  assert.equal(model.page(page(selected, 'declarations', [decl]), selected).items[0].display_name, '<script>alert(1)</script>');
  const foreign = structuredClone(page(selected, 'declarations', [decl])); foreign.subject.image_revision = 'image-2';
  assert.throws(() => model.page(foreign, selected), /foreign page/);
  const unknown = structuredClone(selected); unknown.command = 'run';
  assert.throws(() => model.summary(unknown), /unexpected fields/);
  const authority = structuredClone(selected); authority.execution = true;
  assert.throws(() => model.summary(authority), /authority claim/);
});

test('candidate side and context target must be explicit', () => {
  const selected = summary(); selected.subject = { ...selected.subject, kind: 'candidate', candidate_revision: 'candidate-1', side: 'base' };
  selected.mode = 'context'; selected.target = 'auth';
  assert.equal(model.summary(selected).subject.side, 'base');
  selected.subject.side = 'current';
  assert.throws(() => model.summary(selected), /unsupported value/);
});

test('search uses IDs names and paths without executing supplied text', () => {
  assert.deepEqual(model.search([decl], 'AUTH'), [decl]);
  assert.deepEqual(model.search([decl], 'main.spx'), [decl]);
  assert.deepEqual(model.search([decl], '<script>'), [decl]);
  assert.deepEqual(model.search([decl], 'missing'), []);
});

test('parallel sites remain inspectable under visual aggregation', () => {
  const grouped = model.groupRelations([rel('site-2'), rel('site-1')]);
  assert.equal(grouped.length, 1); assert.deepEqual(grouped[0].sites.map(row => row.site_id), ['site-2', 'site-1']);
});

test('layout is stable for cycles and disconnected modules', () => {
  const nodes = [{ key: 'c' }, { key: 'a' }, { key: 'b' }, { key: 'isolated' }];
  const edges = [{ from: 'a', to: 'b' }, { from: 'b', to: 'a' }, { from: 'b', to: 'c' }];
  const first = layout(nodes, edges), second = layout([...nodes].reverse(), [...edges].reverse());
  assert.deepEqual([...first.positions], [...second.positions]);
  assert.ok(first.positions.has('isolated'));
  assert.ok(first.width >= 360 && first.height >= 240);
});
