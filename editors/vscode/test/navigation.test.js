'use strict';
// Navigation-by-meaning mapping checks. No compiler or VS Code process is
// started; the spawn seam receives a scripted child so the bounds are exact.
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { EventEmitter } = require('node:events');
const {
  MAX_OUTPUT_BYTES, TIMEOUT_MS, QUERY_SCHEMA,
  queryArguments, docArguments, contextArguments, agentInspectArguments, parseSchemaDocument, CONTEXT_MAX_BYTES, parseQueryResult,
  renamePatch, impactArguments, patchArguments, impactSummary, graphArguments, cleanupPlan, agentRunArguments, toRange, header, declarationItems, referenceItems, lensRecords, runCommand, failureReason
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
});

test('a query result is accepted only with its schema, and malformed matches are dropped', () => {
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
  assert.equal(parseQueryResult(mixed).matches.length, 1);
  const bare = parseQueryResult(JSON.stringify({ ...result, matches: [{ ...tick, location: { line: 2, column: 3, start: -1, end: 'x' } }] }));
  assert.deepEqual(bare.matches[0].location, { line: 2, column: 3, start: null, end: null });
  assert.deepEqual(toRange(bare.matches[0].location), { startLine: 1, startColumn: 2, endLine: 1, endColumn: 3 });
});

test('declaration items are in source order with zero-based name ranges and headers', () => {
  const items = declarationItems(parseQueryResult(text));
  assert.deepEqual(items.map(item => item.id), ['clock.logical_tick', 'app.main']);
  assert.deepEqual(items[0], {
    label: 'logical_tick', description: 'function · clock.logical_tick', detail: 'fn logical_tick(value: i64) -> i64',
    id: 'clock.logical_tick', kind: 'function', range: { startLine: 5, startColumn: 3, endLine: 5, endColumn: 15 }
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
  kill() { this.killed = true; }
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
  assert.equal(CONTEXT_MAX_BYTES, 8192);
  assert.deepEqual(agentInspectArguments(at('agent.json')), ['agent', 'inspect', at('agent.json')]);
  assert.equal(parseSchemaDocument('{"schema":"semaprax.agent-graph.v1","agent_id":"a"}\n', 'semaprax.agent-graph.').agent_id, 'a');
  assert.equal(parseSchemaDocument('{"schema":"semaprax.doc.v1"}', 'semaprax.agent-graph.'), null);
  assert.equal(parseSchemaDocument('{"kind":"x"}', 'semaprax.'), null);
  assert.equal(parseSchemaDocument('[1]', 'semaprax.'), null);
  assert.equal(parseSchemaDocument('nope', 'semaprax.'), null);
  assert.equal(parseSchemaDocument(7, 'semaprax.'), null);
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
