'use strict';
// Compiler-admitted repair selectors only. Report intentions and numeric values
// are descriptive: they are never reconstructed, hashed or sent as repairs.
const { exact, digest, parse } = require('./protocol');
const { hash } = require('./review');
const MAX_REPORT = 1024 * 1024, MAX_ATTEMPT = 2 * MAX_REPORT;
const LITERAL = 'retag_integer_literal_to_retained_return_type';
const BORROW = 'borrow_owned_byte_field_without_staging';
const INTEGER_TYPES = ['i64', 'i32', 'u8', 'usize'];
const RESERVED_ROOTS = new Set(['module', 'use', 'fn', 'let', 'mut', 'if', 'else', 'while', 'match', 'true', 'false', 'requires', 'ensures',
  'uses', 'permit', 'unsafe', 'return', 'own', 'borrow', 'shared', 'self', 'super']);
const LEGACY = 'assign_function_id_is_a_breaking_identity_rebase_and_not_a_stable_identity_preserving_candidate_change';
const NONCLAIMS = ['not_general_diagnostic_repair', 'no_invalid_source_or_hir_admission', 'no_automatic_repair_selection'];
const REQUIREMENTS = ['preserve_stable_identity', 'preserve_public_exports', 'update_all_callers', 'no_new_effects', 'no_new_capabilities',
  'preserve_contracts', 'revalidate_ownership_and_cleanup', 'preserve_project_profile_admission', 'preserve_admitted_core_targets'];
