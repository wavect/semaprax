'use strict';

// Candidate-change projection. This module consumes exact compiler reports and
// the existing closed explorer host; it never matches declarations by name,
// opens source, or derives a combined semantic revision.
const explorer = typeof module !== 'undefined' && module.exports ? require('./model.js') : globalThis.SemapraxExplorerModel;

const CATALOG_SCHEMA = 'semaprax.project-candidate-semantic-delta-catalog.v1';
const DELTA_SCHEMA = 'semaprax.project-candidate-semantic-delta.v1';
const SOURCE_REVIEW_SCHEMA = 'semaprax.project-candidate-source-review.v1';
const MAX_ROOTS = 65536;
const MAX_PAGES = 1024;
const MAX_SOURCE_FILES = 16;
const MAX_SOURCE_REVIEW_BYTES = 16 * 1024 * 1024;

function fail(reason) { throw new TypeError(`explorer changes ${reason}`); }
function plain(value) { return value !== null && typeof value === 'object' && !Array.isArray(value) && (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null); }
function string(value, label) { if (typeof value !== 'string' || !value || value.length > 4096 || value.includes('\0')) fail(`invalid ${label}`); return value; }
function digest(value, label) { if (typeof value !== 'string' || !/^sha256:[0-9a-f]{64}$/.test(value)) fail(`invalid ${label}`); return value; }
function exact(value, required) {
  if (!plain(value) || Object.keys(value).length !== required.length || required.some(key => !Object.hasOwn(value, key))) fail('unexpected report fields');
  return value;
}
function compact(value) {
  exact(value, ['id', 'name', 'kind', 'path', 'module', 'fragment_digest']);
  for (const key of ['id', 'name', 'kind', 'path', 'module', 'fragment_digest']) string(value[key], `compact ${key}`);
  return value;
}

function sourcePath(value) {
  string(value, 'source path');
  if (value.length > 240 || !value.endsWith('.spx') || value.startsWith('/') || value.startsWith('\\') || /^[A-Za-z]:/.test(value) || value.split('/').some(part => !part || part === '.' || part === '..')) fail('source path');
  return value;
}

function sourceBytes(value) {
  string(value, 'source text');
  if (value.includes('\u0000') || new TextEncoder().encode(value).length > MAX_SOURCE_REVIEW_BYTES) fail('source text');
  return value;
}

// Source text is admitted only through the immutable compiler source-review
// report. The viewer never reads a path, produces a diff, or treats text as
// executable input.
function sourceReview(value, candidateRevision, baseProjectRevision, candidateProjectRevision) {
  exact(value, ['schema', 'base_project_revision', 'candidate_project_revision', 'candidate_revision', 'source_authority', 'files', 'report_revision']);
  if (value.schema !== SOURCE_REVIEW_SCHEMA || value.source_authority !== false) fail('source review schema');
  for (const key of ['base_project_revision', 'candidate_project_revision', 'candidate_revision', 'report_revision']) string(value[key], `source review ${key}`);
  if (value.candidate_revision !== candidateRevision || value.base_project_revision !== baseProjectRevision || value.candidate_project_revision !== candidateProjectRevision || !Array.isArray(value.files) || value.files.length > MAX_SOURCE_FILES) fail('source review binding');
  let used = 0, previous = null;
  const files = value.files.map(file => {
    exact(file, ['path', 'base_source', 'candidate_source', 'base_digest', 'candidate_digest', 'source_diff', 'source_diff_digest']);
    const path = sourcePath(file.path);
    if (previous !== null && previous >= path) fail('source review order');
    previous = path;
    const base_source = sourceBytes(file.base_source), candidate_source = sourceBytes(file.candidate_source), source_diff = sourceBytes(file.source_diff);
    for (const key of ['base_digest', 'candidate_digest', 'source_diff_digest']) string(file[key], `source review ${key}`);
    if (base_source === candidate_source || !source_diff) fail('source review diff');
    used += new TextEncoder().encode(base_source).length + new TextEncoder().encode(candidate_source).length + new TextEncoder().encode(source_diff).length;
    if (used > MAX_SOURCE_REVIEW_BYTES) fail('source review capacity');
    return Object.freeze({ path, base_source, candidate_source, base_digest: file.base_digest, candidate_digest: file.candidate_digest, source_diff, source_diff_digest: file.source_diff_digest });
  });
  return Object.freeze({ base_project_revision: value.base_project_revision, candidate_project_revision: value.candidate_project_revision, candidate_revision: value.candidate_revision, report_revision: value.report_revision, files: Object.freeze(files) });
}

