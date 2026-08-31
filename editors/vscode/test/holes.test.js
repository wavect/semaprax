'use strict';
// Authored controller evidence only; no Node, compiler or editor execution.
const test = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { HoleDraft } = require('../holes');
const { ALLOWED, McpClient } = require('../protocol');
const { hash, canonical } = require('../review');

const digest = n => 'sha256:' + n.toString(16).padStart(64, '0');
const IMAGE = digest(1), CANDIDATE = digest(2), PROJECT = digest(3);
const FACETS = ['scope', 'calls', 'obligations', 'constructors'];
const HOLES = [
  { holeId: 'body', target: 'calculator.add', kind: 'body' },
  { holeId: 'expression', target: 'calculator.subtract', kind: 'expression', expressionId: 'calculator.subtract/expr/1' },
  { holeId: 'contract', target: 'calculator.divide', kind: 'contract', expressionId: 'calculator.divide/requires/1' },
];
const SCHEMAS = {
  body: 'semaprax.project-candidate-hole-context.v1',
  expression: 'semaprax.project-candidate-expression-hole-context.v1',
  contract: 'semaprax.project-candidate-contract-expression-hole-context.v1',
};
const INTENTS = { body: 'replace_function_body', expression: 'replace_expression', contract: 'replace_contract_expression' };
const OPEN = { body: 'hole/open', expression: 'hole/open-expression', contract: 'hole/open-contract-expression' };
function draftHandle(revision) {
  return { schema: 'semaprax.image-draft-handle.v1', draft_revision: revision,
    source_candidate_revision: CANDIDATE, report_bytes: 512, source_authority: false, buildable: false };
}
function candidateHandle() {
  return { schema: 'semaprax.image-candidate-handle.v1', candidate_revision: digest(40),
    project_revision: digest(41), base_revision: PROJECT, report_bytes: 1024, source_authority: false, tests: 'not_run' };
}
function expressionRow(phase = 'body', replaceable = true) {
  return { expression_id: phase === 'body' ? HOLES[1].expressionId : HOLES[2].expressionId,
    phase, kind: 'binary', expected_type: phase === 'body' ? 'i64' : 'bool', ownership: 'value',
    source_span: { start: 100, end: 109, line: 8, column: 5 }, replaceable,
    reason: replaceable ? 'requires_typed_constructor_and_full_project_revalidation' : 'no_unique_authored_ast_origin',
    scope: [{ name: 'left', value_id: 'calculator.add/parameter/0', type: 'i64', ownership: 'value', mutable: false }] };
}
function catalog(kind, target) {
  const unavailable = expressionRow(kind === 'contract' ? 'ensures' : 'body', false);
  unavailable.expression_id += '/unavailable';
  return { schema: kind === 'contract' ? 'semaprax.project-contract-expression-catalog.v1' : 'semaprax.project-expression-catalog.v1',
    candidate_digest: CANDIDATE, project_revision: PROJECT, target,
    source: { path: 'src/core.spx', module: 'calculator.core', source_revision: digest(10), source_digest: digest(11) },
    declared_effect_budget: [], expressions: [expressionRow(kind === 'contract' ? 'requires' : 'body'), unavailable],
    limits: { max_expressions: 4096, max_depth: 256, max_scope_facts: 16384, max_bytes: 1048576 },
    nonclaims: ['lexical_scope_is_not_owned_value_liveness', 'no_source_or_commit_authority'] };
}
function draftCatalog(kind, target, revision) {
  const old = catalog(kind, target);
  return { schema: 'semaprax.project-draft-expression-catalog.v1', draft_revision: revision,
    last_valid_revision: digest(60), last_valid_candidate_digest: digest(61), target,
    region: kind === 'contract' ? 'contract' : 'body', source: old.source,
    declared_effect_budget: old.declared_effect_budget, expressions: old.expressions, limits: old.limits,
    materializable: false, source_authority: false, validation: 'pending_fill_full_source_replay',
    evidence_class: 'last_valid_expression_inventory_not_draft_validation', selection_admission: 'requires_hole_open_validation', nonclaims: old.nonclaims };
}
function context(hole, revision) {
  const body = hole.kind === 'body', contract = hole.kind === 'contract';
  const common = { schema: SCHEMAS[hole.kind], draft_digest: revision, hole_id: hole.holeId, hole_handle: digest(12),
    target: hole.target, last_valid_revision: PROJECT, expected_type_id: contract ? 'bool' : 'i64',
    scope: body ? [{ id: 'calculator.add/parameter/0', name: 'left', type_id: 'i64', ownership: 'value' }] : expressionRow().scope,
    effect_policy: { allowed: [], forbidden: contract ? 'all_effects_in_contract_predicates' : 'all_undeclared_effects', module_permits: [] },
    accessible_calls: [{ id: 'calculator.add', binding: 'add', return_type_id: 'i64', parameters: [{ name: 'left', type_id: 'i64', ownership: 'value' }, { name: 'right', type_id: 'i64', ownership: 'value' }], effects: [], within_effect_budget: true, basis: 'existing_local_or_authenticated_import_binding', admission: 'requires_fill_revalidation' }],
    prior_body_proof: { basis: 'last_valid_body_not_the_unfilled_hole', loan_plan: {}, cleanup_plan: {} },
    obligations: ['return_expected_type', 'no_new_effects_or_capabilities'], constructor_owner: 'semaprax.semantic-change.v1',
    constructor_kinds: ['i64', 'place', 'builtin_call'], intent_kind: INTENTS[hole.kind],
    validation: 'pending_fill_full_source_replay', materializable: false, source_authority: false,
    evidence_class: 'descriptive_context_not_candidate_validation' };
  if (body) Object.assign(common, { path: 'src/core.spx', module: 'calculator.core', source_revision: digest(10), contracts: { requires: [], ensures: [] } });
  else Object.assign(common, { expression_id: hole.expressionId, expected_ownership: 'value',
    source: catalog(hole.kind, hole.target).source, selected_expression: expressionRow(contract ? 'requires' : 'body'),
    nonclaims: ['lexical_scope_is_not_owned_value_liveness', 'no_execution_or_publication_authority'] });
  if (contract) common.effect_policy.enclosing_declared_effects = [];
  return common;
}
function navigation(hole, revision) {
  const full = context(hole, revision);
  const contextRevision = hash('semaprax.project-hole-context.v1\0', canonical(full) + '\n');
  const items = {
    scope: full.scope.map(row => ({ id: row.id || row.value_id, name: row.name, type_id: row.type_id || row.type, ownership: row.ownership, mutable: hole.kind === 'body' ? null : row.mutable })),
    calls: full.accessible_calls, obligations: full.obligations, constructors: full.constructor_kinds,
  };
  const facets = FACETS.map(facet => ({ facet, count: items[facet].length,
    reference: hash('semaprax.project-hole-facet.v1\0', canonical({ draft_revision: revision, hole_id: hole.holeId, context_revision: contextRevision, facet }) + '\n') }));
  const summary = { schema: 'semaprax.project-hole-summary.v1', context_schema: full.schema, context_revision: contextRevision,
    draft_revision: revision, hole_id: hole.holeId, hole_handle: full.hole_handle, target: hole.target,
    last_valid_revision: PROJECT, expected_type_id: full.expected_type_id, expected_ownership: full.expected_ownership || null,
    intent_kind: full.intent_kind, effect_policy: { ...full.effect_policy, enclosing_declared_effects: full.effect_policy.enclosing_declared_effects || null },
    facets, full_context_method: 'hole/query', materializable: false, source_authority: false,
    validation: 'pending_fill_full_source_replay', evidence_class: 'descriptive_context_not_candidate_validation' };
  return { full, summary, page(facet, offset = 0, limit = 16) {
    const selected = items[facet].slice(offset, offset + limit), total = items[facet].length;
    return { schema: 'semaprax.project-hole-page.v1', draft_revision: revision, hole_id: hole.holeId,
      context_revision: contextRevision, facet, reference: facets.find(row => row.facet === facet).reference,
      total, offset, next_offset: offset + selected.length < total ? offset + selected.length : null,
      items: selected, source_authority: false };
  } };
}
class Server {
  constructor() { this.queue = []; this.calls = []; }
  expect(method, payload, image = IMAGE) { this.queue.push({ method, payload, image }); return this; }
  async call(method, params) {
    this.calls.push({ method, params: structuredClone(params) });
    if (method === 'hole/discard' && this.queue[0]?.method !== method) {
      assert.equal(params.image_revision, IMAGE);
      return { image_revision: IMAGE, payload: { schema: 'semaprax.image-draft-discard.v1', draft_revision: params.draft_revision, discarded: true, source_unchanged: true } };
    }
    const next = this.queue.shift(); assert.ok(next, `unexpected ${method}`); assert.equal(method, next.method);
    assert.equal(params.image_revision, IMAGE);
    if (next.payload instanceof Error) throw next.payload;
    return { image_revision: next.image, payload: structuredClone(next.payload) };
  }
}
function state(controller) {
  return structuredClone({ draftRevision: controller.draftRevision, sourceCandidate: controller.sourceCandidate,
    pending: controller.pending, hasFilled: controller.hasFilled });
}
async function prepared(holes = HOLES) {
  const server = new Server(), controller = new HoleDraft(server.call.bind(server), IMAGE, CANDIDATE);
  for (const [index, hole] of holes.entries()) {
    if (hole.kind !== 'body') {
      server.expect(controller.draftRevision !== null ? 'hole/expression-catalog' :
        hole.kind === 'contract' ? 'candidate/contract-expression-catalog' : 'expression/catalog',
        controller.draftRevision !== null ? draftCatalog(hole.kind, hole.target, controller.draftRevision) : catalog(hole.kind, hole.target));
      await controller.expressionChoices(hole.kind, hole.target);
    }
    server.expect(OPEN[hole.kind], draftHandle(digest(20 + index)));
    await controller.open(hole.kind, hole.target, hole.holeId, hole.expressionId);
  }
  return { server, controller };
}
const protocolFailure = error => error.protocolInvalid === true;

