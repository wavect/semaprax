'use strict';

const semapraxExplorerHostModel = typeof module !== 'undefined' && module.exports ? require('./model.js') : globalThis.SemapraxExplorerModel;

function queryKey(query) {
  return JSON.stringify({ mode: query.mode, target: query.target || null, direction: query.direction || 'both', depth: query.depth === undefined ? 1 : query.depth, side: query.side || 'current' });
}
const EVIDENCE_METHODS = new Set(['image/function-summary', 'image/facet', 'image/dependency-summary', 'image/analysis-coverage', 'candidate/function-summary', 'candidate/function-facet', 'candidate/dependency-summary', 'candidate/contract-delta', 'candidate/ownership-delta', 'candidate/analysis-coverage']);
function evidenceRequest(request) {
  if (!request || !EVIDENCE_METHODS.has(request.method) || !request.subject || !['image', 'candidate'].includes(request.subject.kind) ||
      !(request.target === null || (typeof request.target === 'string' && request.target.length <= 4096)) ||
      !(request.facet === null || ['contracts', 'effects', 'ownership', 'loans', 'cleanup'].includes(request.facet))) throw new TypeError('unsupported explorer evidence request');
  return { method: request.method, subject: request.subject, target: request.target, facet: request.facet };
}

// The offline host has no fetch, filesystem, process, or editor channel. A
// detail absent from the exported snapshot remains explicitly not bundled.
function snapshotHost(bundle) {
  if (!bundle || !Array.isArray(bundle.views) || bundle.views.length > 32) throw new TypeError('invalid explorer bundle');
  const views = new Map();
  for (const entry of bundle.views) {
    if (!entry || !entry.query || !Array.isArray(entry.pages) || entry.pages.length > 1024) throw new TypeError('invalid explorer view');
    const selected = semapraxExplorerHostModel.summary(entry.summary);
    const key = queryKey(entry.query);
    if (views.has(key)) throw new TypeError('duplicate explorer view');
    const pages = new Map();
    for (const value of entry.pages) {
      semapraxExplorerHostModel.page(value, selected);
      const pageKey = JSON.stringify([value.view, value.handle, value.cursor, value.page_size, value.max_bytes]);
      if (pages.has(pageKey)) throw new TypeError('duplicate explorer page');
      pages.set(pageKey, value);
    }
    views.set(key, { selected, pages });
  }
  let active = null;
  const changes = bundle.changes || null;
  if (changes && (!changes.catalog || !Array.isArray(changes.details) || changes.details.length > 256)) throw new TypeError('invalid bundled changes');
  return Object.freeze({
    offline: true,
    hasView(query) { return views.has(queryKey(query)); },
    async summary(query) {
      const found = views.get(queryKey(query));
      if (!found) throw new Error('not bundled');
      active = found;
      return found.selected;
    },
    async page(request) {
      if (!active || active.selected !== request.summary) throw new Error('stale snapshot view');
      const key = JSON.stringify([request.view, request.handle, request.cursor, request.page_size, request.max_bytes]);
      const value = active.pages.get(key);
      if (!value) throw new Error('not bundled');
      return value;
    },
    async readEvidence(request) { evidenceRequest(request); throw new Error('not bundled'); },
    async evidence() { throw new Error('not bundled'); },
    async deltaCatalog(candidateRevision) {
      if (!changes || changes.catalog.candidate_digest !== candidateRevision) throw new Error('not bundled');
      return changes.catalog;
    },
    async semanticDelta(candidateRevision, target) {
      if (!changes || changes.catalog.candidate_digest !== candidateRevision) throw new Error('not bundled');
      const detail = changes.details.find(row => row.target === target);
      if (!detail) throw new Error('not bundled');
      return detail.report;
    },
    async exportReport() { throw new Error('not bundled'); }
  });
}

// The editor transport is deliberately closed. The extension validates the
// same request again against its live panel generation and retained subject.
function vscodeHost(port, generation) {
  if (!port || typeof port.postMessage !== 'function' || typeof port.addEventListener !== 'function' || !Number.isSafeInteger(generation) || generation < 0) throw new TypeError('invalid explorer editor port');
  let serial = 0, disposed = false;
  const pending = new Map();
  port.addEventListener('message', event => {
    const data = event.data;
    if (!data || data.type !== 'semaprax-explorer-response' || data.generation !== generation || !Number.isSafeInteger(data.requestId)) return;
    const request = pending.get(data.requestId);
    if (!request) return;
    pending.delete(data.requestId); clearTimeout(request.timer);
    if (data.ok === true) request.resolve(data.value);
    else request.reject(new Error(typeof data.error === 'string' ? data.error : 'editor refused explorer request'));
  });
  function call(action, value) {
    if (disposed) return Promise.reject(new Error('editor explorer host disposed'));
    return new Promise((resolve, reject) => {
      const requestId = ++serial;
      const timer = setTimeout(() => { pending.delete(requestId); reject(new Error('editor explorer request timed out')); }, 15000);
      pending.set(requestId, { resolve, reject, timer });
      port.postMessage({ type: 'semaprax-explorer-request', generation, requestId, action, value });
    });
  }
  const host = {
    summary(query) { return call('summary', query); },
    page(request) { return call('page', { view: request.view, handle: request.handle, cursor: request.cursor, page_size: request.page_size, max_bytes: request.max_bytes, subject: request.summary.subject, artifact_digest: request.summary.artifact_digest }); },
    readEvidence(request) { return call('readEvidence', evidenceRequest(request)); },
    deltaCatalog(candidateRevision) { return call('deltaCatalog', { candidateRevision }); },
    semanticDelta(candidateRevision, target) { return call('semanticDelta', { candidateRevision, target }); },
    reveal(sourceReference) { return call('reveal', { sourceReference }); },
    exportReport(format) { return call('export', { format }); },
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const request of pending.values()) {
        clearTimeout(request.timer);
        request.reject(new Error('editor explorer host disposed'));
      }
      pending.clear();
    }
  };
  return Object.freeze(host);
}

const semapraxExplorerHostsApi = { snapshotHost, vscodeHost };
if (typeof module !== 'undefined' && module.exports) module.exports = semapraxExplorerHostsApi;
else globalThis.SemapraxExplorerHosts = semapraxExplorerHostsApi;
