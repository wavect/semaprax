'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const model = require('../model.js');
const changes = require('../changes.js');
const evidence = require('../evidence.js');
const { createExplorer } = require('../view.js');
const { ExplorerCache } = require('../cache.js');

const digest = `sha256:${'a'.repeat(64)}`;
function canonical(value) { if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`; if (value && typeof value === 'object') return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`; return JSON.stringify(value); }
function hash(domain, value) { const bytes = Buffer.from(value), length = Buffer.alloc(8); length.writeBigUInt64LE(BigInt(bytes.length)); return `sha256:${crypto.createHash('sha256').update(domain).update(length).update(bytes).digest('hex')}`; }
class Node {
  constructor(ownerDocument, tag) { this.ownerDocument = ownerDocument; this.tag = tag; this.children = []; this.listeners = new Map(); this.dataset = {}; this.style = {}; this.hidden = false; this.textContent = ''; }
  append(...nodes) { for (const node of nodes) { this.children.push(node); node.parentNode = this; } }
  removeChild(node) { this.children.splice(this.children.indexOf(node), 1); }
  get firstChild() { return this.children[0] || null; }
  setAttribute() {}
  addEventListener(name, callback) { this.listeners.set(name, callback); }
  remove() { if (this.parentNode) this.parentNode.removeChild(this); }
  setPointerCapture() {}
}
class Document {
  createElement(tag) { return new Node(this, tag); }
  createElementNS(_namespace, tag) { return new Node(this, tag); }
  createTextNode(text) { const node = new Node(this, '#text'); node.textContent = text; return node; }
}
function find(node, text) {
  if (node.textContent === text) return node;
  for (const child of node.children) { const found = find(child, text); if (found) return found; }
  return null;
}
function tick() { return new Promise(resolve => setImmediate(resolve)); }
function subject() { return { kind: 'candidate', image_revision: 'image-1', project_revision: 'candidate-project', workspace_revision: 'candidate-workspace', project_graph_digest: digest, candidate_revision: 'candidate-1', side: 'candidate' }; }
function summary() { return { schema: model.SCHEMA, kind: 'summary', subject: subject(), mode: 'overview', target: null, query: { direction: 'both', depth: 1, max_nodes: 256, max_bytes: 262144 }, artifact_digest: digest, truncation: { truncated: false, reason: null }, coverage: { owner: 'workspace_graph', complete_within_retained_graph: true }, inventories: model.VIEWS.map(view => ({ view, total_items: 0, handle: digest })), source_authority: false, execution: false, publication_authority: false, nonclaims: [] }; }
function page(selected, view) { return { schema: model.SCHEMA, kind: 'page', subject: selected.subject, mode: selected.mode, target: selected.target, query: selected.query, artifact_digest: digest, truncation: selected.truncation, coverage: selected.coverage, view, handle: digest, cursor: null, offset: 0, total_items: 0, page_size: 32, max_bytes: 65536, next_cursor: null, items: [], source_authority: false, execution: false, publication_authority: false, nonclaims: [] }; }
function compact(name) { return { id: 'stable-id', name, kind: 'function', path: 'm/new.spx', module: 'm', fragment_digest: 'b'.repeat(64) }; }
function catalog() { return { schema: changes.CATALOG_SCHEMA, candidate_digest: 'candidate-1', base_project_revision: 'base-project', project_revision: 'candidate-project', roots: [{ target: 'stable-id', change: 'moved', base: compact('old_name'), candidate: compact('new_name') }], selection_basis: 'fixture', source_changes: [], nonclaims: [] }; }
function delta() { return { schema: changes.DELTA_SCHEMA, candidate_digest: 'candidate-1', target: 'stable-id', base_project_revision: 'base-project', project_revision: 'candidate-project', base_workspace_revision: 'base-workspace', workspace_revision: 'candidate-workspace', base_image_digest: digest, image_digest: digest, presence: 'modified', source_bindings: {}, facets: [], target_artifacts: {}, test_plan: {}, evidence_class: 'descriptive_recomputable_compiler_projection', comparison: 'exact_values_plus_separate_provenance_insensitive_projection_equality', omitted_equal_payloads: true, limits: {}, nonclaims: [] }; }
function impactSummary(side, target) { return { schema: model.SCHEMA, kind: 'summary', subject: { ...subject(), side, project_revision: `${side}-project`, workspace_revision: `${side}-workspace` }, mode: 'impact', target, query: { direction: 'both', depth: 1, max_nodes: 256, max_bytes: 262144 }, artifact_digest: digest, truncation: { truncated: false, reason: null }, coverage: { owner: 'workspace_analysis', mode: 'impact', complete_within_query: true }, inventories: [{ view: 'modules', total_items: 0, handle: digest }, { view: 'declarations', total_items: 2, handle: digest }, { view: 'relations', total_items: 1, handle: digest }, { view: 'frontier', total_items: 0, handle: digest }], source_authority: false, execution: false, publication_authority: false, nonclaims: [] }; }
function impactPage(selected, view) { const side = selected.subject.side; const declaration = id => ({ node_key: `${side}:${id}`, id, identity_origin: 'explicit', kind: 'function', display_name: id, owner_id: null, module: 'm', path: 'm/a.spx', source_reference: { kind: 'authenticated_source_reference_unavailable_in_analysis_projection' } }); const relation = { family: 'call', from: `${side}:caller`, to: `${side}:stable-id`, direction: 'reverse', site_id: `${side}:caller:stable-id`, provenance: { retained: true } }; const items = view === 'declarations' ? [declaration('stable-id'), declaration('caller')] : view === 'relations' ? [relation] : []; return { schema: model.SCHEMA, kind: 'page', subject: selected.subject, mode: selected.mode, target: selected.target, query: selected.query, artifact_digest: digest, truncation: selected.truncation, coverage: selected.coverage, view, handle: digest, cursor: null, offset: 0, total_items: items.length, page_size: 32, max_bytes: 65536, next_cursor: null, items, source_authority: false, execution: false, publication_authority: false, nonclaims: [] }; }
function sourceReview() {
  const file = { path: 'm/new.spx', base_source: 'fn old_name() {}\n', candidate_source: 'fn new_name() {}\n', source_diff: '--- m/new.spx\n+++ m/new.spx\n' };
  file.base_digest = hash('semaprax.semantic-review.source-digest.v1\0', file.base_source);
  file.candidate_digest = hash('semaprax.semantic-review.source-digest.v1\0', file.candidate_source);
  file.source_diff_digest = hash('semaprax.candidate.source-diff.v1\0', file.source_diff);
  const review = { schema: changes.SOURCE_REVIEW_SCHEMA, base_project_revision: 'base-project', candidate_project_revision: 'candidate-project', candidate_revision: 'candidate-1', source_authority: false, files: [file] };
  return { ...review, report_revision: hash(`${changes.SOURCE_REVIEW_SCHEMA}\0`, `${canonical(review)}\n`) };
}