test('mixed three-hole planning, partial fills and explicit completion never install an unfinished candidate', async () => {
  const { server, controller } = await prepared();
  assert.equal(controller.sourceCandidate, CANDIDATE); assert.equal(controller.pending.length, 3); assert.equal(controller.hasFilled, false);
  assert.deepEqual(controller.pending.map(row => row.kind), ['body', 'expression', 'contract']);
  const opens = server.calls.filter(call => Object.values(OPEN).includes(call.method));
  assert.equal(opens[0].params.draft_revision, undefined);
  assert.equal(opens[1].params.draft_revision, digest(20));
  assert.equal(opens[2].params.expression_id, HOLES[2].expressionId);
  const planned = state(controller), count = server.calls.length;
  await assert.rejects(controller.complete()); assert.equal(server.calls.length, count); assert.deepEqual(state(controller), planned);
  for (const [index, hole] of HOLES.entries()) {
    server.expect('hole/fill', draftHandle(digest(30 + index)));
    await controller.fill(hole.holeId, hole.kind === 'contract' ? { kind: 'bool', value: true } : { kind: 'i64', value: 7 });
    assert.equal(controller.pending.length, 2 - index); assert.equal(controller.hasFilled, true);
    assert.equal(controller.sourceCandidate, CANDIDATE);
  }
  assert.equal(server.calls.filter(call => call.method === 'hole/complete').length, 0);
  const current = draftCatalog('expression', HOLES[1].target, controller.draftRevision);
  current.expressions[0].expression_id = 'calculator.subtract/expr/after-fill';
  server.expect('hole/expression-catalog', current);
  const choices = await controller.expressionChoices('expression', HOLES[1].target);
  assert.equal(server.calls.at(-1).params.draft_revision, digest(32));
  assert.equal(server.calls.at(-1).params.candidate_revision, undefined);
  server.expect('hole/open-expression', draftHandle(digest(33)));
  await controller.open('expression', HOLES[1].target, 'later', choices[0].expression_id);
  assert.equal(controller.pending.length, 1); assert.equal(controller.hasFilled, true);
  assert.equal(server.calls.some(call => call.params.candidate_revision === current.last_valid_candidate_digest), false);
  server.expect('hole/fill', draftHandle(digest(34)));
  await controller.fill('later', { kind: 'i64', value: 9 });
  const completed = candidateHandle(); server.expect('hole/complete', completed);
  assert.deepEqual(await controller.complete(), completed);
  assert.equal(server.calls.filter(call => call.method === 'hole/complete').length, 1);
  assert.equal(server.queue.length, 0);
});

