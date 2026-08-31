'use strict';
// Authored, unrun controller regressions. These are schema-shaped server mocks,
// not evidence that a compiler admitted the represented source or repair.
const test = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { Repairs } = require('../repairs');
const { ALLOWED, McpClient, parse } = require('../protocol');
const { hash, canonical } = require('../review');

const digest = n => 'sha256:' + n.toString(16).padStart(64, '0');
const IMAGE = { image_revision: digest(1), project_revision: digest(2) };
const BASE = digest(3), CURRENT = digest(4), ORIGINAL = digest(5), ATTEMPT = digest(6), REPAIR = digest(7);
const TARGET = 'repair.inspect';
const intent = { kind: 'replace_function_body', target: TARGET, body: { kind: 'u8', value: 42 } };
const REQUIREMENTS = ['preserve_stable_identity', 'preserve_public_exports', 'update_all_callers',
  'no_new_effects', 'no_new_capabilities', 'preserve_contracts', 'revalidate_ownership_and_cleanup',
  'preserve_project_profile_admission', 'preserve_admitted_core_targets'];
const NONCLAIMS = ['not_general_diagnostic_repair', 'no_invalid_source_or_hir_admission', 'no_automatic_repair_selection'];
function candidate(revision = BASE, project = CURRENT) {
  return { schema: 'semaprax.image-candidate-handle.v1', candidate_revision: revision,
    project_revision: project, base_revision: ORIGINAL, report_bytes: 1024, source_authority: false, tests: 'not_run' };
}
function summary(revision = ATTEMPT, bytes = 1024) {
  return { schema: 'semaprax.project-candidate-attempt-summary.v1', attempt_revision: revision,
    base_candidate_revision: BASE, base_project_revision: CURRENT, state: 'rejected', diagnostic_count: 1,
    report_bytes: bytes, materializable: false, checked_image: false, source_authority: false };
}
function rejected(revision = ATTEMPT, bytes = 1024) {
  return { schema: 'semaprax.image-candidate-attempt-outcome.v1', status: 'rejected', candidate: null, attempt: summary(revision, bytes) };
}
function accepted(handle = candidate(digest(8), digest(9))) {
  return { schema: 'semaprax.image-candidate-attempt-outcome.v1', status: 'accepted', candidate: handle, attempt: null };
}
function change(body) {
  return { schema: 'semaprax.semantic-change.v1', base_revision: CURRENT,
    intent: { kind: 'replace_function_body', target: TARGET, body: structuredClone(body) }, requirements: [...REQUIREMENTS] };
}
function repair() {
  return { repair_id: REPAIR, class: 'retag_integer_literal_to_retained_return_type', target: TARGET,
    from_type: 'u8', expected_type: 'i64', preserved_integer_value: 42,
    change: change({ kind: 'i64', value: 42 }),
    semantic_change_intent: { kind: 'repair_diagnostic', target: TARGET, rejected_intent: intent, repair_id: REPAIR },
    validated_candidate_revision: digest(8), validation: 'normal_full_candidate_apply',
    evidence_owner: 'retained_target_return_type_and_full_candidate_admission', tests: 'not_run', source_authority: false };
}
function borrowRepair() {
  const row = repair();
  delete row.from_type; delete row.expected_type; delete row.preserved_integer_value;
  row.class = 'borrow_owned_byte_field_without_staging'; row.diagnostic_code = 'SPX-T266';
  row.replacement_count = 1; row.replacements = [{ field: 'repair.packet.bytes', root: 'packet' }];
  row.evidence_owner = 'closed_builtin_projection_pattern_and_full_candidate_admission';
  const body = { kind: 'builtin_call', target: 'core.bytes.as-slice', arguments: [
    { kind: 'field_place', target: 'repair.packet.bytes', root: 'packet' }] };
  row.change = change(body);
  row.semantic_change_intent.rejected_intent = { kind: 'replace_function_body', target: TARGET,
    body: { ...body, arguments: [{ kind: 'project', target: 'repair.packet.bytes', base: { kind: 'place', name: 'packet' } }] } };
  return row;
}
function catalog(revision = ATTEMPT, rows = [repair()]) {
  return { schema: 'semaprax.project-candidate-repair-catalog.v1', attempt_revision: revision,
    base_candidate_revision: BASE, base_project_revision: CURRENT, repairs: rows,
    availability_reason: rows.length ? 'one_compiler_admitted_typed_repair' : 'no_supported_repair_class',
    legacy_identity_repair: 'assign_function_id_is_a_breaking_identity_rebase_and_not_a_stable_identity_preserving_candidate_change',
    tests: 'not_run', source_authority: false, nonclaims: NONCLAIMS };
}
function discarded(revision) {
  return { schema: 'semaprax.image-attempt-discard.v1', attempt_revision: revision, discarded: true, source_unchanged: true };
}
const semantic = code => Object.assign(new Error(code), { semantic: true });
const protocolFailure = error => error.protocolInvalid === true;
class Server {
  constructor() { this.queue = []; this.calls = []; this.live = new Set(); this.maximumLive = 0; this.onDiscard = null; }
  expect(method, payload, image = IMAGE) { this.queue.push({ method, payload, image }); return this; }
  async call(method, params) {
    this.calls.push({ method, params: structuredClone(params) });
    assert.equal(params.image_revision, IMAGE.image_revision);
    if (method === 'attempt/discard') this.onDiscard?.(params.attempt_revision);
    let next;
    if (method === 'attempt/discard' && this.queue[0]?.method !== method) next = { method, payload: discarded(params.attempt_revision), image: IMAGE };
    else next = this.queue.shift();
    assert.ok(next, `unexpected ${method}`); assert.equal(next.method, method);
    if (next.payload instanceof Error) throw next.payload;
    const payload = structuredClone(next.payload);
    if (method === 'candidate/attempt' && payload.status === 'rejected' && payload.attempt) {
      this.live.add(payload.attempt.attempt_revision); this.maximumLive = Math.max(this.maximumLive, this.live.size);
    }
    if (method === 'attempt/discard') this.live.delete(params.attempt_revision);
    return { ...next.image, payload };
  }
}
function fresh() {
  const server = new Server(), controller = new Repairs(server.call.bind(server), IMAGE, candidate());
  return { server, controller };
}
async function prepared(revision = ATTEMPT, bytes = 1024) {
  const value = fresh(); value.server.expect('candidate/attempt', rejected(revision, bytes));
  await value.controller.tryIntent(intent); return value;
}
async function terminal(controller, server) {
  const count = server.calls.length;
  await assert.rejects(controller.tryIntent(intent));
  assert.equal(server.calls.length, count, 'terminal controller must not retry or retire an uncertain handle');
}

