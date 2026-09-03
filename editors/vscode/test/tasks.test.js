'use strict';
// Schema-shaped task-tool controller checks. These do not claim MCP standard
// task augmentation or that the compiler executed the represented report.
const test = require('node:test');
const assert = require('node:assert/strict');
const { CandidateTestTask, METHODS, BLIND_SPOTS, hash } = require('../tasks');
const { ALLOWED } = require('../protocol');

const digest = n => 'sha256:' + n.toString(16).padStart(64, '0');
const BINDING = Object.freeze({ image_revision: digest(1), project_revision: digest(2), candidate_revision: digest(3) });
const TASK = digest(4);
const AUTHORITY = Object.freeze({ source_write: false, process: false, network: false, target_runtime: false, publication: false });
const common = (schema, state = 'running', extra = {}) => ({ schema, ...BINDING, task_revision: TASK, state,
  terminal: !['queued', 'running'].includes(state), cancellation_requested: state === 'cancelled', source_authority: false,
  authority: { ...AUTHORITY }, blind_spots: [...BLIND_SPOTS], ...extra });
const start = (extra = {}) => common('semaprax.image-candidate-test-task-start.v1', 'queued', {
  report_digest: null, passed: null, before_step: null, steps_used: null, max_steps: 1000000, diagnostics: [], ...extra
});
const status = (state, extra = {}) => common('semaprax.image-candidate-test-task-status.v1', state, {
  report_digest: null, passed: null, before_step: null, steps_used: null, max_steps: 1000000, diagnostics: [], ...extra
});
const cancelled = extra => common('semaprax.image-candidate-test-task-cancel.v1', 'cancelled', {
  report_digest: null, passed: null, before_step: 1, steps_used: 0, max_steps: 1000000, diagnostics: [], cancel_observed: true, ...extra
});
const result = (raw, reportDigest, extra = {}) => ({
  schema: 'semaprax.image-candidate-test-task-result-chunk.v1', ...BINDING, task_revision: TASK,
  source_authority: false, authority: { ...AUTHORITY }, blind_spots: [...BLIND_SPOTS],
  report_schema: 'semaprax.project-candidate-test-report.v1', report_digest: reportDigest,
  offset: 0, total_bytes: Buffer.byteLength(raw), chunk: raw, next_offset: null, complete: true, ...extra
});

class Server {
  constructor(rows = []) { this.rows = rows; this.calls = []; }
  async call(method, params) {
    this.calls.push({ method, params: structuredClone(params) });
    const next = this.rows.shift(); assert.ok(next, `unexpected ${method}`);
    assert.equal(next.method, method); const payload = typeof next.payload === 'function' ? await next.payload() : next.payload;
    if (payload instanceof Error) throw payload;
    return { image_revision: BINDING.image_revision, project_revision: BINDING.project_revision, payload: structuredClone(payload) };
  }
}

function report(passed = true) {
  return JSON.stringify({ schema: 'semaprax.project-candidate-test-report.v1', candidate_digest: BINDING.candidate_revision,
    project_revision: BINDING.project_revision, passed }) + '\n';
}

test('only the four explicit Semaprax task tools enter the editor allowlist', () => {
  assert.deepEqual(METHODS, ['candidate/test-task-start', 'candidate/test-task-status', 'candidate/test-task-cancel', 'candidate/test-task-result']);
  for (const method of METHODS) assert.equal(ALLOWED.has(method), true);
  for (const method of ['candidate/test', 'candidate/build', 'candidate/commit', 'tasks/get', 'tasks/cancel', 'notifications/cancelled']) assert.equal(ALLOWED.has(method), false);
});

test('completed task reconstructs one bound digest-checked report without widening authority', async () => {
  const raw = report(), reportDigest = hash('semaprax.candidate-test.report.v1\0', raw);
  const server = new Server([
    { method: METHODS[0], payload: start() },
    { method: METHODS[1], payload: status('completed', { report_digest: reportDigest, passed: true, steps_used: 12 }) },
    { method: METHODS[3], payload: result(raw, reportDigest) }
  ]);
  const controller = new CandidateTestTask(server.call.bind(server), BINDING), observed = [];
  const value = await controller.run(row => observed.push(row.state));
  assert.equal(value.raw, raw); assert.equal(value.report.passed, true); assert.deepEqual(observed, ['queued', 'completed']);
  assert.deepEqual(server.calls, [
    { method: METHODS[0], params: { image_revision: BINDING.image_revision, candidate_revision: BINDING.candidate_revision } },
    { method: METHODS[1], params: { image_revision: BINDING.image_revision, task_revision: TASK } },
    { method: METHODS[3], params: { image_revision: BINDING.image_revision, task_revision: TASK, offset: 0, max_bytes: 524288 } }
  ]);
  assert.equal(controller.requestCancel(), false);
});

