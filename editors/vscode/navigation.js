'use strict';
// Navigation by meaning: the pure half. Nothing here touches VS Code, the
// filesystem or a process; extension.js supplies those through arguments so
// every decision below is testable with `node --test`.
//
// The compiler's `query <file> --json` prints one `semaprax.query.v1` line:
// {schema, module, revision, filters, matches:[{kind, id, name, persistent,
// signature, location{line,column,start,end}, effects, calls, called_by}]}.
// `line` and `column` are one-based and name the declaration's own name
// token; `start`/`end` are byte offsets of that token. `doc <file>` prints the
// module's Markdown documentation.
const path = require('node:path');

const MAX_OUTPUT_BYTES = 4 * 1024 * 1024;
const TIMEOUT_MS = 30 * 1000;
const QUERY_SCHEMA = 'semaprax.query.v1';

// The exact argument vector for one bounded declaration query. Only the
// filters navigation needs are exposed; each is a plain string.
function queryArguments(file, filters = {}) {
  const args = ['query', file];
  if (typeof filters.kind === 'string' && filters.kind) args.push('--kind', filters.kind);
  if (typeof filters.calls === 'string' && filters.calls) args.push('--calls', filters.calls);
  if (typeof filters.calledBy === 'string' && filters.calledBy) args.push('--called-by', filters.calledBy);
  args.push('--json');
  return args;
}

function docArguments(file) { return ['doc', file]; }

function safeOffset(value) { return Number.isSafeInteger(value) && value >= 0 ? value : null; }

function strings(value) {
  return Array.isArray(value) && value.every(item => typeof item === 'string') ? value.slice() : null;
}

// One validated match, or null when any field is not what the compiler prints.
function parseMatch(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const { kind, id, name, signature, location } = value;
  if ([kind, id, name, signature].some(field => typeof field !== 'string') || !id) return null;
  if (!location || typeof location !== 'object' || Array.isArray(location)) return null;
  if (!Number.isSafeInteger(location.line) || location.line < 1 || !Number.isSafeInteger(location.column) || location.column < 1) return null;
  const effects = strings(value.effects), calls = strings(value.calls), calledBy = strings(value.called_by);
  if (!effects || !calls || !calledBy) return null;
  return {
    kind, id, name, signature,
    persistent: value.persistent === true,
    location: { line: location.line, column: location.column, start: safeOffset(location.start), end: safeOffset(location.end) },
    effects, calls, calledBy
  };
}

// The whole query result, or null when the document is not a query result.
// Malformed matches are dropped rather than trusted.
function parseQueryResult(text) {
  if (typeof text !== 'string') return null;
  let value;
  try { value = JSON.parse(text.trim()); } catch { return null; }
  if (!value || typeof value !== 'object' || Array.isArray(value) || value.schema !== QUERY_SCHEMA) return null;
  if (typeof value.module !== 'string' || typeof value.revision !== 'string' || !Array.isArray(value.matches)) return null;
  return { module: value.module, revision: value.revision, matches: value.matches.map(parseMatch).filter(Boolean) };
}

// Zero-based editor range of the declaration's name token. A span's byte
// length widens the range on its own line; a bare position is one wide.
function toRange(location) {
  const line = location.line - 1, column = location.column - 1;
  const width = location.start !== null && location.end !== null && location.end > location.start ? location.end - location.start : 1;
  return { startLine: line, startColumn: column, endLine: line, endColumn: column + width };
}

// The first signature line that is not an `@id` attribute.
function header(signature) {
  return signature.split('\n').find(line => line.trim() && !line.trim().startsWith('@id(')) || '';
}

function byPosition(first, second) {
  return first.location.line - second.location.line || first.location.column - second.location.column;
}

// Quick-pick records for every declaration of the module, in source order.
function declarationItems(result) {
  return result.matches.slice().sort(byPosition).map(match => ({
    label: match.name,
    description: `${match.kind} · ${match.id}`,
    detail: header(match.signature),
    id: match.id,
    kind: match.kind,
    range: toRange(match.location)
  }));
}

// Quick-pick records for the callers a `--calls <target>` query returned.
function referenceItems(result, target) {
  return declarationItems(result).map(item => ({ ...item, description: `${item.kind} · ${item.id} · calls ${target}` }));
}

function contractCounts(signature) {
  let requires = 0, ensures = 0;
  for (const line of signature.split('\n')) {
    const trimmed = line.trim();
    if (trimmed.startsWith('requires ')) requires++;
    else if (trimmed.startsWith('ensures ')) ensures++;
  }
  return { requires, ensures };
}

// Code-lens records above every declaration: its stable identity, its
// effects when it uses any, and its contract counts when it declares any.
function lensRecords(result) {
  const lenses = [];
  for (const match of result.matches.slice().sort(byPosition)) {
    const range = toRange(match.location);
    lenses.push({ range, title: `${match.persistent ? '@id' : 'auto id'} ${match.id}` });
    if (match.effects.length) lenses.push({ range, title: `uses { ${match.effects.join(', ')} }` });
    const { requires, ensures } = contractCounts(match.signature);
    if (requires || ensures) lenses.push({ range, title: `requires ${requires} · ensures ${ensures}` });
  }
  return lenses;
}

// Run one bounded read-only compiler command. `spawnFn` is Node's spawn or a
// test double; the child is killed when it exceeds the byte or time budget and
// the result says so instead of returning partial output as truth.
function runCommand(spawnFn, compiler, args, cwd, options = {}) {
  const maxBytes = options.maxBytes ?? MAX_OUTPUT_BYTES, timeoutMs = options.timeoutMs ?? TIMEOUT_MS;
  return new Promise(resolve => {
    let stdout = [], stderr = [], bytes = 0, settled = false, timedOut = false, truncated = false;
    const child = spawnFn(compiler, args, { shell: false, windowsHide: true, cwd, stdio: ['ignore', 'pipe', 'pipe'] });
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

// The reason a run cannot be trusted, or null when it exited with status zero.
function failureReason(result, compiler) {
  if (result.error) return `could not start ${compiler}: ${result.error}`;
  if (result.timedOut) return `command timed out after ${TIMEOUT_MS / 1000}s`;
  if (result.truncated) return `command output exceeded ${MAX_OUTPUT_BYTES} bytes`;
  if (result.code !== 0) return `command exited with status ${result.code}`;
  return null;
}

module.exports = {
  MAX_OUTPUT_BYTES, TIMEOUT_MS, QUERY_SCHEMA,
  queryArguments, docArguments, parseQueryResult, toRange, header, declarationItems, referenceItems, lensRecords, runCommand, failureReason,
  cwdOf: file => path.dirname(file)
};
