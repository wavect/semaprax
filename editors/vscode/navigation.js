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
const { SourceIndex, locationRange } = require('./positions');

const MAX_OUTPUT_BYTES = 4 * 1024 * 1024;
const TIMEOUT_MS = 30 * 1000;
const QUERY_SCHEMA = 'semaprax.query.v1';
// A module that imports another with `use` has no standalone meaning: the
// compiler answers `SPX-G172` for it and only the project route can resolve it.
// The project result is a different document — `project_revision` and
// `graph_revision` instead of one `revision`, a per-match `path` and
// `source_revision`, and array-form locations — so it is parsed separately and
// normalised into the same match shape the rest of this module consumes.
const PROJECT_QUERY_SCHEMA = 'semaprax.project-query.v1';

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

// One bounded `context` query for a declaration's ownership, contract, and
// effect facts: depth one, the three facet filters, and a fixed byte budget.
const CONTEXT_MAX_BYTES = 8192;
// `context <file|project>` also answers for an authenticated project, which is
// the only route that resolves a module with `use` imports. The compiler
// rejects `--filters` for a project input, so the facet selection is dropped
// there and the whole bounded projection is shown instead.
function contextArguments(subject, id, project = false) {
  const args = ['context', subject, id, '--depth', '1'];
  if (!project) args.push('--filters', 'contracts,ownership,effects');
  args.push('--max-bytes', String(CONTEXT_MAX_BYTES));
  return args;
}

// Safe rename goes through the compiler's replay-checked semantic patch: the
// editor only authors the patch text and asks `impact`, then `patch`.
const IDENTIFIER = /^[a-z_][a-z0-9_]*$/;
function renamePatch(revision, id, newName) {
  if (typeof revision !== 'string' || !/^sha256:[0-9a-f]{64}$/.test(revision)) throw new Error('A graph revision is required to author a rename');
  if (typeof id !== 'string' || !id) throw new Error('A stable identity is required to author a rename');
  if (typeof newName !== 'string' || !IDENTIFIER.test(newName) || newName.length > 128) throw new Error('A new name must be a lowercase identifier: [a-z_][a-z0-9_]*');
  return `base ${revision}\nrename ${id} to ${newName}\n`;
}
function impactArguments(file, patch) { return ['impact', file, patch]; }
function patchArguments(file, patch) { return ['patch', file, patch]; }

// What a rename would touch, from `semaprax.semantic-impact.v1`, or null.
function impactSummary(text) {
  const value = parseSchemaDocument(text, 'semaprax.semantic-impact.');
  if (!value || !Array.isArray(value.changes)) return null;
  const changes = value.changes.filter(change => change && typeof change === 'object');
  const consumers = new Set();
  for (const change of changes) {
    for (const consumer of Array.isArray(change.source_consumers) ? change.source_consumers : []) {
      if (consumer && typeof consumer.id === 'string') consumers.add(consumer.id);
    }
  }
  return {
    baseRevision: typeof value.base_revision === 'string' ? value.base_revision : null,
    candidateRevision: typeof value.candidate_revision === 'string' ? value.candidate_revision : null,
    changes: changes.length,
    consumers: [...consumers].sort()
  };
}

// The cleanup plan the module graph records for one function, or null.
function graphArguments(file) { return ['graph', file]; }
function cleanupPlan(text, id) {
  const value = parseSchemaDocument(text, 'semaprax.graph.');
  if (!value || !Array.isArray(value.nodes)) return null;
  const node = value.nodes.find(node => node && node.kind === 'function' && node.id === id);
  if (!node || !node.cleanup || typeof node.cleanup !== 'object') return null;
  return { id, revision: typeof value.revision === 'string' ? value.revision : null, cleanup: node.cleanup };
}

// `agent run` with the caller's task and transcript; `output` selects the
// receipt (default), the trace, or the evidence document.
function agentRunArguments(definition, task, transcript, output) {
  const args = ['agent', 'run', definition, task, transcript];
  if (output === 'trace') args.push('--trace');
  else if (output === 'evidence') args.push('--evidence');
  return args;
}

// `agent inspect` over a saved AgentDefinition v1 document.
function agentInspectArguments(file) { return ['agent', 'inspect', file]; }

// A JSON document whose top-level `schema` starts with `prefix`, or null.
function parseSchemaDocument(text, prefix) {
  if (typeof text !== 'string') return null;
  let value;
  try { value = JSON.parse(text.trim()); } catch { return null; }
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  if (typeof value.schema !== 'string' || !value.schema.startsWith(prefix)) return null;
  return value;
}

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

// An absolute path inside `root`, or null when the compiler named an absolute
// path or one that escapes the project root. A match the editor cannot bind to
// an authenticated file of the project is dropped, never opened.
function resolveInRoot(root, relative) {
  if (typeof relative !== 'string' || !relative || path.isAbsolute(relative)) return null;
  const base = path.resolve(root), resolved = path.resolve(base, relative);
  return resolved === base || resolved.startsWith(base + path.sep) ? resolved : null;
}