test('overview opens with only bounded module and declaration pages', async () => {
  const calls = [];
  const host = {
    async summary() { calls.push('summary'); return summary(); },
    async page(request) { calls.push(`page:${request.view}`); return page(request.summary, request.view); }
  };
  const document = new Document(); const root = new Node(document, 'root');
  createExplorer(root, host, { side: 'candidate' });
  for (let index = 0; index < 12; index += 1) await tick();
  assert.deepEqual(calls, ['summary', 'page:modules', 'page:declarations']);
});

test('repeated open and destroy releases host state while retaining only bounded immutable pages', async () => {
  const cache = new ExplorerCache({ maxEntries: 2, maxBytes: 4096, maxEntryBytes: 2048 });
  let disposed = 0, pages = 0;
  for (let cycle = 0; cycle < 3; cycle++) {
    const host = {
      async summary() { return summary(); },
      async page(request) { pages++; return page(request.summary, request.view); },
      dispose() { disposed++; }
    };
    const document = new Document(); const root = new Node(document, 'root');
    const explorer = createExplorer(root, host, { side: 'candidate', cache });
    for (let index = 0; index < 12; index += 1) await tick();
    explorer.destroy();
    assert.equal(root.children.length, 0, `cycle ${cycle} must detach its DOM/listeners`);
  }
  assert.equal(disposed, 3);
  assert.equal(pages, 2, 'later openings reuse the exact immutable first pages');
  assert.equal(cache.stats().entries, 2);
  assert.ok(cache.stats().retainedBytes <= 4096);
});

test('candidate catalog, target delta, and selected evidence remain separate lazy reads', async () => {
  const calls = [];
  const host = {
    async summary() { calls.push('summary'); return summary(); },
    async page(request) { calls.push(`page:${request.view}`); return page(request.summary, request.view); },
    async deltaCatalog(revision) { calls.push(`catalog:${revision}`); return catalog(); },
    async semanticDelta(revision, target) { calls.push(`delta:${revision}:${target}`); return delta(); },
    async readEvidence(request) { calls.push(`evidence:${request.method}`); return { schema: evidence.SCHEMA, subject: request.subject, method: request.method, target: request.target, facet: request.facet, state: 'available', compact: { stable_id: request.target, count: 1 }, omitted: [], nonclaims: ['tests not run'], source_authority: false, execution: false }; }
  };
  const document = new Document(); const root = new Node(document, 'root');
  createExplorer(root, host, { side: 'candidate' });
  for (let index = 0; index < 16; index += 1) await tick();
  assert.ok(calls.includes('catalog:candidate-1'));
  assert.ok(!calls.some(call => call.startsWith('delta:') || call.startsWith('evidence:')));
  find(root, 'new_name').parentNode.listeners.get('click')();
  await tick(); await tick();
  assert.ok(calls.includes('delta:candidate-1:stable-id'));
  assert.ok(!calls.some(call => call.startsWith('evidence:')));
  find(root, 'Declaration').listeners.get('click')();
  await tick(); await tick();
  assert.deepEqual(calls.filter(call => call.startsWith('evidence:')), ['evidence:candidate/function-summary']);
});

