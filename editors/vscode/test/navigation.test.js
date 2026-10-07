'use strict';
// Navigation-by-meaning mapping checks. No compiler or VS Code process is
// started; the spawn seam receives a scripted child so the bounds are exact.
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { EventEmitter } = require('node:events');
const {
  MAX_OUTPUT_BYTES, TIMEOUT_MS, QUERY_SCHEMA,
  queryArguments, docArguments, contextArguments, rustImportContextArguments, rustCandidateArguments, parseRustCandidates, parseRustImportContext, RUST_CONTEXT_MAX_BYTES, agentInspectArguments, parseSchemaDocument, CONTEXT_MAX_BYTES, parseQueryResult,
  SourceIndex, PROJECT_QUERY_SCHEMA, parseProjectQueryResult, resolveInRoot, renamePatch, impactArguments, patchArguments, impactSummary, graphArguments, cleanupPlan, agentRunArguments, toRange, header, declarationItems, referenceItems, lensRecords, runCommand, failureReason
} = require('../navigation');

const root = path.resolve(path.sep, 'work');
const at = (...parts) => path.join(root, ...parts);

const tick = {
  kind: 'function', id: 'clock.logical_tick', name: 'logical_tick', persistent: true,
  signature: '@id("clock.logical_tick")\nfn logical_tick(value: i64) -> i64\n    uses { clock.read }\n    ensures result == value + 1\n',
  location: { line: 6, column: 4, start: 78, end: 90 }, effects: ['clock.read'], calls: [], called_by: ['app.main']
};
const main = {
  kind: 'function', id: 'app.main', name: 'main', persistent: true,
  signature: '@id("app.main")\nfn main() -> i64\n    uses { clock.read }\n',
  location: { line: 13, column: 4, start: 170, end: 174 }, effects: ['clock.read'], calls: ['clock.logical_tick'], called_by: []
};
const result = { schema: QUERY_SCHEMA, module: 'examples.effects', revision: 'sha256:00', filters: {}, matches: [main, tick] };
const text = JSON.stringify(result) + '\n';

test('query and doc argument vectors are exact and end with the JSON flag', () => {
  assert.deepEqual(queryArguments(at('m.spx')), ['query', at('m.spx'), '--json']);
  assert.deepEqual(queryArguments(at('m.spx'), { kind: 'function,method' }), ['query', at('m.spx'), '--kind', 'function,method', '--json']);
  assert.deepEqual(queryArguments(at('m.spx'), { calls: 'a.b', calledBy: 'c.d', kind: '' }), ['query', at('m.spx'), '--calls', 'a.b', '--called-by', 'c.d', '--json']);
  assert.deepEqual(queryArguments(at('m.spx'), { kind: 42 }), ['query', at('m.spx'), '--json']);
  assert.deepEqual(docArguments(at('m.spx')), ['doc', at('m.spx')]);
  assert.deepEqual(docArguments(at('semaprax.toml'), 'src/core.spx'), ['doc', at('semaprax.toml'), '--module', 'src/core.spx']);
});

test('a query result is accepted only with its schema, and only when every match is well formed', () => {
  const parsed = parseQueryResult(text);
  assert.equal(parsed.module, 'examples.effects');
  assert.equal(parsed.revision, 'sha256:00');
  assert.equal(parsed.matches.length, 2);
  assert.deepEqual(parsed.matches[1].calledBy, ['app.main']);
  assert.equal(parseQueryResult('not json'), null);
  assert.equal(parseQueryResult(JSON.stringify({ ...result, schema: 'semaprax.doc.v1' })), null);
  assert.equal(parseQueryResult(JSON.stringify({ ...result, matches: 'x' })), null);
  assert.equal(parseQueryResult(JSON.stringify([result])), null);
  assert.equal(parseQueryResult(42), null);
  const mixed = JSON.stringify({ ...result, matches: [
    tick,
    { ...tick, location: { line: 0, column: 1 } },
    { ...tick, id: '' },
    { ...tick, effects: 'clock.read' },
    { ...tick, location: null },
    'nope'
  ] });
  assert.equal(parseQueryResult(mixed), null);
  const bare = parseQueryResult(JSON.stringify({ ...result, matches: [{ ...tick, location: { line: 2, column: 3, start: -1, end: 'x' } }] }));
  assert.deepEqual(bare.matches[0].location, { line: 2, column: 3, start: null, end: null });
  assert.deepEqual(toRange(bare.matches[0].location), { startLine: 1, startColumn: 2, endLine: 1, endColumn: 3 });
});

