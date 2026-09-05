// Test facade for the WebAssembly lane of Bounded Language Network I/O v1.
//
// Usage: node network_io_wasm_facade.mjs <app.wasm> <export> <fixture.json|-> <hostile|->
//
// The facade instantiates a network-lane module with the frozen command `env`
// plus the seven network imports, serving a `semaprax.network-fixture.v1`
// document as the provider. A synchronous Wasm import cannot block on Node's
// asynchronous `net` sockets, so no socket is ever opened here; real sockets on
// Wasm are out of scope for v1. `-` as the fixture means the invocation was
// given no network provider (every network import answers AUTHORITY_DENIED).
//
// The optional hostile mode makes the provider break its contract in one exact
// way so the module's fail-closed edges can be exercised:
//   status-7        connect answers a status outside 0..=6
//   handle-9        connect reports a handle outside 1..=8
//   count-over-max  stream reports more bytes than the caller's bound
//   bad-token       recv reports an owned token the arena never issued
//
// Output, one record per line:
//   validate <0|1>
//   ok <result> <stdout-hex|-> <stderr-hex|-> <arena> <handles> <dirty>
//   failure <status> <network-status> <input-status> <stdout-len> <stderr-len> <arena> <dirty>
//   invariant <network-status> <input-status> <stdout-len> <stderr-len> <arena> <dirty>
//   trap <message>
import fs from 'node:fs';

const [wasmPath, symbol, fixturePath, hostileArg] = process.argv.slice(2);
const hostile = hostileArg && hostileArg !== '-' ? hostileArg : null;
const bytes = fs.readFileSync(wasmPath);
console.log(`validate ${WebAssembly.validate(bytes) ? 1 : 0}`);

const fixture = fixturePath && fixturePath !== '-' ? JSON.parse(fs.readFileSync(fixturePath, 'utf8')) : null;
if (fixture && fixture.schema !== 'semaprax.network-fixture.v1') throw Error('fixture schema');

const MAX_HANDLES = 8, MAX_HOST = 253, MAX_PORT = 65535, MAX_CHUNK = 65536, MAX_TOTAL = 1048576, MAX_WAIT = 30000;
const MEMORY_BYTES = 393216, STDOUT_BASE = 131072, STDERR_BASE = 196608, STAGE_BASE = 262144;
const encoder = new TextEncoder(), decoder = new TextDecoder('utf-8', { fatal: true });

let instance = null, next = 1;
const entries = new Map();
const memory = () => new Uint8Array(instance.exports.memory.buffer);
const view = () => new DataView(instance.exports.memory.buffer);
const word = c => BigInt.asUintN(64, c);
const raw = c => { const w = word(c); return { w, n: Number(w & 0xffffffffn), h: Number((w >> 32n) & 0xffffffffn) }; };
const read = c => {
  const d = raw(c);
  if ((d.h & 0xc0000000) === 0x40000000) {
    const p = (d.h & 0xffff) * 8, k = (d.h >>> 16) & 0x1fff, v = view();
    if (p + 32 > v.byteLength || v.getUint32(p, true) !== k || v.getUint32(p + 4, true) !== p || Number(v.getBigUint64(p + 24, true)) !== d.n) throw Error('descriptor');
    const root = v.getBigInt64(p + 8, true), off = Number(v.getBigUint64(p + 16, true)), all = read(root);
    if (off > all.length || d.n > all.length - off) throw Error('range');
    return all.slice(off, off + d.n);
  }
  if ((d.h & 0x80000000) !== 0) {
    const b = entries.get(d.h & 0x7fffffff);
    if (!b || b.length !== d.n) throw Error('token');
    return b;
  }
  const mem = memory();
  if (d.h > mem.length - d.n) throw Error('fixed');
  return mem.slice(d.h, d.h + d.n);
};
const alloc = b => { const k = next++; entries.set(k, new Uint8Array(b)); return BigInt.asIntN(64, ((0x80000000n | BigInt(k)) << 32n) | BigInt(b.length)); };
const carrier = (root, len) => BigInt.asIntN(64, (BigInt(root >>> 0) << 32n) | BigInt(len >>> 0));
const out = (p, value) => view().setBigInt64(p, BigInt(value), true);
const concat = parts => { const n = parts.reduce((a, b) => a + b.length, 0), all = new Uint8Array(n); let at = 0; for (const p of parts) { all.set(p, at); at += p.length; } return all; };
const isPrefix = (a, b) => a.length <= b.length && a.every((v, i) => v === b[i]);