test('rejection remains evidence until an explicitly advertised selector is applied', async () => {
  const { controller, server } = await prepared();
  assert.equal(controller.attemptRevision, ATTEMPT); assert.equal(controller.sourceCandidate, BASE);
  assert.deepEqual(server.calls[0].params, { image_revision: IMAGE.image_revision, candidate_revision: BASE, intent });
  server.expect('attempt/summary', summary()); assert.deepEqual(await controller.summary(), summary());
  const before = server.calls.length;
  await assert.rejects(controller.apply(REPAIR)); assert.equal(server.calls.length, before);
  server.expect('attempt/repair-catalog', catalog()); assert.deepEqual(await controller.catalog(), catalog());
  assert.equal(server.calls.some(row => row.method === 'attempt/repair-apply'), false);
  await assert.rejects(controller.apply(digest(99))); assert.equal(server.calls.length, before + 1);
  server.expect('attempt/repair-apply', candidate(digest(8), digest(9)));
  assert.deepEqual(await controller.apply(REPAIR), candidate(digest(8), digest(9)));
  assert.deepEqual(server.calls.at(-2), { method: 'attempt/repair-apply', params: {
    image_revision: IMAGE.image_revision, attempt_revision: ATTEMPT, repair_id: REPAIR } });
  assert.equal(controller.attemptRevision, null); assert.equal(server.live.size, 0);
  await terminal(controller, server);
});

test('both supported repair classes and empty availability remain descriptive, never auto applied', async () => {
  for (const rows of [[repair()], [borrowRepair()], []]) {
    const { controller, server } = await prepared();
    server.expect('attempt/repair-catalog', catalog(ATTEMPT, rows));
    assert.deepEqual(await controller.catalog(), catalog(ATTEMPT, rows));
    assert.deepEqual(server.calls.map(row => row.method), ['candidate/attempt', 'attempt/repair-catalog']);
    if (!rows.length) { await assert.rejects(controller.apply(REPAIR)); assert.equal(server.calls.length, 2); }
    assert.deepEqual(await controller.discard(), discarded(ATTEMPT));
  }
});

