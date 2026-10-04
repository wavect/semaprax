'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { HotReload } = require('../hot-reload');
class Child extends EventEmitter {
  constructor() { super(); this.stdout = new EventEmitter(); this.stderr = new EventEmitter(); this.writes = []; this.stdin = { write: value => { this.writes.push(value); return true; } }; this.killed = false; }
  kill() { this.killed = true; }
  send(value) { this.stdout.emit('data', Buffer.from(JSON.stringify(value) + '\n')); }
}
const row = (id, event = 'status') => ({ schema: 'semaprax.hot-reload-control.v1', id, event, generation: 0, active_project_revision: 'sha256:' + 'a'.repeat(64) });
test('explicit interpreter startup uses direct bounded JSONL and accepts fragmented responses', () => {
  let child; const reload = new HotReload((bin, args, options) => { child = new Child(); assert.equal(bin, '/tool/semaprax'); assert.deepEqual(args, ['dev', '/project/semaprax.toml', '--jsonl']); assert.equal(options.shell, false); return child; }, '/tool/semaprax', '/project/semaprax.toml');
  const seen = []; reload.on('status', value => seen.push(value)); reload.start();
  assert.match(child.writes[0], /"op":"start"/);
  const bytes = Buffer.from(JSON.stringify(row(1, 'started')) + '\n'); child.stdout.emit('data', bytes.subarray(0, 9)); child.stdout.emit('data', bytes.subarray(9));
  assert.equal(seen[0].event, 'started'); reload.stop(); assert.equal(child.killed, true);
});
test('malformed output and late output after Stop cannot revive the session', () => {
  let child; const reload = new HotReload(() => (child = new Child()), '/tool/semaprax', '/project/semaprax.toml'); const terminal = []; reload.on('terminal', value => terminal.push(value)); reload.start();
  child.stdout.emit('data', Buffer.from('{bad}\n')); assert.equal(child.killed, true); assert.deepEqual(terminal, ['malformed, stale, or unsolicited hot reload response']);
  const stopped = new HotReload(() => (child = new Child()), '/tool/semaprax', '/project/semaprax.toml'); stopped.start(); stopped.stop(); child.send(row(1, 'activated')); assert.equal(stopped.detail().event, 'stopped');
});
test('dirty editor state is distinct from the retained active revision and history is bounded', () => {
  let child; const reload = new HotReload(() => (child = new Child()), '/tool/semaprax', '/project/semaprax.toml'); reload.start(); child.send(row(1, 'started')); reload.markDirty();
  assert.equal(reload.detail().dirty, true); assert.match(reload.detail().detail, /older than the editor/);
  for (let id = 2; id <= 34; id++) { reload.request('status'); child.send(row(id, 'status')); }
  assert.ok(reload.detail().history.length <= 32);
});
