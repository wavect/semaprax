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

const D = 'sha256:' + 'a'.repeat(64), L = 'sha256:' + 'b'.repeat(64);
const skillsDoc = (extra = {}, mode = 'full', locked = L) => JSON.stringify({
  official_switch_off_by: null, status_lines: ['Caveman mode: unknown'],
  skills: [{ id: 'ponytail', version: '4.10.3', digest: D, default_available: true, selected: true, mode, mode_source: 'session', locked_revision: locked, disabled: null, omitted: null, applied_to_model: false },
    { id: 'caveman', version: '3.1.0', digest: D, default_available: true, selected: false, mode: 'off', mode_source: 'shipped-recommendation', locked_revision: null, disabled: null, omitted: null, applied_to_model: false }], ...extra
});

test('skills status parses the CLI document and the bridge document alike', () => {
  const cli = h.parseSkillsStatus(skillsDoc());
  const bridge = h.parseSkillsStatus(skillsDoc({ schema: h.SKILLS_SCHEMA, session: 's1', project: 'p-1', host_owned: [{ id: 'caveman', revision: 'same-revision' }], model_routing: 'not-delegated', delivery: [] }));
  assert.deepEqual(h.skillsAgree(cli, bridge), { agree: true, differences: [] });
  assert.equal(bridge.modelRouting, 'not-delegated');
  assert.deepEqual(h.skillsFacts(cli).ponytail, { mode: 'full', lockedRevision: L });
});
test('agreement notices a mode or pinned-revision difference', () => {
  const a = h.parseSkillsStatus(skillsDoc()), b = h.parseSkillsStatus(skillsDoc({}, 'ultra', D));
  const r = h.skillsAgree(a, b);
  assert.equal(r.agree, false);
  assert.match(r.differences[0], /ponytail/);
});
test('foreign or hostile skills documents are refused', () => {
  assert.throws(() => h.parseSkillsStatus(skillsDoc({ schema: 'other' })), /Unexpected skills/);
  assert.throws(() => h.parseSkillsStatus(JSON.stringify({ skills: [{ id: '../x', mode: 'm', version: '1' }] })), /Invalid skill entry/);
  assert.throws(() => h.parseSkillsStatus(JSON.stringify({ skills: [{ id: 'a', mode: 'm', version: '1', digest: 'nope' }] })), /Invalid skill entry/);
});
test('the status view shows availability, revision, mode, pending updates and disabled reasons', () => {
  const skills = h.parseSkillsStatus(skillsDoc({ official_switch_off_by: 'project', host_owned: [{ id: 'caveman', revision: 'same-revision' }], model_routing: 'not-delegated' }));
  skills.skills[1].disabled = 'mode-off:project';
  const updates = h.parseUpdatesStatus(JSON.stringify({ schema: 'semaprax.updates-report.v1', offline: true, notice: null, sources: [{ id: 'ponytail', active: '4.10.3', candidate: '4.11.0', state: 'candidate' }, { id: 'caveman', active: '3.1.0', candidate: null }] }));
  const text = h.summary(h.parseStatus(status()), skills, updates);
  assert.match(text, /ponytail 4\.10\.3: available, mode full \(session\), pinned sha256:bbbb/);
  assert.match(text, /disabled: mode-off:project/);
  assert.match(text, /all optional skills disabled by project/);
  assert.match(text, /host already owns caveman/);
  assert.match(text, /update pending: ponytail 4\.10\.3 -> 4\.11\.0/);
  assert.match(text, /model routing: not-delegated/);
  assert.doesNotMatch(text, /Jev|Laya/);
});
test('an unavailable skills query is a stated reason, never a silent omission', () => {
  const text = h.summary(h.parseStatus(status()), { unavailable: 'unknown harness verb' }, { unavailable: 'offline' });
  assert.match(text, /Default skills unavailable: unknown harness verb/);
  assert.doesNotMatch(h.summary(h.parseStatus(status())), /Default skills/);
});
test('skills argv uses the CLI project id derivation', () => {
  const id = h.projectId('/tmp/x');
  assert.match(id, /^p-[0-9a-f]{16}$/);
  assert.deepEqual(h.skillsStatusArgv('/tmp/x', 's1'), ['skills', 'status', '--json', '--project', id, '--session', 's1']);
  assert.deepEqual(h.updatesStatusArgv(), ['updates', 'status', '--json', '--offline']);
});
