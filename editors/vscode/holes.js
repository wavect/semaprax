'use strict';
// Saved-source draft handles only. No filesystem, publication, execution,
// recovery import or implicit completion authority enters this controller.
const { exact, digest } = require('./protocol');
const { canonical, hash } = require('./review');
const MAX_CONTEXT = 1024 * 1024, MAX_PAGE = 65536, MAX_ITEMS = 16384;
const FACETS = ['scope', 'calls', 'obligations', 'constructors'];
const KINDS = {
  body: ['hole/open', 'semaprax.project-candidate-hole-context.v1', 'replace_function_body'],
  expression: ['hole/open-expression', 'semaprax.project-candidate-expression-hole-context.v1', 'replace_expression'],
  contract: ['hole/open-contract-expression', 'semaprax.project-candidate-contract-expression-hole-context.v1', 'replace_contract_expression']
};
const own = value => ['value', 'own', 'borrow', 'shared'].includes(value);
const text = (value, max = MAX_CONTEXT) => typeof value === 'string' && Buffer.byteLength(value) <= max;
const selector = (value, max) => text(value, max) && value.length > 0 && !value.includes('\0');
const index = (value, max = MAX_ITEMS) => Number.isSafeInteger(value) && value >= 0 && value <= max;
const strings = value => Array.isArray(value) && value.length <= MAX_ITEMS && value.every(item => text(item));
const copy = value => JSON.parse(JSON.stringify(value));
function assert(condition, message) { if (!condition) throw new Error(message); }
function checked(action) {
  try { return action(); } catch (error) { error.protocolInvalid = true; throw error; }
}
function bounded(value, max) { assert(Buffer.byteLength(JSON.stringify(value)) + 1 <= max, 'Hole report byte limit'); }
function scope(row, compact) {
  exact(row, compact ? ['id', 'name', 'type_id', 'ownership', 'mutable'] : ['value_id', 'name', 'type', 'ownership', 'mutable']);
  assert(text(row[compact ? 'id' : 'value_id']) && text(row.name) && text(row[compact ? 'type_id' : 'type']) && own(row.ownership) &&
    (typeof row.mutable === 'boolean' || (compact && row.mutable === null)), 'Invalid hole scope row');
}
function callRow(row) {
  exact(row, ['id', 'binding', 'return_type_id', 'parameters', 'effects', 'within_effect_budget', 'basis', 'admission']);
  assert(text(row.id) && text(row.binding) && text(row.return_type_id) && strings(row.effects) && typeof row.within_effect_budget === 'boolean' &&
    row.basis === 'existing_local_or_authenticated_import_binding' && row.admission === 'requires_fill_revalidation' &&
    Array.isArray(row.parameters) && row.parameters.length <= MAX_ITEMS, 'Invalid accessible call row');
  for (const param of row.parameters) {
    exact(param, ['name', 'type_id', 'ownership']);
    assert(text(param.name) && text(param.type_id) && own(param.ownership), 'Invalid accessible call parameter');
  }
}
function draftHandle(row, candidate) {
  exact(row, ['schema', 'draft_revision', 'source_candidate_revision', 'report_bytes', 'source_authority', 'buildable']);
  assert(row.schema === 'semaprax.image-draft-handle.v1' && digest(row.draft_revision) && row.source_candidate_revision === candidate &&
    index(row.report_bytes, MAX_CONTEXT) && row.report_bytes > 0 && row.source_authority === false && row.buildable === false, 'Invalid draft handle');
  return row;
}
function candidateHandle(row) {
  exact(row, ['schema', 'candidate_revision', 'project_revision', 'base_revision', 'report_bytes', 'source_authority', 'tests']);
  assert(row.schema === 'semaprax.image-candidate-handle.v1' && digest(row.candidate_revision) && digest(row.project_revision) && digest(row.base_revision) &&
    index(row.report_bytes, 16 * MAX_CONTEXT) && row.report_bytes > 0 && row.source_authority === false && row.tests === 'not_run', 'Invalid completed candidate handle');
  return row;
}
function discarded(row, revision) {
  exact(row, ['schema', 'draft_revision', 'discarded', 'source_unchanged']);
  assert(row.schema === 'semaprax.image-draft-discard.v1' && row.draft_revision === revision && row.discarded === true && row.source_unchanged === true, 'Invalid draft discard receipt');
}

