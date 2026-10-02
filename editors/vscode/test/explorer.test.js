'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { ExplorerScheduler, message, pageRequest, readChangeReport } = require('../explorer');

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
  assert.equal(message({ type: 'semaprax-explorer-request', generation: 4, requestId: 1, action: 'summary' }, 4).action, 'summary');
  assert.equal(message({ type: 'semaprax-explorer-request', generation: 4, requestId: 2, action: 'deltaCatalog' }, 4).action, 'deltaCatalog');
  assert.equal(message({ type: 'semaprax-explorer-request', generation: 4, requestId: 3, action: 'semanticDelta' }, 4).action, 'semanticDelta');
  assert.equal(message({ type: 'semaprax-explorer-request', generation: 3, requestId: 1, action: 'tools/call' }, 4), null);
  const summary = { inventories: [{ view: 'modules', handle: 'sha256:' + 'a'.repeat(64) }] }, cursors = new Map([['modules', null]]);
  const request = { view: 'modules', handle: 'sha256:' + 'a'.repeat(64), cursor: null, page_size: 32, max_bytes: 65536 };
  assert.deepEqual(pageRequest(request, summary, cursors), request);
  assert.equal(pageRequest({ ...request, cursor: 'forged' }, summary, cursors), null);
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
