'use strict';
// Check-on-save mapping checks. No compiler or VS Code process is started; the
// spawn seam receives a scripted child so the byte and time bounds are exact.
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { EventEmitter } = require('node:events');
const {
  MANIFEST, MAX_OUTPUT_BYTES, TIMEOUT_MS,
  findManifest, checkSubject, parseDiagnosticLines, parseCheckOutput, checkOutcome, toDiagnosticRecords, DiagnosticLedger, runCheck
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

// A run whose output the adapter cannot classify must never be published as a
// clean check. `checkOutcome` is the single decision the extension consults
// before it touches the ledger, so these cases pin the whole matrix.
const verified = { status: 'verified', path: '/work/app/src/main.spx', revision: 'sha256:' + 'a'.repeat(64) };
const errorLine = line({ code: 'SPX-P104', severity: 'error', message: 'expected `module`', path: 'src/main.spx', location: { line: 1, column: 1, start: 0, end: 6 }, help: null });
const warningLine = line({ code: 'SPX-J102', severity: 'warning', message: 'manifest missing', path: null, location: null, help: null });
const run = (code, stdout, extra = {}) => ({ code, stdout, stderr: '', timedOut: false, truncated: false, ...extra });

test('output is classified into diagnostics, the verified record, and malformed lines', () => {
  const parsed = parseCheckOutput([warningLine, line(verified), 'not json\n', line({ status: 'verified' }), line({ code: 'X', severity: 'fatal', message: 'm' })].join(''));
  assert.equal(parsed.diagnostics.length, 1);
  assert.deepEqual(parsed.verified, { path: verified.path, revision: verified.revision });
  assert.equal(parsed.verifiedCount, 1);
  assert.equal(parsed.malformed, 3);
  assert.deepEqual(parseCheckOutput(line({ status: 'verified', name: 'calculator', revision: verified.revision })).verified, { name: 'calculator', revision: verified.revision });
  assert.deepEqual(parseCheckOutput(''), { diagnostics: [], verified: null, verifiedCount: 0, malformed: 0 });
  // The lenient reader stays available for callers that only want the rows.
  assert.equal(parseDiagnosticLines([warningLine, 'not json\n'].join('')).length, 1);
});

test('an ordinary warning/success stream and an ordinary error stream stay usable', () => {
  const clean = checkOutcome(run(0, warningLine + line(verified)), at('bin', 'semaprax'));
  assert.equal(clean.status, 'verified');
  assert.equal(clean.failure, null);
  assert.equal(clean.diagnostics.length, 1);
  assert.deepEqual(clean.verified, { path: verified.path, revision: verified.revision });

  const failing = checkOutcome(run(1, warningLine + errorLine), at('bin', 'semaprax'));
  assert.equal(failing.status, 'diagnostics');
  assert.equal(failing.failure, null);
  assert.equal(failing.diagnostics.length, 2);
  assert.equal(failing.verified, null);
});

test('exit 1 with empty, malformed, or success-only output is a check failure', () => {
  for (const [stdout, expected] of [
    ['', 'check exited 1 without reporting an error diagnostic'],
    ['{broken json\n', 'check printed 1 line that is neither a diagnostic nor a verified record'],
    [warningLine, 'check exited 1 without reporting an error diagnostic'],
    [line(verified), 'check exited 1 after printing a verified record']
  ]) {
    const outcome = checkOutcome(run(1, stdout), at('bin', 'semaprax'));
    assert.equal(outcome.status, 'failed', stdout);
    assert.equal(outcome.failure, expected);
    assert.deepEqual(outcome.diagnostics, []);
  }
});

test('exit 0 with malformed output or an error diagnostic cannot report verification', () => {
  for (const [stdout, expected] of [
    ['{broken json\n', 'check printed 1 line that is neither a diagnostic nor a verified record'],
    ['', 'check exited 0 without printing a verified record'],
    [warningLine, 'check exited 0 without printing a verified record'],
    [errorLine + line(verified), 'check exited 0 after reporting 1 error diagnostic'],
    [line(verified) + line(verified), 'check printed 2 verified records'],
    [line(verified) + '{"partial":\n', 'check printed 1 line that is neither a diagnostic nor a verified record']
  ]) {
    const outcome = checkOutcome(run(0, stdout), at('bin', 'semaprax'));
    assert.equal(outcome.status, 'failed', stdout);
    assert.equal(outcome.failure, expected);
  }
});

test('a killed, unstartable, or foreign-status child is a failure whatever it printed', () => {
  const binary = at('bin', 'semaprax');
  assert.equal(checkOutcome(run(null, line(verified), { timedOut: true }), binary).failure, `check timed out after ${TIMEOUT_MS / 1000}s`);
  assert.equal(checkOutcome(run(null, line(verified), { truncated: true }), binary).failure, `check output exceeded ${MAX_OUTPUT_BYTES} bytes`);
  assert.equal(checkOutcome(run(null, '', { error: 'spawn ENOENT' }), binary).failure, `could not start ${binary}: spawn ENOENT`);
  assert.equal(checkOutcome(run(101, line(verified)), binary).failure, 'check exited with status 101');
  assert.equal(checkOutcome(run(2, errorLine), binary).failure, 'check exited with status 2');
});

// The composition the extension performs: run the child, classify it, and only
// then let the ledger replace what the subject owns.
async function publish(ledger, child, stdout, close, subject) {
  const pending = runCheck(spawnInto([], child), at('bin', 'semaprax'), subject);
  if (stdout) child.stdout.emit('data', Buffer.from(stdout));
  if (close !== null) child.emit('close', close);
  const outcome = checkOutcome(await pending, at('bin', 'semaprax'));
  if (outcome.failure) return { failure: outcome.failure, retained: ledger.subjects().includes(subject) };
  return { update: ledger.apply(subject, toDiagnosticRecords(outcome.diagnostics, subject)) };
}

test('a failed check never clears the diagnostics the previous check published', async () => {
  const subject = at('app', MANIFEST);
  const ledger = new DiagnosticLedger();
  const first = await publish(ledger, new Child(), errorLine, 1, subject);
  assert.deepEqual([...first.update.set.keys()], [at('app', 'src', 'main.spx')]);
  assert.deepEqual(ledger.subjects(), [subject]);

  for (const [stdout, close] of [['{broken json\n', 1], ['', 1], [line(verified), 1], ['{broken json\n', 0], ['', 0]]) {
    const outcome = await publish(ledger, new Child(), stdout, close, subject);
    assert.ok(outcome.failure, `${close} ${stdout}`);
    assert.equal(outcome.retained, true);
    assert.deepEqual(ledger.subjects(), [subject], 'the ledger must still own the subject');
  }

  // Only a believable verified run clears them.
  const cleared = await publish(ledger, new Child(), line(verified), 0, subject);
  assert.equal(cleared.failure, undefined);
  assert.deepEqual(cleared.update.clear, [at('app', 'src', 'main.spx')]);
  assert.deepEqual(ledger.subjects(), []);
});

test('a superseded check resolves before classification and leaves the ledger alone', async () => {
  const subject = at('app', MANIFEST);
  const ledger = new DiagnosticLedger();
  await publish(ledger, new Child(), errorLine, 1, subject);
  const running = new Map();
  const stale = new Child(), fresh = new Child();
  let own;
  const pending = runCheck(spawnInto([], stale), at('bin', 'semaprax'), subject, { onChild: child => { own = child; running.set(subject, child); } });
  running.set(subject, fresh);
  stale.emit('close', 0);
  const result = await pending;
  assert.notEqual(running.get(subject), own);
  assert.deepEqual(ledger.subjects(), [subject]);
  assert.equal(result.stdout, '');
});
