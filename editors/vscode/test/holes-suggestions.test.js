'use strict';
// Authored mock-controller regressions only. These closed wire fixtures do not
// claim compiler admission of their expressions. No Node/editor execution here.
const test = require('node:test');
const assert = require('node:assert/strict');
const { HoleDraft } = require('../holes');
const { hash, canonical } = require('../review');

const digest = n => 'sha256:' + n.toString(16).padStart(64, '0');
const IMAGE = digest(1), CANDIDATE = digest(2), PROJECT = digest(3), DRAFT = digest(4);
const clone = value => structuredClone(value);
const TYPES = {
  body: ['hole/open', 'semaprax.project-candidate-hole-context.v1', 'replace_function_body'],
  expression: ['hole/open-expression', 'semaprax.project-candidate-expression-hole-context.v1', 'replace_expression'],
  contract: ['hole/open-contract-expression', 'semaprax.project-candidate-contract-expression-hole-context.v1', 'replace_contract_expression'],
};
const NONCLAIMS = ['not_intent_correctness', 'not_runtime_contract_proof', 'not_complete_expression_search', 'not_liveness_inference'];
const place = name => ({ kind: 'place', name });
const call = (target, names) => ({ kind: 'call', target, arguments: names.map(place) });
function handle(revision = DRAFT) {
  return { schema: 'semaprax.image-draft-handle.v1', draft_revision: revision,
    source_candidate_revision: CANDIDATE, report_bytes: 512, source_authority: false, buildable: false };
}
function summaryFor(kind, revision = DRAFT, holeId = 'selected') {
  const context = digest(10), target = 'calculator.selected';
  return { schema: 'semaprax.project-hole-summary.v1', context_schema: TYPES[kind][1],
    context_revision: context, draft_revision: revision, hole_id: holeId, hole_handle: digest(11), target,
    last_valid_revision: PROJECT, expected_type_id: kind === 'contract' ? 'bool' : 'i64',
    expected_ownership: kind === 'body' ? null : 'value', intent_kind: TYPES[kind][2],
    effect_policy: { allowed: [], forbidden: kind === 'contract' ? 'all_effects_in_contract_predicates' : 'all_undeclared_effects',
      module_permits: [], enclosing_declared_effects: kind === 'contract' ? [] : null },
    facets: ['scope', 'calls', 'obligations', 'constructors'].map(facet => ({ facet, count: 2,
      reference: hash('semaprax.project-hole-facet.v1\0', canonical({ draft_revision: revision, hole_id: holeId, context_revision: context, facet }) + '\n') })),
    full_context_method: 'hole/query', materializable: false, source_authority: false,
    validation: 'pending_fill_full_source_replay', evidence_class: 'descriptive_context_not_candidate_validation' };
}
function catalogue(kind) {
  return { schema: kind === 'contract' ? 'semaprax.project-contract-expression-catalog.v1' : 'semaprax.project-expression-catalog.v1',
    candidate_digest: CANDIDATE, project_revision: PROJECT, target: 'calculator.selected',
    source: { path: 'src/core.spx', module: 'calculator.core', source_revision: digest(12), source_digest: digest(13) },
    declared_effect_budget: [], expressions: [{ expression_id: 'calculator.selected/expr/0', phase: kind === 'contract' ? 'requires' : 'body',
      kind: 'binary', expected_type: kind === 'contract' ? 'bool' : 'i64', ownership: 'value',
      source_span: { start: 1, end: 10, line: 1, column: 1 }, replaceable: true,
      reason: 'requires_typed_constructor_and_full_project_revalidation',
      scope: [{ name: 'left', value_id: 'calculator.selected/parameter/0', type: 'i64', ownership: 'value', mutable: false }] }],
    limits: { max_expressions: 4096, max_depth: 256, max_scope_facts: 16384, max_bytes: 1048576 },
    nonclaims: ['lexical_scope_is_not_owned_value_liveness', 'no_source_or_commit_authority'] };
}
function suggestions(summary) {
  const flag = summary.expected_type_id === 'bool';
  return { schema: 'semaprax.project-hole-fill-suggestions.v1', draft_revision: summary.draft_revision,
    hole_id: summary.hole_id, context_revision: summary.context_revision, last_valid_revision: summary.last_valid_revision,
    expected_type_id: summary.expected_type_id, considered: 2, rejected: 0, search_exhausted: true,
    suggestions: [{ expression: place(flag ? 'flag' : 'left'), preview_draft_revision: digest(20) },
      { expression: call(flag ? 'calculator.not' : 'calculator.subtract', flag ? ['flag'] : ['left', 'right']), preview_draft_revision: digest(21) }],
    validation: 'ordinary_fill_source_replay', tests: 'not_run', source_authority: false, draft_retained: false, nonclaims: [...NONCLAIMS] };
}
class Server {
  constructor() { this.calls = []; this.queue = []; }
  expect(method, payload, image = IMAGE) { this.queue.push({ method, payload, image }); return this; }
  async call(method, params) {
    this.calls.push({ method, params: clone(params) });
    assert.equal(params.image_revision, IMAGE);
    if (method === 'hole/discard' && this.queue[0]?.method !== method) {
      return { image_revision: IMAGE, payload: { schema: 'semaprax.image-draft-discard.v1',
        draft_revision: params.draft_revision, discarded: true, source_unchanged: true } };
    }
    const next = this.queue.shift(); assert.ok(next, `unexpected ${method}`); assert.equal(method, next.method);
    if (next.payload instanceof Error) throw next.payload;
    if (typeof next.payload === 'function') return next.payload();
    return { image_revision: next.image, payload: clone(next.payload) };
  }
}
const state = controller => clone({ draftRevision: controller.draftRevision, sourceCandidate: controller.sourceCandidate,
  pending: controller.pending, hasFilled: controller.hasFilled });
