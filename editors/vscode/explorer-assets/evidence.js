'use strict';

// Lazy, source-free evidence tabs. The host owns transport and converts the
// named compiler report into the closed envelope below; this module neither
// parses source nor sends an executable, apply, test, or publication request.
const SCHEMA = 'semaprax.explorer-evidence-read.v1';
const TABS = Object.freeze(['declaration', 'dependencies', 'contracts_effects', 'ownership_cleanup', 'evidence_limits']);
const STATES = Object.freeze(['available', 'not_applicable', 'not_bundled', 'not_requested', 'unsupported', 'stale', 'error']);
const UNSAFE_KEYS = new Set(['body', 'literal', 'raw', 'raw_report', 'report', 'chunk', 'snippet', 'source_body']);

function fail(reason) { throw new TypeError(`explorer evidence ${reason}`); }
function plain(value) { return value !== null && typeof value === 'object' && !Array.isArray(value) && (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null); }
function text(value, name, empty = false) { if (typeof value !== 'string' || (!empty && !value) || value.length > 4096 || value.includes('\0')) fail(`invalid ${name}`); return value; }
function subject(value) {
  if (!plain(value) || Object.keys(value).length !== 7) fail('subject');
  for (const key of ['kind', 'image_revision', 'project_revision', 'workspace_revision', 'project_graph_digest', 'candidate_revision', 'side']) if (!Object.hasOwn(value, key)) fail('subject fields');
  if (!['image', 'candidate'].includes(value.kind)) fail('subject kind');
  for (const key of ['image_revision', 'project_revision', 'workspace_revision', 'project_graph_digest']) text(value[key], key);
  if (value.kind === 'image') { if (value.side !== 'current' || value.candidate_revision !== null) fail('image subject side'); }
  else { if (!['base', 'candidate'].includes(value.side) || typeof value.candidate_revision !== 'string') fail('candidate subject side'); text(value.candidate_revision, 'candidate revision'); }
  return value;
}
function key(value, depth = 0) {
  if (depth > 16) fail('binding nesting');
  if (value === null || typeof value === 'boolean' || typeof value === 'number' || typeof value === 'string') return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(item => key(item, depth + 1)).join(',')}]`;
  if (!plain(value)) fail('binding data');
  return `{${Object.keys(value).sort().map(name => `${JSON.stringify(name)}:${key(value[name], depth + 1)}`).join(',')}}`;
}
function candidateFinal(value) { return value.kind === 'candidate' && value.side === 'candidate'; }
function imageMethod(value, image, candidate) { return candidateFinal(value) ? candidate : image; }
function functionMethod(value, image, candidate) { return imageMethod(value, image, candidate); }
function safeCompact(value, depth = 0) {
  if (depth > 12) fail('compact nesting');
  if (value === null || typeof value === 'boolean') return value;
  if (typeof value === 'number') { if (!Number.isSafeInteger(value)) fail('compact number'); return value; }
  if (typeof value === 'string') return text(value, 'compact text', true);
  if (Array.isArray(value)) { if (value.length > 128) fail('compact array'); return Object.freeze(value.map(item => safeCompact(item, depth + 1))); }
  if (!plain(value) || Object.keys(value).length > 64) fail('compact object');
  const result = {};
  for (const [name, item] of Object.entries(value)) {
    text(name, 'compact key');
    if (UNSAFE_KEYS.has(name)) fail('source-bearing compact field');
    result[name] = safeCompact(item, depth + 1);
  }
  return Object.freeze(result);
}
function plan(selected, declaration, tab, detail) {
  if (!TABS.includes(tab)) fail('tab');
  const functionTarget = declaration.kind === 'function';
  if (tab === 'declaration') {
    if (!functionTarget) return { state: 'not_applicable', reason: 'function_summary_applies_only_to_retained_resolved_functions' };
    return { method: functionMethod(selected, 'image/function-summary', 'candidate/function-summary'), target: declaration.id, facet: null };
  }
  if (tab === 'dependencies') return { method: imageMethod(selected, 'image/dependency-summary', 'candidate/dependency-summary'), target: declaration.id, facet: null };
  if (tab === 'contracts_effects') {
    if (!functionTarget) return { state: 'not_applicable', reason: 'function_contract_facet_not_applicable_to_this_declaration' };
    if (candidateFinal(selected)) return { method: 'candidate/contract-delta', target: null, facet: null };
    return { method: 'image/facet', target: declaration.id, facet: 'contracts' };
  }
  if (tab === 'ownership_cleanup') {
    if (!functionTarget) return { state: 'not_applicable', reason: 'function_ownership_facets_not_applicable_to_this_declaration' };
    if (candidateFinal(selected)) return { method: 'candidate/ownership-delta', target: null, facet: null };
    if (!['ownership', 'loans', 'cleanup'].includes(detail)) fail('ownership detail');
    return { method: 'image/facet', target: declaration.id, facet: detail };
  }
  return { method: imageMethod(selected, 'image/analysis-coverage', 'candidate/analysis-coverage'), target: null, facet: null };
}
function envelope(value, expected) {
  if (!plain(value) || Object.keys(value).length !== 11) fail('evidence envelope');
  for (const name of ['schema', 'subject', 'method', 'target', 'facet', 'state', 'compact', 'omitted', 'nonclaims', 'source_authority', 'execution']) if (!Object.hasOwn(value, name)) fail('evidence fields');
  if (value.schema !== SCHEMA || key(subject(value.subject)) !== key(expected.subject) || value.method !== expected.method || value.target !== expected.target || value.facet !== expected.facet) fail('foreign evidence');
  if (!STATES.includes(value.state) || value.state !== 'available' || value.source_authority !== false || value.execution !== false || !Array.isArray(value.omitted) || !Array.isArray(value.nonclaims)) fail('evidence status');
  value.omitted.forEach(item => text(item, 'omission')); value.nonclaims.forEach(item => text(item, 'nonclaim'));
  return Object.freeze({ state: 'available', method: value.method, target: value.target, facet: value.facet, subject: value.subject, compact: safeCompact(value.compact), omitted: Object.freeze([...value.omitted]), nonclaims: Object.freeze([...value.nonclaims]) });
}
function stateFor(error) {
  const code = error && typeof error === 'object' ? error.code : null;
  if (STATES.includes(code)) return code;
  if (error && /not bundled/i.test(String(error.message || error))) return 'not_bundled';
  return 'error';
}
function status(state, reason, request = null) { return Object.freeze({ state, reason, request, compact: null, omitted: Object.freeze([]), nonclaims: Object.freeze([]) }); }

function createEvidenceInspector(host, selected, declaration) {
  if (!host || typeof host.readEvidence !== 'function') fail('closed evidence host');
  const checkedSubject = subject(selected);
  if (!plain(declaration) || typeof declaration.id !== 'string' || typeof declaration.kind !== 'string') fail('declaration');
  const cache = new Map();
  return Object.freeze({
    async inspect(tab, options = {}) {
      const detail = options.detail || 'ownership';
      const request = plan(checkedSubject, declaration, tab, detail);
      if (request.state) return status(request.state, request.reason);
      const cacheKey = key({ subject: checkedSubject, declaration: declaration.id, tab, detail, request });
      if (!cache.has(cacheKey)) cache.set(cacheKey, Promise.resolve().then(async () => {
        try {
          return envelope(await host.readEvidence(Object.freeze({ method: request.method, subject: checkedSubject, target: request.target, facet: request.facet })), { ...request, subject: checkedSubject });
        } catch (error) { return status(stateFor(error), String(error && error.message || error), request); }
      }));
      return cache.get(cacheKey);
    }
  });
}

const api = { SCHEMA, TABS, STATES, createEvidenceInspector };
if (typeof module !== 'undefined' && module.exports) module.exports = api;
else globalThis.SemapraxExplorerEvidence = api;