test('semantic fill rejection and malformed mutation handles leave all local state atomic', async () => {
  const { server, controller } = await prepared([HOLES[0]]), before = state(controller);
  const rejected = new Error('SPX-G225 invalid typed constructor'); rejected.semantic = true;
  server.expect('hole/fill', rejected);
  await assert.rejects(controller.fill('body', { kind: 'place', name: 'missing' }), error => error === rejected);
  assert.deepEqual(state(controller), before);
  for (const mutate of [
    row => { row.source_candidate_revision = digest(99); }, row => { row.source_authority = true; },
    row => { row.buildable = true; }, row => { row.extra = 'untrusted'; }, row => { row.report_bytes = -1; },
    row => { row.draft_revision = before.draftRevision; },
  ]) {
    const fresh = await prepared([HOLES[0]]);
    const handle = draftHandle(digest(25)); mutate(handle); fresh.server.expect('hole/fill', handle);
    await assert.rejects(fresh.controller.fill('body', { kind: 'i64', value: 7 }), protocolFailure);
    assert.deepEqual(state(fresh.controller), before);
    const calls = fresh.server.calls.length;
    await assert.rejects(fresh.controller.fill('body', { kind: 'i64', value: 7 }));
    assert.equal(fresh.server.calls.length, calls);
  }
  server.expect('hole/fill', draftHandle(digest(25)), digest(99));
  await assert.rejects(controller.fill('body', { kind: 'i64', value: 7 }), protocolFailure);
  assert.deepEqual(state(controller), before);
});

