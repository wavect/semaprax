'use strict';
// Check-on-save mapping checks. No compiler or VS Code process is started; the
// spawn seam receives a scripted child so the byte and time bounds are exact.
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { EventEmitter } = require('node:events');
const {
  MANIFEST, MAX_OUTPUT_BYTES, TIMEOUT_MS,
  findManifest, checkSubject, parseDiagnosticLines, toDiagnosticRecords, DiagnosticLedger, runCheck
} = require('../diagnostics');

const root = path.resolve(path.sep, 'work');
const at = (...parts) => path.join(root, ...parts);
const line = value => JSON.stringify(value) + '\n';

test('manifest discovery walks up from the saved file and stops at the nearest manifest', () => {
  const existing = [at('app', MANIFEST), at('app', 'vendor', 'lib', MANIFEST), at('other', 'semaprax.toml.bak')];
  assert.equal(findManifest(at('app', 'src', 'deep', 'main.spx'), existing), at('app', MANIFEST));
  assert.equal(findManifest(at('app', 'vendor', 'lib', 'x.spx'), existing), at('app', 'vendor', 'lib', MANIFEST));
  assert.equal(findManifest(at('app', 'vendor', 'x.spx'), existing), at('app', MANIFEST));
  assert.equal(findManifest(at('other', 'x.spx'), existing), null);
  assert.equal(findManifest(path.join(path.sep, 'x.spx'), existing), null);
  assert.equal(findManifest(at('app', 'x.spx'), candidate => candidate === at('app', MANIFEST)), at('app', MANIFEST));
});

test('the check subject is the manifest when found and the file itself otherwise', () => {
  assert.equal(checkSubject(at('app', 'src', 'main.spx'), [at('app', MANIFEST)]), at('app', MANIFEST));
  assert.equal(checkSubject(at('lone', 'main.spx'), []), at('lone', 'main.spx'));
  assert.equal(checkSubject(at('app', MANIFEST), []), at('app', MANIFEST));
});

test('malformed stdout lines are skipped and well-formed ones keep every field', () => {
  const good = { code: 'SPX-P104', severity: 'error', message: 'expected `module`', path: 'src/main.spx', location: { line: 3, column: 7, start: 20, end: 26 }, help: 'start the file with `module name;`' };
  const text = [
    'not json',
    line([good]),
    line({ severity: 'error', message: 'no code' }),
    line({ code: 'SPX-X', severity: 'fatal', message: 'unknown severity' }),
    line({ code: 'SPX-X', severity: 'warning' }),
    line(null),
    line(good),
    line({ code: 'SPX-J102', severity: 'warning', message: 'manifest missing', path: null, location: null, help: null }),
    line({ code: 'SPX-B', severity: 'error', message: 'bad location', path: 'a.spx', location: { line: 0, column: 1 }, help: 42 }),
    line({ code: 'SPX-C', severity: 'error', message: 'bad offsets', path: 'a.spx', location: { line: 2, column: 2, start: -1, end: 'x' } }),
    '{"code":"SPX-T","severity":"error","message":"trunc'
  ].join('');
  const rows = parseDiagnosticLines(text);
  assert.deepEqual(rows, [
    { code: 'SPX-P104', severity: 'error', message: 'expected `module`', path: 'src/main.spx', location: { line: 3, column: 7, start: 20, end: 26 }, help: 'start the file with `module name;`' },
    { code: 'SPX-J102', severity: 'warning', message: 'manifest missing', path: null, location: null, help: null },
    { code: 'SPX-B', severity: 'error', message: 'bad location', path: 'a.spx', location: null, help: null },
    { code: 'SPX-C', severity: 'error', message: 'bad offsets', path: 'a.spx', location: { line: 2, column: 2, start: null, end: null }, help: null }
  ]);
  assert.deepEqual(parseDiagnosticLines(''), []);
  assert.deepEqual(parseDiagnosticLines(undefined), []);
});

test('records map severity, zero-based ranges, span widths, subject fallback and appended help', () => {
  const subject = at('app', MANIFEST);
  const rows = parseDiagnosticLines([
    line({ code: 'SPX-P104', severity: 'error', message: 'expected `module`', path: 'src/main.spx', location: { line: 3, column: 7, start: 20, end: 26 }, help: 'start with `module name;`' }),
    line({ code: 'SPX-W1', severity: 'warning', message: 'unused', path: at('app', 'src', 'lib.spx'), location: { line: 1, column: 1 }, help: null }),
    line({ code: 'SPX-J102', severity: 'error', message: 'manifest problem', path: null, location: null, help: null }),
    line({ code: 'SPX-Z', severity: 'error', message: 'inverted span', path: 'z.spx', location: { line: 5, column: 4, start: 9, end: 3 } })
  ].join(''));
  const records = toDiagnosticRecords(rows, subject);
  assert.deepEqual(records, [
    { path: at('app', 'src', 'main.spx'), severity: 'error', code: 'SPX-P104', range: { startLine: 2, startColumn: 6, endLine: 2, endColumn: 12 }, message: 'SPX-P104: expected `module`\nstart with `module name;`' },
    { path: at('app', 'src', 'lib.spx'), severity: 'warning', code: 'SPX-W1', range: { startLine: 0, startColumn: 0, endLine: 0, endColumn: 1 }, message: 'SPX-W1: unused' },
    { path: subject, severity: 'error', code: 'SPX-J102', range: { startLine: 0, startColumn: 0, endLine: 0, endColumn: 1 }, message: 'SPX-J102: manifest problem' },
    { path: at('app', 'z.spx'), severity: 'error', code: 'SPX-Z', range: { startLine: 4, startColumn: 3, endLine: 4, endColumn: 4 }, message: 'SPX-Z: inverted span' }
  ]);
  const lone = toDiagnosticRecords(parseDiagnosticLines(line({ code: 'SPX-P104', severity: 'error', message: 'm', path: 'main.spx', location: { line: 1, column: 2 } })), at('lone', 'main.spx'));
  assert.equal(lone[0].path, at('lone', 'main.spx'));
});

