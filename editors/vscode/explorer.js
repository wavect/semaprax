'use strict';
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { parse } = require('./protocol');

const VIEWS = new Set(['modules', 'declarations', 'relations', 'frontier']);
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const MAX_REPORT_BYTES = 8 * 1024 * 1024;
const EVIDENCE_METHODS = new Set(['image/function-summary', 'image/facet', 'image/dependency-summary', 'image/analysis-coverage', 'candidate/function-summary', 'candidate/function-facet', 'candidate/dependency-summary', 'candidate/contract-delta', 'candidate/ownership-delta', 'candidate/analysis-coverage']);
const EVIDENCE_FACETS = new Set(['contracts', 'effects', 'ownership', 'loans', 'cleanup']);
const stableId = value => typeof value === 'string' && value.length > 0 && value.length <= 512 && !/[\u0000-\u001f\u007f]/.test(value);
const plain = value => value && typeof value === 'object' && !Array.isArray(value) && (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null);
const digest = value => typeof value === 'string' && /^sha256:[0-9a-f]{64}$/.test(value);
const closed = (value, keys) => plain(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));

// The MCP client remains deliberately serial. This layer coalesces immutable
// explorer reads before they reach it and forgets queued work on a new view.
class ExplorerScheduler {
  constructor(call) { this.call = call; this.pending = new Map(); this.queue = []; this.running = false; this.generation = 0; }
  clear() {
    this.generation++;
    for (const item of this.queue) item.reject(new Error('Explorer selection changed'));
    this.queue = []; this.pending.clear();
  }
  read(key, work) {
    if (this.pending.has(key)) return this.pending.get(key);
    const generation = this.generation;
    const result = new Promise((resolve, reject) => { this.queue.push({ key, generation, work, resolve, reject }); this.drain(); });
    this.pending.set(key, result); result.finally(() => this.pending.delete(key)).catch(() => {}); return result;
  }
  async drain() {
    if (this.running) return; this.running = true;
    while (this.queue.length) {
      const item = this.queue.shift();
      if (item.generation !== this.generation) { item.reject(new Error('Explorer selection changed')); continue; }
      try { item.resolve(await item.work()); } catch (error) { item.reject(error); }
    }
    this.running = false;
  }
}