async function prepared(kind = 'body') {
  const server = new Server(), controller = new HoleDraft(server.call.bind(server), IMAGE, CANDIDATE);
  if (kind !== 'body') {
    server.expect(kind === 'contract' ? 'candidate/contract-expression-catalog' : 'expression/catalog', catalogue(kind));
    await controller.expressionChoices(kind, 'calculator.selected');
  }
  server.expect(TYPES[kind][0], handle());
  await controller.open(kind, 'calculator.selected', 'selected', kind === 'body' ? undefined : 'calculator.selected/expr/0');
  server.expect('hole/summary', summaryFor(kind));
  const summary = await controller.summary('selected');
  return { server, controller, summary };
}
const protocolFailure = error => error.protocolInvalid === true;
async function closedAfter(controller, server) {
  const before = server.calls.length;
  await assert.rejects(controller.fill('selected', place('left')));
  await assert.rejects(controller.discard());
  assert.equal(server.calls.length, before, 'terminal controllers cannot issue writes');
}

test('body, expression and contract suggestions are descriptive copies until an explicit ordinary fill', async () => {
  for (const kind of Object.keys(TYPES)) {
    const { server, controller, summary } = await prepared(kind), before = state(controller);
    const report = suggestions(summary); server.expect('hole/fill-suggestions', report);
    assert.deepEqual(await controller.fillSuggestions(summary), report);
    assert.deepEqual(state(controller), before);
    assert.deepEqual(server.calls.at(-1), { method: 'hole/fill-suggestions', params: {
      image_revision: IMAGE, draft_revision: DRAFT, hole_id: 'selected' } });
    const reads = server.calls.filter(row => !row.method.includes('catalog'));
    assert.deepEqual(reads.map(row => row.method), [TYPES[kind][0], 'hole/summary', 'hole/fill-suggestions']);
    assert.equal(server.calls.some(row => ['hole/fill', 'hole/complete', 'hole/discard', 'candidate/open'].includes(row.method)), false);
    server.expect('hole/fill', handle(report.suggestions[0].preview_draft_revision));
    await controller.fill('selected', report.suggestions[0].expression);
    assert.equal(controller.draftRevision, report.suggestions[0].preview_draft_revision);
    assert.equal(controller.pending.length, 0); assert.equal(controller.hasFilled, true);
    assert.equal(server.calls.filter(row => row.method === 'hole/complete').length, 0);
    assert.deepEqual(server.calls.slice(-2).map(row => row.method), ['hole/fill', 'hole/discard']);
  }
});

test('empty and attempt-limited reports keep their precise restricted-search meaning', async () => {
  for (const exhausted of [true, false]) {
    const { server, controller, summary } = await prepared(), before = state(controller);
    const report = suggestions(summary);
    Object.assign(report, { considered: exhausted ? 0 : 32, rejected: exhausted ? 0 : 32,
      search_exhausted: exhausted, suggestions: [] });
    server.expect('hole/fill-suggestions', report);
    assert.deepEqual(await controller.fillSuggestions(summary), report);
    assert.deepEqual(state(controller), before);
  }
  const { server, controller, summary } = await prepared();
  const report = suggestions(summary);
  Object.assign(report, { considered: 32, rejected: 0, search_exhausted: false,
    suggestions: Array.from({ length: 32 }, (_, index) => ({ expression: place(`p${index}`), preview_draft_revision: digest(50 + index) })) });
  server.expect('hole/fill-suggestions', report);
  assert.equal((await controller.fillSuggestions(summary)).suggestions.length, 32);
});

