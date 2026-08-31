'use strict';
// Authored evidence only; not executed during implementation.
const test = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { McpClient, parse, inner, exact, invalidates, ALLOWED, MAX_REQUEST, MAX_RESPONSE } = require('../protocol');
const D = 'sha256:' + 'a'.repeat(64);
function result(payload = {}) { return { schema: 'semaprax.image-agent-result.v5', protocol: 'semaprax.image-agent-protocol.v5', image_revision: D, project_revision: D, payload }; }
function wrapped(payload = {}) { return { content: [{ type: 'text', text: JSON.stringify({ jsonrpc: '2.0', id: 0, result: result(payload) }) }], isError: false }; }
class Child extends EventEmitter {
  constructor() {
    super(); this.stdout = new EventEmitter(); this.stderr = new EventEmitter(); this.stdin = new EventEmitter(); this.writes = []; this.killed = false;
    this.stdin.write = (line, callback) => { this.writes.push(line); if (callback) callback(); return true; };
  }
  kill() { this.killed = true; }
  send(value) { this.stdout.emit('data', Buffer.from(JSON.stringify(value) + '\n')); }
}
test('strict parser rejects duplicate keys, hostile depth, invalid UTF16 and rounded intent integers', () => {
  assert.throws(() => exact({ 'a\0b': 1 }, ['a', 'b']), /fields/);
  for (const value of ['{"a":1,"a":2}', '{"a":{"x":1,"x":2}}', '"\\ud800"', '['.repeat(130) + '0' + ']'.repeat(130), '{"x":1} trailing']) assert.throws(() => parse(value));
  assert.throws(() => parse('{"value":9007199254740993}', 65536, true));
  assert.throws(() => parse('{"value":1.5}', 65536, true));
  assert.deepEqual(parse('{"value":42}', 65536, true), { value: 42 });
  // Catalog schema bounds can be descriptive u64 numbers, never intent values.
  assert.equal(typeof parse('{"maximum":18446744073709551615}').maximum, 'number');
  assert.throws(() => parse('"' + 'x'.repeat(100) + '"', 100));
});
test('ordinary semantic rejection preserves the valid candidate while live-source drift invalidates it', () => {
  assert.equal(invalidates({ semantic: true, message: 'SPX-G225 invalid constructor' }), false);
  assert.equal(invalidates({ semantic: true, message: 'SPX-G224 candidate selector rejected' }), false);
  assert.equal(invalidates({ semantic: true, message: 'SPX-G223 report capacity' }), false);
  for (const code of ['SPX-J102', 'SPX-J103', 'SPX-G221', 'SPX-G282', 'SPX-G283']) assert.equal(invalidates({ semantic: true, message: code }), true);
  assert.equal(invalidates(new Error('framing failed')), true);
});
test('inner v5 text remains exact, id0 is mandatory and semantic rejection is not hidden', () => {
  const message = wrapped({ source_authority: false });
  assert.equal(inner(message).raw, message.content[0].text);
  assert.deepEqual(inner(message).payload, { source_authority: false });
  const wrong = structuredClone(message); wrong.content[0].text = wrong.content[0].text.replace('"id":0', '"id":1');
  assert.throws(() => inner(wrong), /identity/);
  assert.throws(() => inner({ ...message, isError: true }));
  const failure = { content: [{ type: 'text', text: JSON.stringify({ jsonrpc: '2.0', id: 0, error: { code: -32000, message: 'SPX-G282 stale image' } }) }], isError: true };
  assert.throws(() => inner(failure), error => error.semantic === true && /SPX-G282/.test(error.message));
});
test('single-flight requests match IDs across fragmented UTF8 and reject terminal unsolicited output', async () => {
  const child = new Child(); const failures = []; const client = new McpClient(child, error => failures.push(error));
  const pending = client.request('ping', {});
  await assert.rejects(client.request('ping', {}), /pending/);
  const id = JSON.parse(child.writes[0]).id;
  const bytes = Buffer.from(JSON.stringify({ jsonrpc: '2.0', id, result: { label: 'é' } }) + '\n');
  const boundary = bytes.indexOf(Buffer.from('é')) + 1;
  child.stdout.emit('data', bytes.subarray(0, boundary)); child.stdout.emit('data', bytes.subarray(boundary));
  assert.deepEqual(await pending, { label: 'é' });
  child.send({ jsonrpc: '2.0', id: 'unsolicited', result: {} });
  assert.equal(client.closed, true); assert.equal(child.killed, true); assert.equal(failures.length, 1);
});
test('timeout and response overflow terminate without retrying any request', async () => {
  const child = new Child(); const client = new McpClient(child, () => {}, 5);
  await assert.rejects(client.request('ping', {}), /timed out/);
  assert.equal(child.writes.length, 1); assert.equal(child.killed, true);
  const huge = new Child(); const bounded = new McpClient(huge);
  const pending = bounded.request('ping', {});
  huge.stdout.emit('data', Buffer.alloc(MAX_RESPONSE + 1));
  await assert.rejects(pending, /byte limit/); assert.equal(huge.killed, true);
  const outgoing = new Child(); const requestBound = new McpClient(outgoing);
  await assert.rejects(requestBound.request('ping', { text: 'x'.repeat(MAX_REQUEST) }), /byte limit/);
  assert.equal(outgoing.writes.length, 0); requestBound.stop();
});
test('even a host-granted tool cannot escape the fixed editor authority allowlist', async () => {
  const child = new Child(); const client = new McpClient(child);
  for (const method of ['candidate/build', 'candidate/test', 'candidate/commit', 'candidate/commit-report', 'hole/archive-restore']) {
    client.tools.add(method);
    assert.equal(ALLOWED.has(method), false);
    await assert.rejects(client.call(method, { approved: true }), /allowlist/);
  }
  assert.equal(child.writes.length, 0); client.stop();
});
test('initialize pages discover only the exact selected safe tool mapping', async () => {
  const child = new Child(); const client = new McpClient(child);
  const write = child.stdin.write;
  child.stdin.write = (line, callback) => {
    write(line, callback); const request = JSON.parse(line); if (!request.id) return true;
    queueMicrotask(() => child.send({ jsonrpc: '2.0', id: request.id, result:
      request.method === 'initialize' ? { protocolVersion: '2025-11-25', capabilities: { tools: {} } } :
      request.params.cursor === undefined ? { tools: [{ name: 'workspace__open' }, { name: 'candidate__commit' }], nextCursor: 'next' } :
      { tools: [{ name: 'candidate__source-review' }] } })); return true;
  };
  await client.initialize();
  assert.deepEqual([...client.tools].sort(), ['candidate/source-review', 'workspace/open']);
  assert.equal(child.writes.filter(line => JSON.parse(line).method === 'notifications/initialized').length, 1);
  client.stop();
});
test('catalog rejects oversized tool names before retaining them', async () => {
  const child = new Child(), client = new McpClient(child);
  child.stdin.write = (line, callback) => {
    if (callback) callback(); const request = JSON.parse(line); if (!request.id) return true;
    queueMicrotask(() => child.send({ jsonrpc: '2.0', id: request.id, result: request.method === 'initialize' ?
      { protocolVersion: '2025-11-25', capabilities: { tools: {} } } : { tools: [{ name: 'x'.repeat(129) }] } })); return true;
  };
  await assert.rejects(client.initialize(), /invalid or excessive/);
  assert.equal(client.tools.size, 0); client.stop();
});
