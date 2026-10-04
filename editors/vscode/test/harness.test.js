'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const h = require('../harness');

const status = overrides => JSON.stringify({
  schema: 'semaprax.harness-status.v1', config_digest: 'sha256:abc', lock: 'absent', inactive: ['x.example/notes'], ok: true,
  bindings: [
    { kind: 'context.repository', state: 'trusted', provider_id: 'ai.example/ctx', provider_version: '1.0.0', reason: 'single trusted installation', candidates: [{ provider_id: 'ai.example/ctx', verdict: 'compatible', detail: 'ok' }] },
    { kind: 'command.view', state: 'fallback', provider_id: 'semaprax.host/command-view', provider_version: '1', reason: 'builtin', candidates: [] },
    { kind: 'model.generate', state: 'unavailable', provider_id: '', provider_version: '', reason: 'none', candidates: [] }
  ], ...overrides
});

test('selected providers come straight from the status contract', () => {
  const parsed = h.parseStatus(status());
  assert.deepEqual(h.selectedProviders(parsed), { 'command.view': 'semaprax.host/command-view', 'context.repository': 'ai.example/ctx' });
});
test('summary states the observed scope and does not invent routing', () => {
  const text = h.summary(h.parseStatus(status()));
  assert.match(text, /only Semaprax-routed calls are observed/);
  assert.match(text, /host-controlled/);
  assert.doesNotMatch(text, /Jev|Laya/);
  assert.match(text, /Inactive extension x\.example\/notes/);
});
test('tree nests candidates under their capability', () => {
  const nodes = h.tree(h.parseStatus(status()));
  assert.equal(nodes.length, 3);
  assert.equal(nodes[0].children[0].label, 'ai.example/ctx: compatible');
});
test('foreign or malformed documents are refused', () => {
  assert.throws(() => h.parseStatus(status({ schema: 'other' })), /Unexpected/);
  assert.throws(() => h.parseStatus('{'), SyntaxError);
  assert.throws(() => h.parseStatus(status({ bindings: [{ kind: 1 }] })), /Invalid harness binding/);
});
test('actions build argv for the CLI and refuse hostile provider ids', () => {
  assert.deepEqual(h.statusArgv('/p'), ['status', '--json', '--project', '/p']);
  assert.deepEqual(h.inspectArgv('ai.example/ctx'), ['inspect', 'ai.example/ctx']);
  assert.deepEqual(h.enableArgv('ai.example/ctx'), ['trust', 'ai.example/ctx']);
  assert.deepEqual(h.disableArgv('ai.example/ctx'), ['revoke', 'ai.example/ctx']);
  for (const bad of ['', 'a', '../x', 'a/b; rm', '--flag/x', 'A/B', 'a/b/c']) assert.throws(() => h.inspectArgv(bad), /Invalid provider id/, bad);
  assert.deepEqual(h.command('/bin/semaprax', ['inspect', 'a/b']), { file: '/bin/semaprax', args: ['harness', 'inspect', 'a/b'] });
});