test('closed report bindings and counts fail atomically and close the controller', async () => {
  const corruptions = [
    row => { row.schema = 'semaprax.project-hole-summary.v1'; },
    row => { row.draft_revision = digest(90); }, row => { row.hole_id = 'foreign'; },
    row => { row.context_revision = digest(91); }, row => { row.last_valid_revision = digest(92); },
    row => { row.expected_type_id = 'bool'; }, row => { row.source_authority = true; },
    row => { row.draft_retained = true; }, row => { row.tests = 'passed'; },
    row => { row.validation = 'shape_only'; }, row => { row.extra = true; },
    row => { delete row.context_revision; }, row => { row.nonclaims.pop(); },
    row => { row.considered = 33; }, row => { row.considered = -1; },
    row => { row.considered = 1.5; }, row => { row.considered = '2'; },
    row => { row.rejected = 3; }, row => { row.rejected = -1; },
    row => { row.considered = 3; }, row => { row.search_exhausted = 1; },
    row => { row.suggestions[0].preview_draft_revision = DRAFT; },
    row => { row.suggestions[0].preview_draft_revision = 'invalid'; },
    row => { row.suggestions[0].preview_draft_revision = digest(20) + '\n'; },
    row => { row.suggestions[0].extra = 'untrusted'; }, row => { row.suggestions = {}; },
  ];
  for (const corrupt of corruptions) {
    const { server, controller, summary } = await prepared(), before = state(controller);
    const report = suggestions(summary); corrupt(report); server.expect('hole/fill-suggestions', report);
    await assert.rejects(controller.fillSuggestions(summary), protocolFailure);
    assert.deepEqual(state(controller), before); await closedAfter(controller, server);
  }
  const { server, controller, summary } = await prepared(), before = state(controller);
  server.expect('hole/fill-suggestions', suggestions(summary), digest(99));
  await assert.rejects(controller.fillSuggestions(summary), protocolFailure);
  assert.deepEqual(state(controller), before); await closedAfter(controller, server);
});

test('suggestions contain only bounded identifier places or direct calls with place arguments', async () => {
  const expressions = [
    { kind: 'i64', value: 7 }, { kind: 'builtin_call', target: 'core.bytes.len', arguments: [] },
    { kind: 'place', name: 7 }, place(''), place('λ'), place('a.b'), place('a\0b'), place('input\n'), place('a'.repeat(129)),
    place('let'), place('self'), place('return'), { kind: 'place', name: 'left', extra: true },
    { kind: 'call', target: 7, arguments: [] }, call('', []), call('bad\nidentity', []), call('x'.repeat(4097), []), call('calculator.selected', []),
    { kind: 'call', target: 'calculator.other', arguments: [7] },
    { kind: 'call', target: 'calculator.other', arguments: [call('calculator.third', [])] },
    { kind: 'call', target: 'calculator.other', arguments: [{ kind: 'i64', value: 0 }] },
    { kind: 'call', target: 'calculator.other', arguments: {} }, call('calculator.other', Array(65).fill('left')),
    { ...call('calculator.other', ['left']), type_arguments: [] },
  ];
  for (const expression of expressions) {
    const { server, controller, summary } = await prepared(), before = state(controller);
    const report = suggestions(summary); report.suggestions[0].expression = expression;
    server.expect('hole/fill-suggestions', report);
    await assert.rejects(controller.fillSuggestions(summary), protocolFailure);
    assert.deepEqual(state(controller), before); await closedAfter(controller, server);
  }
  // The boundary itself remains admitted by the structural controller. These
  // mock expressions are not assertions of compiler scope/type correctness.
  const { server, controller, summary } = await prepared();
  const report = suggestions(summary);
  report.suggestions[0].expression = place('x'.repeat(128));
  report.suggestions[1].expression = call('x'.repeat(4096), Array(64).fill('left'));
  server.expect('hole/fill-suggestions', report);
  assert.deepEqual(await controller.fillSuggestions(summary), report);
});

test('the report byte cap applies before any display or state adoption', async () => {
  const { server, controller, summary } = await prepared(), before = state(controller);
  const report = suggestions(summary);
  Object.assign(report, { considered: 32, rejected: 0, suggestions: Array.from({ length: 32 }, (_, index) => ({
    expression: call('x'.repeat(4096), []), preview_draft_revision: digest(50 + index) })) });
  assert.ok(Buffer.byteLength(JSON.stringify(report)) > 65536);
  server.expect('hole/fill-suggestions', report);
  await assert.rejects(controller.fillSuggestions(summary), protocolFailure);
  assert.deepEqual(state(controller), before); await closedAfter(controller, server);
});

