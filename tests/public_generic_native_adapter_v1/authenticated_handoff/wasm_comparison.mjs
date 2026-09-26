// Measured Core Wasm column against the compiler-emitted provider, through
// its production export inventory only. There is no test-only counter export:
// endpoint entry is observed as the absence of an input handle plus a refused
// call on it, allocation as linear-memory growth (first and repeated attempt),
// and live handles as the provider-close status (7 while any handle is live,
// 0 when none is).
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const cases = JSON.parse(readFileSync('cases.json', 'utf8'));
const module = await WebAssembly.compile(readFileSync('provider.wasm'));
assert.deepEqual(WebAssembly.Module.imports(module), []);
const exported = WebAssembly.Module.exports(module).map(entry => entry.name);
assert.deepEqual(exported, cases.exports, 'closed production export inventory');
const descriptor = readFileSync('descriptor.bin'), binding = readFileSync('binding.bin');
const hex = text => Buffer.from(text, 'hex');
const lane = raw => ({ status: Number(raw & 0xffffffffn), value: Number(raw >> 32n) });
async function session() {
    const { exports: api } = await WebAssembly.instantiate(module, {});
    const reserved = lane(api.spx_pg_v1_scratch_reserve(65536));
    assert.equal(reserved.status, 0);
    const scratch = reserved.value;
    new Uint8Array(api.memory.buffer).set(descriptor, scratch);
    new Uint8Array(api.memory.buffer).set(binding, scratch + descriptor.length);
    const opened = lane(api.spx_pg_v1_open(scratch, descriptor.length, scratch + descriptor.length, binding.length));
    assert.equal(opened.status, 0);
    assert.ok(opened.value > 0);
    return { api, scratch, provider: opened.value };
}
function prepare({ api, scratch, provider }, frame) {
    new Uint8Array(api.memory.buffer).set(frame, scratch);
    return lane(api.spx_pg_v1_input_prepare(provider, scratch, frame.length));
}
function canonical(state) {
    const { api, scratch, provider } = state;
    const prepared = prepare(state, hex(cases.canonical));
    assert.equal(prepared.status, 0);
    assert.ok(prepared.value > 0);
    let called;
    try {
        called = lane(api.spx_pg_v1_call(provider, prepared.value));
    } catch (error) {
        // A trap is recorded, not retried: the instance is no longer usable.
        assert.ok(error instanceof WebAssembly.RuntimeError);
        return { status: `trap:${error.message}` };
    }
    // A consumed input cannot be dispatched twice.
    const again = lane(api.spx_pg_v1_call(provider, prepared.value));
    let bytes = '';
    if (called.status === 0) {
        const size = lane(api.spx_pg_v1_result_export(called.value, scratch, 0));
        assert.equal(size.status, 12);
        const copied = lane(api.spx_pg_v1_result_export(called.value, scratch, size.value));
        assert.deepEqual(copied, { status: 0, value: size.value });
        bytes = Buffer.from(new Uint8Array(api.memory.buffer, scratch, size.value)).toString('hex');
        assert.equal(api.spx_pg_v1_result_release(called.value), 0);
    } else {
        // Existing target contract: failed call retains its input until release.
        assert.equal(api.spx_pg_v1_value_release(prepared.value), 0);
    }
    return { status: called.status, again: again.status, bytes,
        consumed: api.spx_pg_v1_value_release(prepared.value) };
}
const rows = [];
// The prepare entry takes (provider, frame pointer, frame length): no ticket
// generation, ownership or cleanup-plan argument exists to substitute.
const probe = await session();
rows.push({ id: 'prepare_arity', value: probe.api.spx_pg_v1_input_prepare.length });
assert.equal(probe.api.spx_pg_v1_provider_close(probe.provider), 0);
// ABI v1 is superseded for the compiled provider: the same facts under a v1
// binding fail its byte-exact binding replay at open and mint no provider.
{
    const { exports: api } = await WebAssembly.instantiate(module, {});
    const scratch = lane(api.spx_pg_v1_scratch_reserve(65536)).value;
    const v1 = readFileSync('binding_v1.bin');
    new Uint8Array(api.memory.buffer).set(descriptor, scratch);
    new Uint8Array(api.memory.buffer).set(v1, scratch + descriptor.length);
    rows.push({ id: 'v1_binding', ...lane(api.spx_pg_v1_open(scratch, descriptor.length, scratch + descriptor.length, v1.length)) });
}
for (const entry of cases.hostile) {
    const state = await session();
    const before = state.api.memory.buffer.byteLength;
    const prepared = prepare(state, hex(entry.frame));
    const call = lane(state.api.spx_pg_v1_call(state.provider, prepared.value));
    const grown = state.api.memory.buffer.byteLength - before;
    // A repeated refusal separates the one-time private reservation from any
    // per-attempt growth.
    const middle = state.api.memory.buffer.byteLength;
    assert.deepEqual(prepare(state, hex(entry.frame)), prepared);
    const repeat = state.api.memory.buffer.byteLength - middle;
    // The refused attempt left no live handle and no partial state: the same
    // provider then admits, executes and settles the canonical input.
    const recovered = canonical(state);
    const trapped = typeof recovered.status === 'string';
    rows.push({ id: entry.id, status: prepared.status, handle: prepared.value,
        call: call.status, grown, repeat, recovered: recovered.status === 0 ? recovered.bytes : recovered.status,
        close: trapped ? 'trapped' : state.api.spx_pg_v1_provider_close(state.provider) });
}
// Lifecycle order: before scratch reserve (even over host-grown memory) open
// refuses with 7, so no provider exists; input preparation with a guessed
// handle refuses with 8. Neither reads unbacked memory or traps.
{
    const { exports: api } = await WebAssembly.instantiate(module, {});
    const scratch = api.spx_pg_v1_scratch_ptr();
    const end = scratch + descriptor.length + binding.length;
    api.memory.grow(Math.ceil(end / 65536) - api.memory.buffer.byteLength / 65536);
    new Uint8Array(api.memory.buffer).set(descriptor, scratch);
    new Uint8Array(api.memory.buffer).set(binding, scratch + descriptor.length);
    const opened = lane(api.spx_pg_v1_open(scratch, descriptor.length, scratch + descriptor.length, binding.length));
    const prepared = lane(api.spx_pg_v1_input_prepare(1, scratch, 65536));
    rows.push({ id: 'prepare_before_scratch_reserve', open: opened.status, status: prepared.status, value: prepared.value });
}
const state = await session();
const before = state.api.memory.buffer.byteLength;
const run = canonical(state);
const liveClose = typeof run.status === 'string' ? 'trapped' : state.api.spx_pg_v1_provider_close(state.provider);
rows.push({ id: 'canonical', ...run, grown: state.api.memory.buffer.byteLength - before, close: liveClose });
process.stdout.write(JSON.stringify(rows));
