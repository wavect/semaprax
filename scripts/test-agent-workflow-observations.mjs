#!/usr/bin/env node
// MF-06: every admitted tools/call leaves exactly one terminal observation, and
// direct-wrapper outcomes are classified without changing bytes or errors.
// Fake caller-owned transports only; no network, model, or tokenizer process.
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  MCP_PROTOCOL_VERSION, ToolPayloadObserver, connectMcpWorkflowTransport, observeDirectWorkflowTransport,
} from '../packages/semaprax-agent-workflow/dist/index.js';

const line = (value) => `${JSON.stringify(value)}\n`;
const initResult = (id) => line({ jsonrpc: '2.0', id, result: {
  protocolVersion: MCP_PROTOCOL_VERSION, capabilities: { tools: { listChanged: false } },
  serverInfo: { name: 'fake', version: '0' } } });
const v5Request = (id = 'req-1') => line({ jsonrpc: '2.0', id, method: 'semaprax/ping', params: { image_revision: `sha256:${'b'.repeat(64)}` } });
const SECRET = 'SECRET-PROMPT-CODE-TOKEN-123';
const okInner = line({ jsonrpc: '2.0', id: 0, result: { note: SECRET } });
const errInner = line({ jsonrpc: '2.0', id: 0, error: { code: -32000, message: SECRET } });
const outerOk = (id, text, isError = false) => line({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text }], isError } });

function harness(onCall) {
  const calls = { exchange: 0, notify: 0 };
  const wire = {
    sessionId: 's1',
    exchange(frame) {
      const request = JSON.parse(frame);
      if (request.method === 'initialize') return initResult(request.id);
      calls.exchange += 1;
      return onCall(request, calls.exchange);
    },
    notify() { calls.notify += 1; },
  };
  const events = [];
  const observer = new ToolPayloadObserver({ sessionId: 's1', sink: (e) => { events.push(e); } });
  return { wire, observer, events, calls };
}

async function mcpCall(onCall, ...more) {
  const h = harness(onCall);
  const transport = await connectMcpWorkflowTransport(h.wire, h.observer);
  let outcome;
  for (const id of ['req-1', ...more]) {
    try { outcome = { value: await transport.exchange(v5Request(id)) }; } catch (error) { outcome = { error }; }
  }
  await h.observer.drain();
  return { ...h, outcome, observed: h.observer.events() };
}

const expectOne = (observed, outcome, { text = false } = {}) => {
  assert.equal(observed.length, 1, JSON.stringify(observed));
  assert.equal(observed[0].outcome, outcome);
  assert.equal(observed[0].boundary, 'mcp_content_0_text');
  if (text) assert.ok(observed[0].bytes > 0);
  else assert.deepEqual([observed[0].status, observed[0].bytes, observed[0].digest, observed[0].tokens], ['incomplete', null, null, null]);
  assert.ok(!JSON.stringify(observed).includes(SECRET));
};

test('mcp success is observed once and bytes are unchanged', async () => {
  const r = await mcpCall((req) => outerOk(req.id, okInner));
  assert.equal(r.outcome.value, JSON.stringify({ jsonrpc: '2.0', id: 'req-1', result: { note: SECRET } }));
  expectOne(r.observed, 'success', { text: true });
});

test('mcp application error text is observed once as error', async () => {
  const r = await mcpCall((req) => outerOk(req.id, errInner, true));
  expectOne(r.observed, 'error', { text: true });
});

test('mcp tools/call timeout leaves one timeout observation with unknown counts', async () => {
  const original = new Error('request timeout');
  const r = await mcpCall(() => { throw original; });
  assert.equal(r.outcome.error, original);
  expectOne(r.observed, 'timeout');
});

test('mcp async disconnect leaves one incomplete observation', async () => {
  const original = new Error('socket closed');
  const r = await mcpCall(async () => { throw original; });
  assert.equal(r.outcome.error, original);
  assert.equal(r.calls.exchange, 1);
  expectOne(r.observed, 'incomplete');
});

