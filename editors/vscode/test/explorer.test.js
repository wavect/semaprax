'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { ExplorerScheduler, message, pageRequest, readChangeReport, readEvidence, openExplorer } = require('../explorer');

test('the packaged viewer is an exact, hashed copy of the shared viewer assets', () => {
  const root = path.join(__dirname, '..');
  const manifest = JSON.parse(fs.readFileSync(path.join(root, 'explorer-assets.manifest.json'), 'utf8'));
  assert.equal(manifest.schema, 'semaprax.vscode-explorer-assets.v1');
  assert.deepEqual(Object.keys(manifest.assets).sort(), ['cache.js', 'changes.js', 'evidence.js', 'explorer.css', 'hosts.js', 'layout.js', 'model.js', 'view.js']);
  for (const [file, expected] of Object.entries(manifest.assets)) {
    const shared = fs.readFileSync(path.join(root, '..', '..', 'ui', 'semantic-explorer', file));
    const packaged = fs.readFileSync(path.join(root, 'explorer-assets', file));
    assert.deepEqual(packaged, shared, `${file} must not fork viewer logic`);
    assert.equal(crypto.createHash('sha256').update(packaged).digest('hex'), expected, `${file} manifest digest`);
  }
});

test('explorer requests coalesce immutable reads and discard queued obsolete work', async () => {
  const seen = [], scheduler = new ExplorerScheduler(async value => { seen.push(value); return value; });
  const one = scheduler.read('same', () => scheduler.call('one'));
  const two = scheduler.read('same', () => scheduler.call('two'));
  assert.equal(await one, 'one'); assert.equal(await two, 'one'); assert.deepEqual(seen, ['one']);
  let release;
  const blocker = scheduler.read('block', () => new Promise(resolve => { release = resolve; }));
  const old = scheduler.read('old', () => scheduler.call('old'));
  scheduler.clear(); release('done');
  assert.equal(await blocker, 'done');
  await assert.rejects(old, /selection changed/);
});

test('panel messages and page cursors are closed over the retained summary', () => {
  const request = { type: 'semaprax-explorer-request', generation: 4, requestId: 1, action: 'summary', value: { mode: 'overview', target: null, direction: 'both', depth: 1, side: 'current' } };
  assert.equal(message(request, 4).action, 'summary');
  assert.equal(message({ ...request, requestId: 2, action: 'deltaCatalog', value: { candidateRevision: 'sha256:' + 'a'.repeat(64) } }, 4).action, 'deltaCatalog');
  assert.equal(message({ ...request, requestId: 3, action: 'semanticDelta', value: { target: 'calculator.add' } }, 4).action, 'semanticDelta');
  assert.equal(message({ ...request, generation: 3, action: 'tools/call' }, 4), null);
  assert.equal(message({ ...request, foreign: 'ignored before this check' }, 4), null);
  const summary = { inventories: [{ view: 'modules', handle: 'sha256:' + 'a'.repeat(64) }] }, cursors = new Map([['modules', null]]);
  const page = { view: 'modules', handle: 'sha256:' + 'a'.repeat(64), cursor: null, page_size: 32, max_bytes: 65536 };
  assert.deepEqual(pageRequest(page, summary, cursors), page);
  assert.equal(pageRequest({ ...page, cursor: 'forged' }, summary, cursors), null);
  assert.equal(pageRequest({ ...page, extra: true }, summary, cursors), null);
});

test('actual webview handler ignores stale and arbitrary RPC messages before invocation', async () => {
  let receive, dispose;
  const panel = {
    webview: {
      cspSource: 'vscode-webview://test',
      asWebviewUri(uri) { return uri.path; },
      postMessage() {},
      onDidReceiveMessage(listener) { receive = listener; return { dispose() {} }; }
    },
    onDidDispose(listener) { dispose = listener; }
  };
  const vscode = { ViewColumn: { Beside: 2 }, window: { createWebviewPanel() { return panel; } } };
  const extensionUri = { path: path.join(__dirname, '..'), with(change) { return { ...this, ...change, with: this.with }; } };
  let invoked = 0;
  const state = { panel: null, panelGeneration: 0, live: () => true, image: () => 'sha256:' + 'a'.repeat(64), candidate: () => null, invoke: async () => { invoked++; } };
  openExplorer(vscode, { extensionUri }, state, { mode: 'overview', target: null, direction: 'both', depth: 1, side: 'current' });
  await receive({ type: 'semaprax-explorer-request', generation: 0, requestId: 1, action: 'tools/call', value: {} });
  await receive({ type: 'semaprax-explorer-request', generation: 0, requestId: 2, action: 'summary', value: { mode: 'overview', target: null, direction: 'both', depth: 1, side: 'current' }, foreign: true });
  await receive({ type: 'semaprax-explorer-request', generation: 9, requestId: 3, action: 'summary', value: { mode: 'overview', target: null, direction: 'both', depth: 1, side: 'current' } });
  assert.equal(invoked, 0);
  dispose();
});

test('candidate change reports are reassembled only from the selected immutable subject', async () => {
  const candidate = 'sha256:' + 'b'.repeat(64);
  const report = JSON.stringify({ schema: 'semaprax.project-candidate-semantic-delta-catalog.v1', candidate_digest: candidate });
  const calls = [];
  const invoke = async (_method, params) => {
    calls.push(params);
    const offset = params.offset || 0, chunk = report.slice(offset, offset + 20), next = offset + Buffer.byteLength(chunk);
    return { payload: { schema: 'semaprax.image-semantic-delta-chunk.v1', candidate_revision: candidate, target: null, report_schema: 'semaprax.project-candidate-semantic-delta-catalog.v1', offset, total_bytes: Buffer.byteLength(report), chunk, next_offset: next === Buffer.byteLength(report) ? null : next, source_authority: false } };
  };
  assert.deepEqual(await readChangeReport(invoke, 'sha256:' + 'a'.repeat(64), candidate, null), JSON.parse(report));
  assert.ok(calls.length > 1);
  await assert.rejects(readChangeReport(async () => ({ payload: { schema: 'semaprax.image-semantic-delta-chunk.v1', candidate_revision: candidate, target: 'forged', report_schema: 'semaprax.project-candidate-semantic-delta-catalog.v1', offset: 0, total_bytes: 0, chunk: '', next_offset: null } }), 'sha256:' + 'a'.repeat(64), candidate, null), /chunk/);
});