test('replacement attempts retire only the previous adopted handle and invalidate its advertised repairs', async () => {
  const { controller, server } = await prepared();
  server.expect('attempt/repair-catalog', catalog()); await controller.catalog();
  server.expect('candidate/attempt', rejected(digest(20)));
  server.onDiscard = old => { assert.equal(old, ATTEMPT); assert.equal(controller.attemptRevision, digest(20)); };
  await controller.tryIntent(intent);
  const count = server.calls.length; await assert.rejects(controller.apply(REPAIR)); assert.equal(server.calls.length, count);
  server.onDiscard = null;
  for (let n = 21; n < 45; n++) {
    server.expect('candidate/attempt', rejected(digest(n))); await controller.tryIntent(intent);
    assert.equal(server.live.size, 1); assert.equal(controller.attemptRevision, digest(n));
  }
  assert.equal(server.maximumLive, 2, 'one old and one newly returned attempt during adoption only');
  const discards = server.calls.filter(row => row.method === 'attempt/discard').length;
  server.expect('candidate/attempt', rejected(digest(44))); await controller.tryIntent(intent);
  assert.equal(server.calls.filter(row => row.method === 'attempt/discard').length, discards);
  await controller.discard(); assert.equal(server.live.size, 0);
});

test('accepted attempts expose only validated candidate handles and retire previous rejection', async () => {
  const { controller, server } = await prepared(); server.expect('candidate/attempt', accepted());
  server.onDiscard = old => { assert.equal(old, ATTEMPT); assert.equal(controller.attemptRevision, null); };
  assert.deepEqual(await controller.tryIntent(intent), accepted());
  assert.equal(server.live.size, 0); await terminal(controller, server);
});

test('wrong accepted/rejected arms or predecessor bindings are terminal and do not retire known state', async () => {
  const mutations = [
    value => { value.status = 'accepted'; }, value => { value.candidate = candidate(); },
    value => { value.attempt = null; }, value => { value.extra = true; },
    value => { value.attempt.base_candidate_revision = digest(99); },
    value => { value.attempt.base_project_revision = ORIGINAL; },
    value => { value.attempt.checked_image = true; }, value => { value.attempt.materializable = true; },
    value => { value.attempt.diagnostic_count = 0; }, value => { value.attempt.source_authority = true; },
  ];
  for (const mutate of mutations) {
    const { controller, server } = await prepared(); const payload = rejected(digest(20)); mutate(payload);
    server.expect('candidate/attempt', payload); await assert.rejects(controller.tryIntent(intent), protocolFailure);
    assert.equal(controller.attemptRevision, ATTEMPT);
    assert.equal(server.calls.some(row => row.method === 'attempt/discard'), false); await terminal(controller, server);
  }
  for (const mutate of [value => { value.attempt = summary(); }, value => { value.candidate.base_revision = CURRENT; },
    value => { value.candidate.tests = 'passed'; }, value => { value.candidate.source_authority = true; }]) {
    const { controller, server } = fresh(); const payload = accepted(); mutate(payload);
    server.expect('candidate/attempt', payload); await assert.rejects(controller.tryIntent(intent), protocolFailure); await terminal(controller, server);
  }
});

test('catalog identities and repair authority cannot be substituted after rejection', async () => {
  for (const mutate of [value => { value.attempt_revision = digest(99); },
    value => { value.base_candidate_revision = digest(99); }, value => { value.base_project_revision = ORIGINAL; },
    value => { value.source_authority = true; }, value => { value.repairs.push(repair()); },
    value => { value.repairs[0].source_authority = true; }, value => { value.repairs[0].validation = 'unchecked'; },
    value => { value.repairs[0].repair_id = 'chosen-by-client'; }]) {
    const { controller, server } = await prepared(); const payload = catalog(); mutate(payload);
    server.expect('attempt/repair-catalog', payload); await assert.rejects(controller.catalog(), protocolFailure);
    assert.equal(controller.attemptRevision, ATTEMPT); await terminal(controller, server);
  }
});

test('repair apply binds the exact advertised validated candidate, never a replacement body supplied by the client', async () => {
  const { controller, server } = await prepared(); server.expect('attempt/repair-catalog', catalog()); await controller.catalog();
  server.expect('attempt/repair-apply', candidate(digest(99), digest(9)));
  await assert.rejects(controller.apply(REPAIR), protocolFailure);
  assert.equal(controller.attemptRevision, ATTEMPT); assert.equal(server.calls.at(-1).params.intent, undefined);
  assert.equal(server.calls.some(row => row.method === 'attempt/discard'), false); await terminal(controller, server);
});

