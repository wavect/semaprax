'use strict';
// Check-on-save diagnostics: the pure half. Nothing here touches VS Code, the
// filesystem or a process; extension.js supplies those through arguments so
// every decision below is testable with `node --test`.
//
// The compiler's `check <subject> --json` prints one JSON object per stdout
// line: {code, severity, message, path, location{line,column,start,end}, help}
// with `path`, `location` and `help` nullable. `line` and `column` are
// one-based; `start`/`end` are byte offsets into the file.
const path = require('node:path');
const { TextDecoder } = require('node:util');
const { SourceIndex, locationRange } = require('./positions');

const MANIFEST = 'semaprax.toml';
const MAX_OUTPUT_BYTES = 4 * 1024 * 1024;
const TIMEOUT_MS = 30 * 1000;
const SEVERITIES = new Set(['error', 'warning']);

// Nearest `semaprax.toml` at or above the saved file's directory, or null.
// `existing` is either a predicate over absolute paths or an iterable of the
// absolute paths that exist, so the walk never reads the filesystem itself.
function findManifest(file, existing) {
  const exists = typeof existing === 'function' ? existing : (set => candidate => set.has(candidate))(new Set(existing));
  let directory = path.dirname(path.resolve(file));
  for (;;) {
    const candidate = path.join(directory, MANIFEST);
    if (exists(candidate)) return candidate;
    const parent = path.dirname(directory);
    if (parent === directory) return null;
    directory = parent;
  }
}

// What `check` is asked to verify for a saved file: its project manifest when
// one is found, otherwise the file alone.
function checkSubject(file, existing) {
  const resolved = path.resolve(file);
  if (path.basename(resolved) === MANIFEST) return resolved;
  return findManifest(resolved, existing) || resolved;
}

// One compiler diagnostic, or null when the value is not one.
function toDiagnosticRow(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  if (typeof value.code !== 'string' || typeof value.message !== 'string' || !SEVERITIES.has(value.severity)) return null;
  const location = value.location && typeof value.location === 'object' && !Array.isArray(value.location) ? value.location : null;
  return {
    code: value.code,
    severity: value.severity,
    message: value.message,
    path: typeof value.path === 'string' && value.path ? value.path : null,
    location: location && Number.isSafeInteger(location.line) && location.line >= 1 && Number.isSafeInteger(location.column) && location.column >= 1
      ? { line: location.line, column: location.column, start: safeOffset(location.start), end: safeOffset(location.end) }
      : null,
    help: typeof value.help === 'string' && value.help ? value.help : null
  };
}

// The `{"status":"verified", …}` record `check --json` prints on the run it
// verified, or null. The compiler names either the checked source `path` or
// the checked project `name`, always with the revision it verified.
function toVerifiedRecord(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  if (value.status !== 'verified' || typeof value.revision !== 'string' || !value.revision) return null;
  const subject = typeof value.path === 'string' && value.path ? { path: value.path } : typeof value.name === 'string' && value.name ? { name: value.name } : null;
  return subject ? { ...subject, revision: value.revision } : null;
}

// Every stdout line of one `check --json` run, classified. A line is a
// diagnostic, the verified record, or malformed: a partial trailing line after
// truncation, an unknown severity, a foreign schema, or plain text. Malformed
// output is counted, never silently dropped, so a caller can refuse to report
// a clean result it did not actually observe.
function parseCheckOutput(text) {
  const diagnostics = [], verified = [];
  let malformed = 0;
  if (typeof text !== 'string') return { diagnostics, verified: null, verifiedCount: 0, malformed };
  for (const line of text.split('\n')) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    let value;
    try { value = JSON.parse(trimmed); } catch { malformed++; continue; }
    const row = toDiagnosticRow(value);
    if (row) { diagnostics.push(row); continue; }
    const record = toVerifiedRecord(value);
    if (record) { verified.push(record); continue; }
    malformed++;
  }
  return { diagnostics, verified: verified[0] || null, verifiedCount: verified.length, malformed };
}

// One compiler diagnostic per stdout line. Lines that are not a JSON object
// with a string `code`, a known `severity` and a string `message` are skipped
// rather than trusted; a partial trailing line after truncation is one such.
// Callers that must not report a clean check use `checkOutcome` instead, which
// refuses output this function would silently discard.
function parseDiagnosticLines(text) {
  return parseCheckOutput(text).diagnostics;
}

