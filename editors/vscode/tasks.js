'use strict';
const crypto = require('node:crypto');
const { exact, digest, parse } = require('./protocol');

const METHODS = Object.freeze([
  'candidate/test-task-start', 'candidate/test-task-status',
  'candidate/test-task-cancel', 'candidate/test-task-result'
]);
const STATES = new Set(['queued', 'running', 'completed', 'cancelled', 'failed']);
const BLIND_SPOTS = Object.freeze([
  'native_and_wasm_runtime', 'deployment_configuration', 'generated_artifacts',
  'external_api_behavior', 'runtime_environment', 'external_consumers'
]);
const AUTHORITY_KEYS = Object.freeze(['source_write', 'process', 'network', 'target_runtime', 'publication']);
const MAX_REPORT = 2 * 1024 * 1024;
const RESULT_CHUNK = 512 * 1024;
const POLL_MS = 25;
const REPORT_SCHEMA = 'semaprax.project-candidate-test-report.v1';
const REPORT_DOMAIN = 'semaprax.candidate-test.report.v1\0';

function hash(domain, text) {
  const bytes = Buffer.from(text, 'utf8'), length = Buffer.alloc(8);
  length.writeBigUInt64LE(BigInt(bytes.length));
  return 'sha256:' + crypto.createHash('sha256').update(domain).update(length).update(bytes).digest('hex');
}
function integer(value) { return Number.isSafeInteger(value) && value >= 0; }
function nullable(value, predicate) { return value === null || predicate(value); }
function binding(value, schema, expected) {
  if (value.schema !== schema || value.image_revision !== expected.image_revision ||
      value.project_revision !== expected.project_revision || value.candidate_revision !== expected.candidate_revision ||
      !digest(value.task_revision) || value.source_authority !== false) throw new Error('Invalid candidate test-task binding');
  exact(value.authority, AUTHORITY_KEYS);
  if (AUTHORITY_KEYS.some(key => value.authority[key] !== false) || !Array.isArray(value.blind_spots) ||
      value.blind_spots.length !== BLIND_SPOTS.length || value.blind_spots.some((row, index) => row !== BLIND_SPOTS[index])) {
    throw new Error('Candidate test task widened authority or hid an analysis blind spot');
  }
  return value;
}
function common(value, schema, expected) {
  if (value.schema !== schema || value.image_revision !== expected.image_revision ||
      value.project_revision !== expected.project_revision || value.candidate_revision !== expected.candidate_revision ||
      !digest(value.task_revision) || !STATES.has(value.state) || typeof value.terminal !== 'boolean' ||
      value.terminal !== !['queued', 'running'].includes(value.state) || typeof value.cancellation_requested !== 'boolean' ||
      value.source_authority !== false) throw new Error('Invalid candidate test-task binding');
  return binding(value, schema, expected);
}
function start(value, expected) {
  status(value, 'semaprax.image-candidate-test-task-start.v1', expected);
  if (value.state !== 'queued' || value.terminal || value.cancellation_requested) throw new Error('Invalid candidate test-task start state');
  return value;
}
function status(value, schema, expected, cancel = false) {
  const keys = ['schema', 'image_revision', 'project_revision', 'candidate_revision', 'task_revision', 'state',
    'terminal', 'cancellation_requested', 'source_authority', 'authority', 'blind_spots', 'report_digest',
    'passed', 'before_step', 'steps_used', 'max_steps', 'diagnostics'];
  if (cancel) keys.push('cancel_observed');
  exact(value, keys); common(value, schema, expected);
  if (!nullable(value.report_digest, digest) || !nullable(value.passed, row => typeof row === 'boolean') ||
      !nullable(value.before_step, integer) || !nullable(value.steps_used, integer) || !integer(value.max_steps) ||
      value.max_steps < 1 || !Array.isArray(value.diagnostics) || value.diagnostics.length > 64 ||
      value.diagnostics.some(row => !row || typeof row !== 'object' || Array.isArray(row))) {
    throw new Error('Invalid candidate test-task status');
  }
  if (cancel && value.cancel_observed !== true) throw new Error('Invalid candidate test-task cancellation receipt');
  if (['queued', 'running'].includes(value.state) && (value.report_digest !== null || value.passed !== null || value.before_step !== null || value.steps_used !== null || value.terminal || value.diagnostics.length)) throw new Error('Active test task claims a terminal result');
  if (value.state === 'completed' && (!digest(value.report_digest) || typeof value.passed !== 'boolean' || !integer(value.steps_used) || value.before_step !== null || value.diagnostics.length)) throw new Error('Completed test task lacks one exact report');
  if (value.state === 'cancelled' && (!value.cancellation_requested || value.report_digest !== null || value.passed !== null ||
      !nullable(value.before_step, integer) || !nullable(value.steps_used, integer) || value.diagnostics.length)) throw new Error('Cancelled test task claims completion');
  if (value.state === 'failed' && (value.report_digest !== null || value.passed !== null || value.before_step !== null || !value.diagnostics.length)) throw new Error('Failed test task lacks diagnostics');
  return value;
}
function result(value, expected, expectedTask, offset, expectedDigest) {
  exact(value, ['schema', 'image_revision', 'project_revision', 'candidate_revision', 'task_revision', 'source_authority',
    'authority', 'blind_spots', 'report_schema', 'report_digest', 'offset', 'total_bytes', 'chunk', 'next_offset', 'complete']);
  binding(value, 'semaprax.image-candidate-test-task-result-chunk.v1', expected);
  if (value.task_revision !== expectedTask || value.report_schema !== REPORT_SCHEMA || value.report_digest !== expectedDigest ||
      value.offset !== offset || !integer(value.total_bytes) || value.total_bytes < 1 || value.total_bytes > MAX_REPORT ||
      typeof value.chunk !== 'string' || typeof value.complete !== 'boolean' ||
      !nullable(value.next_offset, integer) || value.complete !== (value.next_offset === null)) {
    throw new Error('Invalid candidate test-task result chunk');
  }
  return value;
}
function verifyReport(text, binding, expectedDigest, expectedPassed) {
  if (hash(REPORT_DOMAIN, text) !== expectedDigest) throw new Error('Candidate test report digest mismatch');
  const value = parse(text, MAX_REPORT);
  if (value.schema !== REPORT_SCHEMA || value.candidate_digest !== binding.candidate_revision ||
      value.project_revision !== binding.project_revision || value.passed !== expectedPassed) {
    throw new Error('Candidate test report binding mismatch');
  }
  return value;
}
function delay(ms, wake) {
  return Promise.race([new Promise(resolve => setTimeout(resolve, ms)), wake]);
}