test('summary and page bind every hole/context/reference and require ordered progressing bounded counts', async () => {
  const { server, controller } = await prepared([HOLES[0]]);
  const fixture = navigation(HOLES[0], controller.draftRevision);
  for (const mutate of [
    row => { row.source_authority = true; }, row => { row.hole_id = 'foreign'; },
    row => { row.context_schema = SCHEMAS.contract; }, row => { row.target = 'foreign.target'; },
    row => { row.facets[0].reference = digest(99); }, row => { row.facets.reverse(); },
  ]) {
    const fresh = await prepared([HOLES[0]]);
    const bad = structuredClone(fixture.summary); mutate(bad); fresh.server.expect('hole/summary', bad);
    await assert.rejects(fresh.controller.summary('body'), protocolFailure);
  }
  server.expect('hole/summary', fixture.summary);
  const summary = await controller.summary('body'); assert.deepEqual(summary, fixture.summary);
  for (const facet of FACETS) {
    const page = fixture.page(facet); server.expect('hole/page', page);
    assert.deepEqual(await controller.page(summary, facet), page);
  }
  for (const mutate of [
    row => { row.hole_id = 'foreign'; }, row => { row.draft_revision = digest(99); },
    row => { row.context_revision = digest(99); }, row => { row.reference = digest(99); },
    row => { row.facet = 'scope'; }, row => { row.offset = 1; }, row => { row.total = 99; },
    row => { row.items = []; row.next_offset = 0; }, row => { row.next_offset = 0; },
    row => { row.next_offset = null; }, row => { row.source_authority = true; },
    row => { row.items = ['one', 'two']; row.next_offset = 2; },
  ]) {
    const fresh = await prepared([HOLES[0]]);
    fresh.server.expect('hole/summary', fixture.summary);
    const selected = await fresh.controller.summary('body');
    const page = fixture.page('obligations', 0, 1); mutate(page); fresh.server.expect('hole/page', page);
    await assert.rejects(fresh.controller.page(selected, 'obligations', 0, 1), protocolFailure);
  }
  const first = fixture.page('obligations', 0, 1), last = fixture.page('obligations', 1, 1);
  server.expect('hole/page', first).expect('hole/page', last);
  assert.equal((await controller.page(summary, 'obligations', 0, 1)).next_offset, 1);
  assert.equal((await controller.page(summary, 'obligations', 1, 1)).next_offset, null);
  const requests = server.calls.length;
  await assert.rejects(controller.page(summary, 'scope', 16385, 1));
  await assert.rejects(controller.page(summary, 'scope', 0, 65));
  assert.equal(server.calls.length, requests);
});