// Whether one finished `check --json` run may be believed, and why not.
// `check` exits 0 after printing exactly one verified record and no error, and
// exits 1 after printing at least one error diagnostic and no verified record.
// Every other combination — a killed child, a foreign status, unparsed output,
// an error with status 0, or a verified record with status 1 — is a check
// failure whose diagnostics the editor must not publish as the current truth.
function checkOutcome(result, compiler = 'the selected compiler') {
  const failed = failure => ({ status: 'failed', failure, diagnostics: [], verified: null });
  if (result.error) return failed(`could not start ${compiler}: ${result.error}`);
  if (result.timedOut) return failed(`check timed out after ${TIMEOUT_MS / 1000}s`);
  if (result.truncated) return failed(`check output exceeded ${MAX_OUTPUT_BYTES} bytes`);
  if (result.invalidUtf8) return failed('check output is not valid UTF-8');
  if (result.code !== 0 && result.code !== 1) return failed(`check exited with status ${result.code}`);
  const parsed = parseCheckOutput(result.stdout);
  if (parsed.malformed) return failed(parsed.malformed === 1 ? 'check printed 1 line that is neither a diagnostic nor a verified record' : `check printed ${parsed.malformed} lines that are neither a diagnostic nor a verified record`);
  if (parsed.verifiedCount > 1) return failed(`check printed ${parsed.verifiedCount} verified records`);
  const errors = parsed.diagnostics.filter(row => row.severity === 'error').length;
  if (result.code === 0) {
    if (errors) return failed(`check exited 0 after reporting ${errors} error diagnostic${errors === 1 ? '' : 's'}`);
    if (!parsed.verified) return failed('check exited 0 without printing a verified record');
    return { status: 'verified', failure: null, diagnostics: parsed.diagnostics, verified: parsed.verified };
  }
  if (parsed.verified) return failed('check exited 1 after printing a verified record');
  if (!errors) return failed('check exited 1 without reporting an error diagnostic');
  return { status: 'diagnostics', failure: null, diagnostics: parsed.diagnostics, verified: null };
}

// The bounded stdout bytes as text, or null when they are not whole UTF-8:
// a malformed sequence or a truncated final scalar. A BOM is kept as text,
// exactly as the former lossy conversion kept it.
const machineDecoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
function strictUtf8(bytes) {
  try { return machineDecoder.decode(bytes); } catch { return null; }
}

function safeOffset(value) { return Number.isSafeInteger(value) && value >= 0 ? value : null; }

// Editor-shaped records: absolute `path`, zero-based `range`, and the message
// the user reads. A diagnostic without a path lands on the checked subject;
// one without a location lands at the start of its file.
//
// `sources` maps an absolute path to the exact saved bytes the compiler read
// (a Buffer, a string, a `SourceIndex`, or null when they are unavailable), so
// this module reads no file itself. With the source, the compiler's UTF-8 byte
// span becomes a real multiline UTF-16 range; without it, the fallback is the
// compiler's one-based line and column with the span's byte width, exact on
// ASCII. A bare position is one character wide either way.
function toDiagnosticRecords(rows, subject, cwd = path.dirname(subject), sources = () => null) {
  const indexes = new Map();
  const indexOf = file => {
    if (!indexes.has(file)) {
      const source = sources(file);
      indexes.set(file, source instanceof SourceIndex ? source : source === null || source === undefined ? null : new SourceIndex(source));
    }
    return indexes.get(file);
  };
  return rows.map(row => {
    const file = row.path ? path.resolve(cwd, row.path) : subject;
    return {
      path: file,
      severity: row.severity,
      code: row.code,
      range: locationRange(row.location, indexOf(file)),
      message: row.help ? `${row.code}: ${row.message}\n${row.help}` : `${row.code}: ${row.message}`
    };
  });
}

// Every subject's retained contribution, so a re-check can replace exactly
// what that subject reported. A file may be reported by more than one subject
// (overlapping projects, or a standalone and a project check of one file);
// its published rows are the merge of every retained contribution, and it is
// cleared only when none remains. The merge is deterministic: subjects in
// sorted order, each in the compiler's own order, and a record identical in
// severity, code, message and range to one an earlier subject already
// contributed appears once while both subjects keep owning it.
class DiagnosticLedger {
  constructor() {
    this.contributions = new Map(); // subject -> Map<path, records[]>
    this.owners = new Map(); // path -> Set<subject>
  }
  // Replaces `subject`'s contribution and retires each subject in
  // `options.retire` (an obsolete owner, such as a standalone check of a file a
  // project now owns). Returns { set: Map<path, records[]>, clear: path[] }
  // for exactly the files whose published rows this changes.
  apply(subject, records, options = {}) {
    const affected = new Set();
    const drop = owner => {
      for (const file of this.contributions.get(owner)?.keys() || []) {
        affected.add(file);
        const owners = this.owners.get(file);
        owners.delete(owner);
        if (!owners.size) this.owners.delete(file);
      }
      this.contributions.delete(owner);
    };
    for (const owner of options.retire || []) if (owner !== subject) drop(owner);
    drop(subject);
    const contribution = new Map();
    for (const record of records) {
      if (!contribution.has(record.path)) contribution.set(record.path, []);
      contribution.get(record.path).push(record);
    }
    if (contribution.size) this.contributions.set(subject, contribution);
    for (const file of contribution.keys()) {
      affected.add(file);
      if (!this.owners.has(file)) this.owners.set(file, new Set());
      this.owners.get(file).add(subject);
    }
    const set = new Map(), clear = [];
    for (const file of [...affected].sort()) {
      const rows = this.merged(file);
      if (rows.length) set.set(file, rows); else clear.push(file);
    }
    return { set, clear };
  }
  merged(file) {
    const rows = [], earlier = new Set();
    for (const owner of [...(this.owners.get(file) || [])].sort()) {
      const keys = [];
      for (const record of this.contributions.get(owner).get(file)) {
        const key = JSON.stringify([record.severity, record.code, record.message, record.range]);
        if (earlier.has(key)) continue;
        keys.push(key); rows.push(record);
      }
      for (const key of keys) earlier.add(key);
    }
    return rows;
  }
  // Retires one subject: its files are re-published from the remaining owners
  // or cleared when it was their last.
  release(subject) { return this.apply(subject, []); }
  subjects() { return [...this.contributions.keys()].sort(); }
  // The files `subject` currently contributes to, for staleness checks.
  paths(subject) { return [...(this.contributions.get(subject)?.keys() || [])].sort(); }
}