// Fixture provider state. Connections bind in order of successful connects;
// handles are dense, invocation-scoped, and never reused.
const connections = (fixture ? fixture.connections : []).map(c => ({
  host: c.host, port: c.port,
  pending: (c.recv || []).map(s => encoder.encode(s)),
  expectSend: c.expect_send === undefined ? null : encoder.encode(c.expect_send),
  ready: c.ready !== false, sent: [], readStarted: false,
}));
let nextConnection = 0, nextHandle = 1, total = 0;
const handles = new Map();
const take = (c, max) => {
  while (c.pending.length && c.pending[0].length === 0) c.pending.shift();
  if (!c.pending.length) return new Uint8Array(0);
  const head = c.pending[0];
  if (head.length <= max) { c.pending.shift(); return head; }
  c.pending[0] = head.subarray(max);
  return head.subarray(0, max);
};
const firstRead = c => {
  if (c.readStarted) return 0;
  c.readStarted = true;
  if (c.expectSend) { const sent = concat(c.sent); if (sent.length !== c.expectSend.length || !isPrefix(sent, c.expectSend)) return 5; }
  return 0;
};
const deliver = (h, max) => {
  const c = handles.get(h);
  if (!c) return [3];
  if (max > MAX_CHUNK) return [4];
  const early = firstRead(c);
  if (early) return [early];
  const chunk = take(c, max);
  total += chunk.length;
  if (total > MAX_TOTAL) return [4];
  return [0, chunk];
};
const network = {
  spx_network_connect_v1: (root, len, port, p) => {
    if (!fixture) return 6;
    let host;
    try { host = decoder.decode(read(carrier(root, len))); } catch { return 2; }
    if (len === 0 || len > MAX_HOST || host.includes('\0') || port < 1 || port > MAX_PORT) return 2;
    if (nextHandle > MAX_HANDLES) return 4;
    const c = connections[nextConnection];
    if (!c || c.host !== host || c.port !== port) return 1;
    nextConnection += 1;
    const h = nextHandle++;
    handles.set(h, c);
    out(p, hostile === 'handle-9' ? 9 : h);
    return hostile === 'status-7' ? 7 : 0;
  },
  spx_network_send_v1: (h, root, len, p) => {
    if (!fixture) return 6;
    const c = handles.get(h);
    if (!c) return 3;
    total += len;
    if (total > MAX_TOTAL) return 4;
    c.sent.push(new Uint8Array(read(carrier(root, len))));
    if (c.expectSend && !isPrefix(concat(c.sent), c.expectSend)) return 5;
    out(p, len);
    return 0;
  },
  spx_network_recv_v1: (h, max, p) => {
    if (!fixture) return 6;
    const [status, chunk] = deliver(h, max);
    if (status) return status;
    out(p, hostile === 'bad-token' ? BigInt.asIntN(64, ((0x80000000n | 77n) << 32n) | BigInt(chunk.length)) : alloc(chunk));
    return 0;
  },
  spx_network_stream_stdout_v1: (h, dst, max, p) => {
    if (!fixture) return 6;
    const [status, chunk] = deliver(h, max);
    if (status) return status;
    memory().set(chunk, dst);
    out(p, hostile === 'count-over-max' ? max + 1 : chunk.length);
    return 0;
  },
  spx_network_wait_v1: (h, timeout, p) => {
    if (!fixture) return 6;
    const c = handles.get(h);
    if (!c) return 3;
    if (timeout > MAX_WAIT) return 4;
    if (!c.ready) { c.ready = true; out(p, 0); return 0; }
    out(p, c.pending.some(b => b.length) ? 1 : 2);
    return 0;
  },
  spx_network_close_v1: h => (fixture ? (handles.delete(h) ? 0 : 3) : 6),
  spx_network_settle_v1: () => { handles.clear(); },
};

const env = {
  spx_add: (a, b) => a + b, spx_sub: (a, b) => a - b, spx_mul: (a, b) => a * b, spx_div: (a, b) => a / b, spx_rem: (a, b) => a % b, spx_neg: a => -a,
  spx_contract_fail: () => { throw Error('contract'); },
  spx_bytes_copy: c => alloc(read(c)),
  spx_bytes_get: (c, i) => { const b = read(c), n = Number(i); return n < b.length ? b[n] : -1; },
  spx_bytes_drop: c => { const d = raw(c); if ((d.h & 0x80000000) === 0 || !entries.delete(d.h & 0x7fffffff)) throw Error('drop'); },
  spx_bytes_as_slice: c => { read(c); return c; },
  spx_command_args_len_v1: () => 0n,
  spx_command_arg_utf8_v1: () => 1,
  spx_command_stdin_read_v1: () => 3,
  spx_command_owned_bytes_validate_v1: c => { try { const d = raw(c), b = entries.get(d.h & 0x7fffffff); return (d.h & 0x80000000) !== 0 && b && b.length === d.n ? 0 : 1; } catch { return 1; } },
  ...network,
};

const result = await WebAssembly.instantiate(bytes, { env: Object.freeze(env) });
instance = result.instance;
if (instance.exports.memory.buffer.byteLength !== MEMORY_BYTES) throw Error('memory');
let value;
try {
  value = instance.exports[symbol]();
} catch (error) {
  console.log(`trap ${String(error && error.message).replace(/\s+/g, '_')}`);
  process.exit(0);
}
const status = instance.exports.__spx_data_status_v1.value;
const networkStatus = instance.exports.__spx_network_status_v1.value;
const inputStatus = instance.exports.__spx_command_input_status_v1.value;
const sl = instance.exports.__spx_stdout_length_v1.value, el = instance.exports.__spx_stderr_length_v1.value;
const mem = memory();
let dirty = 0;
for (let i = STDOUT_BASE + sl; i < STDERR_BASE; i += 1) if (mem[i] !== 0) dirty += 1;
for (let i = STDERR_BASE + el; i < STAGE_BASE; i += 1) if (mem[i] !== 0) dirty += 1;
for (let i = STAGE_BASE; i < MEMORY_BYTES; i += 1) if (mem[i] !== 0) dirty += 1;
const hex = b => (b.length ? Array.from(b, v => v.toString(16).padStart(2, '0')).join('') : '-');
if (status === 0) {
  console.log(`ok ${value} ${hex(mem.subarray(STDOUT_BASE, STDOUT_BASE + sl))} ${hex(mem.subarray(STDERR_BASE, STDERR_BASE + el))} ${entries.size} ${handles.size} ${dirty}`);
} else if (status === -1) {
  console.log(`invariant ${networkStatus} ${inputStatus} ${sl} ${el} ${entries.size} ${dirty}`);
} else {
  console.log(`failure ${status} ${networkStatus} ${inputStatus} ${sl} ${el} ${entries.size} ${dirty}`);
}