test('successful fills stale earlier summaries and full context remains explicitly descriptive', async () => {
  const { server, controller } = await prepared();
  const fixture = navigation(HOLES[1], controller.draftRevision);
  server.expect('hole/summary', fixture.summary).expect('hole/query', fixture.full);
  const summary = await controller.summary('expression');
  assert.deepEqual(await controller.context('expression'), fixture.full);
  server.expect('hole/fill', draftHandle(digest(30)));
  await controller.fill('body', { kind: 'i64', value: 1 });
  const requests = server.calls.length;
  await assert.rejects(controller.page(summary, 'scope')); assert.equal(server.calls.length, requests);
  for (const mutate of [row => { row.target = 'foreign.target'; }, row => { row.draft_digest = digest(99); }, row => { row.source_authority = true; }, row => { row.materializable = true; }]) {
    const fresh = await prepared([HOLES[1]]);
    const full = navigation(HOLES[1], fresh.controller.draftRevision).full; mutate(full); fresh.server.expect('hole/query', full);
    await assert.rejects(fresh.controller.context('expression'), protocolFailure);
  }
  assert.equal(controller.pending.length, 2); assert.equal(controller.hasFilled, true);
});

test('expression catalog selection retains only replaceable rows of the selected body or contract owner', async () => {
  const server = new Server(), controller = new HoleDraft(server.call.bind(server), IMAGE, CANDIDATE);
  await assert.rejects(controller.open('expression', HOLES[1].target, 'unselected', HOLES[1].expressionId));
  assert.equal(server.calls.length, 0);
  for (const kind of ['expression', 'contract']) {
    const target = kind === 'expression' ? HOLES[1].target : HOLES[2].target;
    const report = catalog(kind, target), method = kind === 'expression' ? 'expression/catalog' : 'candidate/contract-expression-catalog';
    server.expect(method, report);
    assert.deepEqual(await controller.expressionChoices(kind, target), [report.expressions[0]]);
    for (const key of ['target', 'candidate_digest']) {
      const fresh = new Server(), selected = new HoleDraft(fresh.call.bind(fresh), IMAGE, CANDIDATE);
      const bad = structuredClone(report); bad[key] = key === 'target' ? 'wrong.owner' : digest(99); fresh.expect(method, bad);
      await assert.rejects(selected.expressionChoices(kind, target), protocolFailure);
    }
  }
  assert.equal(controller.pending.length, 0); assert.equal(controller.draftRevision, null);
});

