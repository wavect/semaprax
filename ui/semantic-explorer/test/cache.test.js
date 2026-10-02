'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { ExplorerCache, MAX_ENTRIES, MAX_RETAINED_BYTES, MAX_ENTRY_BYTES, requestKey } = require('../cache.js');

const digest = `sha256:${'a'.repeat(64)}`;
const subject = Object.freeze({
  kind: 'candidate', image_revision: digest, project_revision: digest,
  workspace_revision: digest, project_graph_digest: digest,
  candidate_revision: `sha256:${'b'.repeat(64)}`, side: 'candidate'
});
const query = Object.freeze({ direction: 'both', depth: 1, max_nodes: 256, max_bytes: 65536 });
function summaryRequest(changes = {}) { return { kind: 'summary', subject, target: null, mode: 'overview', query, artifact_digest: digest, ...changes }; }
function pageRequest(changes = {}) {
  return { kind: 'page', subject, target: null, mode: 'overview', query, artifact_digest: digest,
    view: 'declarations', handle: digest, cursor: null, options: { page_size: 32, max_bytes: 65536 }, ...changes };
}
function response(kind, value = 'ok') { return { kind, value }; }

test('cache key binds every immutable subject and page selector', () => {
  const base = pageRequest();
  const fields = [
    ['subject', { ...subject, image_revision: `sha256:${'c'.repeat(64)}` }],
    ['subject', { ...subject, candidate_revision: `sha256:${'c'.repeat(64)}` }],
    ['subject', { ...subject, side: 'base' }], ['target', '@id:changed'],
    ['mode', 'impact'], ['query', { ...query, depth: 2 }], ['artifact_digest', `sha256:${'c'.repeat(64)}`],
    ['view', 'relations'], ['handle', `sha256:${'c'.repeat(64)}`], ['cursor', 'next'],
    ['options', { page_size: 16, max_bytes: 65536 }]
  ];
  for (const [field, value] of fields) assert.notEqual(requestKey(base), requestKey({ ...base, [field]: value }), `key must include ${field}`);
});

test('cache retains only immutable responses and returns independent values', () => {
  const cache = new ExplorerCache();
  const request = summaryRequest();
  assert.equal(cache.set(request, response('summary', { rows: [1] })), true);
  const first = cache.get(request); first.value.rows.push(2);
  assert.deepEqual(cache.get(request), response('summary', { rows: [1] }));
  assert.equal(cache.set(request, { kind: 'page' }), false, 'request and response kinds must agree');
  assert.equal(cache.set(request, response('summary', { source_body: 'private' })), false, 'source bodies are never retained');
  assert.equal(cache.set(request, response('summary', { credentials: 'private' })), false, 'credentials are never retained');
});

test('cache is an LRU bounded by entry count and retained UTF-8 bytes', () => {
  const cache = new ExplorerCache({ maxEntries: 2, maxBytes: 110, maxEntryBytes: 64 });
  const one = summaryRequest({ target: 'one' }), two = summaryRequest({ target: 'two' }), three = summaryRequest({ target: 'three' });
  assert.equal(cache.set(one, response('summary', 'α'.repeat(10))), true);
  assert.equal(cache.set(two, response('summary', 'β'.repeat(10))), true);
  assert.ok(cache.get(one), 'reading moves one to most-recent');
  assert.equal(cache.set(three, response('summary', 'γ'.repeat(10))), true);
  assert.equal(cache.get(two), null, 'least-recent entry is evicted');
  assert.ok(cache.get(one)); assert.ok(cache.get(three));
  assert.equal(cache.set(summaryRequest({ target: 'large' }), response('summary', 'x'.repeat(100))), false);
  assert.ok(cache.stats().retainedBytes <= 110);
  cache.clear(); assert.deepEqual(cache.stats(), { entries: 0, retainedBytes: 0 });
});

test('cache ceilings are fixed at the Explorer contract limits', () => {
  assert.throws(() => new ExplorerCache({ maxEntries: MAX_ENTRIES + 1 }), /limits/);
  assert.throws(() => new ExplorerCache({ maxBytes: MAX_RETAINED_BYTES + 1 }), /limits/);
  assert.throws(() => new ExplorerCache({ maxEntryBytes: MAX_ENTRY_BYTES + 1 }), /limits/);
});
