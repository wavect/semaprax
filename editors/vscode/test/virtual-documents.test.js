'use strict';
// Content-store lifecycle for the read-only `semaprax-review` and
// `semaprax-token-report` views (REF-20): admission under item, aggregate and
// count limits, a pin while the view is being opened, release on genuine
// retirement with exact quota recovery, and rollback of a failed open.
const test = require('node:test');
const assert = require('node:assert/strict');
const { ContentStore, REVIEW_LIMITS, TOKEN_REPORT_LIMITS } = require('../virtual-documents');

const small = () => new ContentStore({ maxItemBytes: 8, maxTotalBytes: 16, maxCount: 3, exhausted: 'budget reached' });

test('more than the count limit of sequential create/close cycles succeed', () => {
  const store = new ContentStore(REVIEW_LIMITS);
  for (let index = 0; index < REVIEW_LIMITS.maxCount * 3; index++) {
    const uri = `semaprax-review:/${index}/view.json`;
    store.admit(uri, 'x'.repeat(1024));
    store.unpin(uri);
    store.closed(uri, false);
    assert.equal(store.size, 0);
    assert.equal(store.bytes, 0);
  }
});

test('live limits stay enforced and retiring a view recovers exactly its quota', () => {
  const store = small();
  store.admit('a', '12345678');
  assert.throws(() => store.admit('big', '123456789'), /budget reached/, 'per-item limit');
  store.admit('b', '1234');
  assert.throws(() => store.admit('c', '12345'), /budget reached/, 'aggregate limit');
  store.admit('c', '1234');
  assert.equal(store.bytes, 16);
  assert.throws(() => store.admit('d', ''), /budget reached/, 'count limit');
  for (const uri of ['a', 'b', 'c']) store.unpin(uri);
  store.closed('b', false);
  assert.equal(store.bytes, 12);
  assert.equal(store.size, 2);
  store.closed('b', false); store.release('b');
  assert.equal(store.bytes, 12, 'a double release does not underflow');
  store.admit('d', '1234');
  assert.equal(store.bytes, 16);
  // Multi-byte text is counted in UTF-8 bytes.
  const utf8 = small();
  assert.throws(() => utf8.admit('e', '\u{1F600}\u{1F600}\u{1F600}'), /budget reached/);
});

test('invalidation replaces content and keeps the byte accounting exact', () => {
  const store = small();
  store.admit('a', '12345678'); store.unpin('a');
  store.set('a', '12');
  assert.equal(store.bytes, 2);
  assert.equal(store.get('a'), '12');
  store.set('absent', 'x');
  assert.equal(store.has('absent'), false, 'an expired view is never recreated');
  store.closed('a', false);
  assert.equal(store.bytes, 0);
});

test('a close while the view is being opened or is still open elsewhere keeps its content', () => {
  const store = small();
  store.admit('a', 'text');
  // setTextDocumentLanguage emits close then open during creation.
  store.closed('a', false);
  assert.equal(store.get('a'), 'text', 'the creation pin holds the content');
  store.unpin('a');
  // Another editor still shows the same document.
  store.closed('a', true);
  assert.equal(store.get('a'), 'text');
  store.closed('a', false);
  assert.equal(store.has('a'), false);
  assert.equal(store.get('a'), undefined, 'an expired URI resolves to nothing');
});

test('a failed open rolls back its insertion and released views drop their associations', () => {
  const released = [];
  const store = new ContentStore({ ...TOKEN_REPORT_LIMITS, onRelease: uri => released.push(uri) });
  store.admit('t', 'report');
  store.rollback('t');
  assert.equal(store.size, 0); assert.equal(store.bytes, 0);
  store.admit('u', 'report'); store.unpin('u');
  store.admit('v', 'report'); store.unpin('v');
  store.closed('u', false);
  store.clear();
  assert.deepEqual(released, ['t', 'u', 'v']);
  assert.equal(store.size, 0); assert.equal(store.bytes, 0);
});

test('token reports are bounded like review views and recover after disposal', () => {
  assert.equal(TOKEN_REPORT_LIMITS.maxCount, 64);
  assert.equal(TOKEN_REPORT_LIMITS.maxItemBytes, 16 * 1024 * 1024);
  const store = new ContentStore(TOKEN_REPORT_LIMITS);
  for (let index = 0; index < 64; index++) { store.admit(`r${index}`, 'report'); store.unpin(`r${index}`); }
  assert.throws(() => store.admit('r64', 'report'), /Token report view budget reached/, 'the first over-bound view is rejected');
  store.closed('r0', false);
  store.admit('r64', 'report');
  assert.equal(store.size, 64);
});