function catalog(value) {
  exact(value, ['schema', 'candidate_digest', 'base_project_revision', 'project_revision', 'roots', 'selection_basis', 'source_changes', 'nonclaims']);
  if (value.schema !== CATALOG_SCHEMA) fail('catalog schema');
  for (const key of ['candidate_digest', 'base_project_revision', 'project_revision', 'selection_basis']) string(value[key], key);
  if (!Array.isArray(value.roots) || value.roots.length > MAX_ROOTS || !Array.isArray(value.source_changes) || !Array.isArray(value.nonclaims)) fail('catalog inventory');
  const seen = new Set();
  value.roots.forEach(root => {
    exact(root, ['target', 'change', 'base', 'candidate']);
    string(root.target, 'target');
    if (seen.has(root.target)) fail('duplicate target');
    seen.add(root.target);
    if (!['added', 'removed', 'modified', 'moved'].includes(root.change)) fail('catalog change');
    if (root.base !== null) compact(root.base);
    if (root.candidate !== null) compact(root.candidate);
    if (root.change === 'added' && (root.base !== null || root.candidate === null)) fail('added presence');
    if (root.change === 'removed' && (root.base === null || root.candidate !== null)) fail('removed presence');
    if (['modified', 'moved'].includes(root.change) && (root.base === null || root.candidate === null)) fail('paired presence');
    if (root.base && root.base.id !== root.target || root.candidate && root.candidate.id !== root.target) fail('target identity');
  });
  return value;
}

function delta(value, root) {
  exact(value, ['schema', 'candidate_digest', 'target', 'base_project_revision', 'project_revision', 'base_workspace_revision', 'workspace_revision', 'base_image_digest', 'image_digest', 'presence', 'source_bindings', 'facets', 'target_artifacts', 'test_plan', 'evidence_class', 'comparison', 'omitted_equal_payloads', 'limits', 'nonclaims']);
  if (value.schema !== DELTA_SCHEMA) fail('delta schema');
  for (const key of ['candidate_digest', 'target', 'base_project_revision', 'project_revision', 'base_workspace_revision', 'workspace_revision', 'base_image_digest', 'image_digest', 'presence', 'evidence_class', 'comparison']) string(value[key], `delta ${key}`);
  if (value.target !== root.target) fail('delta target');
  if (!plain(value.source_bindings) || !plain(value.target_artifacts) || !plain(value.test_plan) || !plain(value.limits) || !Array.isArray(value.nonclaims) || typeof value.omitted_equal_payloads !== 'boolean' || !Array.isArray(value.facets) || value.facets.length > MAX_ROOTS) fail('delta envelope');
  const names = new Set();
  const facets = value.facets.map(facet => {
    const required = ['facet', 'change', 'exact_equal', 'projection_equal_without_provenance', 'base_digest', 'candidate_digest', 'base_bytes', 'candidate_bytes'];
    const optional = ['base', 'candidate'];
    if (!plain(facet) || required.some(key => !Object.hasOwn(facet, key)) || Object.keys(facet).some(key => !required.includes(key) && !optional.includes(key))) fail('facet fields');
    string(facet.facet, 'facet');
    if (names.has(facet.facet)) fail('duplicate facet');
    names.add(facet.facet);
    if (!['added', 'removed', 'modified', 'unchanged', 'provenance_only'].includes(facet.change) || typeof facet.exact_equal !== 'boolean' || typeof facet.projection_equal_without_provenance !== 'boolean') fail('facet status');
    digest(facet.base_digest, 'facet base digest'); digest(facet.candidate_digest, 'facet candidate digest');
    if (!Number.isSafeInteger(facet.base_bytes) || facet.base_bytes < 0 || !Number.isSafeInteger(facet.candidate_bytes) || facet.candidate_bytes < 0) fail('facet bytes');
    if (facet.exact_equal && (!facet.projection_equal_without_provenance || facet.change !== 'unchanged')) fail('facet exact equality');
    if (facet.change === 'provenance_only' && facet.projection_equal_without_provenance !== true) fail('facet provenance equality');
    if (facet.projection_equal_without_provenance && (Object.hasOwn(facet, 'base') || Object.hasOwn(facet, 'candidate'))) fail('equal facet payload');
    if (!facet.projection_equal_without_provenance && (!Object.hasOwn(facet, 'base') || !Object.hasOwn(facet, 'candidate'))) fail('changed facet payload');
    return Object.freeze({ facet: facet.facet, change: facet.change, exact_equal: facet.exact_equal, projection_equal_without_provenance: facet.projection_equal_without_provenance });
  });
  const evidence = Object.freeze({
    evidence_class: typeof value.evidence_class === 'string' ? value.evidence_class : 'not_reported_by_selected_delta',
    comparison: typeof value.comparison === 'string' ? value.comparison : 'not_reported_by_selected_delta',
    target_artifacts_change: plain(value.target_artifacts) && typeof value.target_artifacts.change === 'string' ? value.target_artifacts.change : 'not_reported_by_selected_delta'
  });
  return Object.freeze({ target: root.target, facets, evidence });
}