test('semantic selector errors preserve the attempt while transport uncertainty forbids retry', async () => {
  const { controller, server } = await prepared();
  for (const method of ['attempt/summary', 'attempt/repair-catalog']) {
    server.expect(method, semantic('SPX-G243 stale attempt'));
    await assert.rejects(method === 'attempt/summary' ? controller.summary() : controller.catalog(), /SPX-G243/);
    assert.equal(controller.attemptRevision, ATTEMPT);
  }
  server.expect('candidate/attempt', semantic('SPX-G225 malformed constructor'));
  await assert.rejects(controller.tryIntent(intent), /SPX-G225/); assert.equal(controller.attemptRevision, ATTEMPT);
  server.expect('attempt/repair-catalog', catalog()); await controller.catalog();
  server.expect('attempt/repair-apply', semantic('SPX-G270 stale repair'));
  await assert.rejects(controller.apply(REPAIR), /SPX-G270/); assert.equal(controller.attemptRevision, ATTEMPT);
  server.expect('attempt/repair-apply', new Error('output failed after possible server adoption'));
  await assert.rejects(controller.apply(REPAIR), protocolFailure); await terminal(controller, server);
  assert.equal(server.calls.some(row => row.method === 'attempt/discard'), false);
});

test('retirement failure after adoption is terminal and never rolls back or repeats the successful operation', async () => {
  for (const mode of ['replacement', 'accepted', 'repair']) {
    const { controller, server } = await prepared();
    if (mode === 'repair') { server.expect('attempt/repair-catalog', catalog()); await controller.catalog(); }
    server.expect(mode === 'repair' ? 'attempt/repair-apply' : 'candidate/attempt',
      mode === 'replacement' ? rejected(digest(20)) : mode === 'accepted' ? accepted() : candidate(digest(8), digest(9)));
    server.expect('attempt/discard', semantic('SPX-G243 old attempt disappeared'));
    await assert.rejects(mode === 'repair' ? controller.apply(REPAIR) : controller.tryIntent(intent), protocolFailure);
    assert.equal(controller.attemptRevision, mode === 'replacement' ? digest(20) : null);
    await terminal(controller, server);
    assert.equal(server.calls.filter(row => row.method === 'attempt/discard').length, 1);
  }
});

test('host image and project drift fail closed without confusing the candidate project with the host image', async () => {
  for (const image of [{ ...IMAGE, image_revision: digest(99) }, { ...IMAGE, project_revision: CURRENT }]) {
    const { controller, server } = await prepared(); server.expect('attempt/summary', summary(), image);
    await assert.rejects(controller.summary(), protocolFailure); await terminal(controller, server);
  }
});

function fullReport(message = 'type mismatch é') {
  return { schema: 'semaprax.project-candidate-attempt.v1', base_candidate_revision: BASE,
    base_project_revision: CURRENT, state: 'rejected', change: change(intent.body),
    target_provenance: { id: TARGET, kind: 'function', identity_origin: 'explicit', owner: null,
      path: 'src/repair.spx', module: 'repair', source_revision: digest(40), source_digest: digest(41),
      evidence_owner: 'retained_verified_predecessor_semantic_index' },
    diagnostics: [{ index: 0, code: 'SPX-T002', severity: 'error', message, path: null, span: null, help: null,
      location_basis: 'uncommitted_attempt_or_constructor_input_not_authenticated_base_span' }],
    materializable: false, checked_image: false, source_authority: false, tests: 'not_run',
    nonclaims: ['no_invalid_source_or_hir_retained', 'diagnostic_spans_do_not_identify_verified_base_expressions', 'no_automatic_repair_or_authority'] };
}
function reportChunks(raw, revision = hash('semaprax.project-candidate-attempt.v1\0', raw)) {
  const bytes = Buffer.from(raw), result = [];
  for (let offset = 0; offset < bytes.length;) {
    let end = Math.min(offset + 65536, bytes.length);
    while (end < bytes.length && (bytes[end] & 0xc0) === 0x80) end--;
    result.push({ schema: 'semaprax.image-attempt-report-chunk.v1', attempt_revision: revision,
      report_schema: 'semaprax.project-candidate-attempt.v1', offset, total_bytes: bytes.length,
      chunk: bytes.subarray(offset, end).toString('utf8'), next_offset: end === bytes.length ? null : end,
      materializable: false, source_authority: false });
    offset = end;
  }
  return result;
}
async function withReport(raw) {
  const revision = hash('semaprax.project-candidate-attempt.v1\0', raw);
  const value = await prepared(revision, Buffer.byteLength(raw));
  return { ...value, revision, chunks: reportChunks(raw, revision) };
}