test('evidence reads are closed, exact-subject-bound, and source-free', async () => {
  const image = 'sha256:' + 'a'.repeat(64), project = 'sha256:' + 'b'.repeat(64), workspace = 'sha256:' + 'c'.repeat(64), graph = 'sha256:' + 'd'.repeat(64);
  const subject = { kind: 'image', image_revision: image, project_revision: project, workspace_revision: workspace, project_graph_digest: graph, candidate_revision: null, side: 'current' };
  const calls = [];
  const invoke = async (method, params) => {
    calls.push([method, params]);
    return { image_revision: image, project_revision: project, payload: {
      schema: 'semaprax.image-function-summary.v1', image_revision: image, project_revision: project, workspace_revision: workspace, project_graph_digest: graph,
      id: 'calculator.add', parameter_count: 2, return_type_id: 'Int', effects: ['pure'], requires_count: 1, ensures_count: 0,
      facets: [{ facet: 'contracts', handle: image }], source_authority: false, target_execution: false, source_body: 'must never escape'
    } };
  };
  const result = await readEvidence(invoke, { image: () => image, candidate: () => null }, subject, { method: 'image/function-summary', subject, target: 'calculator.add', facet: null });
  assert.deepEqual(calls, [['image/function-summary', { image_revision: image, target: 'calculator.add' }]]);
  assert.deepEqual(result.compact, { id: 'calculator.add', parameter_count: 2, return_type_id: 'Int', effects: ['pure'], requires_count: 1, ensures_count: 0, facets: ['contracts'] });
  assert.equal(JSON.stringify(result).includes('must never escape'), false);
  await assert.rejects(readEvidence(invoke, { image: () => image, candidate: () => null }, subject, { method: 'candidate/function-summary', subject, target: 'calculator.add', facet: null }), /stale|binding|candidate/);
});

test('candidate evidence deltas retain only inventories and persistent IDs', async () => {
  const image = 'sha256:' + 'a'.repeat(64), base = 'sha256:' + 'b'.repeat(64), project = 'sha256:' + 'c'.repeat(64), workspace = 'sha256:' + 'd'.repeat(64), graph = 'sha256:' + 'e'.repeat(64), candidate = 'sha256:' + 'f'.repeat(64);
  const subject = { kind: 'candidate', image_revision: image, project_revision: project, workspace_revision: workspace, project_graph_digest: graph, candidate_revision: candidate, side: 'candidate' };
  const report = JSON.stringify({ schema: 'semaprax.project-candidate-contract-delta.v1', candidate_digest: candidate, base_project_revision: base, project_revision: project, base_workspace_revision: base, workspace_revision: workspace, inventory: { base_functions: 2, candidate_functions: 2, affected_functions: 1 }, functions: [{ id: 'calculator.helper', change: 'modified', candidate: { predicates: ['literal source text'] } }], execution: false, source_authority: false, nonclaims: ['no_source_authority'] });
  const calls = [];
  const invoke = async (method, params) => {
    calls.push([method, params]);
    return { image_revision: image, project_revision: base, payload: { schema: 'semaprax.image-contract-delta-chunk.v1', report_schema: 'semaprax.project-candidate-contract-delta.v1', image_revision: image, candidate_revision: candidate, offset: 0, total_bytes: Buffer.byteLength(report), chunk: report, next_offset: null, source_authority: false } };
  };
  const result = await readEvidence(invoke, { image: () => image, candidate: () => candidate }, subject, { method: 'candidate/contract-delta', subject, target: null, facet: null });
  assert.equal(calls.length, 1);
  assert.equal(result.compact.changed[0].id, 'calculator.helper');
  assert.equal(JSON.stringify(result).includes('literal source text'), false);
  assert.deepEqual(result.omitted, ['source bodies', 'raw report payloads', 'literal-bearing checked expressions', 'source spans']);
});

test('candidate function summary accepts its derived image while binding the selected candidate', async () => {
  const image = 'sha256:' + 'a'.repeat(64), derived = 'sha256:' + 'b'.repeat(64), candidate = 'sha256:' + 'c'.repeat(64);
  const project = 'sha256:' + 'd'.repeat(64), workspace = 'sha256:' + 'e'.repeat(64), graph = 'sha256:' + 'f'.repeat(64);
  const subject = { kind: 'candidate', image_revision: image, project_revision: project, workspace_revision: workspace, project_graph_digest: graph, candidate_revision: candidate, side: 'candidate' };
  const invoke = async () => ({ image_revision: image, payload: { schema: 'semaprax.project-candidate-function-summary.v1', image_revision: derived, project_revision: project, workspace_revision: workspace, project_graph_digest: graph, candidate_revision: candidate, id: 'calculator.add', parameter_count: 2, return_type_id: 'Int', effects: [], requires_count: 0, ensures_count: 0, facets: [], source_authority: false } });
  const result = await readEvidence(invoke, { image: () => image, candidate: () => candidate }, subject, { method: 'candidate/function-summary', subject, target: 'calculator.add', facet: null });
  assert.equal(result.state, 'available');
});