function identityStatus(root) {
  // The compact compiler catalogue binds the target identity but does not
  // carry an identity-origin/persistence field. Never infer persistence from
  // an implementation spelling such as an `auto:` prefix.
  return 'persistence_not_reported';
}

function facetStatus(facets) {
  if (!facets) return 'not_loaded';
  if (facets.every(facet => facet.exact_equal)) return 'exact_equal';
  if (facets.every(facet => facet.change === 'provenance_only')) return 'provenance_only';
  if (facets.every(facet => facet.projection_equal_without_provenance)) return 'projection_equal_without_provenance';
  return 'modified';
}

function changeRow(root, selectedDelta = null) {
  if (!root || !plain(root)) fail('root');
  const checked = catalog({ schema: CATALOG_SCHEMA, candidate_digest: 'candidate', base_project_revision: 'base', project_revision: 'candidate-project', roots: [root], selection_basis: 'fixture', source_changes: [], nonclaims: [] }).roots[0];
  const selected = selectedDelta === null ? null : delta(selectedDelta, checked);
  // `moved` is only a location fact until a target-level report was fetched.
  return Object.freeze({
    target: checked.target,
    declaration_status: checked.change,
    facet_status: facetStatus(selected && selected.facets),
    identity_status: identityStatus(checked),
    base: checked.base,
    candidate: checked.candidate,
    base_ghost: checked.change === 'removed',
    candidate_only: checked.change === 'added',
    facets: selected ? selected.facets : null,
    evidence_applicability: selected ? selected.evidence : Object.freeze({ status: 'not_loaded' }),
    evidence: selected ? 'semantic_delta' : 'semantic_delta_catalog',
    nonclaim: 'projection equality is not behavioral equivalence'
  });
}

function changeList(value) {
  const checked = catalog(value);
  return Object.freeze({
    candidate_revision: checked.candidate_digest,
    base_project_revision: checked.base_project_revision,
    candidate_project_revision: checked.project_revision,
    rows: Object.freeze(checked.roots.map(root => changeRow(root))),
    empty_state: checked.roots.length === 0 ? 'No changes in this admitted comparison' : null,
    nonclaim: 'an empty admitted comparison is not safe-to-merge evidence'
  });
}

async function inventory(host, selected, view) {
  const entry = selected.inventories.find(row => row.view === view);
  if (!entry) fail('missing inventory');
  const rows = [];
  let cursor = null;
  for (let pageCount = 0; pageCount < MAX_PAGES; pageCount += 1) {
    const response = explorer.page(await host.page({ summary: selected, view, handle: entry.handle, cursor, page_size: 128, max_bytes: 64 * 1024 }), selected);
    if (response.offset !== rows.length || response.cursor !== cursor) fail('nonsequential impact page');
    rows.push(...response.items);
    if (response.next_cursor === null) {
      if (rows.length !== entry.total_items) fail('short impact inventory');
      return Object.freeze(rows);
    }
    cursor = response.next_cursor;
  }
  fail('impact pagination bound');
}

async function loadImpact(host, target, side, query = {}) {
  if (!host || typeof host.summary !== 'function' || typeof host.page !== 'function') fail('closed host');
  string(target, 'impact target');
  if (!['base', 'candidate'].includes(side)) fail('impact side');
  const request = Object.freeze({ mode: 'impact', target, side, direction: 'both', depth: query.depth === undefined ? 1 : query.depth, max_nodes: query.max_nodes === undefined ? 256 : query.max_nodes, max_bytes: query.max_bytes === undefined ? 256 * 1024 : query.max_bytes });
  const selected = explorer.summary(await host.summary(request));
  if (selected.subject.kind !== 'candidate' || selected.subject.side !== side || selected.mode !== 'impact' || selected.target !== target) fail('foreign impact subject');
  const [declarations, relations, frontier] = await Promise.all(['declarations', 'relations', 'frontier'].map(view => inventory(host, selected, view)));
  return Object.freeze({ side, target, subject: selected.subject, coverage: selected.coverage, truncation: selected.truncation, declarations, relations, frontier });
}

