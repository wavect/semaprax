'use strict';

// Lazy, source-free evidence tabs. The host owns transport and converts the
// named compiler report into the closed envelope below; this module neither
// parses source nor sends an executable, apply, test, or publication request.
const SCHEMA = 'semaprax.explorer-evidence-read.v1';
const INDEX_SCHEMA = 'semaprax.explorer-evidence-index.v1';
const TABS = Object.freeze(['declaration', 'dependencies', 'contracts_effects', 'ownership_cleanup', 'evidence_limits']);
const STATES = Object.freeze(['available', 'not_applicable', 'not_bundled', 'not_requested', 'unsupported', 'stale', 'error']);
const INDEX_SLOTS = Object.freeze(['function_summary', 'dependency_summary', 'analysis_coverage', 'contract_delta', 'ownership_delta']);
const UNSAFE_KEYS = new Set(['body', 'literal', 'raw', 'raw_report', 'report', 'chunk', 'snippet', 'source_body', 'source', 'source_text', 'source_files', 'base_source', 'candidate_source', 'source_diff', 'text']);

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
function count(value, name) { if (!Number.isSafeInteger(value) || value < 0) fail(`invalid ${name}`); return value; }
function exact(value, names, name) {
  if (!plain(value) || Object.keys(value).length !== names.length || names.some(key => !Object.hasOwn(value, key))) fail(`invalid ${name}`);
  return value;
}
function strings(value, name) {
  if (!Array.isArray(value) || value.length > 128) fail(`invalid ${name}`);
  return Object.freeze(value.map(item => text(item, name)));
}
function inventory(value, names, name) {
  if (!plain(value) || Object.keys(value).some(key => !names.includes(key))) fail(`invalid ${name}`);
  const result = {};
  for (const key of names) if (Object.hasOwn(value, key)) result[key] = count(value[key], `${name} ${key}`);
  return Object.freeze(result);
}
const COVERAGE_COUNTS = Object.freeze(['functions', 'templates', 'instances', 'nominal_types', 'interfaces', 'interface_imports']);
const DELTA_COUNTS = Object.freeze(['base_functions', 'candidate_functions', 'base_predicates', 'candidate_predicates', 'base_functions_with_contracts', 'candidate_functions_with_contracts', 'unchanged_functions', 'affected_functions', 'base_source_only_functions', 'candidate_source_only_functions', 'base_types', 'candidate_types', 'affected_types']);
function offlineCompact(slot, value) {
  // Offline exports carry only the source-free shapes created by explorer.js.
  // This allowlist deliberately excludes generic nested report values, even
  // when they happen not to use a source-looking key.
  if (slot === 'function_summary') {
    exact(value, ['id', 'parameter_count', 'return_type_id', 'effects', 'requires_count', 'ensures_count', 'facets'], slot);
    return Object.freeze({ id: text(value.id, 'summary id'), parameter_count: count(value.parameter_count, 'parameter count'), return_type_id: text(value.return_type_id, 'return type'), effects: strings(value.effects, 'effect'), requires_count: count(value.requires_count, 'requires count'), ensures_count: count(value.ensures_count, 'ensures count'), facets: strings(value.facets, 'facet') });
  }
  if (slot === 'dependency_summary') {
    exact(value, ['target', 'kind', 'facets', 'test_reachable'], slot);
    if (!Array.isArray(value.facets) || value.facets.length > 128 || typeof value.test_reachable !== 'boolean') fail('invalid dependency summary');
    return Object.freeze({ target: text(value.target, 'dependency target'), kind: text(value.kind, 'dependency kind'), facets: Object.freeze(value.facets.map(row => { exact(row, ['view', 'total_items'], 'dependency facet'); return Object.freeze({ view: text(row.view, 'dependency view'), total_items: count(row.total_items, 'dependency count') }); })), test_reachable: value.test_reachable });
  }
  if (slot === 'analysis_coverage') {
    exact(value, ['inventory', 'areas'], slot);
    if (!Array.isArray(value.areas) || value.areas.length > 128) fail('invalid coverage areas');
    return Object.freeze({ inventory: inventory(value.inventory, COVERAGE_COUNTS, 'coverage inventory'), areas: Object.freeze(value.areas.map(row => { exact(row, ['area', 'status'], 'coverage area'); return Object.freeze({ area: text(row.area, 'coverage area'), status: text(row.status, 'coverage status') }); })) });
  }
  if (slot === 'contract_delta' || slot === 'ownership_delta') {
    exact(value, ['inventory', 'changed'], slot);
    if (!Array.isArray(value.changed) || value.changed.length > 65536) fail('invalid delta changes');
    return Object.freeze({ inventory: inventory(value.inventory, DELTA_COUNTS, 'delta inventory'), changed: Object.freeze(value.changed.map(row => { exact(row, ['id', 'change'], 'delta change'); return Object.freeze({ id: text(row.id, 'change id'), change: text(row.change, 'change kind') }); })) });
  }
  fail('offline compact slot');
}
function read(key, method, target, facet = null) { return Object.freeze({ key, method, target, facet }); }
function plan(selected, declaration, tab, detail, offline = false) {
  if (!TABS.includes(tab)) fail('tab');
  const functionTarget = declaration.kind === 'function';
  if (tab === 'declaration') {
    if (!functionTarget) return { state: 'not_applicable', reason: 'function_summary_applies_only_to_retained_resolved_functions' };
    return { requests: [read('declaration', functionMethod(selected, 'image/function-summary', 'candidate/function-summary'), declaration.id)] };
  }
  if (tab === 'dependencies') return { requests: [read('dependencies', imageMethod(selected, 'image/dependency-summary', 'candidate/dependency-summary'), declaration.id)] };
  if (tab === 'contracts_effects') {
    if (!functionTarget) return { state: 'not_applicable', reason: 'function_contract_facet_not_applicable_to_this_declaration' };
    if (candidateFinal(selected)) {
      // Deltas are whole-candidate reports and therefore never take a target.
      // Offline bundles deliberately retain only the compact delta inventory.
      if (offline) return { requests: [read('contract_changes', 'candidate/contract-delta', null)] };
      return { requests: [
        read('contract_changes', 'candidate/contract-delta', null),
        read('declared_effects', 'candidate/function-summary', declaration.id),
        read('checked_contracts', 'candidate/function-facet', declaration.id, 'contracts')
      ] };
    }
    return { requests: [
      read('declared_effects', 'image/function-summary', declaration.id),
      read('checked_contracts', 'image/facet', declaration.id, 'contracts')
    ] };
  }
  if (tab === 'ownership_cleanup') {
    if (!functionTarget) return { state: 'not_applicable', reason: 'function_ownership_facets_not_applicable_to_this_declaration' };
    if (!['ownership', 'loans', 'cleanup'].includes(detail)) fail('ownership detail');
    const method = functionMethod(selected, 'image/facet', 'candidate/function-facet');
    if (candidateFinal(selected) && offline) return { requests: [read('ownership_changes', 'candidate/ownership-delta', null)] };
    const facts = [
      read('ownership', method, declaration.id, 'ownership'),
      read('loan_plan', method, declaration.id, 'loans'),
      read('cleanup_plan', method, declaration.id, 'cleanup')
    ];
    return { requests: candidateFinal(selected) ? [read('ownership_changes', 'candidate/ownership-delta', null), ...facts] : facts };
  }
  return { requests: [read('analysis_coverage', imageMethod(selected, 'image/analysis-coverage', 'candidate/analysis-coverage'), offline ? declaration.id : null)] };
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
  const message = String(error && error.message || error);
  // The VS Code message bridge retains an error's text but not its structured
  // code. Keep the compiler's ordinary closed refusals distinct at that edge.
  if (/not bundled/i.test(message)) return 'not_bundled';
  if (/not requested/i.test(message)) return 'not_requested';
  if (/\bstale\b/i.test(message)) return 'stale';
  if (/\bunsupported\b|not selected|host permission|capability.*(?:absent|denied|required)/i.test(message)) return 'unsupported';
  return 'error';
}
function status(state, reason, request = null) { return Object.freeze({ state, reason, request, compact: null, omitted: Object.freeze([]), nonclaims: Object.freeze([]) }); }
function combined(subject, reads, results) {
  if (reads.length === 1) return results[0];
  const compact = {}, omissions = new Set(), nonclaims = new Set();
  for (let index = 0; index < reads.length; index++) {
    compact[reads[index].key] = results[index].compact;
    for (const item of results[index].omitted) omissions.add(item);
    for (const item of results[index].nonclaims) nonclaims.add(item);
  }
  return Object.freeze({ state: 'available', method: 'combined_read_only_evidence', target: null, facet: null,
    subject, compact: Object.freeze(compact), omitted: Object.freeze([...omissions].sort()), nonclaims: Object.freeze([...nonclaims].sort()) });
}