test('the ledger replaces exactly the files a subject owned and clears the stale ones', () => {
  const ledger = new DiagnosticLedger();
  const subject = at('app', MANIFEST), other = at('lone', 'main.spx');
  const record = (file, code) => ({ path: file, severity: 'error', code, range: { startLine: 0, startColumn: 0, endLine: 0, endColumn: 1 }, message: code });
  let update = ledger.apply(subject, [record(at('app', 'a.spx'), 'A1'), record(at('app', 'b.spx'), 'B1'), record(at('app', 'a.spx'), 'A2')]);
  assert.deepEqual([...update.set.keys()].sort(), [at('app', 'a.spx'), at('app', 'b.spx')]);
  assert.equal(update.set.get(at('app', 'a.spx')).length, 2);
  assert.deepEqual(update.clear, []);
  ledger.apply(other, [record(other, 'L1')]);

  update = ledger.apply(subject, [record(at('app', 'b.spx'), 'B2')]);
  assert.deepEqual([...update.set.keys()], [at('app', 'b.spx')]);
  assert.deepEqual(update.clear, [at('app', 'a.spx')]);
  assert.deepEqual(ledger.subjects(), [subject, other].sort());

  update = ledger.apply(subject, []);
  assert.equal(update.set.size, 0);
  assert.deepEqual(update.clear, [at('app', 'b.spx')]);
  assert.deepEqual(ledger.subjects(), [other]);
  assert.deepEqual(ledger.release(other), [other]);
  assert.deepEqual(ledger.subjects(), []);
  assert.deepEqual(ledger.release(other), []);
});

class Child extends EventEmitter {
  constructor() { super(); this.stdout = new EventEmitter(); this.stderr = new EventEmitter(); this.killed = false; }
  kill() { this.killed = true; return true; }
}
const spawnInto = (calls, child) => (command, args, options) => { calls.push({ command, args, options }); return child; };

test('a check spawns the selected binary directly with check <subject> --json and no shell', async () => {
  const calls = [], child = new Child(), subject = at('app', MANIFEST);
  const pending = runCheck(spawnInto(calls, child), at('bin', 'semaprax'), subject);
  assert.deepEqual(calls, [{ command: at('bin', 'semaprax'), args: ['check', subject, '--json'], options: { shell: false, windowsHide: true, cwd: at('app'), stdio: ['ignore', 'pipe', 'pipe'] } }]);
  child.stdout.emit('data', Buffer.from('{"code":"SPX-P104","severity":"error",'));
  child.stdout.emit('data', Buffer.from('"message":"m","path":"a.spx","location":null,"help":null}\n'));
  child.stderr.emit('data', Buffer.from('note\n'));
  child.emit('close', 1);
  const result = await pending;
  assert.equal(result.code, 1); assert.equal(result.timedOut, false); assert.equal(result.truncated, false);
  assert.equal(result.stderr, 'note\n'); assert.equal(child.killed, false);
  assert.equal(parseDiagnosticLines(result.stdout).length, 1);
});

test('output beyond the byte budget kills the child and discards the partial stream', async () => {
  const calls = [], child = new Child();
  const pending = runCheck(spawnInto(calls, child), at('bin', 'semaprax'), at('app', MANIFEST), { maxBytes: 16 });
  child.stdout.emit('data', Buffer.alloc(10, 0x20));
  child.stderr.emit('data', Buffer.alloc(7, 0x20));
  const result = await pending;
  assert.equal(result.truncated, true); assert.equal(result.code, null); assert.equal(child.killed, true);
  child.emit('close', 0);
  assert.equal(MAX_OUTPUT_BYTES, 4 * 1024 * 1024);
});

test('a silent child is killed at the deadline and reported as timed out', async () => {
  const calls = [], child = new Child();
  const result = await runCheck(spawnInto(calls, child), at('bin', 'semaprax'), at('app', MANIFEST), { timeoutMs: 5 });
  assert.equal(result.timedOut, true); assert.equal(result.code, null); assert.equal(child.killed, true);
  assert.equal(TIMEOUT_MS, 30 * 1000);
});

test('a spawn failure resolves with its error instead of throwing into the save handler', async () => {
  const calls = [], child = new Child();
  const pending = runCheck(spawnInto(calls, child), at('bin', 'missing'), at('app', MANIFEST));
  child.emit('error', new Error('spawn ENOENT'));
  const result = await pending;
  assert.equal(result.error, 'spawn ENOENT'); assert.equal(result.code, null);
});
