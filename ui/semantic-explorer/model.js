// Host-neutral, source-free projection of compiler-owned explorer pages.
'use strict';

const SCHEMA = 'semaprax.explorer-view.v1';
const VIEWS = Object.freeze(['modules', 'declarations', 'relations', 'frontier']);
const FAMILIES = Object.freeze(['function_import', 'type_import', 'call', 'type_reference', 'effect_requirement', 'capability_authority']);
const MODES = Object.freeze(['overview', 'context', 'impact']);
const DIRECTIONS = Object.freeze(['forward', 'reverse', 'both']);
const MAX_SUMMARY_BYTES = 64 * 1024;
const MAX_PAGE_BYTES = 512 * 1024;
const MAX_TEXT_BYTES = 4096;

function fail(reason) { throw new TypeError(`explorer ${reason}`); }
function plain(value) { return value !== null && typeof value === 'object' && !Array.isArray(value) && (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null); }
function keys(value, required, optional = []) {
  if (!plain(value)) fail('object required');
  const actual = Object.keys(value);
  if (required.some(key => !Object.hasOwn(value, key)) || actual.some(key => !required.includes(key) && !optional.includes(key))) fail('unexpected fields');
}
function bytes(value) { return new TextEncoder().encode(value).length; }
function stable(value, depth = 0) {
  if (depth > 32) fail('nested data');
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return JSON.stringify(value);
  if (typeof value === 'number') { if (!Number.isSafeInteger(value)) fail('unsafe number'); return JSON.stringify(value); }
  if (Array.isArray(value)) return `[${value.map(item => stable(item, depth + 1)).join(',')}]`;
  if (!plain(value)) fail('invalid data');
  return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${stable(value[key], depth + 1)}`).join(',')}}`;
}
function string(value, allowEmpty = false) {
  if (typeof value !== 'string' || (!allowEmpty && !value) || bytes(value) > MAX_TEXT_BYTES) fail('invalid string');
  return value;
}
function integer(value, minimum = 0, maximum = Number.MAX_SAFE_INTEGER) {
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) fail('invalid count');
  return value;
}
function array(value, maximum) { if (!Array.isArray(value) || value.length > maximum) fail('invalid inventory'); return value; }
function choice(value, choices) { if (!choices.includes(value)) fail('unsupported value'); return value; }
function digest(value) { if (typeof value !== 'string' || !/^sha256:[0-9a-f]{64}$/.test(value)) fail('invalid digest'); return value; }

function subject(value) {
  keys(value, ['kind', 'image_revision', 'project_revision', 'workspace_revision', 'project_graph_digest', 'candidate_revision', 'side']);
  choice(value.kind, ['image', 'candidate']);
  for (const name of ['image_revision', 'project_revision', 'workspace_revision', 'project_graph_digest']) string(value[name]);
  if (value.kind === 'image') {
    if (value.side !== 'current' || value.candidate_revision !== null) fail('image side');
  } else {
    choice(value.side, ['base', 'candidate']); string(value.candidate_revision);
  }
  return value;
}

function coverage(value) {
  keys(value, ['owner'], ['mode', 'complete_within_retained_graph', 'complete_within_query']);
  choice(value.owner, ['workspace_graph', 'workspace_analysis']);
  if (value.owner === 'workspace_graph' && value.complete_within_retained_graph !== true) fail('graph coverage');
  if (value.owner === 'workspace_analysis') {
    choice(value.mode, ['context', 'impact']);
    if (value.complete_within_query !== null && typeof value.complete_within_query !== 'boolean') fail('analysis coverage');
  }
  return value;
}

function truncation(value) {
  if (!plain(value) || typeof value.truncated !== 'boolean' || bytes(stable(value)) > MAX_TEXT_BYTES) fail('truncation');
  return value;
}

function summary(value) {
  if (bytes(JSON.stringify(value)) > MAX_SUMMARY_BYTES) fail('summary too large');
  keys(value, ['schema', 'kind', 'subject', 'mode', 'target', 'query', 'artifact_digest', 'truncation', 'coverage', 'inventories', 'source_authority', 'execution', 'publication_authority', 'nonclaims']);
  if (value.schema !== SCHEMA || value.kind !== 'summary') fail('schema mismatch');
  subject(value.subject); choice(value.mode, MODES);
  if (value.mode === 'overview') {
    if (value.target !== null) fail('overview target');
  } else {
    string(value.target);
  }
  keys(value.query, ['direction', 'depth', 'max_nodes', 'max_bytes']);
  choice(value.query.direction, DIRECTIONS);
  integer(value.query.depth, 0, 3); integer(value.query.max_nodes, 1, 256); integer(value.query.max_bytes, 1024, 256 * 1024);
  digest(value.artifact_digest); coverage(value.coverage); truncation(value.truncation);
  if (value.source_authority !== false || value.execution !== false || value.publication_authority !== false) fail('authority claim');
  array(value.nonclaims, 64).forEach(item => string(item));
  const seen = new Set();
  array(value.inventories, 4).forEach(row => {
    keys(row, ['view', 'total_items', 'handle']); choice(row.view, VIEWS);
    if (seen.has(row.view)) fail('duplicate inventory'); seen.add(row.view);
    integer(row.total_items); digest(row.handle);
  });
  if (seen.size !== 4) fail('missing inventory');
  return value;
}