test('declaration items are in source order with zero-based name ranges and headers', () => {
  const items = declarationItems(parseQueryResult(text));
  assert.deepEqual(items.map(item => item.id), ['clock.logical_tick', 'app.main']);
  // A module query names no file: those fields belong to a project match.
  assert.deepEqual(items[0], {
    label: 'logical_tick', description: 'function · clock.logical_tick', detail: 'fn logical_tick(value: i64) -> i64',
    id: 'clock.logical_tick', kind: 'function', path: null, file: null, sourceRevision: null,
    range: { startLine: 5, startColumn: 3, endLine: 5, endColumn: 15 }
  });
  assert.equal(header('@id("x")\n'), '');
  assert.equal(referenceItems(parseQueryResult(text), 'clock.logical_tick')[1].description, 'function · app.main · calls clock.logical_tick');
});

test('code lenses name the identity, effects, and contract counts of each declaration', () => {
  const lenses = lensRecords(parseQueryResult(text));
  assert.deepEqual(lenses.map(lens => lens.title), [
    '@id clock.logical_tick', 'uses { clock.read }', 'requires 0 · ensures 1',
    '@id app.main', 'uses { clock.read }'
  ]);
  assert.deepEqual(lenses[0].range, { startLine: 5, startColumn: 3, endLine: 5, endColumn: 15 });
  const pure = lensRecords({ matches: [{ ...tick, persistent: false, effects: [], signature: 'fn f() -> i64\n' }] });
  assert.deepEqual(pure.map(lens => lens.title), ['auto id clock.logical_tick']);
});

class Child extends EventEmitter {
  constructor() { super(); this.stdout = new EventEmitter(); this.stderr = new EventEmitter(); this.killed = false; }
  kill() { this.killed = true; queueMicrotask(() => this.emit('close', null)); }
}
const spawnInto = (calls, child) => (command, args, options) => { calls.push({ command, args, options }); return child; };

test('a command spawns the selected binary directly with its exact arguments and no shell', async () => {
  const calls = [], child = new Child(), file = at('app', 'm.spx');
  const pending = runCommand(spawnInto(calls, child), at('bin', 'semaprax'), queryArguments(file), path.dirname(file));
  assert.deepEqual(calls, [{ command: at('bin', 'semaprax'), args: ['query', file, '--json'], options: { shell: false, windowsHide: true, cwd: at('app'), stdio: ['ignore', 'pipe', 'pipe'] } }]);
  child.stdout.emit('data', Buffer.from(text.slice(0, 20)));
  child.stdout.emit('data', Buffer.from(text.slice(20)));
  child.emit('close', 0);
  const result = await pending;
  assert.equal(result.code, 0); assert.equal(result.stdout, text); assert.equal(child.killed, false);
  assert.equal(failureReason(result, 'x'), null);
  assert.equal(failureReason({ code: 1 }, 'x'), 'command exited with status 1');
  assert.equal(failureReason({ error: 'ENOENT' }, at('bin', 'semaprax')), `could not start ${at('bin', 'semaprax')}: ENOENT`);
});

test('output beyond the byte budget and silence past the deadline kill the child', async () => {
  const calls = [], child = new Child();
  const pending = runCommand(spawnInto(calls, child), at('bin', 'semaprax'), ['doc', at('m.spx')], root, { maxBytes: 16 });
  child.stdout.emit('data', Buffer.alloc(17, 0x20));
  const truncated = await pending;
  assert.equal(truncated.truncated, true); assert.equal(truncated.code, null); assert.equal(child.killed, true);
  assert.equal(failureReason(truncated, 'x'), `command output exceeded ${MAX_OUTPUT_BYTES} bytes`);
  const slow = new Child();
  const late = await runCommand(spawnInto([], slow), at('bin', 'semaprax'), ['doc', at('m.spx')], root, { timeoutMs: 5 });
  assert.equal(late.timedOut, true); assert.equal(slow.killed, true);
  assert.equal(failureReason(late, 'x'), `command timed out after ${TIMEOUT_MS / 1000}s`);
});

test('context and agent inspect argument vectors are exact and schema documents are checked by prefix', () => {
  assert.deepEqual(contextArguments(at('m.spx'), 'app.main'), ['context', at('m.spx'), 'app.main', '--depth', '1', '--filters', 'contracts,ownership,effects', '--max-bytes', String(CONTEXT_MAX_BYTES)]);
  assert.deepEqual(rustImportContextArguments(at('m.spx'), 'regex::Regex::is_match', at('regex.index.json')),
    ['context', at('m.spx'), 'regex::Regex::is_match', '--max-bytes', String(RUST_CONTEXT_MAX_BYTES), '--rust-index', at('regex.index.json')]);
  assert.deepEqual(rustImportContextArguments(at('m.spx'), 'regex::Regex::is_match'),
    ['context', at('m.spx'), 'regex::Regex::is_match', '--max-bytes', String(RUST_CONTEXT_MAX_BYTES)]);
  assert.equal(CONTEXT_MAX_BYTES, 8192);
  assert.equal(RUST_CONTEXT_MAX_BYTES, 4096);
  assert.deepEqual(agentInspectArguments(at('agent.json')), ['agent', 'inspect', at('agent.json')]);
  assert.equal(parseSchemaDocument('{"schema":"semaprax.agent-graph.v1","agent_id":"a"}\n', 'semaprax.agent-graph.').agent_id, 'a');
  assert.equal(parseSchemaDocument('{"schema":"semaprax.doc.v1"}', 'semaprax.agent-graph.'), null);
  assert.equal(parseSchemaDocument('{"kind":"x"}', 'semaprax.'), null);
  assert.equal(parseSchemaDocument('[1]', 'semaprax.'), null);
  assert.equal(parseSchemaDocument('nope', 'semaprax.'), null);
  assert.equal(parseSchemaDocument(7, 'semaprax.'), null);
});