test('edited, foreign and obsolete summary copies fail before RPC without discarding a valid draft', async () => {
  const { server, controller, summary } = await prepared(), before = state(controller);
  for (const edit of [row => { row.context_revision = digest(90); }, row => { row.hole_id = 'foreign'; },
    row => { row.target = 'foreign.target'; }, row => { row.facets[0].count++; }, row => { row.expected_type_id = 'bool'; }]) {
    const altered = clone(summary); edit(altered); const calls = server.calls.length;
    await assert.rejects(controller.fillSuggestions(altered));
    assert.equal(server.calls.length, calls); assert.deepEqual(state(controller), before);
  }
  const foreign = await prepared('contract'), calls = server.calls.length;
  await assert.rejects(controller.fillSuggestions(foreign.summary)); assert.equal(server.calls.length, calls);
  server.expect('hole/fill-suggestions', suggestions(summary));
  await controller.fillSuggestions(clone(summary)); // Exact value copies are current selectors.
  server.expect('hole/open', handle(digest(6)));
  await controller.open('body', 'calculator.second', 'second');
  const afterOpen = server.calls.length;
  await assert.rejects(controller.fillSuggestions(summary)); assert.equal(server.calls.length, afterOpen);
  server.expect('hole/summary', summaryFor('body', digest(6)));
  const current = await controller.summary('selected');
  server.expect('hole/fill', handle(digest(7)));
  await controller.fill('second', place('left'));
  const afterFill = server.calls.length;
  await assert.rejects(controller.fillSuggestions(current)); assert.equal(server.calls.length, afterFill);
  assert.equal(controller.pending[0].holeId, 'selected'); assert.equal(controller.hasFilled, true);
});

test('semantic rejection preserves usable state and mutation of a returned display copy cannot affect later reads', async () => {
  const { server, controller, summary } = await prepared(), before = state(controller);
  const rejected = new Error('SPX-G231 bounded report unavailable'); rejected.semantic = true;
  server.expect('hole/fill-suggestions', rejected);
  await assert.rejects(controller.fillSuggestions(summary), error => error === rejected);
  assert.deepEqual(state(controller), before);
  const report = suggestions(summary); server.expect('hole/fill-suggestions', report);
  const display = await controller.fillSuggestions(summary);
  display.draft_revision = digest(90); display.suggestions[0].expression.name = 'tampered'; display.nonclaims.length = 0;
  assert.deepEqual(state(controller), before);
  assert.equal(server.calls.some(row => row.method === 'hole/fill'), false);
  server.expect('hole/fill-suggestions', report);
  assert.deepEqual(await controller.fillSuggestions(summary), report);
  assert.equal(server.queue.length, 0);
});

test('busy suggestion reads forbid racing writes and use an immutable summary snapshot across await', async () => {
  const { server, controller, summary } = await prepared(), before = state(controller);
  const report = suggestions(summary); let release;
  server.expect('hole/fill-suggestions', () => new Promise(resolve => { release = resolve; }));
  const pending = controller.fillSuggestions(summary), calls = server.calls.length;
  await assert.rejects(controller.fill('selected', place('left')));
  await assert.rejects(controller.discard());
  await assert.rejects(controller.fillSuggestions(summary)); assert.equal(server.calls.length, calls);
  summary.context_revision = digest(99); summary.last_valid_revision = digest(98);
  release({ image_revision: IMAGE, payload: report });
  assert.deepEqual(await pending, report);
  assert.deepEqual(state(controller), before);
  assert.equal(server.calls.some(row => row.method === 'hole/fill' || row.method === 'hole/discard'), false);
});

test('late discard-only and malformed responses close the controller without issuing cleanup or later writes', async () => {
  for (const discarded of [true, false]) {
    const { server, controller, summary } = await prepared(), before = state(controller);
    let resolve, reject;
    server.expect('hole/fill-suggestions', () => new Promise((accept, fail) => { resolve = accept; reject = fail; }));
    const pending = controller.fillSuggestions(summary);
    const checked = assert.rejects(pending, error => discarded ? error.discardOnly === true : protocolFailure(error));
    if (discarded) { const error = new Error('Source epoch changed while awaiting the response'); error.discardOnly = true; reject(error); }
    else { const report = suggestions(summary); report.draft_retained = true; resolve({ image_revision: IMAGE, payload: report }); }
    await checked; assert.deepEqual(state(controller), before); await closedAfter(controller, server);
    assert.equal(server.calls.some(row => row.method === 'hole/fill' || row.method === 'hole/discard'), false);
  }
});
