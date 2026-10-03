'use strict';

// Immutable explorer responses are local UI data. They are deliberately keyed
// by the complete compiler subject and request envelope; selection alone is
// never enough to authorize reuse after a refresh or candidate change.
const MAX_ENTRIES = 256;
const MAX_RETAINED_BYTES = 32 * 1024 * 1024;
const MAX_ENTRY_BYTES = 1024 * 1024;
const FORBIDDEN_VALUE_KEYS = new Set([
  'source', 'source_body', 'source_text', 'source_bytes', 'credential',
  'credentials', 'authorization', 'token', 'secret', 'password', 'private_key'
]);
const encoder = new TextEncoder();

function plain(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value) &&
    (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null);
}

function canonical(value, depth = 0) {
  if (depth > 64) throw new TypeError('explorer cache value is too deeply nested');
  if (value === null || typeof value === 'boolean' || typeof value === 'string') return JSON.stringify(value);
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value)) throw new TypeError('explorer cache value has an unsafe number');
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) return `[${value.map(item => canonical(item, depth + 1)).join(',')}]`;
  if (!plain(value)) throw new TypeError('explorer cache value must be JSON data');
  return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonical(value[key], depth + 1)}`).join(',')}}`;
}

function countBytes(serialized) { return encoder.encode(serialized).byteLength; }

function requestKey(request) {
  if (!plain(request) || !plain(request.subject) || !plain(request.query)) throw new TypeError('explorer cache request requires subject and query');
  if (!['summary', 'page'].includes(request.kind)) throw new TypeError('explorer cache request kind');
  const page = request.kind === 'page';
  if (page && (typeof request.view !== 'string' || typeof request.handle !== 'string' || !plain(request.options))) {
    throw new TypeError('explorer cache page request');
  }
  return canonical({
    kind: request.kind,
    subject: request.subject,
    artifact_digest: request.artifact_digest || null,
    side: request.subject.side,
    target: request.target === undefined ? null : request.target,
    mode: request.mode,
    query: request.query,
    view: page ? request.view : null,
    handle: page ? request.handle : null,
    cursor: page ? (request.cursor === undefined ? null : request.cursor) : null,
    options: page ? request.options : null
  });
}

function containsSensitiveValue(value, depth = 0) {
  if (depth > 64 || value === null || typeof value !== 'object') return false;
  if (Array.isArray(value)) return value.some(item => containsSensitiveValue(item, depth + 1));
  for (const [key, child] of Object.entries(value)) {
    if (FORBIDDEN_VALUE_KEYS.has(key) || containsSensitiveValue(child, depth + 1)) return true;
  }
  return false;
}

class ExplorerCache {
  constructor(options = {}) {
    const maxEntries = options.maxEntries === undefined ? MAX_ENTRIES : options.maxEntries;
    const maxBytes = options.maxBytes === undefined ? MAX_RETAINED_BYTES : options.maxBytes;
    const maxEntryBytes = options.maxEntryBytes === undefined ? MAX_ENTRY_BYTES : options.maxEntryBytes;
    if (!Number.isSafeInteger(maxEntries) || maxEntries < 1 || maxEntries > MAX_ENTRIES ||
        !Number.isSafeInteger(maxBytes) || maxBytes < 1 || maxBytes > MAX_RETAINED_BYTES ||
        !Number.isSafeInteger(maxEntryBytes) || maxEntryBytes < 1 || maxEntryBytes > MAX_ENTRY_BYTES) {
      throw new TypeError('explorer cache limits');
    }
    this.maxEntries = maxEntries;
    this.maxBytes = maxBytes;
    this.maxEntryBytes = maxEntryBytes;
    this.entries = new Map();
    this.summaryAliases = new Map();
    this.retainedBytes = 0;
  }

  get(request) {
    const key = requestKey(request);
    const entry = this.entries.get(key);
    if (!entry) return null;
    this.entries.delete(key);
    this.entries.set(key, entry);
    return JSON.parse(entry.serialized);
  }

  getSummary(alias) {
    if (typeof alias !== 'string' || !alias || alias.length > 8192) return null;
    const key = this.summaryAliases.get(alias), entry = key && this.entries.get(key);
    if (!entry) { this.summaryAliases.delete(alias); return null; }
    this.entries.delete(key);
    this.entries.set(key, entry);
    return JSON.parse(entry.serialized);
  }

  set(request, value, summaryAlias = null) {
    const key = requestKey(request);
    if (!plain(value) || value.kind !== request.kind || containsSensitiveValue(value)) return false;
    const serialized = canonical(value);
    const bytes = countBytes(serialized);
    if (bytes > this.maxEntryBytes || bytes > this.maxBytes) return false;
    const previous = this.entries.get(key);
    if (previous) {
      this.entries.delete(key);
      this.retainedBytes -= previous.bytes;
    }
    this.entries.set(key, { bytes, serialized });
    this.retainedBytes += bytes;
    if (request.kind === 'summary' && typeof summaryAlias === 'string' && summaryAlias && summaryAlias.length <= 8192) {
      this.summaryAliases.set(summaryAlias, key);
    }
    while (this.entries.size > this.maxEntries || this.retainedBytes > this.maxBytes) {
      const oldest = this.entries.entries().next().value;
      this.entries.delete(oldest[0]);
      this.retainedBytes -= oldest[1].bytes;
      for (const [alias, cached] of this.summaryAliases) if (cached === oldest[0]) this.summaryAliases.delete(alias);
    }
    return true;
  }

  clear() {
    this.entries.clear();
    this.summaryAliases.clear();
    this.retainedBytes = 0;
  }

  stats() { return Object.freeze({ entries: this.entries.size, retainedBytes: this.retainedBytes }); }
}

const semapraxExplorerCacheApi = Object.freeze({
  ExplorerCache, MAX_ENTRIES, MAX_RETAINED_BYTES, MAX_ENTRY_BYTES, requestKey
});
if (typeof module !== 'undefined' && module.exports) module.exports = semapraxExplorerCacheApi;
else globalThis.SemapraxExplorerCache = semapraxExplorerCacheApi;
