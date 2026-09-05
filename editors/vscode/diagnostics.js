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

function safeOffset(value) { return Number.isSafeInteger(value) && value >= 0 ? value : null; }

// Editor-shaped records: absolute `path`, zero-based `range`, and the message
// the user reads. A diagnostic without a path lands on the checked subject;
// one without a location lands at the start of its file. A span's byte length
// widens the range on its own line; a bare position is one character wide.
function toDiagnosticRecords(rows, subject, cwd = path.dirname(subject)) {
  return rows.map(row => {
    const file = row.path ? path.resolve(cwd, row.path) : subject;
    const line = row.location ? row.location.line - 1 : 0;
    const column = row.location ? row.location.column - 1 : 0;
    const span = row.location && row.location.start !== null && row.location.end !== null && row.location.end > row.location.start
      ? row.location.end - row.location.start : 1;
    return {
      path: file,
      severity: row.severity,
      code: row.code,
      range: { startLine: line, startColumn: column, endLine: line, endColumn: column + span },
      message: row.help ? `${row.code}: ${row.message}\n${row.help}` : `${row.code}: ${row.message}`
    };
  });
}

// Which files hold diagnostics for which checked subject, so a re-check can
// replace exactly the entries it owns and clear the ones that went away.
class DiagnosticLedger {
  constructor() { this.owned = new Map(); }
  // Returns { set: Map<path, records[]>, clear: path[] } for the subject.
  apply(subject, records) {
    const set = new Map();
    for (const record of records) {
      if (!set.has(record.path)) set.set(record.path, []);
      set.get(record.path).push(record);
    }
    const previous = this.owned.get(subject) || new Set();
    const clear = [...previous].filter(file => !set.has(file)).sort();
    if (set.size) this.owned.set(subject, new Set(set.keys())); else this.owned.delete(subject);
    return { set, clear };
  }
  release(subject) { return this.apply(subject, []).clear; }
  subjects() { return [...this.owned.keys()].sort(); }
}

// Run one bounded `check <subject> --json`. `spawnFn` is Node's spawn or a
// test double; the child is killed when it exceeds the byte or time budget and
// the result says so instead of returning partial output as truth.
function runCheck(spawnFn, compiler, subject, options = {}) {
  const maxBytes = options.maxBytes ?? MAX_OUTPUT_BYTES, timeoutMs = options.timeoutMs ?? TIMEOUT_MS;
  return new Promise(resolve => {
    let stdout = [], stderr = [], bytes = 0, settled = false, timedOut = false, truncated = false;
    const child = spawnFn(compiler, ['check', subject, '--json'], {
      shell: false, windowsHide: true, cwd: path.dirname(subject), stdio: ['ignore', 'pipe', 'pipe']
    });
    const finish = result => {
      if (settled) return;
      settled = true; clearTimeout(timer);
      resolve({ stdout: Buffer.concat(stdout).toString('utf8'), stderr: Buffer.concat(stderr).toString('utf8'), ...result });
    };
    const timer = setTimeout(() => { timedOut = true; child.kill(); finish({ code: null, timedOut, truncated }); }, timeoutMs);
    const collect = sink => chunk => {
      bytes += chunk.length;
      if (bytes > maxBytes) { truncated = true; child.kill(); finish({ code: null, timedOut, truncated }); return; }
      sink.push(chunk);
    };
    child.stdout.on('data', collect(stdout));
    child.stderr.on('data', collect(stderr));
    child.on('error', error => finish({ code: null, timedOut, truncated, error: String(error.message || error) }));
    child.on('close', code => finish({ code, timedOut, truncated }));
    if (typeof options.onChild === 'function') options.onChild(child);
  });
}

module.exports = {
  MANIFEST, MAX_OUTPUT_BYTES, TIMEOUT_MS,
  findManifest, checkSubject, parseDiagnosticLines, parseCheckOutput, checkOutcome, toDiagnosticRecords, DiagnosticLedger, runCheck
};