function item(view, value) {
  if (view === 'modules') {
    keys(value, ['module', 'path', 'declaration_count', 'relation_count', 'source_reference']);
    string(value.module); string(value.path, true); integer(value.declaration_count); integer(value.relation_count);
    sourceReference(value.source_reference);
  } else if (view === 'declarations') {
    keys(value, ['node_key', 'id', 'identity_origin', 'kind', 'display_name', 'owner_id', 'module', 'path', 'source_reference']);
    for (const name of ['node_key', 'id', 'kind', 'display_name']) string(value[name]);
    for (const name of ['identity_origin', 'owner_id', 'module', 'path']) if (value[name] !== null) string(value[name]);
    sourceReference(value.source_reference);
  } else if (view === 'relations') {
    keys(value, ['family', 'from', 'to', 'direction', 'site_id', 'provenance']);
    choice(value.family, FAMILIES); choice(value.direction, DIRECTIONS);
    for (const name of ['from', 'to', 'site_id']) string(value[name]);
    if (!plain(value.provenance) || bytes(JSON.stringify(value.provenance)) > MAX_TEXT_BYTES) fail('invalid provenance');
  } else {
    if (!plain(value) || bytes(JSON.stringify(value)) > MAX_TEXT_BYTES) fail('invalid frontier');
  }
  return value;
}

function sourceReference(value) {
  if (value === null) fail('missing source status');
  if (Object.hasOwn(value, 'kind')) {
    keys(value, ['kind']);
    choice(value.kind, ['non_file_node', 'authenticated_source_reference_unavailable_in_analysis_projection']);
    return;
  }
  keys(value, ['path', 'source_revision', 'source_digest']);
  string(value.path); string(value.source_revision); string(value.source_digest);
}

function page(value, selected) {
  summary(selected);
  if (bytes(JSON.stringify(value)) > MAX_PAGE_BYTES) fail('page too large');
  keys(value, ['schema', 'kind', 'subject', 'mode', 'target', 'query', 'artifact_digest', 'truncation', 'coverage', 'view', 'handle', 'cursor', 'offset', 'total_items', 'page_size', 'max_bytes', 'next_cursor', 'items', 'source_authority', 'execution', 'publication_authority', 'nonclaims']);
  if (value.schema !== SCHEMA || value.kind !== 'page' || stable(value.subject) !== stable(selected.subject) || value.mode !== selected.mode || value.target !== selected.target || stable(value.query) !== stable(selected.query) || value.artifact_digest !== selected.artifact_digest || stable(value.coverage) !== stable(selected.coverage) || stable(value.truncation) !== stable(selected.truncation)) fail('foreign page');
  if (value.source_authority !== false || value.execution !== false || value.publication_authority !== false || stable(value.nonclaims) !== stable(selected.nonclaims)) fail('page authority drift');
  const inventory = selected.inventories.find(row => row.view === value.view);
  if (!inventory || value.handle !== inventory.handle || value.total_items !== inventory.total_items) fail('foreign inventory');
  if (value.cursor !== null) string(value.cursor);
  if (value.next_cursor !== null) string(value.next_cursor);
  integer(value.offset, 0, inventory.total_items); integer(value.page_size, 1, 128); integer(value.max_bytes, 1024, MAX_PAGE_BYTES);
  if (value.items.length > value.page_size || value.offset + value.items.length > inventory.total_items) fail('invalid page range');
  array(value.items, 128).forEach(row => item(value.view, row));
  if ((value.offset + value.items.length < inventory.total_items) !== (value.next_cursor !== null)) fail('invalid continuation');
  return value;
}

function index(rows, field) {
  const result = new Map();
  for (const row of rows) {
    if (result.has(row[field])) fail('duplicate identity');
    result.set(row[field], row);
  }
  return result;
}

function search(rows, query) {
  const needle = string(query, true).trim().toLocaleLowerCase();
  if (!needle) return rows;
  return rows.filter(row => [row.id, row.display_name, row.path].some(value => typeof value === 'string' && value.toLocaleLowerCase().includes(needle)));
}

function groupRelations(rows) {
  const groups = new Map();
  for (const row of rows) {
    const key = JSON.stringify([row.from, row.to, row.family]);
    const group = groups.get(key) || { from: row.from, to: row.to, family: row.family, sites: [] };
    group.sites.push(row); groups.set(key, group);
  }
  return [...groups.values()].sort((a, b) => JSON.stringify([a.from, a.to, a.family]).localeCompare(JSON.stringify([b.from, b.to, b.family])));
}

const semapraxExplorerModelApi = { SCHEMA, VIEWS, FAMILIES, summary, page, index, search, groupRelations };
if (typeof module !== 'undefined' && module.exports) module.exports = semapraxExplorerModelApi;
else globalThis.SemapraxExplorerModel = semapraxExplorerModelApi;
