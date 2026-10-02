'use strict';
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { parse } = require('./protocol');

const VIEWS = new Set(['modules', 'declarations', 'relations', 'frontier']);
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const MAX_REPORT_BYTES = 8 * 1024 * 1024;
const stableId = value => typeof value === 'string' && value.length > 0 && value.length <= 512 && !/[\u0000-\u001f\u007f]/.test(value);
const plain = value => value && typeof value === 'object' && !Array.isArray(value) && (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null);

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
  if (!plain(value) || value.type !== 'semaprax-explorer-request' || value.generation !== generation || !Number.isSafeInteger(value.requestId) || value.requestId < 1 || value.requestId > 1000000 || typeof value.action !== 'string') return null;
  if (!['summary', 'page', 'readEvidence', 'reveal', 'deltaCatalog', 'semanticDelta'].includes(value.action)) return null;
  return value;
}
function pageRequest(value, summary, cursors) {
  if (!plain(value) || !VIEWS.has(value.view) || !DIGEST.test(value.handle) || !(value.cursor === null || typeof value.cursor === 'string' && value.cursor.length <= 128) || !Number.isSafeInteger(value.page_size) || value.page_size < 1 || value.page_size > 128 || !Number.isSafeInteger(value.max_bytes) || value.max_bytes < 1024 || value.max_bytes > 512 * 1024) return null;
  const inventory = summary?.inventories?.find(row => row.view === value.view);
  if (!inventory || inventory.handle !== value.handle || cursors.get(value.view) !== value.cursor) return null;
  return { view: value.view, handle: value.handle, cursor: value.cursor, page_size: value.page_size, max_bytes: value.max_bytes };
}
function summaryQuery(value, allowedSide) {
  if (!plain(value) || !['overview', 'context', 'impact'].includes(value.mode) || value.side !== allowedSide ||
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
function wrapped(file) { return `(function(){\n${fs.readFileSync(path.join(__dirname, 'explorer-assets', file), 'utf8')}\n})();`; }
function html(webview, extensionUri, generation, label) {
  const nonce = crypto.randomBytes(16).toString('base64');
  const style = webview.asWebviewUri(extensionUri.with({ path: extensionUri.path + '/explorer-assets/explorer.css' }));
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource}; script-src 'nonce-${nonce}'; img-src ${webview.cspSource};"><link rel="stylesheet" href="${style}"></head><body><div id="app"></div><script nonce="${nonce}">${wrapped('model.js')}</script><script nonce="${nonce}">${wrapped('layout.js')}</script><script nonce="${nonce}">${wrapped('cache.js')}</script><script nonce="${nonce}">${wrapped('changes.js')}</script><script nonce="${nonce}">${wrapped('evidence.js')}</script><script nonce="${nonce}">${wrapped('hosts.js')}</script><script nonce="${nonce}">${wrapped('view.js')}</script><script nonce="${nonce}">const port={postMessage:m=>acquireVsCodeApi().postMessage(m),addEventListener:(n,f)=>window.addEventListener(n,f)};SemapraxExplorerView.createExplorer(document.getElementById('app'),SemapraxExplorerHosts.vscodeHost(port,${generation}),{label:${JSON.stringify(label)}});</script></body></html>`;
}

function openExplorer(vscode, context, state, query) {
  const generation = ++state.panelGeneration;
  state.panel?.dispose();
  const panel = vscode.window.createWebviewPanel('semapraxExplorer', 'SEMAPRAX Explorer', vscode.ViewColumn.Beside, { enableScripts: true, retainContextWhenHidden: false, localResourceRoots: [context.extensionUri.with({ path: context.extensionUri.path + '/explorer-assets' })] });
  state.panel = panel; const scheduler = new ExplorerScheduler(state.invoke); let summary = null; const cursors = new Map();
  panel.webview.html = html(panel.webview, context.extensionUri, generation, `${query.mode} · ${query.side}`);
  let selectedQuery = null;
  const reply = (requestId, ok, value) => panel.webview.postMessage({ type: 'semaprax-explorer-response', generation, requestId, ok, ...(ok ? { value } : { error: String(value?.message || value).slice(0, 1024) }) });
  const subscription = panel.webview.onDidReceiveMessage(async raw => {
    const request = message(raw, generation); if (!request || state.panel !== panel || !state.live()) return;
    try {
      if (request.action === 'summary') {
        const next = summaryQuery(request.value, query.side); if (!next) throw new Error('Invalid explorer summary query');
        const key = JSON.stringify([state.image(), next.side === 'current' ? null : state.candidate(), next]);
        const method = next.side === 'current' ? 'image/explorer-summary' : 'candidate/explorer-summary';
        const params = next.side === 'current' ? { image_revision: state.image(), mode: next.mode, target: next.target, direction: next.direction, depth: next.depth, max_nodes: 256, analysis_max_bytes: 65536 } : { image_revision: state.image(), candidate_revision: state.candidate(), side: next.side, mode: next.mode, target: next.target, direction: next.direction, depth: next.depth, max_nodes: 256, analysis_max_bytes: 65536 };
        const result = (await scheduler.read(key, () => state.invoke(method, params))).payload;
        if (!plain(result) || result.schema !== 'semaprax.explorer-view.v1' || result.kind !== 'summary' || result.mode !== next.mode || result.target !== next.target || result.subject?.side !== next.side || (next.side !== 'current' && result.subject?.candidate_revision !== state.candidate()) || !Array.isArray(result.inventories)) throw new Error('Explorer summary binding mismatch');
        summary = result; selectedQuery = next; cursors.clear(); for (const row of summary.inventories) cursors.set(row.view, null); reply(request.requestId, true, summary);
      } else if (request.action === 'page') {
        const page = pageRequest(request.value, summary, cursors); if (!page) throw new Error('Invalid explorer page request');
        if (!selectedQuery) throw new Error('Explorer summary required before page');
        const method = selectedQuery.side === 'current' ? 'image/explorer-page' : 'candidate/explorer-page';
        const params = selectedQuery.side === 'current' ? { image_revision: state.image(), mode: selectedQuery.mode, target: selectedQuery.target, direction: selectedQuery.direction, depth: selectedQuery.depth, max_nodes: 256, analysis_max_bytes: 65536, ...page } : { image_revision: state.image(), candidate_revision: state.candidate(), side: selectedQuery.side, mode: selectedQuery.mode, target: selectedQuery.target, direction: selectedQuery.direction, depth: selectedQuery.depth, max_nodes: 256, analysis_max_bytes: 65536, ...page };
        const result = (await scheduler.read(JSON.stringify([method, params]), () => state.invoke(method, params))).payload; cursors.set(page.view, result.next_cursor); reply(request.requestId, true, result);
      } else if (request.action === 'readEvidence') {
        const value = request.value, methods = new Set(['image/function-summary', 'image/facet', 'image/dependency-summary', 'image/analysis-coverage', 'candidate/function-summary', 'candidate/function-facet', 'candidate/dependency-summary', 'candidate/contract-delta', 'candidate/ownership-delta', 'candidate/analysis-coverage']);
        if (!plain(value) || !methods.has(value.method) || !summary || JSON.stringify(value.subject) !== JSON.stringify(summary.subject) || !(value.target === null || stableId(value.target)) || !(value.facet === null || ['contracts', 'effects', 'ownership', 'loans', 'cleanup'].includes(value.facet))) throw new Error('Invalid explorer evidence request');
        throw new Error('Explorer evidence tabs are unavailable until their closed host envelope is selected');
      } else if (request.action === 'deltaCatalog') {
        if (!summary || summary.subject?.kind !== 'candidate' || request.value?.candidateRevision !== state.candidate()) throw new Error('Invalid candidate change catalog request');
        const result = await scheduler.read(JSON.stringify(['catalog', state.image(), state.candidate()]), () => readChangeReport(state.invoke, state.image(), state.candidate(), null));
        reply(request.requestId, true, result);
      } else if (request.action === 'semanticDelta') {
        const target = request.value?.target;
        if (!summary || summary.subject?.kind !== 'candidate' || request.value?.candidateRevision !== state.candidate() || !stableId(target)) throw new Error('Invalid candidate change request');
        const result = await scheduler.read(JSON.stringify(['delta', state.image(), state.candidate(), target]), () => readChangeReport(state.invoke, state.image(), state.candidate(), target));
        reply(request.requestId, true, result);
      } else throw new Error('Source reveal is unavailable until the host can retain the exact source bytes');
    } catch (error) { reply(request.requestId, false, error); }
  });
  panel.onDidDispose(() => { subscription.dispose(); scheduler.clear(); if (state.panel === panel) state.panel = null; });
  return panel;
}
module.exports = { ExplorerScheduler, message, pageRequest, readChangeReport, openExplorer, stableId };