test('opening any hole and filling a hole invalidate all prior candidate and draft expression choices', async () => {
  const server = new Server(), controller = new HoleDraft(server.call.bind(server), IMAGE, CANDIDATE);
  const hole = HOLES[1];
  server.expect('expression/catalog', catalog('expression', hole.target));
  await controller.expressionChoices('expression', hole.target);
  server.expect('hole/open', draftHandle(digest(20)));
  await controller.open('body', HOLES[0].target, 'body');
  let count = server.calls.length;
  await assert.rejects(controller.open('expression', hole.target, 'stale-candidate', hole.expressionId));
  assert.equal(server.calls.length, count);
  server.expect('hole/expression-catalog', draftCatalog('expression', hole.target, digest(20)));
  await controller.expressionChoices('expression', hole.target);
  server.expect('hole/open', draftHandle(digest(21)));
  await controller.open('body', 'calculator.multiply', 'multiply');
  count = server.calls.length;
  await assert.rejects(controller.open('expression', hole.target, 'stale-draft', hole.expressionId));
  assert.equal(server.calls.length, count);
  server.expect('hole/expression-catalog', draftCatalog('expression', hole.target, digest(21)));
  await controller.expressionChoices('expression', hole.target);
  server.expect('hole/fill', draftHandle(digest(22)));
  await controller.fill('body', { kind: 'i64', value: 7 });
  count = server.calls.length;
  await assert.rejects(controller.open('expression', hole.target, 'stale-fill', hole.expressionId));
  assert.equal(server.calls.length, count);
  const contract = draftCatalog('contract', HOLES[2].target, digest(22));
  contract.expressions[0].expression_id = 'calculator.divide/requires/after-fill';
  server.expect('hole/expression-catalog', contract);
  const selected = await controller.expressionChoices('contract', HOLES[2].target);
  assert.equal(server.calls.at(-1).params.region, 'contract');
  server.expect('hole/open-contract-expression', draftHandle(digest(23)));
  await controller.open('contract', HOLES[2].target, 'new-contract', selected[0].expression_id);
  assert.equal(controller.pending.length, 2); assert.equal(controller.sourceCandidate, CANDIDATE);
});

test('draft catalog rejects stale identities, wrong region, malformed provenance and authority claims', async () => {
  for (const mutate of [
    row => { row.draft_revision = digest(99); }, row => { row.last_valid_revision = 'invalid'; },
    row => { row.last_valid_candidate_digest = 'invalid'; }, row => { row.region = 'contract'; },
    row => { row.target = 'foreign.target'; }, row => { row.source_authority = true; },
    row => { row.materializable = true; }, row => { row.selection_admission = 'approved'; },
    row => { row.evidence_class = 'validated'; }, row => { row.candidate_revision = CANDIDATE; },
    row => { row.expressions[0].phase = 'requires'; },
  ]) {
    const { server, controller } = await prepared([HOLES[0]]), before = state(controller);
    const report = draftCatalog('expression', HOLES[1].target, controller.draftRevision); mutate(report);
    server.expect('hole/expression-catalog', report);
    await assert.rejects(controller.expressionChoices('expression', HOLES[1].target), protocolFailure);
    assert.deepEqual(state(controller), before);
  }
});

test('a host without the draft catalog method fails without falling back to the original candidate', async () => {
  const { server, controller } = await prepared([HOLES[0]]), before = state(controller);
  const missing = new Error('Method not available in this host-selected protocol'); missing.semantic = true;
  server.expect('hole/expression-catalog', missing);
  const start = server.calls.length;
  await assert.rejects(controller.expressionChoices('expression', HOLES[1].target), error => error === missing);
  assert.deepEqual(server.calls.slice(start).map(call => call.method), ['hole/expression-catalog']);
  assert.deepEqual(state(controller), before);
  await assert.rejects(controller.open('expression', HOLES[1].target, 'unselected', HOLES[1].expressionId));
  assert.equal(server.calls.length, start + 1);
});