test('Rust hover accepts only bounded read-only compiler context for the exact declared path', () => {
  const value = {
    schema: 'semaprax.rust-api-context.v1',
    authority: { execution: false, publication: false, tool_invocation: false },
    budget: { max_bytes: 4096, used_bytes: 0 },
    index: { status: 'prepared_metadata' },
    package: { name: 'regex', version: '1.13.1', cargo_alias: 'regex_alias' },
    selected_import: { id: 'rust.host.method', path: 'regex::Regex::is_match', signature: 'fn is_match(&self, haystack: &str) -> bool', ownership: 'shared', support: 'supported' }
  };
  let encoded;
  do {
    encoded = JSON.stringify(value);
    if (value.budget.used_bytes === Buffer.byteLength(encoded)) break;
    value.budget.used_bytes = Buffer.byteLength(encoded);
  } while (true);
  assert.equal(parseRustImportContext(encoded + '\n', 'regex::Regex::is_match').selected_import.ownership, 'shared');
  assert.equal(parseRustImportContext(encoded, 'regex::Regex::find'), null);
  assert.equal(parseRustImportContext(JSON.stringify({ ...value, authority: { ...value.authority, tool_invocation: true } }), 'regex::Regex::is_match'), null);
  assert.equal(parseRustImportContext('x'.repeat(4097), 'regex::Regex::is_match'), null);
});

test('Rust completion consumes only canonical bounded candidate metadata for the exact prefix', () => {
  const prefix = 'regex::Regex::';
  assert.deepEqual(rustCandidateArguments(at('m.spx'), prefix, at('regex.index.json')),
    ['context', at('m.spx'), prefix, '--max-bytes', '4096', '--rust-index', at('regex.index.json'), '--candidates']);
  const value = {
    schema: 'semaprax.rust-api-candidates.v1', prefix,
    authority: { execution: false, publication: false, tool_invocation: false },
    budget: { max_bytes: 4096, used_bytes: 0 },
    index: { status: 'prepared_metadata' },
    items: [{ path: 'regex::Regex::is_match', signature: 'fn is_match(&self, haystack: &str) -> bool', support: 'supported' }],
    truncation: { omitted_items: 1, truncated: true }
  };
  let encoded;
  do {
    encoded = JSON.stringify(value);
    if (value.budget.used_bytes === Buffer.byteLength(encoded)) break;
    value.budget.used_bytes = Buffer.byteLength(encoded);
  } while (true);
  assert.equal(parseRustCandidates(encoded + '\n', prefix).items[0].path, 'regex::Regex::is_match');
  assert.equal(parseRustCandidates(encoded, 'regex::Regex::find'), null);
  assert.equal(parseRustCandidates(JSON.stringify({ ...value, authority: { ...value.authority, execution: true } }), prefix), null);
  assert.equal(parseRustCandidates('x'.repeat(4097), prefix), null);
});

test('a safe rename authors exactly the replay-checked patch text and rejects bad names', () => {
  const revision = 'sha256:' + 'ab'.repeat(32);
  assert.equal(renamePatch(revision, 'math.add', 'checked_add'), `base ${revision}\nrename math.add to checked_add\n`);
  for (const bad of ['Add', '1x', 'a-b', '', 'x'.repeat(129), 42]) assert.throws(() => renamePatch(revision, 'math.add', bad), /lowercase identifier/);
  assert.throws(() => renamePatch('sha256:00', 'math.add', 'ok'), /graph revision/);
  assert.throws(() => renamePatch(revision, '', 'ok'), /stable identity/);
  assert.deepEqual(impactArguments(at('m.spx'), at('r.spatch')), ['impact', at('m.spx'), at('r.spatch')]);
  assert.deepEqual(patchArguments(at('m.spx'), at('r.spatch')), ['patch', at('m.spx'), at('r.spatch')]);
  const impact = JSON.stringify({ schema: 'semaprax.semantic-impact.v1', base_revision: revision, candidate_revision: revision,
    changes: [{ kind: 'rename', target: 'math.add', source_consumers: [{ id: 'app.main' }, { id: 'app.other' }] }, { kind: 'rename', target: 'x', source_consumers: [{ id: 'app.main' }] }, 'junk'] });
  assert.deepEqual(impactSummary(impact), { baseRevision: revision, candidateRevision: revision, changes: 2, consumers: ['app.main', 'app.other'] });
  assert.equal(impactSummary('{"schema":"semaprax.semantic-review.v1","changes":[]}'), null);
  assert.equal(impactSummary('{"schema":"semaprax.semantic-impact.v1"}'), null);
});

