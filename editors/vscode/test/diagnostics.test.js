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
  assert.deepEqual(ledger.release(other).clear, [other]);
  assert.deepEqual(ledger.subjects(), []);
  assert.deepEqual(ledger.release(other).clear, []);
});

class Child extends EventEmitter {
  constructor() { super(); this.stdout = new EventEmitter(); this.stderr = new EventEmitter(); this.killed = false; }
  kill() { this.killed = true; queueMicrotask(() => this.emit('close', null)); return true; }
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

// Byte offsets are UTF-8; VS Code positions are zero-based lines and UTF-16
// code units, and a span may cross lines. These cases pin the shared mapper
// and the fallback used when the saved source is unavailable.
const { SourceIndex } = require('../positions');

test('a byte span becomes a UTF-16, possibly multiline, editor range', () => {
  const text = '\u{1F600} true\nsecond line';
  const index = new SourceIndex(Buffer.from(text, 'utf8'));
  assert.equal(Buffer.byteLength(text), 21);
  // `true` occupies bytes 5..9; the astral character is two UTF-16 units.
  assert.deepEqual(index.range(5, 9), { startLine: 0, startColumn: 3, endLine: 0, endColumn: 7 });
  // The whole source ends on the second line, not at column 21 of the first.
  assert.deepEqual(index.range(0, 21), { startLine: 0, startColumn: 0, endLine: 1, endColumn: 11 });
  assert.deepEqual(index.position(21), { line: 1, character: 11 });
});

test('the mapper counts CRLF, combining sequences, tabs, and the end of file', () => {
  const crlf = new SourceIndex(Buffer.from('one\r\ntwo\r\n', 'utf8'));
  assert.equal(crlf.lineCount, 3);
  // The `\r` is not line content: an offset inside the break is the line end.
  assert.deepEqual(crlf.position(3), { line: 0, character: 3 });
  assert.deepEqual(crlf.position(4), { line: 0, character: 3 });
  assert.deepEqual(crlf.position(5), { line: 1, character: 0 });
  assert.deepEqual(crlf.position(10), { line: 2, character: 0 });

  // A base plus a combining mark is two UTF-16 units and three UTF-8 bytes.
  const combining = new SourceIndex(Buffer.from('éx', 'utf8'));
  assert.deepEqual(combining.range(0, 3), { startLine: 0, startColumn: 0, endLine: 0, endColumn: 2 });
  assert.deepEqual(combining.range(3, 4), { startLine: 0, startColumn: 2, endLine: 0, endColumn: 3 });

  // A tab is one code unit; the editor renders the width, the mapper does not.
  assert.deepEqual(new SourceIndex('\ta').range(0, 2), { startLine: 0, startColumn: 0, endLine: 0, endColumn: 2 });
  assert.deepEqual(new SourceIndex('').position(0), { line: 0, character: 0 });
});

test('unusable offsets are rejected rather than turned into a wrong range', () => {
  const index = new SourceIndex(Buffer.from('\u{1F600}ab', 'utf8'));
  assert.equal(index.position(2), null, 'an offset inside a code point is not a position');
  assert.equal(index.position(7), null, 'an offset past the saved source is not a position');
  assert.equal(index.position(-1), null);
  assert.equal(index.position(1.5), null);
  assert.equal(index.range(5, 4), null, 'a reversed span is rejected');
  assert.equal(index.range(0, 99), null);
});

test('diagnostic records use the saved source when it is supplied and fall back when it is not', () => {
  const subject = at('app', MANIFEST);
  const main = at('app', 'src', 'main.spx');
  const text = '\u{1F600} true\nsecond line';
  const rows = parseDiagnosticLines([
    line({ code: 'SPX-U1', severity: 'error', message: 'astral', path: 'src/main.spx', location: { line: 1, column: 3, start: 5, end: 9 }, help: null }),
    line({ code: 'SPX-U2', severity: 'error', message: 'multiline', path: 'src/main.spx', location: { line: 1, column: 1, start: 0, end: 21 }, help: null }),
    line({ code: 'SPX-U3', severity: 'error', message: 'torn offset', path: 'src/main.spx', location: { line: 1, column: 1, start: 2, end: 9 }, help: null }),
    line({ code: 'SPX-U4', severity: 'error', message: 'no span', path: 'src/main.spx', location: { line: 2, column: 3 }, help: null })
  ].join(''));
  const sources = file => (file === main ? Buffer.from(text, 'utf8') : null);
  assert.deepEqual(toDiagnosticRecords(rows, subject, path.dirname(subject), sources).map(record => record.range), [
    { startLine: 0, startColumn: 3, endLine: 0, endColumn: 7 },
    { startLine: 0, startColumn: 0, endLine: 1, endColumn: 11 },
    // A torn offset falls back to the compiler's own line and column.
    { startLine: 0, startColumn: 0, endLine: 0, endColumn: 7 },
    { startLine: 1, startColumn: 2, endLine: 1, endColumn: 3 }
  ]);
  // Without the saved source the previous line/column convention is kept.
  assert.deepEqual(toDiagnosticRecords(rows, subject).map(record => record.range), [
    { startLine: 0, startColumn: 2, endLine: 0, endColumn: 6 },
    { startLine: 0, startColumn: 0, endLine: 0, endColumn: 21 },
    { startLine: 0, startColumn: 0, endLine: 0, endColumn: 7 },
    { startLine: 1, startColumn: 2, endLine: 1, endColumn: 3 }
  ]);
  // A prepared index is accepted directly, and an unreadable file is null.
  assert.deepEqual(toDiagnosticRecords(rows, subject, path.dirname(subject), file => (file === main ? new SourceIndex(text) : null))[0].range,
    { startLine: 0, startColumn: 3, endLine: 0, endColumn: 7 });
});

// Machine stdout is admitted only as strict UTF-8 (REF-16). A lossy decoder
// would turn a raw malformed byte into U+FFFD inside an otherwise valid record.
const { TextDecoder: StrictDecoder } = require('node:util');
const rawRecord = (middle, suffix = '.spx') => Buffer.concat([
  Buffer.from('{"status":"verified","path":"/fixture/'), middle, Buffer.from(`${suffix}","revision":"sha256:${'a'.repeat(64)}"}\n`)
]);
async function capture(chunks, close = 0, options = {}) {
  const child = new Child();
  const pending = runCheck(spawnInto([], child), at('bin', 'semaprax'), at('app', MANIFEST), options);
  for (const chunk of chunks) child.stdout.emit('data', chunk);
  if (close !== 'pending') child.emit('close', close);
  return pending;
}

test('raw invalid UTF-8 in a verified record or a diagnostic is a transport failure, not replacement text', async () => {
  const bytes = rawRecord(Buffer.from([0xff]));
  assert.throws(() => new StrictDecoder('utf-8', { fatal: true }).decode(bytes), 'the control decoder rejects the original bytes');
  const result = await capture([bytes]);
  assert.equal(result.invalidUtf8, true);
  assert.ok(!String(result.stdout).includes('�'), 'no replacement-decoded text is returned');
  const outcome = checkOutcome(result, at('bin', 'semaprax'));
  assert.equal(outcome.status, 'failed');
  assert.equal(outcome.failure, 'check output is not valid UTF-8');
  assert.deepEqual(outcome.diagnostics, []);
  assert.equal(outcome.verified, null);

  const diagnostic = Buffer.concat([Buffer.from('{"code":"SPX-P104","severity":"error","message":"bad '), Buffer.from([0xc3, 0x28]), Buffer.from('","path":null,"location":null,"help":null}\n')]);
  assert.equal(checkOutcome(await capture([diagnostic], 1)).failure, 'check output is not valid UTF-8');
});

test('a literal U+FFFD and a multibyte scalar split across chunks stay admissible', async () => {
  const literal = await capture([rawRecord(Buffer.from('�', 'utf8'))]);
  assert.equal(literal.invalidUtf8, undefined);
  assert.equal(checkOutcome(literal).status, 'verified');
  assert.equal(checkOutcome(literal).verified.path, '/fixture/�.spx');

  const emoji = rawRecord(Buffer.from('\u{1F600}', 'utf8'));
  const split = emoji.indexOf(0xf0) + 2;
  const joined = await capture([emoji.subarray(0, split), emoji.subarray(split)]);
  assert.equal(checkOutcome(joined).verified.path, '/fixture/\u{1F600}.spx');
  // A truncated final sequence on ordinary completion is rejected.
  const truncated = await capture([Buffer.concat([Buffer.from(line(verified)), Buffer.from([0xf0, 0x9f])])]);
  assert.equal(checkOutcome(truncated).failure, 'check output is not valid UTF-8');
  // Ordinary error and warning-plus-verified streams are unchanged.
  assert.equal(checkOutcome(await capture([Buffer.from(warningLine + line(verified))])).status, 'verified');
  assert.equal(checkOutcome(await capture([Buffer.from(errorLine)], 1)).status, 'diagnostics');
});

test('timeout, byte-cap and spawn failures keep their classification with incomplete trailing UTF-8', async () => {
  const partial = Buffer.from([0x7b, 0xe2, 0x82]);
  const capped = await capture([partial, Buffer.alloc(20, 0x20)], 'pending', { maxBytes: 16 });
  assert.equal(checkOutcome(capped).failure, `check output exceeded ${MAX_OUTPUT_BYTES} bytes`);
  const slow = await capture([partial], 'pending', { timeoutMs: 5 });
  assert.equal(checkOutcome(slow).failure, `check timed out after ${TIMEOUT_MS / 1000}s`);
  const child = new Child();
  const pending = runCheck(spawnInto([], child), at('bin', 'missing'), at('app', MANIFEST));
  child.stdout.emit('data', partial);
  child.emit('error', new Error('spawn ENOENT'));
  assert.equal(checkOutcome(await pending, 'x').failure, 'could not start x: spawn ENOENT');
});

test('a check whose output fails to decode retains the published diagnostics', async () => {
  const subject = at('app', MANIFEST);
  const ledger = new DiagnosticLedger();
  await publish(ledger, new Child(), errorLine, 1, subject);
  const outcome = checkOutcome(await capture([rawRecord(Buffer.from([0xff]))]));
  assert.ok(outcome.failure);
  assert.deepEqual(ledger.subjects(), [subject], 'a decode failure is not a clean result');
});

// Overlapping check subjects (REF-15). A file may be reported by more than one
// subject; each subject's contribution is retained, and a file leaves the
// collection only when no retained contribution remains.
const { obsoleteSubjects } = require('../diagnostics');
const row = (file, code, extra = {}) => ({ path: file, severity: 'error', code, range: { startLine: 0, startColumn: 0, endLine: 0, endColumn: 1 }, message: code, ...extra });
// The extension's publication loop over an in-memory collection.
function publishInto(collection, update) {
  for (const file of update.clear) collection.delete(file);
  for (const [file, rows] of update.set) collection.set(file, rows.map(item => item.code));
  return collection;
}

test('a clean result for one subject keeps another subject\'s diagnostic on a shared file', () => {
  const ledger = new DiagnosticLedger(), collection = new Map();
  const shared = '/fixture/shared.spx', A = '/fixture/a/semaprax.toml', B = '/fixture/b/semaprax.toml';
  publishInto(collection, ledger.apply(A, [row(shared, 'A-error')]));
  publishInto(collection, ledger.apply(B, [row(shared, 'B-error')]));
  assert.deepEqual(collection.get(shared), ['A-error', 'B-error'], 'distinct contributions coexist');
  publishInto(collection, ledger.apply(A, []));
  assert.deepEqual(collection.get(shared), ['B-error'], 'B still owns the file');
  assert.deepEqual(ledger.paths(B), [shared]);
  assert.deepEqual(ledger.paths(A), []);
  publishInto(collection, ledger.apply(B, []));
  assert.equal(collection.has(shared), false, 'the last owner clears the file');
});

test('merged contributions do not depend on completion order and exact duplicates appear once', () => {
  const shared = '/fixture/shared.spx', A = '/fixture/a.spx', B = '/fixture/b.spx';
  const merged = order => {
    const ledger = new DiagnosticLedger(), collection = new Map();
    for (const [subject, rows] of order) publishInto(collection, ledger.apply(subject, rows));
    return collection.get(shared);
  };
  const a = [row(shared, 'X'), row(shared, 'A-only')], b = [row(shared, 'B-only'), row(shared, 'X')];
  assert.deepEqual(merged([[A, a], [B, b]]), merged([[B, b], [A, a]]));
  assert.deepEqual(merged([[A, a], [B, b]]), ['X', 'A-only', 'B-only']);
  // Same code, different severity/message/range: not collapsed.
  const near = [row(shared, 'X', { severity: 'warning' }), row(shared, 'X', { message: 'other' }), row(shared, 'X', { range: { startLine: 1, startColumn: 0, endLine: 1, endColumn: 1 } })];
  assert.equal(merged([[A, [row(shared, 'X')]], [B, near]]).length, 4);
  // An identical record keeps both ownership associations.
  const ledger = new DiagnosticLedger(), collection = new Map();
  publishInto(collection, ledger.apply(A, [row(shared, 'X')]));
  publishInto(collection, ledger.apply(B, [row(shared, 'X')]));
  assert.deepEqual(collection.get(shared), ['X']);
  publishInto(collection, ledger.apply(A, []));
  assert.deepEqual(collection.get(shared), ['X'], 'B\'s identical record survives A becoming clean');
});

test('releasing one subject does not clear another; releasing the last owner clears the file', () => {
  const ledger = new DiagnosticLedger(), collection = new Map();
  const shared = '/fixture/shared.spx', own = '/fixture/a-only.spx', A = '/fixture/a.spx', B = '/fixture/b.spx';
  publishInto(collection, ledger.apply(A, [row(shared, 'A'), row(own, 'A2')]));
  publishInto(collection, ledger.apply(B, [row(shared, 'B')]));
  const released = ledger.release(A);
  assert.deepEqual(released.clear, [own]);
  publishInto(collection, released);
  assert.deepEqual(collection.get(shared), ['B']);
  assert.equal(collection.has(own), false);
  publishInto(collection, ledger.release(B));
  assert.equal(collection.size, 0);
  assert.deepEqual(ledger.subjects(), []);
});

test('a standalone-to-project transition retires the obsolete contribution without hiding current ones', () => {
  const ledger = new DiagnosticLedger(), collection = new Map();
  const file = '/fixture/app/src/main.spx', manifest = '/fixture/app/semaprax.toml', other = '/fixture/lone/x.spx';
  publishInto(collection, ledger.apply(file, [row(file, 'STANDALONE')]));
  publishInto(collection, ledger.apply(other, [row(file, 'OTHER')]));
  // A manifest now owns the file: its standalone subject is obsolete, the
  // independent standalone `other` is not.
  const existing = new Set([manifest, file, other]);
  const retire = obsoleteSubjects(ledger.subjects(), manifest, existing);
  assert.deepEqual(retire, [file]);
  publishInto(collection, ledger.apply(manifest, [row(file, 'PROJECT')], { retire }));
  // Contributions merge in subject order, not completion order.
  assert.deepEqual(collection.get(file), ['PROJECT', 'OTHER']);
  assert.deepEqual(ledger.subjects(), [manifest, other].sort());
  // The manifest is removed: the project subject is retired on the next check
  // of the now-standalone file, whatever that check reports.
  const after = obsoleteSubjects(ledger.subjects(), file, new Set([file, other]));
  assert.deepEqual(after, [manifest]);
  publishInto(collection, ledger.apply(file, [], { retire: after }));
  assert.deepEqual(collection.get(file), ['OTHER']);
  assert.deepEqual(ledger.subjects(), [other]);
});