test('full diagnostic report preserves exact UTF8 and unsafe numeric bytes, never reconstructing an executable repair', async () => {
  // A schema-only report mock exercises raw byte preservation. It does not
  // claim the compiler produced this numeric intention from the safe input.
  const payload = fullReport('é'.repeat(40000));
  payload.change.intent.body.kind = 'i64';
  let raw = canonical(payload) + '\n';
  raw = raw.replace('"value":42', '"value":9007199254740993');
  const { controller, server, revision, chunks } = await withReport(raw);
  assert.ok(chunks.length > 1);
  for (const chunk of chunks) server.expect('attempt/query', chunk);
  assert.equal(await controller.report(), raw);
  assert.ok(raw.includes('9007199254740993'));
  assert.notEqual(hash('semaprax.project-candidate-attempt.v1\0', canonical(parse(raw)) + '\n'), revision,
    'recanonicalizing parsed unsafe integers cannot authenticate the original report');
  const queries = server.calls.filter(row => row.method === 'attempt/query');
  assert.deepEqual(queries.map(row => row.params.offset), chunks.map(row => row.offset));
  assert.ok(queries.every(row => row.params.chunk_bytes === 65536 && row.params.attempt_revision === revision));
  const row = repair(); row.from_type = 'i64'; row.expected_type = 'usize';
  row.preserved_integer_value = Number('9007199254740993');
  row.change.intent.body = { kind: 'usize', value: row.preserved_integer_value };
  row.semantic_change_intent.rejected_intent = { ...intent, body: { kind: 'i64', value: row.preserved_integer_value } };
  server.expect('attempt/repair-catalog', catalog(revision, [row])); await controller.catalog();
  server.expect('attempt/repair-apply', candidate(digest(8), digest(9))); await controller.apply(REPAIR);
  assert.deepEqual(server.calls.find(call => call.method === 'attempt/repair-apply').params,
    { image_revision: IMAGE.image_revision, attempt_revision: revision, repair_id: REPAIR });
  assert.equal(server.calls.filter(call => call.method === 'candidate/attempt').length, 1);
});

test('report chunks reject foreign selectors, byte offsets, nonprogress, oversized inventory and premature completion', async () => {
  const raw = canonical(fullReport('é'.repeat(40000))) + '\n';
  for (const mutate of [value => { value.attempt_revision = digest(99); },
    value => { value.offset = 1; }, value => { value.total_bytes++; }, value => { value.next_offset = 0; },
    value => { value.next_offset = null; }, value => { value.chunk = ''; },
    value => { value.chunk = 'é'; value.next_offset = 2; }, value => { value.next_offset = value.chunk.length; },
    value => { value.materializable = true; }, value => { value.source_authority = true; },
    value => { value.total_bytes = 2 * 1024 * 1024 + 1; }, value => { value.chunk = 'x'.repeat(65537); },
    value => { value.report_schema = 'semaprax.project-candidate.v1'; }, value => { value.extra = false; }]) {
    const { controller, server, chunks, revision } = await withReport(raw); mutate(chunks[0]);
    server.expect('attempt/query', chunks[0]); await assert.rejects(controller.report(), protocolFailure);
    assert.equal(controller.attemptRevision, revision); await terminal(controller, server);
  }
  const { controller, server, chunks } = await withReport(raw);
  chunks[1].chunk = chunks[1].chunk.replace('é', 'à');
  server.expect('attempt/query', chunks[0]).expect('attempt/query', chunks[1]);
  await assert.rejects(controller.report(), protocolFailure); await terminal(controller, server);
});