test('the cleanup plan is read from the module graph by function identity', () => {
  const graph = JSON.stringify({ schema: 'semaprax.graph.v10', revision: 'sha256:00', nodes: [
    { id: 'geometry.point', kind: 'record' },
    { id: 'app.main', kind: 'function', cleanup: { kind: 'plan', slots: [] } },
    { id: 'app.other', kind: 'function' }
  ] });
  assert.deepEqual(cleanupPlan(graph, 'app.main'), { id: 'app.main', revision: 'sha256:00', cleanup: { kind: 'plan', slots: [] } });
  assert.equal(cleanupPlan(graph, 'app.other'), null);
  assert.equal(cleanupPlan(graph, 'geometry.point'), null);
  assert.equal(cleanupPlan('{"schema":"semaprax.doc.v1","nodes":[]}', 'app.main'), null);
  assert.deepEqual(graphArguments(at('m.spx')), ['graph', at('m.spx')]);
  assert.deepEqual(agentRunArguments(at('a.json'), at('t.json'), at('x.json'), 'receipt'), ['agent', 'run', at('a.json'), at('t.json'), at('x.json')]);
  assert.deepEqual(agentRunArguments(at('a.json'), at('t.json'), at('x.json'), 'trace'), ['agent', 'run', at('a.json'), at('t.json'), at('x.json'), '--trace']);
  assert.deepEqual(agentRunArguments(at('a.json'), at('t.json'), at('x.json'), 'evidence'), ['agent', 'run', at('a.json'), at('t.json'), at('x.json'), '--evidence']);
});

// Declaration selections and code lenses use the same coordinate convention
// as diagnostics: zero-based lines, UTF-16 characters, and a real span.
const astral = 'module m;\n\n// \u{1F600}\nfn \u{1F600}_tick() -> i64 { 0 }\n';
const astralIndex = new SourceIndex(Buffer.from(astral, 'utf8'));
const astralMatch = {
  kind: 'function', id: 'm.tick', name: '\u{1F600}_tick', persistent: true,
  signature: '@id("m.tick")\nfn \u{1F600}_tick() -> i64\n',
  // The name token starts after `fn ` on line 4: bytes 22..31, scalar column 4.
  location: { line: 4, column: 4, start: 22, end: 31 }, effects: [], calls: [], called_by: []
};
const astralResult = { schema: QUERY_SCHEMA, module: 'm', revision: 'sha256:01', filters: {}, matches: [astralMatch] };

test('a declaration range past an astral character uses UTF-16 characters', () => {
  assert.equal(astral.indexOf('\u{1F600}_tick'), 20);
  assert.equal(Buffer.byteLength(astral.slice(0, 20)), 22);
  // Without the source the byte width and scalar column are used verbatim.
  assert.deepEqual(toRange(astralMatch.location), { startLine: 3, startColumn: 3, endLine: 3, endColumn: 12 });
  // With it, the range is the name token in editor coordinates.
  assert.deepEqual(toRange(astralMatch.location, astralIndex), { startLine: 3, startColumn: 3, endLine: 3, endColumn: 10 });
  assert.deepEqual(declarationItems(astralResult, astralIndex)[0].range, { startLine: 3, startColumn: 3, endLine: 3, endColumn: 10 });
  assert.deepEqual(referenceItems(astralResult, 'm.other', astralIndex)[0].range, { startLine: 3, startColumn: 3, endLine: 3, endColumn: 10 });
  for (const lens of lensRecords(astralResult, astralIndex)) {
    assert.deepEqual(lens.range, { startLine: 3, startColumn: 3, endLine: 3, endColumn: 10 });
  }
});

test('a declaration whose offsets do not fit the saved source keeps the reported position', () => {
  const other = new SourceIndex('module m;\n');
  assert.deepEqual(toRange(astralMatch.location, other), { startLine: 3, startColumn: 3, endLine: 3, endColumn: 12 });
  assert.deepEqual(toRange({ line: 2, column: 5, start: null, end: null }, astralIndex), { startLine: 1, startColumn: 4, endLine: 1, endColumn: 5 });
});