test('serial requests and explicit discard cannot silently race or acquire execution/archive authority', async () => {
  let release, requests = 0;
  const controller = new HoleDraft(async () => { requests++; return new Promise(resolve => { release = resolve; }); }, IMAGE, CANDIDATE);
  const pending = controller.open('body', 'calculator.add', 'body');
  await assert.rejects(controller.open('body', 'calculator.subtract', 'other'));
  assert.equal(requests, 1); release({ image_revision: IMAGE, payload: draftHandle(digest(20)) }); await pending;
  const { server, controller: removable } = await prepared([HOLES[0]]);
  server.expect('hole/discard', { schema: 'semaprax.image-draft-discard.v1', draft_revision: removable.draftRevision, discarded: true, source_unchanged: true });
  await removable.discard(); assert.equal(removable.draftRevision, null); assert.equal(removable.pending.length, 0);
  const child = new EventEmitter(); child.stdout = new EventEmitter(); child.stderr = new EventEmitter(); child.stdin = new EventEmitter();
  let writes = 0; child.stdin.write = () => { writes++; return true; }; child.kill = () => {};
  const client = new McpClient(child);
  for (const method of ['hole/open', 'hole/open-expression', 'hole/open-contract-expression', 'hole/summary', 'hole/page', 'hole/query', 'hole/expression-catalog', 'hole/fill', 'hole/complete', 'hole/discard', 'expression/catalog', 'candidate/contract-expression-catalog']) assert.equal(ALLOWED.has(method), true, method);
  for (const method of ['candidate/build', 'candidate/test', 'candidate/commit', 'candidate/commit-report', 'hole/archive-export', 'hole/archive-restore', 'hole/recovery-restore']) {
    assert.equal(ALLOWED.has(method), false, method); client.tools.add(method);
    await assert.rejects(client.call(method, {}), /allowlist/);
  }
  assert.equal(writes, 0); client.stop();
});

test('sixteen planned holes and sixteen fills retain at most two owned draft handles', async () => {
  const live = new Set(), calls = []; let sequence = 100, peak = 0, failFill = false;
  const rejected = new Error('SPX-G225 invalid constructor'); rejected.semantic = true;
  const controller = new HoleDraft(async (method, params) => {
    calls.push({ method, params: structuredClone(params) }); assert.equal(params.image_revision, IMAGE);
    if (method === 'hole/discard') {
      assert.equal(live.delete(params.draft_revision), true, 'retire only a live owned handle');
      return { image_revision: IMAGE, payload: { schema: 'semaprax.image-draft-discard.v1', draft_revision: params.draft_revision, discarded: true, source_unchanged: true } };
    }
    if (params.draft_revision) assert.equal(live.has(params.draft_revision), true);
    if (method === 'hole/complete') return { image_revision: IMAGE, payload: candidateHandle() };
    assert.ok(method === 'hole/open' || method === 'hole/fill');
    if (method === 'hole/fill' && failFill) { failFill = false; throw rejected; }
    assert.ok(live.size < 16, 'actual host registry admission limit');
    const revision = digest(sequence++); live.add(revision); peak = Math.max(peak, live.size);
    return { image_revision: IMAGE, payload: draftHandle(revision) };
  }, IMAGE, CANDIDATE);
  for (let index = 0; index < 16; index++) {
    await controller.open('body', `fixture.function.${index}`, `hole${index}`);
    assert.equal(live.size, 1);
  }
  const selected = controller.draftRevision, before = state(controller), discards = calls.filter(call => call.method === 'hole/discard').length;
  failFill = true; await assert.rejects(controller.fill('hole0', { kind: 'i64', value: 0 }), error => error === rejected);
  assert.equal(live.has(selected), true); assert.deepEqual(state(controller), before);
  assert.equal(calls.filter(call => call.method === 'hole/discard').length, discards);
  for (let index = 0; index < 16; index++) {
    await controller.fill(`hole${index}`, { kind: 'i64', value: index });
    assert.equal(live.size, 1);
  }
  assert.equal(peak, 2); assert.equal(calls.some(call => call.method === 'hole/complete'), false);
  await controller.complete(); assert.equal(live.size, 0);
  assert.equal(calls.filter(call => call.method === 'hole/complete').length, 1);
});