for (const [name, reply, outcome, hasText] of [
  ['invalid outer JSON', () => 'not json\n', 'malformed', false],
  ['wrong outer identity', () => outerOk('wrong-id', okInner), 'malformed', false],
  ['invalid content shape', (req) => line({ jsonrpc: '2.0', id: req.id, result: { content: [], isError: false } }), 'malformed', false],
  ['non-text content', (req) => line({ jsonrpc: '2.0', id: req.id, result: { content: [{ type: 'image', text: 'x' }], isError: false } }), 'malformed', false],
  ['invalid inner JSON', (req) => outerOk(req.id, `{"secret":"${SECRET}"`), 'malformed', true],
  ['uncorrelated inner response', (req) => outerOk(req.id, line({ jsonrpc: '2.0', id: 9, result: {} })), 'malformed', true],
]) {
  test(`mcp ${name} leaves exactly one ${outcome} observation`, async () => {
    const r = await mcpCall(reply);
    assert.ok(r.outcome.error instanceof Error);
    assert.equal(r.calls.exchange, 1);
    expectOne(r.observed, outcome, { text: hasText });
  });
}

test('success followed by timeout cannot look like a successful-only session', async () => {
  const r = await mcpCall((req, n) => { if (n === 2) throw new Error('timeout'); return outerOk(req.id, okInner); }, 'req-2');
  assert.deepEqual(r.observed.map((e) => e.outcome), ['success', 'timeout']);
  assert.deepEqual(r.observed.map((e) => e.attemptSequence), [1, 2]);
});

test('invalid outgoing request is not an admitted tools/call and is not observed', async () => {
  const h = harness(() => assert.fail('must not dispatch'));
  const transport = await connectMcpWorkflowTransport(h.wire, h.observer);
  await assert.rejects(transport.exchange('{"jsonrpc":"2.0"}\n'));
  await h.observer.drain();
  assert.equal(h.observer.events().length, 0);
  assert.equal(h.calls.exchange, 0);
});

test('failing sink and throwing observer never alter MCP protocol flow', async () => {
  const h = harness((req) => outerOk(req.id, okInner));
  const observer = new ToolPayloadObserver({ sessionId: 's1', sink: () => { throw new Error('sink'); } });
  const transport = await connectMcpWorkflowTransport(h.wire, observer);
  assert.equal(JSON.parse(await transport.exchange(v5Request())).result.note, SECRET);
  observer.observe = () => { throw new Error('observer'); };
  assert.equal(JSON.parse(await transport.exchange(v5Request())).result.note, SECRET);
  const original = new Error('timeout');
  h.wire.exchange = () => { throw original; };
  await assert.rejects(transport.exchange(v5Request()), (e) => e === original);
});

async function direct(reply) {
  const events = [];
  const observer = new ToolPayloadObserver({ sessionId: 's1', sink: (e) => { events.push(e); } });
  let dispatched = 0;
  const transport = observeDirectWorkflowTransport({ sessionId: 's1', exchange: async () => { dispatched += 1; if (reply instanceof Error) throw reply; return reply; } }, observer);
  let outcome;
  try { outcome = { value: await transport.exchange(v5Request('d1')) }; } catch (error) { outcome = { error }; }
  await observer.drain();
  assert.equal(dispatched, 1);
  assert.equal(events.length, 1);
  return { outcome, event: events[0] };
}

test('direct v5 result, application error, malformed and rejected exchanges are classified', async () => {
  const result = line({ jsonrpc: '2.0', id: 'd1', result: { ok: true } });
  const failure = line({ jsonrpc: '2.0', id: 'd1', error: { code: -32000, message: 'no' } });
  for (const [reply, expected] of [
    [result, 'success'], [failure, 'error'], ['garbage\n', 'malformed'],
    [line({ jsonrpc: '2.0', id: 'other', result: {} }), 'malformed'],
    [line({ jsonrpc: '2.0', id: 'd1', result: {}, error: {} }), 'malformed'],
    [line({ jsonrpc: '2.0', id: 'd1' }), 'malformed'],
  ]) {
    const { outcome, event } = await direct(reply);
    assert.equal(outcome.value, reply, 'response bytes unchanged');
    assert.equal(event.outcome, expected);
  }
  const boom = new Error('timeout talking');
  const { outcome, event } = await direct(boom);
  assert.equal(outcome.error, boom);
  assert.equal(event.outcome, 'timeout');
  assert.equal(event.status, 'incomplete');
});