// Project routing. A module with `use` imports has no standalone meaning, so
// declaration navigation, callers, and code lenses read it from its project.
// The fixture below is the `semaprax query examples/calculator-project/
// semaprax.toml --json` document, abridged to three files.
const projectRoot = at('calculator');
const projectRevision = 'sha256:' + '85'.repeat(32);
const graphRevision = 'sha256:' + '0c'.repeat(32);
const sourceRevision = 'sha256:' + '40'.repeat(32);
const projectMatch = (relative, module, id, name, location, calls = [], calledBy = []) => ({
  path: relative, module, source_revision: sourceRevision, kind: 'function', id, name, persistent: true,
  signature: `@id("${id}")\nfn ${name}() -> i64\n`, location, effects: [], calls, called_by: calledBy
});
const projectText = JSON.stringify({
  schema: 'semaprax.project-query.v1', project: 'calculator',
  project_revision: projectRevision, graph_revision: graphRevision,
  filters: { kinds: [], name: null, id_prefix: null, effect: null, calls: null, called_by: null },
  matches: [
    projectMatch('src/app.spx', 'calculator.app', 'calculator.app.main', 'main', [8, 4, 336, 340], ['calculator.add']),
    projectMatch('src/core.spx', 'calculator.core', 'calculator.add', 'add', [4, 4, 50, 53], [], ['calculator.app.main', 'calculator.tests.main']),
    projectMatch('src/tests.spx', 'calculator.tests', 'calculator.tests.main', 'main', [10, 4, 484, 488], ['calculator.add'])
  ]
}) + '\n';

test('a project query result binds every match to its authenticated file and revisions', () => {
  const parsed = parseProjectQueryResult(projectText, projectRoot);
  assert.equal(parsed.schema, PROJECT_QUERY_SCHEMA);
  assert.equal(parsed.project, 'calculator');
  assert.equal(parsed.projectRevision, projectRevision);
  assert.equal(parsed.graphRevision, graphRevision);
  assert.equal(parsed.revision, null, 'a project result has no single module revision');
  assert.deepEqual(parsed.matches.map(match => match.file), [
    at('calculator', 'src', 'app.spx'), at('calculator', 'src', 'core.spx'), at('calculator', 'src', 'tests.spx')
  ]);
  assert.deepEqual(parsed.matches.map(match => match.sourceRevision), [sourceRevision, sourceRevision, sourceRevision]);
  assert.deepEqual(parsed.matches[1].location, { line: 4, column: 4, start: 50, end: 53 });
  assert.deepEqual(parsed.matches[1].calledBy, ['calculator.app.main', 'calculator.tests.main']);
  // A module result is not a project result and the reverse.
  assert.equal(parseProjectQueryResult(text, projectRoot), null);
  assert.equal(parseQueryResult(projectText), null);
});

test('a project match outside the project root, or without its revision binding, rejects the result', () => {
  const hostile = value => JSON.stringify({
    schema: 'semaprax.project-query.v1', project: 'calculator',
    project_revision: projectRevision, graph_revision: graphRevision, filters: {}, matches: [value]
  });
  const good = projectMatch('src/core.spx', 'calculator.core', 'calculator.add', 'add', [4, 4, 50, 53]);
  assert.equal(parseProjectQueryResult(hostile(good), projectRoot).matches.length, 1);
  for (const broken of [
    { ...good, path: '../outside/core.spx' },
    { ...good, path: at('elsewhere', 'core.spx') },
    { ...good, path: '' },
    { ...good, source_revision: null },
    { ...good, location: { line: 4, column: 4, start: 50, end: 53 } },
    { ...good, location: [4, 4, 50] },
    { ...good, location: [0, 4, 50, 53] },
    { ...good, called_by: [1] }
  ]) {
    assert.equal(parseProjectQueryResult(hostile(broken), projectRoot), null, JSON.stringify(broken.path ?? broken.location));
  }
  assert.equal(resolveInRoot(projectRoot, 'src/core.spx'), at('calculator', 'src', 'core.spx'));
  assert.equal(resolveInRoot(projectRoot, '../escape.spx'), null);
  assert.equal(resolveInRoot(projectRoot, 'src/../../escape.spx'), null);
});

