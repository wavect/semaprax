// Raw compiled Core Wasm provider driver (closed spx_pg_v1 ABI, no imports).
// Emits the shared receipt; the result carrier is reported as frame:<hex>
// and decoded/validated by the Rust harness.
import { readFileSync } from 'node:fs';
const cases = JSON.parse(readFileSync('cases.json', 'utf8'));
const wasm = readFileSync('provider.wasm');
const descriptor = readFileSync('descriptor.bin'), binding = readFileSync('binding.bin');
const module = await WebAssembly.compile(wasm);
if (WebAssembly.Module.imports(module).length !== 0) throw new Error('ambient import');
const lane = raw => ({ status: Number(raw & 0xffffffffn), value: Number((raw >> 32n) & 0xffffffffn) });
const hex = bytes => bytes.length === 0 ? 'e' : Buffer.from(bytes).toString('hex');
for (const c of cases) {
    const { exports: api } = await WebAssembly.instantiate(module, {});
    const reserved = lane(api.spx_pg_v1_scratch_reserve(16 * 1024 * 1024 + 2056));
    if (reserved.status !== 0) throw new Error('reserve');
    const scratch = reserved.value;
    let memory = new Uint8Array(api.memory.buffer);
    memory.set(descriptor, scratch);
    memory.set(binding, scratch + descriptor.length);
    const opened = lane(api.spx_pg_v1_open(scratch, descriptor.length, scratch + descriptor.length, binding.length));
    if (opened.status !== 0) throw new Error('open');
    const cycles = c.kind === 1 ? c.cycles : 1;
    for (let cycle = 0; cycle < cycles; cycle += 1) {
        const label = c.kind === 1 ? `${c.id}#${cycle}` : c.id;
        const notes = [];
        let primary = 0, secondary = '-', leaves = ['-', '-'];
        memory = new Uint8Array(api.memory.buffer);
        memory.set(c.frame, scratch);
        const pagesBefore = api.memory.buffer.byteLength;
        const prepared = lane(api.spx_pg_v1_input_prepare(opened.value, scratch, c.frame.length));
        let trapped = false;
        if (prepared.status !== 0) {
            primary = prepared.status;
            notes.push(`effects=${api.memory.buffer.byteLength === pagesBefore ? 0 : 1}`, `input=${prepared.value !== 0 ? 1 : 0}`);
        } else {
            let called;
            try {
                called = lane(api.spx_pg_v1_call(opened.value, prepared.value));
            } catch (error) {
                trapped = true;
                primary = 'trap';
                notes.push(`trap=${String(error.message).replace(/[^A-Za-z0-9]+/g, '_')}`);
            }
            if (!trapped && called.status !== 0) {
                primary = called.status;
                // Core Wasm retains a failed call's input until explicit release.
                const again = lane(api.spx_pg_v1_call(opened.value, prepared.value));
                notes.push(`dup=${again.status}`, `release=${api.spx_pg_v1_value_release(prepared.value)}`);
            } else if (!trapped) {
                const again = lane(api.spx_pg_v1_call(opened.value, prepared.value));
                const probe = lane(api.spx_pg_v1_result_export(called.value, scratch, 0));
                let exported = probe.status;
                if (probe.status === 12) {
                    if (c.kind === 2) {
                        memory = new Uint8Array(api.memory.buffer);
                        memory.fill(0xa5, scratch, scratch + probe.value);
                        const short = lane(api.spx_pg_v1_result_export(called.value, scratch, probe.value - 1));
                        memory = new Uint8Array(api.memory.buffer);
                        const untouched = memory.slice(scratch, scratch + probe.value).every(byte => byte === 0xa5);
                        notes.push(`short=${short.status}`, `untouched=${untouched ? 1 : 0}`);
                    }
                    const copied = lane(api.spx_pg_v1_result_export(called.value, scratch, probe.value));
                    exported = copied.status;
                    memory = new Uint8Array(api.memory.buffer);
                    const frame = memory.slice(scratch, scratch + copied.value);
                    if (copied.status === 0) leaves = [`frame:${hex(frame)}`, 'frame'];
                    if (c.kind === 2 && copied.status === 0) {
                        const retry = lane(api.spx_pg_v1_result_export(called.value, scratch, probe.value));
                        memory = new Uint8Array(api.memory.buffer);
                        const same = retry.value === copied.value && memory.slice(scratch, scratch + retry.value).every((byte, index) => byte === frame[index]);
                        notes.push(`retry=${retry.status}`, `same=${same ? 1 : 0}`);
                    }
                }
                primary = exported;
                secondary = api.spx_pg_v1_result_release(called.value);
                const stale = lane(api.spx_pg_v1_result_export(called.value, scratch, 64));
                notes.push(`dup=${again.status}`, `stale=${stale.status}`);
            }
        }
        let live = '-';
        if (!trapped && cycle + 1 === cycles) {
            const closed = api.spx_pg_v1_provider_close(opened.value);
            live = closed === 0 ? 0 : 1;
            notes.push(`close=${closed}`);
        }
        if (trapped) cycle = cycles;
        process.stdout.write(`${label} ${primary} ${secondary} - ${live} - - ${leaves[0]} ${leaves[1]} ${notes.join(',') || '-'}\n`);
    }
}