test('change navigation keeps the selected identity and renders the separate impact union witness', async () => {
  const calls = [];
  const host = {
    async summary(query) { calls.push(`summary:${query.mode}:${query.side}`); return query.mode === 'impact' ? impactSummary(query.side, query.target) : summary(); },
    async page(request) { return request.summary.mode === 'impact' ? impactPage(request.summary, request.view) : page(request.summary, request.view); },
    async deltaCatalog() { return catalog(); },
    async semanticDelta() { return delta(); },
    async sourceReview() { return sourceReview(); },
    async readEvidence(request) { return { schema: evidence.SCHEMA, subject: request.subject, method: request.method, target: request.target, facet: request.facet, state: 'available', compact: {}, omitted: [], nonclaims: [], source_authority: false, execution: false }; }
  };
  const document = new Document(); const root = new Node(document, 'root');
  createExplorer(root, host, { side: 'candidate' });
  for (let index = 0; index < 16; index += 1) await tick();
  find(root, 'new_name').parentNode.listeners.get('click')();
  for (let index = 0; index < 32; index += 1) await tick();
  assert.ok(calls.includes('summary:impact:base')); assert.ok(calls.includes('summary:impact:candidate'));
  assert.ok(find(root, 'Potential structural impact'));
  assert.ok(find(root, 'Compiler-provided source diff:'));
  assert.ok(find(root, 'fn old_name() {}\n'));
  find(root, 'caller').parentNode.children[2].listeners.get('click')();
  assert.ok(find(root, 'Returned structural witness path:'));
  find(root, 'Base').listeners.get('click')();
  assert.ok(find(root, 'old_name'));
  find(root, 'Changes').listeners.get('click')();
  assert.ok(find(root, 'Why affected?'));
});

test('a removed compiler root keeps its base-only ghost and never asks the absent candidate side for impact', async () => {
  const calls = [];
  const removed = {
    schema: changes.CATALOG_SCHEMA,
    candidate_digest: 'candidate-1',
    base_project_revision: 'base-project',
    project_revision: 'candidate-project',
    roots: [{
      target: 'calculator.explorer-unused',
      change: 'removed',
      base: { id: 'calculator.explorer-unused', name: 'explorer_unused', kind: 'function', path: 'm/old.spx', module: 'm', fragment_digest: 'c'.repeat(64) },
      candidate: null
    }],
    selection_basis: 'authored_declaration_identity_origin_and_canonical_fragment_changes',
    source_changes: [],
    nonclaims: ['not_complete_dynamic_impact']
  };
  const host = {
    async summary(query) {
      calls.push(`summary:${query.mode}:${query.side}:${query.target || ''}`);
      if (query.mode === 'impact' && query.side === 'candidate') throw new Error('removed target is absent from candidate');
      return query.mode === 'impact' ? impactSummary(query.side, query.target) : summary();
    },
    async page(request) { return request.summary.mode === 'impact' ? impactPage(request.summary, request.view) : page(request.summary, request.view); },
    async deltaCatalog() { return removed; },
    async semanticDelta() { return { ...delta(), target: 'calculator.explorer-unused', presence: 'removed' }; },
    async readEvidence(request) { return { schema: evidence.SCHEMA, subject: request.subject, method: request.method, target: request.target, facet: request.facet, state: 'not_requested', compact: {}, omitted: [], nonclaims: [], source_authority: false, execution: false }; }
  };
  const document = new Document(); const root = new Node(document, 'root');
  createExplorer(root, host, { side: 'candidate' });
  await tick(); await tick(); await tick();
  find(root, 'explorer_unused').parentNode.listeners.get('click')();
  for (let index = 0; index < 16; index += 1) await tick();
  assert.ok(calls.includes('summary:impact:base:calculator.explorer-unused'));
  assert.ok(!calls.some(call => call.startsWith('summary:impact:candidate:')));
  assert.ok(find(root, 'base-only ghost'));
  assert.ok(find(root, 'Base-only structural impact for this removed declaration.'));
});

test('unbundled source review stays explicitly unavailable', async () => {
  const host = { async summary() { return summary(); }, async page(request) { return page(request.summary, request.view); }, async deltaCatalog() { return catalog(); }, async semanticDelta() { return delta(); }, async readEvidence(request) { return { schema: evidence.SCHEMA, subject: request.subject, method: request.method, target: request.target, facet: request.facet, state: 'available', compact: {}, omitted: [], nonclaims: [], source_authority: false, execution: false }; } };
  const document = new Document(); const root = new Node(document, 'root');
  createExplorer(root, host, { side: 'candidate' });
  await tick(); await tick(); await tick();
  find(root, 'new_name').parentNode.listeners.get('click')();
  await tick(); await tick();
  assert.ok(find(root, 'Source diff not bundled.'));
});