test('project declarations, callers, and lenses name the file each match lives in', () => {
  const parsed = parseProjectQueryResult(projectText, projectRoot);
  // Each match is mapped against its own saved source, not the active file.
  const sources = { [at('calculator', 'src', 'core.spx')]: new SourceIndex('x'.repeat(49) + '\nadd extra\n') };
  const items = declarationItems(parsed, match => sources[match.file] || null);
  assert.deepEqual(items.map(item => item.path), ['src/app.spx', 'src/core.spx', 'src/tests.spx']);
  assert.deepEqual(items.map(item => item.file), [
    at('calculator', 'src', 'app.spx'), at('calculator', 'src', 'core.spx'), at('calculator', 'src', 'tests.spx')
  ]);
  assert.equal(items[1].description, 'function · calculator.add · src/core.spx');
  // core.spx has a saved index here: bytes 50..53 are `add` on its second line.
  assert.deepEqual(items[1].range, { startLine: 1, startColumn: 0, endLine: 1, endColumn: 3 });
  // app.spx has none, so its reported line and column are used.
  assert.deepEqual(items[0].range, { startLine: 7, startColumn: 3, endLine: 7, endColumn: 7 });
  assert.equal(referenceItems(parsed, 'calculator.add')[0].description, 'function · calculator.app.main · src/app.spx · calls calculator.add');
  assert.deepEqual(lensRecords(parsed).map(lens => lens.title), [
    '@id calculator.app.main', '@id calculator.add', '@id calculator.tests.main'
  ]);
});

test('the project context route drops the facet filter the compiler refuses', () => {
  const manifest = at('calculator', 'semaprax.toml');
  assert.deepEqual(contextArguments(manifest, 'calculator.add', true),
    ['context', manifest, 'calculator.add', '--depth', '1', '--max-bytes', String(CONTEXT_MAX_BYTES)]);
  assert.deepEqual(contextArguments(at('m.spx'), 'm.f'),
    ['context', at('m.spx'), 'm.f', '--depth', '1', '--filters', 'contracts,ownership,effects', '--max-bytes', String(CONTEXT_MAX_BYTES)]);
});

// Navigation stdout is admitted only as strict UTF-8 (REF-16).
async function navigate(chunks, close = 0, options = {}) {
  const child = new Child();
  const pending = runCommand(spawnInto([], child), at('bin', 'semaprax'), queryArguments(at('m.spx')), root, options);
  for (const chunk of chunks) child.stdout.emit('data', chunk);
  if (close !== 'pending') child.emit('close', close);
  return pending;
}

test('raw invalid UTF-8 in a navigation field is a transport failure, not a successful query', async () => {
  const bad = Buffer.from(JSON.stringify({ ...result, module: 'examples.XX' }) + '\n');
  bad[bad.indexOf('XX')] = 0xff;
  const run = await navigate([bad]);
  assert.equal(run.invalidUtf8, true);
  assert.equal(failureReason(run, 'x'), 'command output is not valid UTF-8');
  assert.equal(parseQueryResult(run.stdout), null);
  // A literal U+FFFD is valid UTF-8 and an emoji split across chunks survives.
  const literal = await navigate([Buffer.from(JSON.stringify({ ...result, module: 'examples.�' }) + '\n')]);
  assert.equal(failureReason(literal, 'x'), null);
  assert.equal(parseQueryResult(literal.stdout).module, 'examples.�');
  const emoji = Buffer.from(JSON.stringify({ ...result, module: 'examples.\u{1F600}' }) + '\n');
  const split = emoji.indexOf(0xf0) + 1;
  const joined = await navigate([emoji.subarray(0, split), emoji.subarray(split)]);
  assert.equal(parseQueryResult(joined.stdout).module, 'examples.\u{1F600}');
  assert.equal(failureReason(await navigate([Buffer.from(text), Buffer.from([0xe2])]), 'x'), 'command output is not valid UTF-8');
});

test('navigation timeout and byte-cap keep their reason with incomplete trailing UTF-8', async () => {
  const capped = await navigate([Buffer.from([0xf0, 0x9f]), Buffer.alloc(20, 0x20)], 'pending', { maxBytes: 16 });
  assert.equal(failureReason(capped, 'x'), `command output exceeded ${MAX_OUTPUT_BYTES} bytes`);
  const slow = await navigate([Buffer.from([0xf0, 0x9f])], 'pending', { timeoutMs: 5 });
  assert.equal(failureReason(slow, 'x'), `command timed out after ${TIMEOUT_MS / 1000}s`);
});

// A query result is admitted whole or not at all (REF-18). Dropping one bad
// row would present the survivors, or an empty list, as a complete answer.
test('one malformed module match makes the whole query result unusable, not a shorter one', () => {
  const malformed = { ...tick, effects: 'clock.read' };
  assert.equal(parseQueryResult(JSON.stringify({ ...result, matches: [malformed] })), null, 'malformed-only is not a successful empty result');
  assert.equal(parseQueryResult(JSON.stringify({ ...result, matches: [main, malformed] })), null, 'mixed rows are not silently shortened');
  for (const broken of [{ ...tick, location: { line: 0, column: 1 } }, { ...tick, id: '' }, { ...tick, location: null }, 'nope', null]) {
    assert.equal(parseQueryResult(JSON.stringify({ ...result, matches: [tick, broken] })), null, JSON.stringify(broken));
  }
  // A genuinely empty valid result stays an ordinary empty answer.
  const empty = parseQueryResult(JSON.stringify({ ...result, matches: [] }));
  assert.deepEqual(empty.matches, []);
  assert.equal(empty.revision, 'sha256:00');
  assert.equal(parseQueryResult(text).matches.length, 2);
});