// Offline bundles can carry only this compact, source-free index. Entries are
// closed over a subject already present in the snapshot, so a row cannot be
// repurposed for another revision or side.
function offlineIndex(value, knownSubjects) {
  if (!plain(value) || Object.keys(value).length !== 2 || value.schema !== INDEX_SCHEMA || !Array.isArray(value.entries) || value.entries.length > 8192 || !Array.isArray(knownSubjects)) fail('offline index');
  const known = new Set(knownSubjects.map(item => key(subject(item))));
  const rows = new Map();
  for (const entry of value.entries) {
    if (!plain(entry) || Object.keys(entry).length !== 5 || !Object.hasOwn(entry, 'subject') || !Object.hasOwn(entry, 'target') || !Object.hasOwn(entry, 'states') || !Object.hasOwn(entry, 'compact') || !Object.hasOwn(entry, 'omitted')) fail('offline entry');
    const checkedSubject = subject(entry.subject), subjectKey = key(checkedSubject), target = entry.target === null ? null : text(entry.target, 'offline target');
    if (!known.has(subjectKey) || !plain(entry.states) || !plain(entry.compact) || !Array.isArray(entry.omitted)) fail('offline binding');
    const stateKeys = Object.keys(entry.states);
    if (!['function_summary', 'dependency_summary', 'analysis_coverage'].every(name => Object.hasOwn(entry.states, name)) || stateKeys.some(name => !INDEX_SLOTS.includes(name))) fail('offline states');
    const compact = {};
    for (const [slot, value] of Object.entries(entry.compact)) {
      if (!INDEX_SLOTS.includes(slot)) fail('offline compact slot');
      compact[slot] = offlineCompact(slot, value);
    }
    const states = {};
    for (const [slot, value] of Object.entries(entry.states)) {
      if (!['available', 'not applicable', 'error'].includes(value)) fail('offline state');
      if (value === 'available' && !Object.hasOwn(compact, slot)) fail('offline available compact');
      states[slot] = value === 'not applicable' ? 'not_applicable' : value;
    }
    const omitted = entry.omitted.map(item => text(item, 'offline omission'));
    const rowKey = key({ subject: checkedSubject, target });
    if (rows.has(rowKey)) fail('duplicate offline entry');
    rows.set(rowKey, Object.freeze({ subject: checkedSubject, target, states: Object.freeze(states), compact: Object.freeze(compact), omitted: Object.freeze(omitted) }));
  }
  return rows;
}