test('failed retirement keeps the newly adopted state and closes without rollback or retries', async () => {
  const { server, controller } = await prepared([HOLES[0]]), old = controller.draftRevision;
  server.expect('hole/open', draftHandle(digest(25))).expect('hole/discard', {
    schema: 'semaprax.image-draft-discard.v1', draft_revision: old, discarded: false, source_unchanged: true,
  });
  await assert.rejects(controller.open('body', 'calculator.multiply', 'extra'), protocolFailure);
  assert.equal(controller.draftRevision, digest(25)); assert.equal(controller.pending.length, 2);
  const count = server.calls.length;
  await assert.rejects(controller.summary('body')); await assert.rejects(controller.fill('body', { kind: 'i64', value: 7 }));
  assert.equal(server.calls.length, count);
  assert.equal(server.calls.filter(call => call.method === 'hole/discard').length, 1);
});

test('source-epoch invalidation discards a pending fill and prevents stale scratch retries', async () => {
  const { server, controller } = await prepared([HOLES[0]]), before = state(controller);
  // The extension's saved-source/epoch fence returns this marker when an
  // asynchronous response outlives the editor selection that submitted it.
  const drift = new Error('Source changed while the request was pending; result discarded'); drift.discardOnly = true;
  server.expect('hole/fill', drift);
  const expression = { kind: 'i64', value: 7 };
  await assert.rejects(controller.fill('body', expression), error => error === drift);
  assert.deepEqual(state(controller), before);
  const calls = server.calls.length;
  await assert.rejects(controller.fill('body', expression));
  await assert.rejects(controller.context('body'));
  assert.equal(server.calls.length, calls);
  assert.equal(server.calls.some(call => call.method === 'hole/discard'), false);
});

test('constructor schemas remain bounded descriptive documents with no request or source authority', async () => {
  const server = new Server(), controller = new HoleDraft(server.call.bind(server), IMAGE, CANDIDATE);
  const schemas = {
    schema: 'semaprax.candidate-constructor-schemas.v1', admission: 'closed_structural_grammar_only', requires_compiler_admission: true,
    documents: ['urn:semaprax.typed-expression.v1', 'urn:semaprax.semantic-change-intent.v1', 'urn:semaprax.semantic-change.v1', 'urn:semaprax.project-candidate-recovery.v1'].map($id => ({
      $id, $schema: 'https://json-schema.org/draft/2020-12/schema', $defs: { expression: { oneOf: [{ type: 'object', additionalProperties: false, required: ['kind', 'value'], properties: { kind: { const: 'i64' }, value: { type: 'integer' } } }] } },
    })),
    limits: { max_change_bytes: 1048576, max_json_value_nodes: 8192, max_json_value_depth: 64, max_expression_nodes: 4096, max_expression_depth: 64 },
    nonclaims: ['not_type_scope_effect_ownership_or_target_admission', 'no_source_or_commit_authority'],
  };
  server.expect('protocol/constructor-schemas', schemas);
  assert.deepEqual(await controller.constructorSchemas(), schemas);
  for (const mutate of [row => { row.requires_compiler_admission = false; }, row => { row.documents.reverse(); }, row => { row.source_authority = true; }, row => { row.limits.max_expression_depth = 65; }]) {
    const fresh = new Server(), selected = new HoleDraft(fresh.call.bind(fresh), IMAGE, CANDIDATE);
    const bad = structuredClone(schemas); mutate(bad); fresh.expect('protocol/constructor-schemas', bad);
    await assert.rejects(selected.constructorSchemas(), protocolFailure);
  }
  assert.equal(controller.draftRevision, null); assert.equal(controller.pending.length, 0);
  assert.ok(server.calls.every(call => call.method === 'protocol/constructor-schemas'));
});
