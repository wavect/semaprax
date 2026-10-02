'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const changes = require('../changes.js');
const model = require('../model.js');

const hex = value => `sha256:${value.repeat(64)}`;
const digest = hex('a');
const compact = (id, name, path, fragment = 'f') => ({ id, name, kind: 'function', path, module: path.split('/')[0], fragment_digest: `${fragment}${'0'.repeat(63)}` });
function root(target, change, base, candidate) { return { target, change, base, candidate }; }
function catalog(roots) { return { schema: changes.CATALOG_SCHEMA, candidate_digest: 'candidate-1', base_project_revision: 'base-1', project_revision: 'candidate-1', roots, selection_basis: 'authored_declaration_identity_origin_and_canonical_fragment_changes', source_changes: [], nonclaims: [] }; }
function delta(target, facets) { return { schema: changes.DELTA_SCHEMA, candidate_digest: 'candidate-1', target, base_project_revision: 'base-1', project_revision: 'candidate-1', base_workspace_revision: 'base-workspace-1', workspace_revision: 'candidate-workspace-1', base_image_digest: digest, image_digest: digest, presence: 'modified', source_bindings: {}, facets, target_artifacts: {}, test_plan: {}, evidence_class: 'descriptive_recomputable_compiler_projection', comparison: 'exact_values_plus_separate_provenance_insensitive_projection_equality', omitted_equal_payloads: true, limits: {}, nonclaims: [] }; }
function facet(name, change, exact, projection) { const result = { facet: name, change, exact_equal: exact, projection_equal_without_provenance: projection, base_digest: digest, candidate_digest: digest, base_bytes: 0, candidate_bytes: 0 }; if (!projection) { result.base = null; result.candidate = null; } return result; }
function sourceReview(candidate = 'candidate-1', base = 'base-1') { return { schema: changes.SOURCE_REVIEW_SCHEMA, base_project_revision: base, candidate_project_revision: 'candidate-project-1', candidate_revision: candidate, source_authority: false, files: [{ path: 'm/main.spx', base_source: 'fn old() {}\n', candidate_source: 'fn new() {}\n', base_digest: digest, candidate_digest: digest, source_diff: '--- m/main.spx\n+++ m/main.spx\n', source_diff_digest: digest }], report_revision: digest }; }

test('catalog rows preserve move plus modification, ghosts, additions, and identity caution', () => {
  const moved = root('rename', 'moved', compact('rename', 'old_name', 'old/a.spx'), compact('rename', 'new_name', 'new/b.spx', 'g'));
  const removed = root('removed', 'removed', compact('removed', 'gone', 'old/a.spx'), null);
  const added = root('added', 'added', null, compact('added', 'new', 'new/a.spx'));
  const automatic = root('auto:temporary', 'modified', compact('auto:temporary', 'x', 'a.spx'), compact('auto:temporary', 'x', 'a.spx', 'g'));
  const rows = changes.changeList(catalog([moved, removed, added, automatic])).rows;
  assert.equal(rows[0].declaration_status, 'moved'); assert.equal(rows[0].facet_status, 'not_loaded');
  assert.equal(rows[1].base_ghost, true); assert.equal(rows[2].candidate_only, true);
  assert.equal(rows[3].identity_status, 'persistence_not_reported');
  const detailed = changes.changeRow(moved, delta('rename', [facet('authored_declaration', 'modified', false, false)]));
  assert.equal(detailed.declaration_status, 'moved'); assert.equal(detailed.facet_status, 'modified');
});

test('facet equality labels remain distinct from behavioral claims and empty is qualified', () => {
  const moved = root('rename', 'moved', compact('rename', 'same', 'a.spx'), compact('rename', 'same', 'b.spx'));
  assert.equal(changes.changeRow(moved, delta('rename', [facet('decl', 'unchanged', true, true)])).facet_status, 'exact_equal');
  assert.equal(changes.changeRow(moved, delta('rename', [facet('decl', 'provenance_only', false, true)])).facet_status, 'provenance_only');
  const empty = changes.changeList(catalog([]));
  assert.equal(empty.empty_state, 'No changes in this admitted comparison');
  assert.match(empty.nonclaim, /not safe-to-merge/);
});

test('same-name declarations retain separate compiler identities and selected signature, contract, and effect facets', () => {
  const removed = root('old-id', 'removed', compact('old-id', 'same_name', 'old/a.spx'), null);
  const added = root('new-id', 'added', null, compact('new-id', 'same_name', 'new/a.spx'));
  const rows = changes.changeList(catalog([removed, added])).rows;
  assert.deepEqual(rows.map(row => row.target), ['old-id', 'new-id']);
  const modified = root('stable-id', 'modified', compact('stable-id', 'f', 'a.spx'), compact('stable-id', 'f', 'a.spx', 'g'));
  const selected = changes.changeRow(modified, delta('stable-id', [
    facet('typed_declaration', 'modified', false, false),
    facet('contracts', 'modified', false, false),
    facet('effect_requirements', 'modified', false, false)
  ]));
  assert.equal(selected.facet_status, 'modified');
  assert.deepEqual(selected.facets.map(row => row.facet), ['typed_declaration', 'contracts', 'effect_requirements']);
});