const text = (value, max = MAX_REPORT) => typeof value === 'string' && Buffer.byteLength(value) <= max;
const selector = (value, max = 4096) => text(value, max) && value.length > 0 && !value.includes('\0');
const index = (value, max) => Number.isSafeInteger(value) && value >= 0 && value <= max;
const copy = value => JSON.parse(JSON.stringify(value));
function assert(condition, message) { if (!condition) throw new Error(message); }
function checked(action) {
  try { return action(); } catch (error) { error.protocolInvalid = true; throw error; }
}
function bounded(value, max = MAX_REPORT) {
  const encoded = JSON.stringify(value);
  assert(typeof encoded === 'string' && Buffer.byteLength(encoded) + 1 <= max, 'Repair report byte limit');
  // This is only a bounded JSON shape check. In particular, report numbers need
  // not be JavaScript-safe integers and never become operation selectors.
  parse(encoded, max);
}
function candidateHandle(row, baseRevision, expectedCandidate) {
  exact(row, ['schema', 'candidate_revision', 'project_revision', 'base_revision', 'report_bytes', 'source_authority', 'tests']);
  assert(row.schema === 'semaprax.image-candidate-handle.v1' && digest(row.candidate_revision) && digest(row.project_revision) &&
    digest(row.base_revision) && (baseRevision === undefined || row.base_revision === baseRevision) &&
    (expectedCandidate === undefined || row.candidate_revision === expectedCandidate) &&
    index(row.report_bytes, 64 * MAX_REPORT) && row.report_bytes > 0 && row.source_authority === false && row.tests === 'not_run', 'Invalid repair candidate handle');
}
function validateCandidateHandle(row, baseRevision, expectedCandidate) {
  return checked(() => { candidateHandle(row, baseRevision, expectedCandidate); return row; });
}
function summaryReport(row, candidate, expectedAttempt) {
  exact(row, ['schema', 'attempt_revision', 'base_candidate_revision', 'base_project_revision', 'state', 'diagnostic_count', 'report_bytes', 'materializable', 'checked_image', 'source_authority']);
  assert(row.schema === 'semaprax.project-candidate-attempt-summary.v1' && digest(row.attempt_revision) &&
    (expectedAttempt === undefined || row.attempt_revision === expectedAttempt) && row.base_candidate_revision === candidate.candidate_revision &&
    row.base_project_revision === candidate.project_revision && row.state === 'rejected' && index(row.diagnostic_count, 256) && row.diagnostic_count > 0 &&
    index(row.report_bytes, MAX_ATTEMPT) && row.report_bytes > 0 && row.materializable === false && row.checked_image === false && row.source_authority === false,
    'Invalid rejected attempt summary');
}
function bodyIntent(intent, target) {
  exact(intent, ['kind', 'target', 'body']);
  assert(intent.kind === 'replace_function_body' && intent.target === target && intent.body && typeof intent.body === 'object' && !Array.isArray(intent.body) &&
    selector(intent.body.kind, 128), 'Invalid descriptive repair body');
}
function changeBinding(change, candidate) {
  exact(change, ['schema', 'base_revision', 'intent', 'requirements']);
  assert(change.schema === 'semaprax.semantic-change.v1' && change.base_revision === candidate.project_revision &&
    Array.isArray(change.requirements) && change.requirements.length === REQUIREMENTS.length && change.requirements.every((item, i) => item === REQUIREMENTS[i]) &&
    change.intent && typeof change.intent === 'object' && !Array.isArray(change.intent), 'Invalid repair change predecessor');
}
function repairRow(row, candidate, target) {
  const common = ['repair_id', 'class', 'target', 'change', 'semantic_change_intent', 'validated_candidate_revision', 'validation', 'evidence_owner', 'tests', 'source_authority'];
  assert(row && (row.class === LITERAL || row.class === BORROW), 'Unsupported compiler repair class');
  exact(row, [...common, ...(row.class === LITERAL ? ['from_type', 'expected_type', 'preserved_integer_value'] : ['diagnostic_code', 'replacement_count', 'replacements'])]);
  assert(digest(row.repair_id) && selector(row.target) && row.target === target && digest(row.validated_candidate_revision) &&
    row.validation === 'normal_full_candidate_apply' && row.tests === 'not_run' && row.source_authority === false, 'Invalid compiler repair binding');
  changeBinding(row.change, candidate);
  bodyIntent(row.change.intent, target);
  exact(row.semantic_change_intent, ['kind', 'target', 'rejected_intent', 'repair_id']);
  assert(row.semantic_change_intent.kind === 'repair_diagnostic' && row.semantic_change_intent.target === target && row.semantic_change_intent.repair_id === row.repair_id,
    'Invalid descriptive repair intention');
  bodyIntent(row.semantic_change_intent.rejected_intent, target);
  if (row.class === LITERAL) {
    assert(INTEGER_TYPES.includes(row.from_type) && INTEGER_TYPES.includes(row.expected_type) && row.from_type !== row.expected_type &&
      Number.isFinite(row.preserved_integer_value) && Number.isInteger(row.preserved_integer_value) &&
      row.evidence_owner === 'retained_target_return_type_and_full_candidate_admission', 'Invalid literal repair metadata');
    exact(row.change.intent.body, ['kind', 'value']);
    exact(row.semantic_change_intent.rejected_intent.body, ['kind', 'value']);
    assert(row.change.intent.body.kind === row.expected_type && row.semantic_change_intent.rejected_intent.body.kind === row.from_type &&
      Number.isFinite(row.change.intent.body.value) && Number.isInteger(row.change.intent.body.value) &&
      Number.isFinite(row.semantic_change_intent.rejected_intent.body.value) && Number.isInteger(row.semantic_change_intent.rejected_intent.body.value), 'Invalid literal repair description');
  } else {
    assert(row.diagnostic_code === 'SPX-T266' && index(row.replacement_count, 4096) && row.replacement_count > 0 &&
      Array.isArray(row.replacements) && row.replacements.length === row.replacement_count &&
      row.evidence_owner === 'closed_builtin_projection_pattern_and_full_candidate_admission', 'Invalid byte-field repair metadata');
    for (const replacement of row.replacements) {
      exact(replacement, ['field', 'root']);
      assert(selector(replacement.field) && selector(replacement.root, 128) && /^[A-Za-z_][A-Za-z0-9_]*$/.test(replacement.root) &&
        !RESERVED_ROOTS.has(replacement.root), 'Invalid byte-field repair selection');
    }
  }
}