class CandidateTestTask {
  #call; #binding; #task = null; #state = null; #cancelRequested = false; #wake = null; #wakeResolve = null; #closed = false;
  constructor(call, binding) {
    if (typeof call !== 'function' || !digest(binding?.image_revision) || !digest(binding?.project_revision) || !digest(binding?.candidate_revision)) throw new Error('Invalid candidate test-task controller');
    this.#call = call; this.#binding = Object.freeze({ ...binding }); this.#resetWake();
  }
  get taskRevision() { return this.#task; }
  get state() { return this.#state; }
  get cancellationRequested() { return this.#cancelRequested; }
  requestCancel() {
    if (this.#closed || (this.#state !== null && this.#state !== 'running')) return false;
    this.#cancelRequested = true; this.#wakeResolve(); return true;
  }
  #resetWake() { this.#wake = new Promise(resolve => { this.#wakeResolve = resolve; }); }
  #params(extra = {}) { return { image_revision: this.#binding.image_revision, task_revision: this.#task, ...extra }; }
  async #invoke(method, params) {
    if (this.#closed) throw new Error('Candidate test-task controller is closed');
    const response = await this.#call(method, params);
    if (response.image_revision !== this.#binding.image_revision || response.project_revision !== this.#binding.project_revision) throw new Error('Candidate test-task response escaped its session revision');
    return response.payload;
  }
  async #cancel() {
    const value = status(await this.#invoke('candidate/test-task-cancel', this.#params()),
      'semaprax.image-candidate-test-task-cancel.v1', this.#binding, true);
    if (value.task_revision !== this.#task) throw new Error('Candidate test-task cancellation did not bind the selected task');
    this.#state = value.state; return value;
  }
  async #poll() {
    const value = status(await this.#invoke('candidate/test-task-status', this.#params()),
      'semaprax.image-candidate-test-task-status.v1', this.#binding);
    if (value.task_revision !== this.#task) throw new Error('Candidate test-task status substituted its task');
    this.#state = value.state; return value;
  }
  async #report(terminal) {
    let offset = 0, total, chunks = [], bytes = 0;
    for (let count = 0; count < 513; count++) {
      const row = result(await this.#invoke('candidate/test-task-result', this.#params({ offset, max_bytes: RESULT_CHUNK })),
        this.#binding, this.#task, offset, terminal.report_digest);
      if (total !== undefined && total !== row.total_bytes) throw new Error('Candidate test-task report size changed');
      total = row.total_bytes;
      const size = Buffer.byteLength(row.chunk);
      if (size < 1 || size > RESULT_CHUNK || bytes + size > total) throw new Error('Candidate test-task result made no bounded progress');
      chunks.push(row.chunk); bytes += size;
      if (row.complete) {
        if (bytes !== total) throw new Error('Candidate test-task result is truncated');
        const raw = chunks.join('');
        return { raw, report: verifyReport(raw, this.#binding, terminal.report_digest, terminal.passed), status: terminal };
      }
      if (row.next_offset !== bytes || row.next_offset <= offset || bytes >= total) throw new Error('Invalid candidate test-task continuation');
      offset = row.next_offset;
    }
    throw new Error('Candidate test-task chunk count exceeded');
  }
  async run(onStatus = () => {}) {
    if (this.#closed || this.#task) throw new Error('Candidate test task already started');
    const value = start(await this.#invoke('candidate/test-task-start', {
      image_revision: this.#binding.image_revision, candidate_revision: this.#binding.candidate_revision
    }), this.#binding);
    this.#task = value.task_revision; this.#state = value.state; onStatus(value);
    try {
      for (;;) {
        let terminal;
        if (this.#cancelRequested) terminal = await this.#cancel();
        else terminal = await this.#poll();
        onStatus(terminal);
        if (terminal.state === 'completed') return await this.#report(terminal);
        if (terminal.state === 'cancelled' || terminal.state === 'failed') return { raw: null, report: null, status: terminal };
        this.#resetWake(); await delay(POLL_MS, this.#wake);
      }
    } finally { this.#closed = true; }
  }
}

module.exports = { CandidateTestTask, METHODS, BLIND_SPOTS, MAX_REPORT, RESULT_CHUNK, REPORT_SCHEMA, hash, binding, common, start, status, result, verifyReport };