function summary(side, target, relationCount = 1, truncated = false) {
  return { schema: model.SCHEMA, kind: 'summary', subject: { kind: 'candidate', image_revision: 'image', project_revision: `${side}-project`, workspace_revision: `${side}-workspace`, project_graph_digest: digest, candidate_revision: 'candidate-1', side }, mode: 'impact', target, query: { direction: 'both', depth: 1, max_nodes: 256, max_bytes: 262144 }, artifact_digest: digest, truncation: { truncated, reason: truncated ? 'bound' : null }, coverage: { owner: 'workspace_analysis', mode: 'impact', complete_within_query: !truncated }, inventories: [{ view: 'modules', total_items: 0, handle: digest }, { view: 'declarations', total_items: 2, handle: digest }, { view: 'relations', total_items: relationCount, handle: digest }, { view: 'frontier', total_items: 0, handle: digest }], source_authority: false, execution: false, publication_authority: false, nonclaims: [] };
}
function page(selected, view, items) { return { schema: model.SCHEMA, kind: 'page', subject: selected.subject, mode: selected.mode, target: selected.target, query: selected.query, artifact_digest: selected.artifact_digest, truncation: selected.truncation, coverage: selected.coverage, view, handle: digest, cursor: null, offset: 0, total_items: selected.inventories.find(row => row.view === view).total_items, page_size: 128, max_bytes: 65536, next_cursor: null, items, source_authority: false, execution: false, publication_authority: false, nonclaims: [] }; }
function declaration(side, id) { return { node_key: `${side}:${id}`, id, identity_origin: 'explicit', kind: 'function', display_name: id, owner_id: null, module: 'm', path: 'm/a.spx', source_reference: { path: 'm/a.spx', source_revision: 'r', source_digest: 'd' } }; }
function edge(side, from, to, direction = 'reverse') { return { family: 'call', from: `${side}:${from}`, to: `${side}:${to}`, direction, site_id: `${side}:${from}:${to}`, provenance: { retained: true } }; }

test('base and candidate impact are loaded separately, unioned by side, and witness uses retained edges', async () => {
  const reports = { base: summary('base', 'root'), candidate: summary('candidate', 'root') };
  const host = { async summary(query) { return reports[query.side]; }, async page(request) { const side = request.summary.subject.side; const items = request.view === 'declarations' ? [declaration(side, 'root'), declaration(side, 'caller')] : request.view === 'relations' ? [edge(side, 'caller', 'root')] : []; return page(request.summary, request.view, items); } };
  const impact = await changes.loadChangeImpact(host, 'root');
  assert.equal(impact.nodes.length, 4); assert.equal(impact.edges.length, 2);
  assert.ok(impact.nodes.some(row => row.side === 'base' && row.node_key === 'base:caller'));
  const witness = changes.whyAffected(impact, 'base', 'base:caller');
  assert.equal(witness.state, 'loaded_structural_witness'); assert.equal(witness.edges.length, 1); assert.equal(witness.edges[0].side, 'base');
});

test('missing or truncated witness stays explicitly qualified', () => {
  const base = { side: 'base', target: 'root', declarations: [declaration('base', 'root')], relations: [], truncation: { truncated: true, reason: 'bound' }, coverage: { complete_within_query: false } };
  const candidate = { side: 'candidate', target: 'root', declarations: [declaration('candidate', 'root')], relations: [], truncation: { truncated: false, reason: null }, coverage: { complete_within_query: true } };
  const impact = changes.unionImpact(base, candidate);
  assert.equal(changes.whyAffected(impact, 'base', 'base:outside').state, 'witness_not_loaded_or_analysis_incomplete');
});

test('removed and added roots retain their one-sided structural inventory state', async () => {
  const reports = { base: summary('base', 'removed'), candidate: summary('candidate', 'added') };
  const host = { async summary(query) { return reports[query.side]; }, async page(request) { const side = request.summary.subject.side; const items = request.view === 'declarations' ? [declaration(side, request.summary.target), declaration(side, 'caller')] : request.view === 'relations' ? [edge(side, 'caller', request.summary.target)] : []; return page(request.summary, request.view, items); } };
  const removed = await changes.loadChangeImpact(host, 'removed', { sides: ['base'] });
  const added = await changes.loadChangeImpact(host, 'added', { sides: ['candidate'] });
  assert.equal(removed.witness_state, 'base_only'); assert.equal(removed.candidate, null);
  assert.equal(added.witness_state, 'candidate_only'); assert.equal(added.base, null);
  assert.equal(changes.whyAffected(removed, 'candidate', 'candidate:removed').state, 'witness_not_loaded_or_analysis_incomplete');
});

test('source review accepts only the exact bound compiler report and never derives a diff', () => {
  const review = changes.sourceReview(sourceReview(), 'candidate-1', 'base-1', 'candidate-project-1');
  assert.equal(review.files[0].source_diff, '--- m/main.spx\n+++ m/main.spx\n');
  assert.throws(() => changes.sourceReview(sourceReview('other'), 'candidate-1', 'base-1', 'candidate-project-1'), /source review binding/);
  assert.throws(() => changes.sourceReview(sourceReview(), 'candidate-1', 'base-1', 'other-project'), /source review binding/);
  const forged = sourceReview(); forged.files[0].path = '../outside.spx';
  assert.throws(() => changes.sourceReview(forged, 'candidate-1', 'base-1', 'candidate-project-1'), /source path/);
  const untrusted = sourceReview(); untrusted.source_authority = true;
  assert.throws(() => changes.sourceReview(untrusted, 'candidate-1', 'base-1', 'candidate-project-1'), /source review schema/);
});
