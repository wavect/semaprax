'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { snapshotHost, vscodeHost } = require('../hosts.js');
const model = require('../model.js');

const digest = `sha256:${'b'.repeat(64)}`;
const query = { mode: 'overview', side: 'current' };
const selected = {
  schema: model.SCHEMA, kind: 'summary',
  subject: { kind: 'image', image_revision: digest, project_revision: digest, workspace_revision: digest, project_graph_digest: digest, candidate_revision: null, side: 'current' },
  mode: 'overview', target: null, query: { direction: 'both', depth: 1, max_nodes: 256, max_bytes: 262144 },
  artifact_digest: digest, truncation: { truncated: false, reason: null }, coverage: { owner: 'workspace_graph', complete_within_retained_graph: true },
  inventories: model.VIEWS.map(view => ({ view, handle: digest, total_items: view === 'modules' ? 1 : 0 })),
  source_authority: false, execution: false, publication_authority: false, nonclaims: []
};
const pages = model.VIEWS.map(view => ({
  schema: model.SCHEMA, kind: 'page', subject: selected.subject, mode: selected.mode, target: null, query: selected.query,
  artifact_digest: digest, truncation: selected.truncation, coverage: selected.coverage,
  view, handle: digest, cursor: null, offset: 0, total_items: view === 'modules' ? 1 : 0, page_size: 32, max_bytes: 65536, next_cursor: null,
  items: view === 'modules' ? [{ module: 'core', path: 'core.spx', declaration_count: 1, relation_count: 0, source_reference: { path: 'core.spx', source_revision: digest, source_digest: digest } }] : [],
  source_authority: false, execution: false, publication_authority: false, nonclaims: []
}));

test('offline and editor adapters deliver the same checked identities', async () => {
  const offline = snapshotHost({ views: [{ query, summary: selected, pages }] });
  let listener;
  const port = {
    addEventListener(type, fn) { assert.equal(type, 'message'); listener = fn; },
    postMessage(request) {
      assert.ok(['summary', 'page'].includes(request.action));
      const value = request.action === 'summary' ? selected : pages.find(row => row.view === request.value.view);
      queueMicrotask(() => listener({ data: { type: 'semaprax-explorer-response', generation: 7, requestId: request.requestId, ok: true, value } }));
    }
  };
  const editor = vscodeHost(port, 7);
  const [offlineSummary, editorSummary] = await Promise.all([offline.summary(query), editor.summary(query)]);
  assert.deepEqual(editorSummary.subject, offlineSummary.subject);
  const request = { summary: offlineSummary, view: 'modules', handle: digest, cursor: null, page_size: 32, max_bytes: 65536 };
  const [offlinePage, editorPage] = await Promise.all([offline.page(request), editor.page(request)]);
  assert.deepEqual(editorPage.items, offlinePage.items);
  assert.equal(model.page(editorPage, editorSummary).items[0].module, 'core');
  await assert.rejects(offline.evidence('unbundled'), /not bundled/);
});

test('offline source review is available only when explicitly bundled', async () => {
  const report = { schema: 'semaprax.project-candidate-source-review.v1', files: [] };
  const bundled = snapshotHost({ views: [{ query, summary: selected, pages }], source_review: report });
  assert.equal(await bundled.sourceReview(), report);
  const absent = snapshotHost({ views: [{ query, summary: selected, pages }] });
  await assert.rejects(absent.sourceReview(), /not bundled/);
});

test('disposing an editor host rejects retained calls and releases their timers', async () => {
  let listener;
  const port = {
    addEventListener(type, fn) { assert.equal(type, 'message'); listener = fn; },
    postMessage() {}
  };
  const editor = vscodeHost(port, 9);
  const pending = editor.summary(query);
  editor.dispose();
  await assert.rejects(pending, /disposed/);
  await assert.rejects(editor.summary(query), /disposed/);
  listener({ data: { type: 'semaprax-explorer-response', generation: 9, requestId: 1, ok: true, value: selected } });
});