function message(value, generation) {
  if (!closed(value, ['type', 'generation', 'requestId', 'action', 'value']) || value.type !== 'semaprax-explorer-request' || value.generation !== generation || !Number.isSafeInteger(value.requestId) || value.requestId < 0 || value.requestId > 1000000 || typeof value.action !== 'string') return null;
  if (!['summary', 'page', 'readEvidence', 'reveal', 'deltaCatalog', 'semanticDelta', 'rendered'].includes(value.action)) return null;
  if (value.action === 'rendered') return value.requestId === 0 ? value : null;
  if (value.requestId < 1) return null;
  return value;
}
function pageRequest(value, summary, cursors) {
  if (!closed(value, ['view', 'handle', 'cursor', 'page_size', 'max_bytes']) || !VIEWS.has(value.view) || !DIGEST.test(value.handle) || !(value.cursor === null || typeof value.cursor === 'string' && value.cursor.length <= 128) || !Number.isSafeInteger(value.page_size) || value.page_size < 1 || value.page_size > 128 || !Number.isSafeInteger(value.max_bytes) || value.max_bytes < 1024 || value.max_bytes > 512 * 1024) return null;
  const inventory = summary?.inventories?.find(row => row.view === value.view);
  if (!inventory || inventory.handle !== value.handle || cursors.get(value.view) !== value.cursor) return null;
  return { view: value.view, handle: value.handle, cursor: value.cursor, page_size: value.page_size, max_bytes: value.max_bytes };
}
function sourceReference(value) {
  if (!plain(value) || Object.keys(value).length !== 4 || typeof value.path !== 'string' || !value.path || !DIGEST.test(value.source_revision) || !DIGEST.test(value.source_digest) || !plain(value.span) || Object.keys(value.span).length !== 4) return null;
  for (const name of ['start', 'end', 'line', 'column']) if (!Number.isSafeInteger(value.span[name]) || value.span[name] < 0) return null;
  if (value.span.end < value.span.start || value.span.line < 1 || value.span.column < 1) return null;
  return value;
}
function retainSourceReferences(result, subject, references) {
  if (subject?.side !== 'current' || result?.view !== 'declarations' || !Array.isArray(result.items)) return;
  for (const row of result.items) {
    const reference = sourceReference(row?.source_reference);
    if (reference) references.set(JSON.stringify(reference), reference);
  }
}
function summaryQuery(value, allowedSide) {
  if (!closed(value, ['mode', 'target', 'direction', 'depth', 'side']) || !['overview', 'context', 'impact'].includes(value.mode) || value.side !== allowedSide ||
      !(value.target === undefined || value.target === null || stableId(value.target)) ||
      !(value.direction === undefined || ['forward', 'reverse', 'both'].includes(value.direction)) ||
      !(value.depth === undefined || Number.isSafeInteger(value.depth) && value.depth >= 0 && value.depth <= 3)) return null;
  if (value.mode === 'overview' && value.target != null || value.mode !== 'overview' && !stableId(value.target)) return null;
  return { mode: value.mode, target: value.target ?? null, direction: value.direction || 'both', depth: value.depth ?? 1, side: value.side };
}
function reportChunk(value, candidate, target, schema, offset) {
  if (!plain(value) || value.schema !== 'semaprax.image-semantic-delta-chunk.v1' || value.candidate_revision !== candidate ||
      value.target !== target || value.report_schema !== schema || value.offset !== offset ||
      !Number.isSafeInteger(value.total_bytes) || value.total_bytes < 0 || value.total_bytes > MAX_REPORT_BYTES ||
      typeof value.chunk !== 'string' || !(value.next_offset === null || Number.isSafeInteger(value.next_offset))) throw new Error('Invalid candidate change report chunk');
  const bytes = Buffer.byteLength(value.chunk), end = offset + bytes;
  if (end > value.total_bytes || (value.next_offset === null ? end !== value.total_bytes : value.next_offset !== end)) throw new Error('Invalid candidate change report continuation');
  return { total: value.total_bytes };
}
async function readChangeReport(invoke, image, candidate, target) {
  const schema = target === null ? 'semaprax.project-candidate-semantic-delta-catalog.v1' : 'semaprax.project-candidate-semantic-delta.v1';
  const method = target === null ? 'candidate/semantic-delta-catalog' : 'candidate/semantic-delta';
  const chunks = [];
  let offset = 0, total = null;
  for (let pages = 0; pages < 256; pages++) {
    const payload = (await invoke(method, { image_revision: image, candidate_revision: candidate, ...(target === null ? {} : { target }), offset, chunk_bytes: 65536 })).payload;
    const next = reportChunk(payload, candidate, target, schema, offset);
    if (total === null) total = next.total;
    if (total !== next.total) throw new Error('Candidate change report size changed');
    chunks.push(payload.chunk);
    if (payload.next_offset === null) {
      const text = chunks.join('');
      if (Buffer.byteLength(text) !== total) throw new Error('Candidate change report size mismatch');
      const result = parse(text, MAX_REPORT_BYTES);
      if (!plain(result) || result.schema !== schema || result.candidate_digest !== candidate || (target !== null && result.target !== target)) throw new Error('Candidate change report binding mismatch');
      return result;
    }
    offset = payload.next_offset;
  }
  throw new Error('Candidate change report page limit');
}
function same(value, expected) { return JSON.stringify(value) === JSON.stringify(expected); }
function evidenceRequest(value, subject) {
  if (!plain(value) || !EVIDENCE_METHODS.has(value.method) || !same(value.subject, subject) ||
      !(value.target === null || stableId(value.target)) || !(value.facet === null || EVIDENCE_FACETS.has(value.facet))) throw new Error('Invalid explorer evidence request');
  if ((value.method === 'image/facet' || value.method === 'candidate/function-facet') && (!stableId(value.target) || !value.facet)) throw new Error('Invalid explorer function facet request');
  return value;
}
function boundedText(value) { return typeof value === 'string' && value.length <= 4096 && !value.includes('\0'); }
function safeList(value, limit = 128) { return Array.isArray(value) && value.length <= limit && value.every(boundedText); }
function responseBinding(result, image) {
  if (!plain(result) || result.image_revision !== image || !plain(result.payload)) throw new Error('Explorer evidence response binding mismatch');
  return result.payload;
}
function subjectBinding(payload, subject, target = undefined) {
  const payloadImage = payload.image_revision ?? payload.image_digest;
  // Candidate reads derive a temporary semantic image from the candidate
  // revision. Its digest is distinct from the live outer session image.
  if (!digest(payloadImage) || (subject.kind === 'image' && payloadImage !== subject.image_revision) ||
      payload.project_revision !== subject.project_revision ||
      (Object.hasOwn(payload, 'workspace_revision') && payload.workspace_revision !== subject.workspace_revision) ||
      (Object.hasOwn(payload, 'project_graph_digest') && payload.project_graph_digest !== subject.project_graph_digest) ||
      (subject.kind === 'candidate' && payload.candidate_revision !== subject.candidate_revision) ||
      (target !== undefined && (payload.target ?? payload.id) !== target)) throw new Error('Explorer evidence payload binding mismatch');
}
function safeInventory(value, names) {
  if (!plain(value)) throw new Error('Invalid explorer evidence inventory');
  const result = {};
  for (const name of names) if (Number.isSafeInteger(value[name]) && value[name] >= 0) result[name] = value[name];
  return result;
}
function compactFunction(payload) {
  if (!boundedText(payload.id) || !Number.isSafeInteger(payload.parameter_count) || !boundedText(payload.return_type_id) ||
      !safeList(payload.effects) || !Number.isSafeInteger(payload.requires_count) || !Number.isSafeInteger(payload.ensures_count) || !Array.isArray(payload.facets)) throw new Error('Invalid function summary evidence');
  return { id: payload.id, parameter_count: payload.parameter_count, return_type_id: payload.return_type_id,
    effects: payload.effects, requires_count: payload.requires_count, ensures_count: payload.ensures_count,
    facets: payload.facets.map(row => { if (!plain(row) || !boundedText(row.facet)) throw new Error('Invalid function facet inventory'); return row.facet; }) };
}
function compactDependencies(payload) {
  if (!boundedText(payload.target) || !boundedText(payload.kind) || !Array.isArray(payload.facets)) throw new Error('Invalid dependency summary evidence');
  return { target: payload.target, kind: payload.kind, facets: payload.facets.map(row => {
    if (!plain(row) || !boundedText(row.view) || !Number.isSafeInteger(row.total_items) || row.total_items < 0) throw new Error('Invalid dependency facet inventory');
    return { view: row.view, total_items: row.total_items };
  }), test_reachable: payload.test_reachable === true };
}
function compactCoverage(payload) {
  if (!plain(payload.inventory) || !Array.isArray(payload.areas)) throw new Error('Invalid analysis coverage evidence');
  return { inventory: safeInventory(payload.inventory, ['functions', 'templates', 'instances', 'nominal_types', 'interfaces', 'interface_imports']), areas: payload.areas.map(row => {
    if (!plain(row) || !boundedText(row.area) || !boundedText(row.status)) throw new Error('Invalid analysis coverage area');
    return { area: row.area, status: row.status };
  }) };
}
async function chunkedEvidenceReport(invoke, image, candidate, method, chunkSchema, reportSchema) {
  const chunks = []; let offset = 0; let total = null;
  for (let pages = 0; pages < 512; pages++) {
    const payload = responseBinding(await invoke(method, { image_revision: image, candidate_revision: candidate, offset, chunk_bytes: 16384 }), image);
    if (payload.schema !== chunkSchema || payload.report_schema !== reportSchema || payload.candidate_revision !== candidate || payload.offset !== offset ||
        !Number.isSafeInteger(payload.total_bytes) || payload.total_bytes < 0 || payload.total_bytes > MAX_REPORT_BYTES ||
        typeof payload.chunk !== 'string' || !(payload.next_offset === null || Number.isSafeInteger(payload.next_offset))) throw new Error('Invalid explorer evidence chunk');
    const end = offset + Buffer.byteLength(payload.chunk);
    if (end > payload.total_bytes || (payload.next_offset === null ? end !== payload.total_bytes : payload.next_offset !== end)) throw new Error('Invalid explorer evidence continuation');
    if (total === null) total = payload.total_bytes;
    if (total !== payload.total_bytes) throw new Error('Explorer evidence size changed');
    chunks.push(payload.chunk);
    if (payload.next_offset === null) return parse(chunks.join(''), MAX_REPORT_BYTES);
    offset = payload.next_offset;
  }
  throw new Error('Explorer evidence page limit');
}
function compactDelta(report, subject, schema, kind) {
  if (!plain(report) || report.schema !== schema || report.candidate_digest !== subject.candidate_revision ||
      report.project_revision !== subject.project_revision || report.workspace_revision !== subject.workspace_revision || !plain(report.inventory)) throw new Error('Explorer evidence delta binding mismatch');
  const rows = [...(Array.isArray(report[kind]) ? report[kind] : []), ...(kind === 'functions' && Array.isArray(report.types) ? report.types : [])];
  if (rows.length > 65536) throw new Error('Explorer evidence delta inventory limit');
  return { inventory: safeInventory(report.inventory, ['base_functions', 'candidate_functions', 'base_predicates', 'candidate_predicates', 'base_functions_with_contracts', 'candidate_functions_with_contracts', 'unchanged_functions', 'affected_functions', 'base_source_only_functions', 'candidate_source_only_functions', 'base_types', 'candidate_types', 'affected_types']),
    changed: rows.map(row => { if (!plain(row) || !stableId(row.id) || !boundedText(row.change)) throw new Error('Invalid explorer evidence delta row'); return { id: row.id, change: row.change }; }) };
}
function envelope(subject, request, compact, nonclaims) {
  return { schema: 'semaprax.explorer-evidence-read.v1', subject, method: request.method, target: request.target, facet: request.facet,
    state: 'available', compact, omitted: ['source bodies', 'raw report payloads', 'literal-bearing checked expressions', 'source spans'], nonclaims,
    source_authority: false, execution: false };
}
async function readEvidence(invoke, state, subject, raw) {
  const request = evidenceRequest(raw, subject), image = state.image(), candidate = state.candidate();
  if (subject.image_revision !== image || (subject.kind === 'candidate' && subject.candidate_revision !== candidate)) throw new Error('Explorer evidence subject is stale');
  if (request.method.startsWith('candidate/') && (subject.kind !== 'candidate' || subject.side !== 'candidate')) throw new Error('Candidate evidence requires the selected final candidate');
  let payload, compact, nonclaims = ['no_source_authority', 'no_execution'];
  if (request.method === 'candidate/contract-delta' || request.method === 'candidate/ownership-delta') {
    if (subject.kind !== 'candidate' || subject.side !== 'candidate' || request.target !== null || request.facet !== null) throw new Error('Invalid candidate delta evidence request');
    const contract = request.method === 'candidate/contract-delta';
    payload = await chunkedEvidenceReport(invoke, image, candidate, request.method, contract ? 'semaprax.image-contract-delta-chunk.v1' : 'semaprax.image-ownership-delta-chunk.v1', contract ? 'semaprax.project-candidate-contract-delta.v1' : 'semaprax.project-candidate-ownership-delta.v1');
    if (payload.source_authority !== false || payload.execution !== false) throw new Error('Explorer evidence delta authority mismatch');
    compact = compactDelta(payload, subject, payload.schema, 'functions');
    nonclaims = safeList(payload.nonclaims) ? payload.nonclaims : nonclaims;
  } else if (request.method === 'image/facet' || request.method === 'candidate/function-facet') {
    const candidateMethod = request.method === 'candidate/function-facet';
    const summary = responseBinding(await invoke(candidateMethod ? 'candidate/function-summary' : 'image/function-summary', candidateMethod ? { image_revision: image, candidate_revision: candidate, target: request.target } : { image_revision: image, target: request.target }), image);
    subjectBinding(summary, subject, request.target);
    const descriptor = Array.isArray(summary.facets) && summary.facets.find(row => plain(row) && row.facet === request.facet && digest(row.handle));
    if (!descriptor) throw new Error('Explorer evidence facet is not advertised for this function');
    payload = responseBinding(await invoke(request.method, candidateMethod
      ? { image_revision: image, candidate_revision: candidate, target: request.target, facet: request.facet, handle: descriptor.handle, cursor: null, page_size: 32, max_bytes: 65536 }
      : { image_revision: image, target: request.target, facet: request.facet, handle: descriptor.handle, cursor: null, page_size: 32, max_bytes: 65536 }), image);
    subjectBinding(payload, subject, request.target);
    if (payload.source_authority !== false) throw new Error('Explorer evidence facet authority mismatch');
    if (payload.facet !== request.facet || !Array.isArray(payload.items) || payload.items.length > 32 || !(payload.next_cursor === null || boundedText(payload.next_cursor))) throw new Error('Invalid explorer evidence facet page');
    compact = { facet: request.facet, displayed_items: payload.items.length, more_items: payload.next_cursor !== null };
    nonclaims = safeList(payload.nonclaims) ? payload.nonclaims : nonclaims;
  } else {
    const params = request.method.startsWith('candidate/') ? { image_revision: image, candidate_revision: candidate, ...(request.target === null ? {} : { target: request.target }) } : { image_revision: image, ...(request.target === null ? {} : { target: request.target }) };
    payload = responseBinding(await invoke(request.method, params), image);
    if (payload.source_authority !== false) throw new Error('Explorer evidence authority mismatch');
    if (request.method.includes('function-summary')) { subjectBinding(payload, subject, request.target); compact = compactFunction(payload); }
    else if (request.method.includes('dependency-summary')) { subjectBinding(payload, subject, request.target); compact = compactDependencies(payload); }
    else if (request.method.includes('analysis-coverage')) { subjectBinding(payload, subject); compact = compactCoverage(payload); }
    else throw new Error('Unsupported explorer evidence method');
    nonclaims = safeList(payload.nonclaims) ? payload.nonclaims : nonclaims;
  }
  return envelope(subject, request, compact, nonclaims);
}
function wrapped(file) { return `(function(){\n${fs.readFileSync(path.join(__dirname, 'explorer-assets', file), 'utf8')}\n})();`; }
function html(webview, extensionUri, generation, label, cacheScope) {
  const nonce = crypto.randomBytes(16).toString('base64');
  const style = webview.asWebviewUri(extensionUri.with({ path: extensionUri.path + '/explorer-assets/explorer.css' }));
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource}; script-src 'nonce-${nonce}'; img-src ${webview.cspSource};"><link rel="stylesheet" href="${style}"></head><body><div id="app"></div><script nonce="${nonce}">${wrapped('model.js')}</script><script nonce="${nonce}">${wrapped('layout.js')}</script><script nonce="${nonce}">${wrapped('cache.js')}</script><script nonce="${nonce}">${wrapped('changes.js')}</script><script nonce="${nonce}">${wrapped('evidence.js')}</script><script nonce="${nonce}">${wrapped('hosts.js')}</script><script nonce="${nonce}">${wrapped('view.js')}</script><script nonce="${nonce}">const vscode=acquireVsCodeApi();const port={postMessage:m=>vscode.postMessage(m),addEventListener:(n,f)=>window.addEventListener(n,f)};SemapraxExplorerView.createExplorer(document.getElementById('app'),SemapraxExplorerHosts.vscodeHost(port,${generation},${JSON.stringify(cacheScope)}),{label:${JSON.stringify(label)},onReady:value=>port.postMessage({type:'semaprax-explorer-request',generation:${generation},requestId:0,action:'rendered',value})});</script></body></html>`;
}