// One validated project match, or null. `location` is array-form here, and the
// match carries the authenticated file it was found in and that file's own
// source revision alongside the fields a module match has.
function parseProjectMatch(value, root) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const { path: relative, module, source_revision: sourceRevision, kind, id, name, signature } = value;
  if ([relative, module, sourceRevision, kind, id, name, signature].some(field => typeof field !== 'string') || !id || !sourceRevision) return null;
  const location = value.location;
  if (!Array.isArray(location) || location.length !== 4 || !location.every(number => Number.isSafeInteger(number) && number >= 0)) return null;
  const [line, column, start, end] = location;
  if (line < 1 || column < 1) return null;
  const file = resolveInRoot(root, relative);
  if (!file) return null;
  const effects = strings(value.effects), calls = strings(value.calls), calledBy = strings(value.called_by);
  if (!effects || !calls || !calledBy) return null;
  return {
    kind, id, name, signature,
    persistent: value.persistent === true,
    path: relative, file, module, sourceRevision,
    location: { line, column, start, end },
    effects, calls, calledBy
  };
}

// The whole project query result, or null when the document is not one.
// `root` is the directory of the manifest the query answered for; matches
// outside it are dropped rather than trusted.
function parseProjectQueryResult(text, root) {
  if (typeof text !== 'string') return null;
  let value;
  try { value = JSON.parse(text.trim()); } catch { return null; }
  if (!value || typeof value !== 'object' || Array.isArray(value) || value.schema !== PROJECT_QUERY_SCHEMA) return null;
  if (typeof value.project !== 'string' || typeof value.project_revision !== 'string' || typeof value.graph_revision !== 'string' || !Array.isArray(value.matches)) return null;
  return {
    schema: PROJECT_QUERY_SCHEMA,
    project: value.project,
    projectRevision: value.project_revision,
    graphRevision: value.graph_revision,
    revision: null,
    matches: value.matches.map(match => parseProjectMatch(match, root)).filter(Boolean)
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

// Zero-based editor range of the declaration's name token. `index` is the
// `SourceIndex` of the exact saved source the query answered for, which turns
// the compiler's UTF-8 byte span into a real UTF-16 range; without it the
// fallback is the compiler's one-based line and column with the span's byte
// width, exact on ASCII. A bare position is one character wide either way.
function toRange(location, index = null) {
  return locationRange(location, index);
}

// The first signature line that is not an `@id` attribute.
function header(signature) {
  return signature.split('\n').find(line => line.trim() && !line.trim().startsWith('@id(')) || '';
}

// Source order within a file, and files in the order the compiler named them.
function byPosition(first, second) {
  const left = first.path || '', right = second.path || '';
  if (left !== right) return left < right ? -1 : 1;
  return first.location.line - second.location.line || first.location.column - second.location.column;
}

// `sources` is either the one `SourceIndex` every match belongs to, or a
// function from a match to the `SourceIndex` of its own authenticated file.
function indexer(sources) {
  return typeof sources === 'function' ? sources : () => sources;
}

// Quick-pick records for every declaration the query returned, in source
// order. A project match carries the authenticated file it lives in.
function declarationItems(result, sources = null) {
  const indexOf = indexer(sources);
  return result.matches.slice().sort(byPosition).map(match => ({
    label: match.name,
    description: match.path ? `${match.kind} · ${match.id} · ${match.path}` : `${match.kind} · ${match.id}`,
    detail: header(match.signature),
    id: match.id,
    kind: match.kind,
    path: match.path || null,
    file: match.file || null,
    sourceRevision: match.sourceRevision || null,
    range: toRange(match.location, indexOf(match))
  }));
}

// Quick-pick records for the callers a `--calls <target>` query returned.
function referenceItems(result, target, sources = null) {
  return declarationItems(result, sources).map(item => ({ ...item, description: `${item.kind} · ${item.id}${item.path ? ` · ${item.path}` : ''} · calls ${target}` }));
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
function lensRecords(result, sources = null) {
  const indexOf = indexer(sources);
  const lenses = [];
  for (const match of result.matches.slice().sort(byPosition)) {
    const range = toRange(match.location, indexOf(match));
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
  MAX_OUTPUT_BYTES, TIMEOUT_MS, QUERY_SCHEMA, PROJECT_QUERY_SCHEMA,
  queryArguments, docArguments, contextArguments, resolveInRoot, parseProjectQueryResult, agentInspectArguments, parseSchemaDocument, CONTEXT_MAX_BYTES, parseQueryResult,
  SourceIndex, renamePatch, impactArguments, patchArguments, impactSummary, graphArguments, cleanupPlan, agentRunArguments, toRange, header, declarationItems, referenceItems, lensRecords, runCommand, failureReason,
  cwdOf: file => path.dirname(file)
};