test('cancellation requested while start is pending becomes one explicit sticky cancel call', async () => {
  let release; const pending = new Promise(resolve => { release = resolve; });
  const server = new Server([
    { method: METHODS[0], payload: () => pending },
    { method: METHODS[2], payload: cancelled() }
  ]);
  const controller = new CandidateTestTask(server.call.bind(server), BINDING);
  const running = controller.run();
  assert.equal(controller.requestCancel(), true); release(start());
  const value = await running;
  assert.equal(value.status.state, 'cancelled'); assert.equal(value.raw, null);
  assert.deepEqual(server.calls.map(row => row.method), [METHODS[0], METHODS[2]]);
  assert.equal(controller.requestCancel(), false);
});

test('cancellation remains available after the compiler returns queued start', async () => {
  const server = new Server([
    { method: METHODS[0], payload: start() },
    { method: METHODS[2], payload: cancelled() }
  ]);
  const controller = new CandidateTestTask(server.call.bind(server), BINDING);
  let accepted = false;
  const value = await controller.run(row => {
    if (row.state === 'queued') accepted = controller.requestCancel();
  });
  assert.equal(accepted, true);
  assert.equal(value.status.state, 'cancelled');
  assert.deepEqual(server.calls.map(row => row.method), [METHODS[0], METHODS[2]]);
});

test('failed execution returns typed diagnostics and never requests result bytes', async () => {
  const diagnostics = [{ code: 'SPX-F109', message: 'worker failed' }];
  const server = new Server([
    { method: METHODS[0], payload: start() },
    { method: METHODS[1], payload: status('failed', { steps_used: 7, diagnostics }) }
  ]);
  const value = await new CandidateTestTask(server.call.bind(server), BINDING).run();
  assert.equal(value.status.state, 'failed'); assert.deepEqual(value.status.diagnostics, diagnostics);
  assert.deepEqual(server.calls.map(row => row.method), [METHODS[0], METHODS[1]]);
});

test('foreign bindings, hidden blind spots, forged digests and nonprogress close without retries', async () => {
  for (const mutate of [
    row => { row.candidate_revision = digest(99); },
    row => { row.authority.network = true; },
    row => { row.blind_spots.pop(); }
  ]) {
    const row = start(); mutate(row); const server = new Server([{ method: METHODS[0], payload: row }]);
    await assert.rejects(new CandidateTestTask(server.call.bind(server), BINDING).run());
    assert.equal(server.calls.length, 1);
  }
  const raw = report(), actual = hash('semaprax.candidate-test.report.v1\0', raw), forged = digest(88);
  const server = new Server([
    { method: METHODS[0], payload: start() },
    { method: METHODS[1], payload: status('completed', { report_digest: forged, passed: true, steps_used: 1 }) },
    { method: METHODS[3], payload: result(raw, forged) }
  ]);
  await assert.rejects(new CandidateTestTask(server.call.bind(server), BINDING).run(), /digest mismatch/);
  assert.notEqual(actual, forged); assert.equal(server.calls.length, 3);
});

test('result pagination requires exact UTF8 byte progress and stable totals', async () => {
  const raw = report(false), first = raw.slice(0, 10), reportDigest = hash('semaprax.candidate-test.report.v1\0', raw);
  const server = new Server([
    { method: METHODS[0], payload: start() },
    { method: METHODS[1], payload: status('completed', { report_digest: reportDigest, passed: false, steps_used: 4 }) },
    { method: METHODS[3], payload: result(first, reportDigest, {
      total_bytes: Buffer.byteLength(raw), next_offset: Buffer.byteLength(first) + 1, complete: false
    }) }
  ]);
  await assert.rejects(new CandidateTestTask(server.call.bind(server), BINDING).run(), /continuation/);
  assert.equal(server.calls.length, 3);
});
