'use strict';
const { TextDecoder } = require('node:util');
const MAX_REQUEST = 128 * 1024;
const MAX_RESPONSE = 8 * 1024 * 1024;
const ALLOWED = new Set(['workspace/open', 'workspace/refresh-preview', 'workspace/refresh',
  'candidate/open', 'candidate/apply-intent', 'candidate/source-review', 'change/catalog',
  'expression/catalog', 'candidate/contract-expression-catalog', 'hole/open', 'hole/open-expression',
  'hole/open-contract-expression', 'hole/query', 'hole/summary', 'hole/page', 'hole/expression-catalog', 'hole/fill-suggestions', 'hole/fill',
  'hole/complete', 'hole/discard', 'protocol/constructor-schemas',
  'candidate/attempt', 'attempt/summary', 'attempt/query', 'attempt/repair-catalog', 'attempt/repair-apply', 'attempt/discard',
  'candidate/test-task-start', 'candidate/test-task-status', 'candidate/test-task-cancel', 'candidate/test-task-result']);
const digest = value => typeof value === 'string' && value.length === 71 && /^sha256:[0-9a-f]{64}$/.test(value);
function exact(value, keys) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Unexpected response fields');
  const actual = Object.keys(value).sort(), expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) throw new Error('Unexpected response fields');
}
function invalidates(error) { return !error.semantic || error.sourceInvalid === true || /SPX-(?:J10[23]|G221|G28[237])\b/.test(error.message); }
// Scan duplicate keys and structural bounds before JSON.parse. Unsafe numeric
// schema limits may occur in tools/list; operational counters are checked as
// safe integers at their use sites. No parsed number grants authority.
function parse(text, max = MAX_RESPONSE, safeNumbers = false) {
  if (typeof text !== 'string' || Buffer.byteLength(text) > max) throw new Error('JSON byte limit');
  let i = 0, nodes = 0;
  const white = () => { while (/\s/.test(text[i] || '') && i < text.length) i++; };
  const string = () => {
    const start = i++;
    while (i < text.length) { const ch = text[i++]; if (ch === '\\') i++; else if (ch === '"') {
      const value = JSON.parse(text.slice(start, i));
      if (/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/.test(value)) throw new Error('Unpaired JSON surrogate');
      return value;
    } }
    throw new Error('Unclosed JSON string');
  };
  const visit = depth => {
    if (++nodes > 262144 || depth > 128) throw new Error('JSON structure limit');
    white(); const ch = text[i];
    if (ch === '"') { string(); return; }
    if (ch === '{' || ch === '[') {
      const object = ch === '{', close = object ? '}' : ']'; const keys = new Set(); i++; white();
      if (text[i] === close) { i++; return; }
      for (;;) {
        white();
        if (object) { if (text[i] !== '"') throw new Error('Object key required'); const key = string(); if (keys.has(key)) throw new Error('Duplicate JSON key'); keys.add(key); white(); if (text[i++] !== ':') throw new Error('JSON colon required'); }
        visit(depth + 1); white(); const token = text[i++]; if (token === close) break; if (token !== ',') throw new Error('JSON separator required');
      }
      return;
    }
    const token = /^(?:true|false|null|-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)/.exec(text.slice(i));
    if (!token) throw new Error('Invalid JSON token');
    if (safeNumbers && /^[-0-9]/.test(token[0]) && !Number.isSafeInteger(Number(token[0]))) throw new Error('Intent numbers must be exact JavaScript safe integers');
    i += token[0].length;
  };
  visit(0); white(); if (i !== text.length) throw new Error('Trailing JSON'); return JSON.parse(text);
}
function inner(result) {
  exact(result, ['content', 'isError']);
  if (typeof result.isError !== 'boolean' || !Array.isArray(result.content) || result.content.length !== 1) throw new Error('Invalid MCP tool result');
  exact(result.content[0], ['type', 'text']);
  if (result.content[0].type !== 'text') throw new Error('Non-text MCP tool result');
  const raw = result.content[0].text, value = parse(raw, 1024 * 1024);
  if (value.jsonrpc !== '2.0' || value.id !== 0) throw new Error('Inner v5 response identity mismatch');
  if (result.isError) {
    exact(value, ['jsonrpc', 'id', 'error']); exact(value.error, ['code', 'message']);
    if (!Number.isSafeInteger(value.error.code) || typeof value.error.message !== 'string') throw new Error('Invalid v5 error shape');
    const failure = new Error(`SEMAPRAX: ${value.error.message.slice(0, 4096)}`); failure.semantic = true;
    failure.sourceInvalid = /SPX-(?:J10[23]|G221|G28[237])\b/.test(value.error.message); throw failure;
  }
  exact(value, ['jsonrpc', 'id', 'result']);
  exact(value.result, ['schema', 'protocol', 'image_revision', 'project_revision', 'payload']);
  if (!value.result.payload || typeof value.result.payload !== 'object' || Array.isArray(value.result.payload)) throw new Error('Expected object payload');
  if (value.result.schema !== 'semaprax.image-agent-result.v5' || value.result.protocol !== 'semaprax.image-agent-protocol.v5' ||
      !digest(value.result.image_revision) || !digest(value.result.project_revision)) throw new Error('Invalid v5 result binding');
  return { raw, ...value.result };
}
class McpClient {
  constructor(child, onFailure = () => {}, timeout = 30000) {
    this.child = child; this.onFailure = onFailure; this.timeout = timeout;
    // Retained bytes of an incomplete frame, kept as the chunks they arrived
    // in. `retained` is their total length, so the response cap is checked
    // without concatenating them; each chunk is scanned for the delimiter
    // exactly once and copied at most once, into its own completed frame.
    this.pending = null; this.chunks = []; this.retained = 0; this.next = 1; this.tools = new Set(); this.closed = false;
    child.stdout.on('data', data => this.receive(data));
    child.stdout.on('end', () => this.fail(new Error('Compiler stdout closed; explicit restart required')));
    child.on('error', error => this.fail(error));
    child.on('exit', () => this.fail(new Error('Compiler exited; explicit restart required')));
    child.stdin.on('error', error => this.fail(error));
    // Drain without displaying or retaining arbitrary compiler stderr.
    child.stderr.on('data', () => {});
  }
  fail(error) {
    if (this.closed) return;
    this.closed = true; const pending = this.pending; this.pending = null;
    if (pending) { clearTimeout(pending.timer); pending.reject(error); }
    this.chunks = []; this.retained = 0; this.child.kill(); this.onFailure(error);
  }
  stop() { this.fail(new Error('Session stopped')); }
  // The bytes of one completed frame: the retained chunks followed by the tail
  // that finished it. A frame that arrived whole is returned without a copy.
  frame(tail) {
    if (!this.chunks.length) return tail;
    this.chunks.push(tail);
    const bytes = Buffer.concat(this.chunks);
    this.chunks = []; this.retained = 0;
    return bytes;
  }
  // One complete LF-delimited response. Strict UTF-8, CR rejection, response
  // identity, serial request semantics, and terminal failure are unchanged.
  accept(bytes) {
    if (!this.pending || bytes.includes(13)) throw new Error('Unexpected MCP response frame');
    const value = parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
    const pending = this.pending;
    if (value.jsonrpc !== '2.0' || value.id !== pending.id) throw new Error('MCP response identity mismatch');
    if (Object.hasOwn(value, 'error')) throw new Error(`MCP error: ${String(value.error.message).slice(0, 4096)}`);
    exact(value, ['jsonrpc', 'id', 'result']);
    this.pending = null; clearTimeout(pending.timer); pending.resolve(value.result);
  }
  receive(data) {
    if (this.closed) return;
    try {
      if (this.retained + data.length > MAX_RESPONSE) throw new Error('MCP response byte limit');
      let start = 0;
      for (;;) {
        const end = data.indexOf(10, start);
        if (end < 0) break;
        const bytes = this.frame(data.subarray(start, end));
        start = end + 1;
        this.accept(bytes);
      }
      if (start < data.length) { this.chunks.push(data.subarray(start)); this.retained += data.length - start; }
    } catch (error) { this.fail(error); }
  }
  request(method, params) {
    if (this.closed || this.pending) return Promise.reject(new Error('Session closed or request already pending'));
    const id = `editor-${this.next++}`; const line = JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n';
    if (Buffer.byteLength(line) > MAX_REQUEST) return Promise.reject(new Error('MCP request byte limit'));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => this.fail(new Error('Compiler request timed out; never automatically retried')), this.timeout);
      this.pending = { id, resolve, reject, timer };
      this.child.stdin.write(line, error => { if (error) this.fail(error); });
    });
  }
  async initialize() {
    const result = await this.request('initialize', { protocolVersion: '2025-11-25', capabilities: {}, clientInfo: { name: 'semaprax-vscode', version: '0.1.0' } });
    if (result.protocolVersion !== '2025-11-25' || !result.capabilities?.tools) throw new Error('Unsupported MCP handshake');
    this.child.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');
    let cursor, pages = 0, catalogBytes = 0; const seen = new Set(), cursors = new Set();
    do {
      const page = await this.request('tools/list', cursor === undefined ? {} : { cursor });
      if (++pages > 256 || !Array.isArray(page.tools) || page.tools.length > 8) throw new Error('Tool catalog bound');
      const pageBytes = Buffer.byteLength(JSON.stringify(page)); catalogBytes += pageBytes;
      if (pageBytes > 900 * 1024 || catalogBytes > 16 * 1024 * 1024) throw new Error('Tool catalog byte bound');
      for (const tool of page.tools) {
        if (typeof tool.name !== 'string' || !/^[A-Za-z0-9_.-]{1,128}$/.test(tool.name) || seen.has(tool.name) || seen.size >= 256) throw new Error('Duplicate, invalid or excessive tool');
        seen.add(tool.name);
        for (const method of ALLOWED) if (tool.name === method.replaceAll('/', '__')) this.tools.add(method);
      }
      const next = page.nextCursor;
      if (next !== undefined && (typeof next !== 'string' || next.length > 128 || cursors.has(next))) throw new Error('Invalid catalog cursor');
      if (next !== undefined) cursors.add(next);
      cursor = next;
    } while (cursor !== undefined);
  }
  async call(method, params) {
    if (!ALLOWED.has(method) || !this.tools.has(method)) throw new Error('Method is outside the editor or host allowlist');
    if (Buffer.byteLength(JSON.stringify({ jsonrpc: '2.0', id: 0, method, params })) > 65536) throw new Error('v5 request byte limit');
    try { return inner(await this.request('tools/call', { name: method.replaceAll('/', '__'), arguments: params })); }
    catch (error) { if (!error.semantic) this.fail(error); throw error; }
  }
}
module.exports = { McpClient, parse, inner, exact, digest, invalidates, ALLOWED, MAX_REQUEST, MAX_RESPONSE };