test('even correctly hashed full reports cannot change provenance, authority or rejected state', async () => {
  for (const mutate of [value => { value.base_candidate_revision = digest(99); },
    value => { value.base_project_revision = ORIGINAL; }, value => { value.state = 'accepted'; },
    value => { value.target_provenance.id = 'other.function'; },
    value => { value.target_provenance.evidence_owner = 'unverified_source'; },
    value => { value.change.base_revision = ORIGINAL; }, value => { value.change.requirements.pop(); },
    value => { value.diagnostics[0].location_basis = 'verified_base_expression'; },
    value => { value.diagnostics[0].index = 1; }, value => { value.diagnostics = []; },
    value => { value.source_authority = true; }, value => { value.checked_image = true; },
    value => { value.nonclaims.pop(); }, value => { value.attempt_revision = digest(99); }]) {
    const report = fullReport(); mutate(report); const raw = canonical(report) + '\n';
    const { controller, server, chunks } = await withReport(raw);
    for (const chunk of chunks) server.expect('attempt/query', chunk);
    await assert.rejects(controller.report(), protocolFailure); await terminal(controller, server);
  }
});

test('single flight and safe input checks prevent a second candidate mutation before the first outcome', async () => {
  let resolve; const calls = [];
  const controller = new Repairs((method, params) => { calls.push({ method, params }); return new Promise(done => { resolve = done; }); }, IMAGE, candidate());
  await assert.rejects(controller.tryIntent({ ...intent, body: { kind: 'usize', value: Number.MAX_SAFE_INTEGER + 1 } }));
  assert.equal(calls.length, 0);
  const pending = controller.tryIntent(intent);
  await assert.rejects(controller.tryIntent(intent), /pending/);
  await assert.rejects(controller.summary(), /pending/); assert.equal(calls.length, 1);
  resolve({ ...IMAGE, payload: rejected() }); await pending;
  assert.equal(controller.attemptRevision, ATTEMPT);
});

test('immutable summary counters and discard receipts require exact bindings before state changes', async () => {
  for (const mutate of [value => { value.attempt_revision = digest(99); },
    value => { value.diagnostic_count++; }, value => { value.report_bytes++; }]) {
    const { controller, server } = await prepared(); const payload = summary(); mutate(payload);
    server.expect('attempt/summary', payload); await assert.rejects(controller.summary(), protocolFailure);
    assert.equal(controller.attemptRevision, ATTEMPT); await terminal(controller, server);
  }
  for (const mutate of [value => { value.attempt_revision = digest(99); },
    value => { value.discarded = false; }, value => { value.source_unchanged = false; }, value => { value.extra = true; }]) {
    const { controller, server } = await prepared(); const payload = discarded(ATTEMPT); mutate(payload);
    server.expect('attempt/discard', payload); await assert.rejects(controller.discard(), protocolFailure);
    assert.equal(controller.attemptRevision, ATTEMPT); await terminal(controller, server);
  }
  const { controller, server } = await prepared();
  server.expect('attempt/discard', semantic('SPX-G243 unknown attempt'));
  await assert.rejects(controller.discard(), /SPX-G243/); assert.equal(controller.attemptRevision, ATTEMPT);
  assert.deepEqual(await controller.discard(), discarded(ATTEMPT));
  assert.equal(controller.attemptRevision, null);
});

test('source invalidation and source epoch races close diagnostic workflows without retirement or retry', async () => {
  for (const failure of [Object.assign(semantic('SPX-G221 stale image'), { sourceInvalid: true }),
    Object.assign(new Error('editor source epoch changed'), { discardOnly: true })]) {
    const { controller, server } = await prepared(); server.expect('attempt/summary', failure);
    await assert.rejects(controller.summary(), protocolFailure); await terminal(controller, server);
    assert.equal(server.calls.some(row => row.method === 'attempt/discard'), false);
  }
});

test('editor repair tools do not grant build, tests, publication, archives or approval', async () => {
  for (const method of ['candidate/attempt', 'attempt/summary', 'attempt/query', 'attempt/repair-catalog', 'attempt/repair-apply', 'attempt/discard']) assert.equal(ALLOWED.has(method), true);
  class Child extends EventEmitter {
    constructor() { super(); this.stdout = new EventEmitter(); this.stderr = new EventEmitter(); this.stdin = new EventEmitter(); this.writes = [];
      this.stdin.write = (line, callback) => { this.writes.push(line); callback?.(); return true; }; }
    kill() {}
  }
  const child = new Child(), client = new McpClient(child);
  for (const method of ['candidate/build', 'candidate/test', 'candidate/commit', 'candidate/commit-report',
    'candidate/archive-restore', 'hole/archive-restore', 'attempt/approve']) {
    client.tools.add(method); assert.equal(ALLOWED.has(method), false);
    await assert.rejects(client.call(method, { approved: true }), /allowlist/);
  }
  assert.equal(child.writes.length, 0); client.stop();
});