class HoleDraft {
  #call; #image; #candidate; #draft = null; #pending = []; #filled = false;
  #busy = false; #closed = false; #summaries = new Map(); #choices = new Map();
  constructor(call, image, candidate) {
    assert(typeof call === 'function' && digest(image) && digest(candidate), 'A current image and candidate are required');
    this.#call = call; this.#image = image; this.#candidate = candidate;
  }
  get draftRevision() { return this.#draft; }
  get sourceCandidate() { return this.#candidate; }
  get pending() { return this.#pending.map(row => ({ ...row })); }
  get hasFilled() { return this.#filled; }
  async #serial(action) {
    assert(!this.#closed, 'This draft controller has been completed or discarded');
    assert(!this.#busy, 'A hole operation is already pending');
    this.#busy = true;
    try { return await action(); }
    catch (error) { if (error.protocolInvalid || error.discardOnly) this.#closed = true; throw error; }
    finally { this.#busy = false; }
  }
  async #invoke(method, params) {
    const response = await this.#call(method, { image_revision: this.#image, ...params });
    return checked(() => {
      assert(response && response.image_revision === this.#image && response.payload && typeof response.payload === 'object' && !Array.isArray(response.payload), 'Hole response image mismatch');
      return response.payload;
    });
  }
  #hole(id) {
    assert(this.#draft !== null, 'Open a typed hole first');
    const hole = this.#pending.find(row => row.holeId === id);
    assert(hole, 'Select a pending hole in this draft'); return hole;
  }
  #planning() { assert(!this.#filled, 'Plan all holes before filling: this editor does not add holes after a successful fill'); }
  async #retire(revision) {
    if (revision === null) return;
    try {
      const report = await this.#invoke('hole/discard', { draft_revision: revision });
      checked(() => discarded(report, revision));
    } catch (cause) {
      this.#closed = true;
      const error = new Error('The new draft or candidate was accepted, but retirement of its prior draft failed. Restart the session; the mutation was not rolled back.');
      error.protocolInvalid = true; error.cause = cause; throw error;
    }
  }
  async constructorSchemas() {
    return this.#serial(async () => {
      const report = await this.#invoke('protocol/constructor-schemas', {});
      checked(() => {
        bounded(report, 256 * 1024);
        exact(report, ['schema', 'documents', 'admission', 'requires_compiler_admission', 'limits', 'nonclaims']);
        assert(report.schema === 'semaprax.candidate-constructor-schemas.v1' && report.admission === 'closed_structural_grammar_only' &&
          report.requires_compiler_admission === true && strings(report.nonclaims) && Array.isArray(report.documents) && report.documents.length === 4, 'Invalid constructor schema bundle');
        const ids = ['urn:semaprax.typed-expression.v1', 'urn:semaprax.semantic-change-intent.v1', 'urn:semaprax.semantic-change.v1', 'urn:semaprax.project-candidate-recovery.v1'];
        report.documents.forEach((document, position) => {
          assert(document && typeof document === 'object' && !Array.isArray(document) && document.$id === ids[position] &&
            document.$schema === 'https://json-schema.org/draft/2020-12/schema' && document.$defs && typeof document.$defs === 'object' && !Array.isArray(document.$defs), 'Invalid constructor schema document identity');
        });
        exact(report.limits, ['max_change_bytes', 'max_json_value_nodes', 'max_json_value_depth', 'max_expression_nodes', 'max_expression_depth']);
        assert(index(report.limits.max_change_bytes, MAX_CONTEXT) && report.limits.max_change_bytes > 0 && report.limits.max_json_value_nodes === 8192 &&
          report.limits.max_json_value_depth === 64 && report.limits.max_expression_nodes === 4096 && report.limits.max_expression_depth === 64, 'Invalid constructor schema limits');
      });
      // Display only: never resolve references, fetch URLs or use these rounded
      // JavaScript numeric bounds as evidence of compiler admission.
      return copy(report);
    });
  }
  async expressionChoices(kind, target) {
    return this.#serial(async () => {
      this.#planning();
      assert((kind === 'expression' || kind === 'contract') && selector(target, 512), 'Select an expression or contract target');
      const schema = kind === 'contract' ? 'semaprax.project-contract-expression-catalog.v1' : 'semaprax.project-expression-catalog.v1';
      const report = await this.#invoke(kind === 'contract' ? 'candidate/contract-expression-catalog' : 'expression/catalog',
        { candidate_revision: this.#candidate, target });
      const rows = checked(() => {
        bounded(report, MAX_CONTEXT);
        exact(report, ['schema', 'candidate_digest', 'project_revision', 'target', 'source', 'declared_effect_budget', 'expressions', 'limits', 'nonclaims']);
        assert(report.schema === schema && report.candidate_digest === this.#candidate && report.target === target && digest(report.project_revision) &&
          strings(report.declared_effect_budget) && strings(report.nonclaims) && Array.isArray(report.expressions) && report.expressions.length <= 4096, 'Invalid expression catalogue binding');
        exact(report.source, ['path', 'module', 'source_revision', 'source_digest']);
        assert(text(report.source.path) && text(report.source.module) && digest(report.source.source_revision) && digest(report.source.source_digest), 'Invalid expression source binding');
        exact(report.limits, ['max_expressions', 'max_depth', 'max_scope_facts', 'max_bytes']);
        assert(report.limits.max_expressions === 4096 && report.limits.max_depth === 256 && report.limits.max_scope_facts === MAX_ITEMS && report.limits.max_bytes === MAX_CONTEXT, 'Invalid expression catalogue limits');
        const ids = new Set(); let scopes = 0;
        for (const row of report.expressions) {
          exact(row, ['expression_id', 'phase', 'kind', 'expected_type', 'ownership', 'source_span', 'replaceable', 'reason', 'scope']);
          assert(selector(row.expression_id, 4096) && !ids.has(row.expression_id) && ['body', 'requires', 'ensures'].includes(row.phase) && text(row.kind) &&
            text(row.expected_type) && own(row.ownership) && typeof row.replaceable === 'boolean' && text(row.reason) && Array.isArray(row.scope), 'Invalid expression choice');
          ids.add(row.expression_id); scopes += row.scope.length; assert(scopes <= MAX_ITEMS, 'Expression scope inventory limit');
          row.scope.forEach(item => scope(item, false));
          exact(row.source_span, ['start', 'end', 'line', 'column']);
          assert(Object.values(row.source_span).every(value => index(value, Number.MAX_SAFE_INTEGER)) && row.source_span.start <= row.source_span.end, 'Invalid expression source span');
          assert(kind !== 'contract' || row.phase !== 'body', 'Contract catalogue contains a body selection');
        }
        return report.expressions.filter(row => row.replaceable && (kind === 'contract' ? row.phase !== 'body' : row.phase === 'body'));
      });
      this.#choices.clear();
      this.#choices.set(JSON.stringify([kind, target]), new Set(rows.map(row => row.expression_id)));
      return copy(rows);
    });
  }
  async open(kind, target, holeId, expressionId) {
    return this.#serial(async () => {
      this.#planning();
      assert(Object.hasOwn(KINDS, kind) && selector(target, 512) && selector(holeId, 128), 'Invalid typed hole selection');
      assert(this.#pending.length < 16 && !this.#pending.some(row => row.holeId === holeId), 'Hole ID is duplicated or the 16-hole limit was reached');
      if (kind === 'body') assert(expressionId === undefined, 'Body holes do not take an expression selector');
      else assert(selector(expressionId, 4096) && this.#choices.get(JSON.stringify([kind, target]))?.has(expressionId), 'Select an expression from the current candidate catalogue first');
      const params = { candidate_revision: this.#candidate, target, hole_id: holeId };
      if (this.#draft !== null) params.draft_revision = this.#draft;
      if (kind !== 'body') params.expression_id = expressionId;
      const old = this.#draft, report = await this.#invoke(KINDS[kind][0], params);
      checked(() => { draftHandle(report, this.#candidate); assert(report.draft_revision !== old, 'Opening a hole did not change the draft identity'); });
      this.#draft = report.draft_revision; this.#pending.push({ holeId, target, kind, ...(kind === 'body' ? {} : { expressionId }) });
      this.#summaries.clear(); await this.#retire(old); return copy(report);
    });
  }
  async summary(holeId) {
    return this.#serial(async () => {
      const hole = this.#hole(holeId), report = await this.#invoke('hole/summary', { draft_revision: this.#draft, hole_id: holeId });
      checked(() => {
        bounded(report, MAX_PAGE);
        exact(report, ['schema', 'context_schema', 'context_revision', 'draft_revision', 'hole_id', 'hole_handle', 'target', 'last_valid_revision', 'expected_type_id',
          'expected_ownership', 'intent_kind', 'effect_policy', 'facets', 'full_context_method', 'materializable', 'source_authority', 'validation', 'evidence_class']);
        assert(report.schema === 'semaprax.project-hole-summary.v1' && report.context_schema === KINDS[hole.kind][1] && digest(report.context_revision) &&
          report.draft_revision === this.#draft && report.hole_id === holeId && report.target === hole.target && digest(report.hole_handle) && digest(report.last_valid_revision) &&
          text(report.expected_type_id) && (hole.kind === 'body' ? report.expected_ownership === null : own(report.expected_ownership)) && report.intent_kind === KINDS[hole.kind][2] &&
          report.full_context_method === 'hole/query' && report.materializable === false && report.source_authority === false && report.validation === 'pending_fill_full_source_replay' &&
          report.evidence_class === 'descriptive_context_not_candidate_validation', 'Invalid compact hole summary binding');
        exact(report.effect_policy, ['allowed', 'forbidden', 'module_permits', 'enclosing_declared_effects']);
        assert(strings(report.effect_policy.allowed) && text(report.effect_policy.forbidden) && strings(report.effect_policy.module_permits) &&
          (hole.kind === 'contract' ? strings(report.effect_policy.enclosing_declared_effects) : report.effect_policy.enclosing_declared_effects === null), 'Invalid hole effect policy');
        assert(Array.isArray(report.facets) && report.facets.length === 4, 'Invalid hole facets');
        report.facets.forEach((row, position) => {
          exact(row, ['facet', 'count', 'reference']);
          assert(row.facet === FACETS[position] && index(row.count) && digest(row.reference), 'Invalid hole facet reference');
          const binding = { draft_revision: this.#draft, hole_id: holeId, context_revision: report.context_revision, facet: row.facet };
          assert(hash('semaprax.project-hole-facet.v1\0', canonical(binding) + '\n') === row.reference, 'Hole facet reference binding mismatch');
        });
      });
      this.#summaries.set(holeId, canonical(report)); return copy(report);
    });
  }
  async page(summary, facet, offset = 0, limit = 16) {
    return this.#serial(async () => {
      assert(summary && this.#summaries.get(summary.hole_id) === canonical(summary) && summary.draft_revision === this.#draft, 'Use a current summary from this controller');
      this.#hole(summary.hole_id);
      const selected = summary.facets.find(row => row.facet === facet);
      assert(selected && index(offset) && offset <= selected.count && index(limit, 64) && limit >= 1, 'Invalid facet page selection');
      const report = await this.#invoke('hole/page', { draft_revision: this.#draft, hole_id: summary.hole_id, reference: selected.reference, offset, limit });
      checked(() => {
        bounded(report, MAX_PAGE);
        exact(report, ['schema', 'draft_revision', 'hole_id', 'context_revision', 'facet', 'reference', 'total', 'offset', 'next_offset', 'items', 'source_authority']);
        assert(report.schema === 'semaprax.project-hole-page.v1' && report.draft_revision === this.#draft && report.hole_id === summary.hole_id &&
          report.context_revision === summary.context_revision && report.facet === facet && report.reference === selected.reference && report.total === selected.count &&
          index(report.total) && report.offset === offset && report.source_authority === false && Array.isArray(report.items) && report.items.length <= limit &&
          offset + report.items.length <= report.total, 'Invalid hole page binding');
        const next = offset + report.items.length;
        assert(next === report.total ? report.next_offset === null : report.items.length > 0 && report.next_offset === next && index(report.next_offset), 'Hole page does not make complete progress');
        for (const item of report.items) {
          if (facet === 'scope') scope(item, true);
          else if (facet === 'calls') callRow(item);
          else assert(text(item), 'Invalid hole string facet');
        }
      });
      return copy(report);
    });
  }
  async context(holeId) {
    return this.#serial(async () => {
      const hole = this.#hole(holeId), report = await this.#invoke('hole/query', { draft_revision: this.#draft, hole_id: holeId });
      checked(() => {
        bounded(report, MAX_CONTEXT);
        assert(report.schema === KINDS[hole.kind][1] && report.draft_digest === this.#draft && report.hole_id === holeId && report.target === hole.target &&
          digest(report.hole_handle) && digest(report.last_valid_revision) && text(report.expected_type_id) && report.intent_kind === KINDS[hole.kind][2] &&
          report.materializable === false && report.source_authority === false && report.validation === 'pending_fill_full_source_replay', 'Invalid full hole context binding');
        if (hole.kind !== 'body') assert(selector(report.expression_id, 4096) && own(report.expected_ownership) &&
          (hole.expressionId === undefined || hole.expressionId === report.expression_id), 'Invalid current expression context selector');
        const summary = this.#summaries.get(holeId);
        if (summary) {
          const selected = JSON.parse(summary);
          assert(selected.hole_handle === report.hole_handle && selected.last_valid_revision === report.last_valid_revision && selected.expected_type_id === report.expected_type_id,
            'Full context differs from the selected summary');
        }
      });
      // Full proof subtrees remain unbundled descriptive JSON. Never hash a
      // reparsed context: its u64 proof numbers may exceed JS exact integers.
      if (hole.kind !== 'body') hole.expressionId = report.expression_id;
      return copy(report);
    });
  }
  async fill(holeId, expression) {
    return this.#serial(async () => {
      this.#hole(holeId);
      assert(expression && typeof expression === 'object' && !Array.isArray(expression), 'A typed expression object is required');
      const old = this.#draft, report = await this.#invoke('hole/fill', { draft_revision: this.#draft, hole_id: holeId, expression });
      checked(() => { draftHandle(report, this.#candidate); assert(report.draft_revision !== old, 'Filling a hole did not change the draft identity'); });
      this.#draft = report.draft_revision; this.#pending = this.#pending.filter(row => row.holeId !== holeId);
      // Surviving expression IDs are revision-scoped and will be obtained from
      // their next current full context; original-candidate choices are stale.
      this.#pending.forEach(row => { delete row.expressionId; });
      this.#filled = true; this.#summaries.clear(); this.#choices.clear(); await this.#retire(old); return copy(report);
    });
  }
  async complete() {
    return this.#serial(async () => {
      assert(this.#draft !== null && this.#pending.length === 0, 'Fill every pending hole before explicitly completing');
      const old = this.#draft, report = await this.#invoke('hole/complete', { draft_revision: this.#draft });
      checked(() => candidateHandle(report));
      this.#draft = null; this.#closed = true; this.#summaries.clear(); await this.#retire(old); return copy(report);
    });
  }
  async discard() {
    return this.#serial(async () => {
      assert(this.#draft !== null, 'No current draft to discard');
      const selected = this.#draft, report = await this.#invoke('hole/discard', { draft_revision: selected });
      checked(() => discarded(report, selected));
      this.#draft = null; this.#pending = []; this.#closed = true; this.#summaries.clear(); this.#choices.clear(); return copy(report);
    });
  }
}
module.exports = { HoleDraft };