function offlineSlot(request) {
  if (request.method.endsWith('function-summary')) return 'function_summary';
  if (request.method.endsWith('dependency-summary')) return 'dependency_summary';
  if (request.method.endsWith('analysis-coverage')) return 'analysis_coverage';
  if (request.method === 'candidate/contract-delta') return 'contract_delta';
  if (request.method === 'candidate/ownership-delta') return 'ownership_delta';
  return null;
}

function offlineEnvelope(index, request) {
  const target = request.target === null ? null : text(request.target, 'offline request target');
  const row = index.get(key({ subject: subject(request.subject), target }));
  const slot = offlineSlot(request);
  if (!row || !slot) return null;
  if (!Object.hasOwn(row.states, slot)) {
    if (slot === 'contract_delta' || slot === 'ownership_delta') return Object.freeze({ state: 'not_applicable', reason: `offline_${slot}_not_applicable` });
    return null;
  }
  if (row.states[slot] !== 'available') return Object.freeze({ state: row.states[slot], reason: `offline_${slot}_${row.states[slot]}` });
  return Object.freeze({ schema: SCHEMA, subject: row.subject, method: request.method, target: request.target, facet: request.facet, state: 'available', compact: row.compact[slot], omitted: row.omitted, nonclaims: Object.freeze(['offline bundled compact evidence']), source_authority: false, execution: false });
}

function createEvidenceInspector(host, selected, declaration) {
  if (!host || typeof host.readEvidence !== 'function') fail('closed evidence host');
  const checkedSubject = subject(selected);
  if (!plain(declaration) || typeof declaration.id !== 'string' || typeof declaration.kind !== 'string') fail('declaration');
  const cache = new Map();
  return Object.freeze({
    async inspect(tab, options = {}) {
      const detail = options.detail || 'ownership';
      const requested = plan(checkedSubject, declaration, tab, detail, host.offline === true);
      if (requested.state) return status(requested.state, requested.reason);
      const cacheKey = key({ subject: checkedSubject, declaration: declaration.id, tab, detail, requested });
      if (!cache.has(cacheKey)) cache.set(cacheKey, Promise.resolve().then(async () => {
        const results = [];
        for (const request of requested.requests) {
          try {
            results.push(envelope(await host.readEvidence(Object.freeze({ method: request.method, subject: checkedSubject, target: request.target, facet: request.facet })), { ...request, subject: checkedSubject }));
          } catch (error) { return status(stateFor(error), String(error && error.message || error), request); }
        }
        return combined(checkedSubject, requested.requests, results);
      }));
      return cache.get(cacheKey);
    }
  });
}

const api = { SCHEMA, INDEX_SCHEMA, TABS, STATES, offlineIndex, offlineEnvelope, createEvidenceInspector };
if (typeof module !== 'undefined' && module.exports) module.exports = api;
else globalThis.SemapraxExplorerEvidence = api;
