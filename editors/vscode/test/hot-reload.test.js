'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { spawn } = require('node:child_process');
const { HotReload } = require('../hot-reload');
class Child extends EventEmitter {
  constructor() { super(); this.stdout = new EventEmitter(); this.stderr = new EventEmitter(); this.writes = []; this.stdin = Object.assign(new EventEmitter(), { write: (value, callback) => { this.writes.push(value); if (this.writeError) { if (this.writeError.sync) throw this.writeError.error; callback?.(this.writeError.error); } return true; } }); this.killed = false; }
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
test('oversized control response is terminal and bounded', () => {
  let child; const reload = new HotReload(() => (child = new Child()), '/tool/semaprax', '/project/semaprax.toml'); const terminal = [];
  reload.on('terminal', value => terminal.push(value)); reload.start();
  child.stdout.emit('data', Buffer.alloc(8193, 0x61));
  assert.equal(reload.detail().event, 'unknown'); assert.deepEqual(terminal, ['response exceeds its bound']); assert.equal(child.killed, true);
});
test('unexpected child exit makes the active session unknown', () => {
  let child; const reload = new HotReload(() => (child = new Child()), '/tool/semaprax', '/project/semaprax.toml'); const terminal = [];
  reload.on('terminal', value => terminal.push(value)); reload.start(); child.send(row(1, 'started')); child.emit('exit', 9);
  assert.equal(reload.detail().event, 'unknown'); assert.deepEqual(terminal, ['process exited without a clean stop']); assert.equal(child.killed, true);
});
test('dirty editor state is distinct from the retained active revision and history is bounded', () => {
  let child; const reload = new HotReload(() => (child = new Child()), '/tool/semaprax', '/project/semaprax.toml'); reload.start(); child.send(row(1, 'started')); reload.markDirty();
  assert.equal(reload.detail().dirty, true); assert.match(reload.detail().detail, /unsaved source/);
  for (let id = 2; id <= 34; id++) { reload.request('status'); child.send(row(id, 'status')); assert.equal(reload.detail().dirty, true); }
  reload.markSaved(); assert.equal(reload.detail().dirty, false); assert.equal(reload.detail().sourceChanged, true); assert.match(reload.detail().detail, /saved file/);
  assert.ok(reload.detail().history.length <= 32);
});
test('rejects malformed revision or generation and bounds outstanding requests', () => {
  let child; const reload = new HotReload(() => (child = new Child()), '/tool/semaprax', '/project/semaprax.toml'); reload.start(); child.send(row(1, 'started'));
  for (let id = 2; id <= 65; id++) reload.request('status');
  assert.throws(() => reload.request('status'), /queue is full/);
  const invalid = new HotReload(() => (child = new Child()), '/tool/semaprax', '/project/semaprax.toml'); invalid.start(); child.send({ ...row(1, 'started'), generation: -1 });
  assert.equal(invalid.detail().event, 'unknown'); assert.equal(child.killed, true);
});
test('accepts the CLI rejected response without inventing generation or revision fields', () => {
  let child; const reload = new HotReload(() => (child = new Child()), '/tool/semaprax', '/project/semaprax.toml'); reload.start(); child.send({ schema: 'semaprax.hot-reload-control.v1', id: 1, event: 'rejected', message: 'session startup rejected' });
  assert.equal(reload.detail().event, 'rejected'); assert.equal(reload.detail().active, null); assert.equal(child.killed, false);
});
test('renders a source-Agent migration refusal as an explicit unsupported state', () => {
  let child; const reload = new HotReload(() => (child = new Child()), '/tool/semaprax', '/project/semaprax.toml'); reload.start(); child.send({ schema: 'semaprax.hot-reload-control.v1', id: 1, event: 'rejected', message: 'source-Agent development sessions require the authenticated source-live migration adapter' });
  assert.equal(reload.detail().event, 'migration_required'); assert.match(reload.detail().detail, /interpreter sessions only/);
});
test('stop marks an unacknowledged activation unknown before bounded forced termination', () => {
  let child; const reload = new HotReload(() => (child = new Child()), '/tool/semaprax', '/project/semaprax.toml'); const terminal=[]; reload.on('terminal', value => terminal.push(value)); reload.start(); child.send(row(1, 'started')); reload.request('activate'); reload.stop();
  assert.equal(reload.detail().event, 'unknown'); assert.match(reload.detail().detail, /acknowledgement was interrupted/); assert.match(child.writes.at(-1), /"op":"stop"/); assert.equal(child.killed, false); assert.equal(terminal.length, 1);
});

const live = () => { let child; const reload = new HotReload(() => (child = new Child()), '/tool/semaprax', '/project/semaprax.toml'); const terminal = []; reload.on('terminal', value => terminal.push(value)); reload.start(); child.send(row(1, 'started')); return { reload, child, terminal }; };
const epipe = () => Object.assign(new Error('write EPIPE'), { code: 'EPIPE' });
test('stdin error events during status are terminal exactly once and release pending work', () => {
  const { reload, child, terminal } = live(); reload.request('status'); child.stdin.emit('error', epipe()); child.stdin.emit('error', epipe());
  assert.equal(reload.detail().event, 'unknown'); assert.equal(terminal.length, 1); assert.equal(reload.pending.size, 0); assert.equal(child.killed, true);
  assert.throws(() => reload.request('status'), /not started/);
});
test('write callback and synchronous write failures take the same terminal path', () => {
  for (const sync of [false, true]) { const { reload, child, terminal } = live(); child.writeError = { sync, error: epipe() }; reload.request('status');
    assert.equal(reload.detail().event, 'unknown'); assert.equal(terminal.length, 1); assert.equal(reload.pending.size, 0); assert.equal(child.killed, true); }
});
test('a failed activate write stays unknown, is never replayed, and settles once', () => {
  const { reload, child, terminal } = live(); const before = child.writes.length; child.writeError = { sync: false, error: epipe() }; reload.request('activate'); child.stdin.emit('error', epipe());
  assert.equal(reload.detail().event, 'unknown'); assert.match(reload.detail().detail, /active state is unknown/); assert.equal(terminal.length, 1); assert.equal(child.writes.length, before + 1);
  reload.stop(); assert.equal(terminal.length, 1);
});
test('a stdin failure while Stop writes is consumed and a late error does not revive the session', () => {
  const { reload, child, terminal } = live(); child.writeError = { sync: true, error: epipe() }; reload.stop();
  assert.equal(reload.detail().event, 'stopped'); assert.equal(terminal.length, 0); assert.equal(child.killed, true); child.stdin.emit('error', epipe()); assert.equal(reload.detail().event, 'stopped'); assert.equal(terminal.length, 0);
});
test('a superseded epoch error cannot change the new session', () => {
  const children = []; const reload = new HotReload(() => { const c = new Child(); children.push(c); return c; }, '/tool/semaprax', '/project/semaprax.toml'); const terminal = [];
  reload.on('terminal', value => terminal.push(value)); reload.start(); children[0].send(row(1, 'started')); reload.stop(); reload.start(); children[1].send(row(3, 'started'));
  children[0].stdin.emit('error', epipe()); assert.equal(reload.detail().event, 'started'); assert.equal(terminal.length, 0); assert.equal(children[1].killed, false);
});
test('real child with closed stdin and open stdout yields a controlled terminal result', async () => {
  const script = `const fs=require('node:fs');fs.closeSync(0);fs.writeSync(1,JSON.stringify({schema:'semaprax.hot-reload-control.v1',id:1,event:'started',generation:0,active_project_revision:'sha256:'+'a'.repeat(64),terminal_uncertainty:false,watch_state:'watching'})+'\\n');setTimeout(()=>{},3000)`;
  let child; const reload = new HotReload((bin, args, options) => (child = spawn(process.execPath, ['-e', script, ...args], { ...options, cwd: __dirname })), process.execPath, require('node:path').resolve('/project/semaprax.toml'));
  const result = await new Promise((resolve, reject) => { const timer = setTimeout(() => reject(new Error('no terminal event')), 4000);
    reload.on('terminal', message => { clearTimeout(timer); resolve(message); });
    reload.on('status', value => { if (value.event === 'started') setTimeout(() => { for (let i = 0; i < 4; i++) try { reload.request('status'); } catch {} }, 100); });
    reload.start(); });
  assert.equal(reload.detail().event, 'unknown'); assert.match(result, /control channel failed/); child.kill();
});
const frames = n => Array.from({ length: n }, (_, i) => Buffer.from(JSON.stringify({ ...row(i + 1, i ? 'status' : 'started'), terminal_uncertainty: false, watch_state: 'watching' }) + '\n'));
const started = n => { const { reload, child, terminal } = (() => { let c; const r = new HotReload(() => (c = new Child()), '/tool/semaprax', '/project/semaprax.toml'); const t = []; r.on('terminal', v => t.push(v)); r.start(); for (let i = 1; i < n; i++) r.request('status'); return { reload: r, child: c, terminal: t }; })(); return { reload, child, terminal }; };
test('identical wire bytes give identical state under every chunking, including more than 8 KiB in one read', () => {
  const wire = Buffer.concat(frames(48)); assert.ok(wire.length > 8192);
  const chunkings = { whole: [wire], perFrame: frames(48), single: Array.from(wire, (_, i) => wire.subarray(i, i + 1)), mixed: [wire.subarray(0, 100), wire.subarray(100, 5000), wire.subarray(5000)] };
  for (const [name, parts] of Object.entries(chunkings)) { const { reload, child, terminal } = started(48); for (const part of parts) child.stdout.emit('data', part);
    assert.equal(terminal.length, 0, name); assert.equal(reload.pending.size, 0, name); assert.equal(reload.detail().event, 'status', name); assert.equal(child.killed, false, name); }
});
test('CRLF-terminated frames are accepted because CR is JSON whitespace', () => {
  const { reload, child } = started(1); child.stdout.emit('data', Buffer.from(JSON.stringify(row(1, 'started')) + '\r\n')); assert.equal(reload.detail().event, 'started'); assert.equal(reload.pending.size, 0);
});
test('one oversized complete or incomplete frame is terminal with bounded memory', () => {
  for (const make of [() => Buffer.alloc(9000, 0x61), () => Buffer.concat([Buffer.alloc(9000, 0x61), Buffer.from('\n')])]) { const { reload, child, terminal } = started(1);
    child.stdout.emit('data', make()); assert.deepEqual(terminal, ['response exceeds its bound']); assert.equal(reload.frameBytes, 0); assert.equal(child.killed, true); }
  const { child, terminal, reload } = started(1); for (let i = 0; i < 9; i++) child.stdout.emit('data', Buffer.alloc(1000, 0x61)); assert.deepEqual(terminal, ['response exceeds its bound']); assert.equal(reload.frameBytes, 0);
  const exact = started(1); exact.child.stdout.emit('data', Buffer.alloc(8191, 0x61)); assert.equal(exact.terminal.length, 0);
});
test('valid UTF-8 survives every code-point split including supplementary characters and literal U+FFFD', () => {
  const message = 'Straße 🧪 � end'; const bytes = Buffer.from(JSON.stringify({ schema: 'semaprax.hot-reload-control.v1', id: 1, event: 'rejected', message }) + '\n');
  for (let cut = 1; cut < bytes.length; cut++) { const { reload, child, terminal } = started(1); child.stdout.emit('data', bytes.subarray(0, cut)); child.stdout.emit('data', bytes.subarray(cut));
    assert.equal(terminal.length, 0, String(cut)); assert.equal(reload.detail().detail, message, String(cut)); }
});
test('malformed UTF-8 and truncated final frames are explicit transport failures', () => {
  const bad = Buffer.concat([Buffer.from('{"schema":"semaprax.hot-reload-control.v1","id":1,"event":"rejected","message":"'), Buffer.from([0xff]), Buffer.from('"}\n')]);
  const a = started(1); a.child.stdout.emit('data', bad); assert.deepEqual(a.terminal, ['malformed, stale, or unsolicited hot reload response']); assert.equal(a.reload.detail().event, 'unknown');
  const b = started(1); b.child.stdout.emit('data', Buffer.from([0x7b, 0xf0, 0x9f])); b.child.stdout.emit('end'); assert.deepEqual(b.terminal, ['response stream ended inside an unterminated frame']);
  const c = started(1); c.child.stdout.emit('data', Buffer.from(JSON.stringify(row(1, 'started')))); c.child.emit('exit', 0); assert.deepEqual(c.terminal, ['process exited without a clean stop']);
});
test('a failure after a dispatched activation stays non-replayable under framing errors', () => {
  const { reload, child, terminal } = started(1); child.stdout.emit('data', Buffer.from(JSON.stringify(row(1, 'started')) + '\n')); const writes = child.writes.length; reload.request('activate'); child.stdout.emit('data', Buffer.alloc(9000, 0x61));
  assert.equal(terminal.length, 1); assert.equal(reload.detail().event, 'unknown'); assert.equal(child.writes.length, writes + 1); assert.throws(() => reload.request('status'), /not started/);
});