function edgeKey(side, edge) { return JSON.stringify([side, edge.from, edge.to, edge.family, edge.direction, edge.site_id]); }
function unionImpact(base, candidate) {
  if (base !== null && (!base || base.side !== 'base')) fail('base impact');
  if (candidate !== null && (!candidate || candidate.side !== 'candidate')) fail('candidate impact');
  if (!base && !candidate) fail('missing impact');
  if (base && candidate && base.target !== candidate.target) fail('impact pair');
  const target = (base || candidate).target;
  const nodes = [];
  for (const result of [base, candidate].filter(Boolean)) for (const row of result.declarations) nodes.push(Object.freeze({ ...row, side: result.side }));
  const edges = [];
  const seen = new Set();
  for (const result of [base, candidate].filter(Boolean)) for (const row of result.relations) {
    const key = edgeKey(result.side, row);
    if (!seen.has(key)) { seen.add(key); edges.push(Object.freeze({ ...row, side: result.side })); }
  }
  edges.sort((left, right) => edgeKey(left.side, left).localeCompare(edgeKey(right.side, right)));
  const incomplete = [base, candidate].filter(Boolean).some(result => result.truncation.truncated || result.coverage.complete_within_query !== true);
  const witness_state = incomplete ? 'analysis_incomplete' : !candidate ? 'base_only' : !base ? 'candidate_only' : 'loaded';
  return Object.freeze({ target, base, candidate, nodes: Object.freeze(nodes), edges: Object.freeze(edges), witness_state });
}

async function loadChangeImpact(host, target, query = {}) {
  // Sequential loads preserve the snapshot host's active-view binding.
  const sides = query.sides === undefined ? ['base', 'candidate'] : query.sides;
  if (!Array.isArray(sides) || !sides.length || sides.some(side => !['base', 'candidate'].includes(side)) || new Set(sides).size !== sides.length) fail('impact sides');
  const base = sides.includes('base') ? await loadImpact(host, target, 'base', query) : null;
  const candidate = sides.includes('candidate') ? await loadImpact(host, target, 'candidate', query) : null;
  return unionImpact(base, candidate);
}

function directed(edge) {
  if (edge.direction === 'reverse') return [[edge.to, edge.from]];
  if (edge.direction === 'both') return [[edge.from, edge.to], [edge.to, edge.from]];
  return [[edge.from, edge.to]];
}

function whyAffected(impact, side, nodeKey) {
  if (!impact || !['base', 'candidate'].includes(side)) fail('witness request');
  string(nodeKey, 'witness node');
  const selected = impact[side];
  if (!selected) return Object.freeze({ state: 'witness_not_loaded_or_analysis_incomplete', edges: [] });
  const root = selected.declarations.find(row => row.id === impact.target);
  if (!root) return Object.freeze({ state: 'witness_not_loaded_or_analysis_incomplete', edges: [] });
  const adjacency = new Map();
  for (const edge of impact.edges.filter(edge => edge.side === side)) for (const [from, to] of directed(edge)) {
    const rows = adjacency.get(from) || []; rows.push({ to, edge }); adjacency.set(from, rows);
  }
  for (const rows of adjacency.values()) rows.sort((left, right) => edgeKey(side, left.edge).localeCompare(edgeKey(side, right.edge)));
  const queue = [{ node: root.node_key, path: [] }], visited = new Set([root.node_key]);
  while (queue.length) {
    const current = queue.shift();
    if (current.node === nodeKey) return Object.freeze({ state: 'loaded_structural_witness', edges: Object.freeze(current.path) });
    for (const next of adjacency.get(current.node) || []) if (!visited.has(next.to)) { visited.add(next.to); queue.push({ node: next.to, path: [...current.path, next.edge] }); }
  }
  return Object.freeze({ state: impact.witness_state === 'analysis_incomplete' ? 'witness_not_loaded_or_analysis_incomplete' : 'no_returned_structural_witness', edges: [] });
}

const api = { CATALOG_SCHEMA, DELTA_SCHEMA, SOURCE_REVIEW_SCHEMA, catalog, delta, sourceReview, changeRow, changeList, loadImpact, unionImpact, loadChangeImpact, whyAffected };
if (typeof module !== 'undefined' && module.exports) module.exports = api;
else globalThis.SemapraxExplorerChanges = api;