test('one malformed or out-of-root project match makes the whole project result unusable', () => {
  const document = matches => JSON.stringify({
    schema: 'semaprax.project-query.v1', project: 'calculator',
    project_revision: projectRevision, graph_revision: graphRevision, filters: {}, matches
  });
  const good = projectMatch('src/core.spx', 'calculator.core', 'calculator.add', 'add', [4, 4, 50, 53]);
  const caller = projectMatch('src/app.spx', 'calculator.app', 'calculator.app.main', 'main', [8, 4, 336, 340], ['calculator.add']);
  assert.equal(parseProjectQueryResult(document([{ ...good, effects: 'x' }]), projectRoot), null);
  assert.equal(parseProjectQueryResult(document([caller, { ...good, effects: 'x' }]), projectRoot), null);
  // An out-of-root row is never opened, and its omission cannot support a
  // claim that nothing else calls the target.
  assert.equal(parseProjectQueryResult(document([caller, { ...caller, path: '../outside/app.spx' }]), projectRoot), null);
  assert.deepEqual(parseProjectQueryResult(document([]), projectRoot).matches, []);
  assert.equal(parseProjectQueryResult(document([caller, good]), projectRoot).matches.length, 2);
});

// In-flight sharing of the one whitelisted read-only lens query (REF-19).
const navigationModule = require('../navigation');
const { SharedLensQueries, lensQueryKey } = navigationModule;
class Token {
  constructor() { this.isCancellationRequested = false; this.listeners = []; }
  onCancellationRequested(listener) { this.listeners.push(listener); return { dispose: () => { this.listeners = this.listeners.filter(item => item !== listener); } }; }
  cancel() { this.isCancellationRequested = true; for (const listener of this.listeners.slice()) listener(); }
}
const settle = () => new Promise(resolve => setImmediate(resolve));
const lensKey = (overrides = {}) => lensQueryKey({ binary: at('bin', 'semaprax'), subject: at('calculator', 'semaprax.toml'), project: true, cwd: projectRoot, args: queryArguments(at('calculator', 'semaprax.toml')), generation: 0, ...overrides });
// The shared start: one bounded runCommand, admitted once (REF-16/REF-18).
const lensStart = (calls, child) => signal => runCommand(spawnInto(calls, child), at('bin', 'semaprax'), queryArguments(at('calculator', 'semaprax.toml')), projectRoot, { signal })
  .then(run => (failureReason(run, 'x') ? null : parseProjectQueryResult(run.stdout, projectRoot)));

test('two simultaneous lens requests for one unchanged project share exactly one query', async () => {
  const registry = new SharedLensQueries(), calls = [], child = new Child();
  const first = registry.request(lensKey(), lensStart(calls, child), new Token());
  const second = registry.request(lensKey(), lensStart(calls, child), new Token());
  await settle();
  assert.equal(calls.length, 1, 'one compiler dispatch for both members');
  child.stdout.emit('data', Buffer.from(projectText)); child.emit('close', 0);
  const [one, two] = await Promise.all([first, second]);
  assert.equal(one, two, 'both subscribers receive the one validated result');
  const app = lensRecords({ ...one, matches: one.matches.filter(match => match.file === at('calculator', 'src', 'app.spx')) });
  const core = lensRecords({ ...two, matches: two.matches.filter(match => match.file === at('calculator', 'src', 'core.spx')) });
  assert.deepEqual(app.map(lens => lens.title), ['@id calculator.app.main']);
  assert.deepEqual(core.map(lens => lens.title), ['@id calculator.add']);
  assert.equal(registry.size, 0, 'nothing is retained after completion');
  // A later request is a new query: there is no warm cache.
  const later = registry.request(lensKey(), lensStart(calls, new Child()), new Token());
  await settle();
  assert.equal(calls.length, 2);
  registry.dispose(); assert.equal(await later, undefined);
});

test('different compilers, subjects, worktrees, arguments or source generations never share', async () => {
  const base = lensKey();
  for (const changed of [{ binary: at('bin', 'other') }, { subject: at('other', 'semaprax.toml'), args: queryArguments(at('other', 'semaprax.toml')) }, { cwd: at('worktree') }, { subject: at('calculator', 'nested', 'semaprax.toml'), args: queryArguments(at('calculator', 'nested', 'semaprax.toml')) }, { generation: 1 }, { project: false }]) {
    assert.notEqual(lensQueryKey({ binary: at('bin', 'semaprax'), subject: at('calculator', 'semaprax.toml'), project: true, cwd: projectRoot, args: queryArguments(at('calculator', 'semaprax.toml')), generation: 0, ...changed }), base, JSON.stringify(changed));
  }
  const registry = new SharedLensQueries();
  let starts = 0;
  const start = () => { starts++; return new Promise(() => {}); };
  registry.request(lensKey(), start); registry.request(lensKey({ generation: 1 }), start);
  await settle();
  assert.equal(starts, 2);
  registry.dispose();
});