// Subjects other than `current` whose ownership the routing no longer
// supports: a standalone file a project manifest now owns, or a project whose
// manifest is gone. `existing` is as for `findManifest`. A clean result is
// never by itself a reason to retire an independent subject.
function obsoleteSubjects(subjects, current, existing) {
  const exists = typeof existing === 'function' ? existing : (set => candidate => set.has(candidate))(new Set(existing));
  return [...subjects].filter(subject => subject !== current && (path.basename(subject) === MANIFEST
    ? !exists(subject)
    : checkSubject(subject, exists) !== subject)).sort();
}

// Run one bounded `check <subject> --json`. `spawnFn` is Node's spawn or a
// test double; the child is killed when it exceeds the byte or time budget and
// the result says so instead of returning partial output as truth. Cancellation
// retains ownership until the child is observed to exit, escalating from
// SIGTERM to SIGKILL under a bounded policy, and cleanup cannot replace the
// selected timeout/overflow status.
function runCheck(spawnFn, compiler, subject, options = {}) {
  const maxBytes = options.maxBytes ?? MAX_OUTPUT_BYTES, timeoutMs = options.timeoutMs ?? TIMEOUT_MS;
  return new Promise(resolve => {
    let stdout = [], stderr = [], bytes = 0, settled = false, timedOut = false, truncated = false;
    const child = spawnFn(compiler, ['check', subject, '--json'], {
      shell: false, windowsHide: true, cwd: path.dirname(subject), stdio: ['ignore', 'pipe', 'pipe']
    });
    let killTimer = null;
    const clearKillEscalation = () => { if (killTimer !== null) { clearTimeout(killTimer); killTimer = null; } };
    const escalateKill = () => {
      try { child.kill('SIGKILL'); } catch {}
    };
    const requestKill = () => {
      try { child.kill(); } catch {}
      if (killTimer === null) killTimer = setTimeout(escalateKill, 2000);
    };
    const finish = result => {
      if (settled) return;
      settled = true; clearTimeout(timer); clearKillEscalation();
      // Stdout is protocol data: it is admitted only as strict UTF-8, never
      // replacement-decoded. Stderr stays lossy human text and is never parsed.
      const text = strictUtf8(Buffer.concat(stdout));
      resolve({ stdout: text === null ? '' : text, stderr: Buffer.concat(stderr).toString('utf8'), ...(text === null ? { invalidUtf8: true } : {}), ...result });
    };
    const timer = setTimeout(() => { timedOut = true; requestKill(); }, timeoutMs);
    const collect = sink => chunk => {
      if (truncated || timedOut) return;
      bytes += chunk.length;
      if (bytes > maxBytes) { truncated = true; requestKill(); return; }
      sink.push(chunk);
    };
    child.stdout.on('data', collect(stdout));
    child.stderr.on('data', collect(stderr));
    child.on('error', error => finish({ code: null, timedOut, truncated, error: String(error.message || error) }));
    child.on('close', code => finish({ code: timedOut || truncated ? null : code, timedOut, truncated }));
    if (typeof options.onChild === 'function') options.onChild(child);
  });
}

module.exports = {
  MANIFEST, MAX_OUTPUT_BYTES, TIMEOUT_MS,
  findManifest, checkSubject, parseDiagnosticLines, parseCheckOutput, checkOutcome, toDiagnosticRecords, DiagnosticLedger, obsoleteSubjects, runCheck, SourceIndex
};