class Repairs {
  #call; #image; #candidate; #attempt = null; #summary = null; #target = null;
  #repairs = new Map(); #busy = false; #closed = false;
  constructor(call, image, candidate) {
    assert(typeof call === 'function', 'A compiler call function is required');
    exact(image, ['image_revision', 'project_revision']);
    assert(digest(image.image_revision) && digest(image.project_revision), 'A current image binding is required');
    candidateHandle(candidate);
    this.#call = call; this.#image = { ...image }; this.#candidate = { ...candidate };
  }
  get attemptRevision() { return this.#attempt; }
  get sourceCandidate() { return this.#candidate.candidate_revision; }
  async #serial(action) {
    assert(!this.#closed, 'This repair controller is closed; start a new controller for a current candidate');
    assert(!this.#busy, 'A diagnostic operation is already pending');
    this.#busy = true;
    try { return await action(); }
    catch (error) { if (error.protocolInvalid || error.discardOnly) this.#closed = true; throw error; }
    finally { this.#busy = false; }
  }
  async #invoke(method, params) {
    let response;
    try { response = await this.#call(method, { image_revision: this.#image.image_revision, ...params }); }
    catch (cause) {
      const error = cause instanceof Error ? cause : new Error('Compiler call failed without a structured error');
      // A compiler semantic rejection occurs before registry mutation. A lost
      // response, source epoch change or other transport failure cannot safely
      // be retried by this controller, even if no handle was adopted locally.
      if (!error.semantic || error.sourceInvalid || error.discardOnly) { error.protocolInvalid = true; this.#closed = true; }
      throw error;
    }
    return checked(() => {
      assert(response && response.image_revision === this.#image.image_revision && response.project_revision === this.#image.project_revision &&
        response.payload && typeof response.payload === 'object' && !Array.isArray(response.payload), 'Diagnostic response image or Project mismatch');
      bounded(response.payload); return response.payload;
    });
  }
  #requireAttempt() { assert(this.#attempt !== null, 'Submit a rejected intention first'); return this.#attempt; }
  async #retire(revision) {
    if (revision === null) return;
    try {
      const report = await this.#invoke('attempt/discard', { attempt_revision: revision });
      checked(() => this.#discarded(report, revision));
    } catch (cause) {
      this.#closed = true;
      const error = new Error('The new attempt or candidate was accepted, but retirement of its prior attempt failed. Restart the session; the mutation was not rolled back.');
      error.protocolInvalid = true; error.cause = cause; throw error;
    }
  }
  #discarded(report, revision) {
    exact(report, ['schema', 'attempt_revision', 'discarded', 'source_unchanged']);
    assert(report.schema === 'semaprax.image-attempt-discard.v1' && report.attempt_revision === revision && report.discarded === true && report.source_unchanged === true,
      'Invalid attempt discard receipt');
  }
  async tryIntent(intent) {
    return this.#serial(async () => {
      // Explicit editor input must be losslessly representable. This is not a
      // constructor validator; the ordinary compiler remains the admission owner.
      const selected = parse(JSON.stringify(intent), 65536, true);
      assert(selected && typeof selected === 'object' && !Array.isArray(selected) && selector(selected.kind, 128), 'A typed intention object is required');
      const report = await this.#invoke('candidate/attempt', { candidate_revision: this.sourceCandidate, intent: selected });
      checked(() => {
        exact(report, ['schema', 'status', 'candidate', 'attempt']);
        assert(report.schema === 'semaprax.image-candidate-attempt-outcome.v1', 'Invalid attempt outcome schema');
        if (report.status === 'accepted') {
          assert(report.attempt === null, 'Accepted outcome retained a rejected attempt'); candidateHandle(report.candidate, this.#candidate.base_revision);
        } else {
          assert(report.status === 'rejected' && report.candidate === null, 'Invalid rejected attempt outcome'); summaryReport(report.attempt, this.#candidate);
          if (report.attempt.attempt_revision === this.#attempt) assert(report.attempt.diagnostic_count === this.#summary.diagnostic_count && report.attempt.report_bytes === this.#summary.report_bytes, 'Repeated attempt summary changed');
        }
      });
      const previous = this.#attempt;
      this.#repairs.clear();
      if (report.status === 'accepted') {
        this.#attempt = null; this.#summary = null; this.#target = null; this.#closed = true;
        await this.#retire(previous);
      } else {
        this.#attempt = report.attempt.attempt_revision; this.#summary = copy(report.attempt); this.#target = typeof selected.target === 'string' ? selected.target : null;
        if (previous !== this.#attempt) await this.#retire(previous);
      }
      return copy(report);
    });
  }
  async summary() {
    return this.#serial(async () => {
      const revision = this.#requireAttempt();
      const report = await this.#invoke('attempt/summary', { attempt_revision: revision });
      checked(() => {
        summaryReport(report, this.#candidate, revision);
        assert(report.diagnostic_count === this.#summary.diagnostic_count && report.report_bytes === this.#summary.report_bytes, 'Immutable attempt summary changed');
      });
      return copy(report);
    });
  }
  async report() {
    return this.#serial(async () => {
      const revision = this.#requireAttempt(), chunks = [];
      let offset = 0;
      for (let page = 0; page < 33; page++) {
        const chunk = await this.#invoke('attempt/query', { attempt_revision: revision, offset, chunk_bytes: 65536 });
        const size = checked(() => {
          exact(chunk, ['schema', 'attempt_revision', 'report_schema', 'offset', 'total_bytes', 'chunk', 'next_offset', 'materializable', 'source_authority']);
          assert(chunk.schema === 'semaprax.image-attempt-report-chunk.v1' && chunk.report_schema === 'semaprax.project-candidate-attempt.v1' &&
            chunk.attempt_revision === revision && chunk.offset === offset && index(chunk.total_bytes, MAX_ATTEMPT) &&
            chunk.total_bytes === this.#summary.report_bytes && text(chunk.chunk, 65536) && chunk.materializable === false && chunk.source_authority === false,
            'Invalid attempt report chunk binding');
          const length = Buffer.byteLength(chunk.chunk);
          assert(length > 0 && offset + length <= chunk.total_bytes, 'Attempt report chunk makes no bounded progress');
          const next = offset + length;
          assert(next === chunk.total_bytes ? chunk.next_offset === null : chunk.next_offset === next && length >= 65533,
            'Invalid attempt report continuation');
          return length;
        });
        chunks.push(chunk.chunk); offset += size;
        if (chunk.next_offset === null) {
          const raw = chunks.join('');
          checked(() => {
            assert(Buffer.byteLength(raw) === this.#summary.report_bytes && raw.endsWith('\n') &&
              hash('semaprax.project-candidate-attempt.v1\0', raw) === revision, 'Attempt report exact-byte digest mismatch');
            const report = parse(raw, MAX_ATTEMPT);
            exact(report, ['schema', 'base_candidate_revision', 'base_project_revision', 'state', 'change', 'target_provenance', 'diagnostics',
              'materializable', 'checked_image', 'source_authority', 'tests', 'nonclaims']);
            assert(report.schema === 'semaprax.project-candidate-attempt.v1' && report.base_candidate_revision === this.sourceCandidate &&
              report.base_project_revision === this.#candidate.project_revision && report.state === 'rejected' && report.materializable === false &&
              report.checked_image === false && report.source_authority === false && report.tests === 'not_run' &&
              Array.isArray(report.diagnostics) && report.diagnostics.length === this.#summary.diagnostic_count,
              'Invalid full attempt report binding');
            const claims = ['no_invalid_source_or_hir_retained', 'diagnostic_spans_do_not_identify_verified_base_expressions', 'no_automatic_repair_or_authority'];
            assert(Array.isArray(report.nonclaims) && report.nonclaims.length === claims.length && report.nonclaims.every((item, i) => item === claims[i]), 'Invalid attempt evidence limits');
            changeBinding(report.change, this.#candidate);
            assert((typeof report.change.intent.target === 'string' ? report.change.intent.target : null) === this.#target, 'Attempt target changed');
            let diagnosticBytes = 0;
            report.diagnostics.forEach((diagnostic, position) => {
              exact(diagnostic, ['index', 'code', 'severity', 'message', 'path', 'span', 'help', 'location_basis']);
              assert(diagnostic.index === position && selector(diagnostic.code) && ['error', 'warning'].includes(diagnostic.severity) && text(diagnostic.message) &&
                (diagnostic.path === null || text(diagnostic.path)) && (diagnostic.help === null || text(diagnostic.help)) &&
                diagnostic.location_basis === 'uncommitted_attempt_or_constructor_input_not_authenticated_base_span', 'Invalid attempt diagnostic');
              for (const value of [diagnostic.code, diagnostic.message, diagnostic.path, diagnostic.help]) if (value !== null) diagnosticBytes += Buffer.byteLength(value);
              assert(diagnosticBytes <= MAX_REPORT, 'Attempt diagnostic text limit');
              if (diagnostic.span !== null) {
                exact(diagnostic.span, ['start', 'end', 'line', 'column']);
                assert(Object.values(diagnostic.span).every(value => index(value, Number.MAX_SAFE_INTEGER)) && diagnostic.span.start <= diagnostic.span.end, 'Invalid descriptive diagnostic span');
              }
            });
            if (report.target_provenance !== null) {
              const source = report.target_provenance;
              exact(source, ['id', 'kind', 'identity_origin', 'owner', 'path', 'module', 'source_revision', 'source_digest', 'evidence_owner']);
              assert(source.id === this.#target && selector(source.id, 4096) && text(source.kind) && text(source.identity_origin) &&
                (source.owner === null || text(source.owner)) && text(source.path) && text(source.module) && digest(source.source_revision) && digest(source.source_digest) &&
                source.evidence_owner === 'retained_verified_predecessor_semantic_index', 'Invalid descriptive target provenance');
            }
          });
          // Raw bytes are returned for a read-only document. Parsed diagnostic
          // paths/spans are never opened or mapped onto authoritative source.
          return raw;
        }
      }
      return checked(() => { throw new Error('Attempt report chunk count limit'); });
    });
  }
  async catalog() {
    return this.#serial(async () => {
      const revision = this.#requireAttempt();
      const report = await this.#invoke('attempt/repair-catalog', { attempt_revision: revision });
      checked(() => {
        exact(report, ['schema', 'attempt_revision', 'base_candidate_revision', 'base_project_revision', 'repairs', 'availability_reason', 'legacy_identity_repair', 'tests', 'source_authority', 'nonclaims']);
        assert(report.schema === 'semaprax.project-candidate-repair-catalog.v1' && report.attempt_revision === revision && report.base_candidate_revision === this.sourceCandidate &&
          report.base_project_revision === this.#candidate.project_revision && Array.isArray(report.repairs) && report.repairs.length <= 1 &&
          selector(report.availability_reason, 512) && report.legacy_identity_repair === LEGACY && report.tests === 'not_run' && report.source_authority === false &&
          Array.isArray(report.nonclaims) && report.nonclaims.length === NONCLAIMS.length && report.nonclaims.every((claim, i) => claim === NONCLAIMS[i]), 'Invalid repair catalogue binding');
        assert((report.repairs.length === 1) === (report.availability_reason === 'one_compiler_admitted_typed_repair'), 'Repair availability does not match inventory');
        report.repairs.forEach(row => repairRow(row, this.#candidate, this.#target));
      });
      // Keep only immutable selectors, not executable data from the report.
      this.#repairs = new Map(report.repairs.map(row => [row.repair_id, row.validated_candidate_revision]));
      return copy(report);
    });
  }
  async apply(repairId) {
    return this.#serial(async () => {
      const revision = this.#requireAttempt();
      assert(digest(repairId) && this.#repairs.has(repairId), 'Select a compiler-advertised repair from this attempt');
      const report = await this.#invoke('attempt/repair-apply', { attempt_revision: revision, repair_id: repairId });
      checked(() => candidateHandle(report, this.#candidate.base_revision, this.#repairs.get(repairId)));
      this.#attempt = null; this.#summary = null; this.#target = null; this.#repairs.clear(); this.#closed = true;
      await this.#retire(revision);
      return copy(report);
    });
  }
  async discard() {
    return this.#serial(async () => {
      const revision = this.#requireAttempt();
      const report = await this.#invoke('attempt/discard', { attempt_revision: revision });
      checked(() => this.#discarded(report, revision));
      this.#attempt = null; this.#summary = null; this.#target = null; this.#repairs.clear(); this.#closed = true;
      return copy(report);
    });
  }
}

module.exports = { Repairs, validateCandidateHandle };