function openExplorer(vscode, context, state, query) {
  const generation = ++state.panelGeneration;
  state.panel?.dispose();
  const panel = vscode.window.createWebviewPanel('semapraxExplorer', 'SEMAPRAX Explorer', vscode.ViewColumn.Beside, { enableScripts: true, retainContextWhenHidden: false, localResourceRoots: [context.extensionUri.with({ path: context.extensionUri.path + '/explorer-assets' })] });
  state.panel = panel; const scheduler = new ExplorerScheduler(state.invoke); let summary = null; const cursors = new Map(); const sourceReferences = new Map();
  panel.webview.html = html(panel.webview, context.extensionUri, generation, `${query.mode} · ${query.side}`, JSON.stringify([state.image(), state.candidate()]));
  let selectedQuery = null;
  const reply = (requestId, ok, value) => panel.webview.postMessage({ type: 'semaprax-explorer-response', generation, requestId, ok, ...(ok ? { value } : { error: String(value?.message || value).slice(0, 1024) }) });
  const subscription = panel.webview.onDidReceiveMessage(async raw => {
    state.message?.(raw);
    const request = message(raw, generation); if (!request || state.panel !== panel || !state.live()) return;
    try {
      if (request.action === 'rendered') {
        const value = request.value;
        if (!summary || !closed(value, ['mode', 'target', 'side', 'loaded']) || value.mode !== summary.mode || value.target !== summary.target || value.side !== summary.subject?.side || !Array.isArray(value.loaded) || value.loaded.some(view => !VIEWS.has(view))) throw new Error('Invalid explorer rendered view');
        state.rendered?.({ mode: value.mode, target: value.target, side: value.side, loaded: [...value.loaded].sort() });
      } else if (request.action === 'summary') {
        const next = summaryQuery(request.value, query.side); if (!next) throw new Error('Invalid explorer summary query');
        const key = JSON.stringify([state.image(), next.side === 'current' ? null : state.candidate(), next]);
        const method = next.side === 'current' ? 'image/explorer-summary' : 'candidate/explorer-summary';
        const params = next.side === 'current' ? { image_revision: state.image(), mode: next.mode, target: next.target, direction: next.direction, depth: next.depth, max_nodes: 256, analysis_max_bytes: 65536 } : { image_revision: state.image(), candidate_revision: state.candidate(), side: next.side, mode: next.mode, target: next.target, direction: next.direction, depth: next.depth, max_nodes: 256, analysis_max_bytes: 65536 };
        const result = (await scheduler.read(key, () => state.invoke(method, params))).payload;
        if (!plain(result) || result.schema !== 'semaprax.explorer-view.v1' || result.kind !== 'summary' || result.mode !== next.mode || result.target !== next.target || result.subject?.side !== next.side || (next.side !== 'current' && result.subject?.candidate_revision !== state.candidate()) || !Array.isArray(result.inventories)) throw new Error('Explorer summary binding mismatch');
        summary = result; selectedQuery = next; cursors.clear(); sourceReferences.clear(); for (const row of summary.inventories) cursors.set(row.view, null); reply(request.requestId, true, summary);
      } else if (request.action === 'page') {
        const page = pageRequest(request.value, summary, cursors); if (!page) throw new Error('Invalid explorer page request');
        if (!selectedQuery) throw new Error('Explorer summary required before page');
        const method = selectedQuery.side === 'current' ? 'image/explorer-page' : 'candidate/explorer-page';
        const params = selectedQuery.side === 'current' ? { image_revision: state.image(), mode: selectedQuery.mode, target: selectedQuery.target, direction: selectedQuery.direction, depth: selectedQuery.depth, max_nodes: 256, analysis_max_bytes: 65536, ...page } : { image_revision: state.image(), candidate_revision: state.candidate(), side: selectedQuery.side, mode: selectedQuery.mode, target: selectedQuery.target, direction: selectedQuery.direction, depth: selectedQuery.depth, max_nodes: 256, analysis_max_bytes: 65536, ...page };
        const result = (await scheduler.read(JSON.stringify([method, params]), () => state.invoke(method, params))).payload; cursors.set(page.view, result.next_cursor); retainSourceReferences(result, summary.subject, sourceReferences); reply(request.requestId, true, result);
      } else if (request.action === 'readEvidence') {
        if (!summary) throw new Error('Explorer summary required before evidence');
        const result = await scheduler.read(JSON.stringify(['evidence', request.value]), () => readEvidence(state.invoke, state, summary.subject, request.value));
        reply(request.requestId, true, result);
      } else if (request.action === 'deltaCatalog') {
        if (!summary || summary.subject?.kind !== 'candidate' || request.value?.candidateRevision !== state.candidate()) throw new Error('Invalid candidate change catalog request');
        const result = await scheduler.read(JSON.stringify(['catalog', state.image(), state.candidate()]), () => readChangeReport(state.invoke, state.image(), state.candidate(), null));
        reply(request.requestId, true, result);
      } else if (request.action === 'semanticDelta') {
        const target = request.value?.target;
        if (!summary || summary.subject?.kind !== 'candidate' || request.value?.candidateRevision !== state.candidate() || !stableId(target)) throw new Error('Invalid candidate change request');
        const result = await scheduler.read(JSON.stringify(['delta', state.image(), state.candidate(), target]), () => readChangeReport(state.invoke, state.image(), state.candidate(), target));
        reply(request.requestId, true, result);
      } else if (request.action === 'reveal') {
        if (!summary || summary.subject?.side !== 'current') throw new Error('Explorer source reveal is unavailable for base or candidate revisions');
        const reference = sourceReference(request.value?.sourceReference);
        const retained = reference && sourceReferences.get(JSON.stringify(reference));
        if (!retained || !same(retained, reference)) throw new Error('Explorer source reference was not retained from this current view');
        if (typeof state.reveal !== 'function') throw new Error('Explorer source reveal is unavailable until the host can retain the exact source bytes');
        await state.reveal(reference); reply(request.requestId, true, { state: 'revealed' });
      } else throw new Error('Unsupported explorer action');
    } catch (error) { reply(request.requestId, false, error); }
  });
  panel.onDidDispose(() => { subscription.dispose(); scheduler.clear(); if (state.panel === panel) state.panel = null; });
  return panel;
}
module.exports = { ExplorerScheduler, message, pageRequest, sourceReference, retainSourceReferences, readChangeReport, readEvidence, openExplorer, stableId };