test('a cancelled subscriber does not disturb another; when all cancel the child is terminated and released', async () => {
  const registry = new SharedLensQueries(), calls = [], child = new Child();
  const leaving = new Token(), staying = new Token();
  const gone = registry.request(lensKey(), lensStart(calls, child), leaving);
  const kept = registry.request(lensKey(), lensStart(calls, child), staying);
  await settle();
  leaving.cancel();
  assert.equal(await gone, undefined, 'a cancelled subscriber gets no result');
  assert.equal(child.killed, false, 'the child is still needed by the other subscriber');
  child.stdout.emit('data', Buffer.from(projectText)); child.emit('close', 0);
  assert.equal((await kept).matches.length, 3);

  const lone = new Child(), token = new Token();
  const abandoned = registry.request(lensKey(), lensStart(calls, lone), token);
  await settle();
  token.cancel();
  assert.equal(await abandoned, undefined);
  assert.equal(registry.size, 0, 'the entry is released once no subscriber remains');
  await settle();
  assert.equal(lone.killed, true, 'the owned child is terminated through the existing path');
  // An already-cancelled token never starts work.
  const before = calls.length, cancelled = new Token(); cancelled.cancel();
  assert.equal(await registry.request(lensKey(), lensStart(calls, new Child()), cancelled), undefined);
  await settle();
  assert.equal(calls.length, before);
});

test('invalidation during a request discards its result and failures are never cached', async () => {
  const registry = new SharedLensQueries(), calls = [], child = new Child();
  const pending = registry.request(lensKey(), lensStart(calls, child), new Token());
  await settle();
  registry.invalidate();
  assert.equal(await pending, undefined, 'a result started before a relevant change is not published');
  await settle();
  assert.equal(child.killed, true);
  assert.equal(registry.size, 0);

  const failing = new Child();
  const failed = registry.request(lensKey(), lensStart(calls, failing), new Token());
  await settle();
  failing.stdout.emit('data', Buffer.from(JSON.stringify({ ...JSON.parse(projectText), matches: [{ effects: 'x' }] })));
  failing.emit('close', 0);
  assert.equal(await failed, null, 'an invalid reply is a failure, not a successful empty result');
  assert.equal(registry.size, 0);
  const retry = new Child();
  const again = registry.request(lensKey(), lensStart(calls, retry), new Token());
  await settle();
  retry.stdout.emit('data', Buffer.from(projectText)); retry.emit('close', 0);
  assert.equal((await again).matches.length, 3, 'a later independent request executes normally');
});

test('the registry is bounded and the runner honours cancellation without losing settlement', async () => {
  const registry = new SharedLensQueries(2);
  let starts = 0;
  const start = () => { starts++; return new Promise(() => {}); };
  registry.request(lensKey({ generation: 1 }), start); registry.request(lensKey({ generation: 2 }), start);
  assert.equal(await registry.request(lensKey({ generation: 3 }), start), undefined, 'a full registry declines instead of queueing');
  await settle();
  assert.equal(starts, 2);
  registry.dispose();
  assert.equal(registry.size, 0);

  const child = new Child(), controller = new AbortController();
  child.kill = function () { this.killed = true; };
  let settled = false;
  const pending = runCommand(spawnInto([], child), at('bin', 'semaprax'), ['query', at('m.spx'), '--json'], root, { signal: controller.signal }).then(value => { settled = true; return value; });
  controller.abort();
  await settle();
  assert.equal(child.killed, true);
  assert.equal(settled, false, 'ownership is kept until the child is observed to exit');
  child.emit('close', null);
  const run = await pending;
  assert.equal(run.cancelled, true);
  assert.equal(failureReason(run, 'x'), 'command was cancelled');
});

test('no mutation-bearing route is shared: only the lens query is whitelisted', () => {
  assert.equal(typeof navigationModule.runCommand, 'function');
  assert.throws(() => lensQueryKey({ binary: at('bin', 'semaprax'), subject: at('m.spx'), project: false, cwd: root, args: patchArguments(at('m.spx'), at('p.spatch')), generation: 0 }), /only the lens query/);
  assert.throws(() => lensQueryKey({ binary: at('bin', 'semaprax'), subject: at('m.spx'), project: false, cwd: root, args: queryArguments(at('m.spx'), { calls: 'x' }), generation: 0 }), /only the lens query/);
});
