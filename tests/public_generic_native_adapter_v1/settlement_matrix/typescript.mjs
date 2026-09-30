// Generated TypeScript caller driver (compiled Core Wasm route). The generated
// package owns framing, admission and lifecycle; a transparent Proxy counts
// the physical exports it invokes without replacing any of them.
import { readFileSync } from 'node:fs';
import { Provider, SemapraxPublicGenericException } from '../dist/index.js';
const [names, cases] = JSON.parse(readFileSync('test/matrix.json', 'utf8'));
const wasm = readFileSync(process.argv[2]);
const originalInstantiate = WebAssembly.instantiate;
let observed;
WebAssembly.instantiate = async (...args) => {
    const instance = await Reflect.apply(originalInstantiate, WebAssembly, args);
    const measured = { spx_pg_v1_call: 'calls', spx_pg_v1_value_release: 'valueReleases', spx_pg_v1_result_release: 'resultReleases', spx_pg_v1_provider_close: 'closes' };
    return { exports: new Proxy({}, {
        get(_target, property) {
            const original = Reflect.get(instance.exports, property);
            if (!Object.hasOwn(measured, property)) return original;
            return (...parameters) => {
                observed[measured[property]] += 1;
                const result = Reflect.apply(original, instance.exports, parameters);
                if (property === 'spx_pg_v1_provider_close') observed.close = result;
                else if (property !== 'spx_pg_v1_call' && result !== 0) observed.releaseFailure = result;
                return result;
            };
        },
    }) };
};
const hex = bytes => bytes.length === 0 ? 'e' : Buffer.from(bytes).toString('hex');
try {
    for (const c of cases) {
        observed = { calls: 0, valueReleases: 0, resultReleases: 0, closes: 0, close: -1, releaseFailure: 0 };
        const provider = await Provider.open(wasm);
        const cycles = c.kind === 1 ? c.cycles : 1;
        for (let cycle = 0; cycle < cycles; cycle += 1) {
            const label = c.kind === 1 ? `${c.id}#${cycle}` : c.id;
            const input = { [names[0]]: Uint8Array.from(c.left), [names[1]]: Uint8Array.from(c.right) };
            let primary = 0, leaves = ['-', '-'], kind = 'ok';
            try {
                const result = provider.transform(input);
                leaves = [hex(result[names[0]]), hex(result[names[1]])];
            } catch (error) {
                if (error instanceof SemapraxPublicGenericException) {
                    kind = error.detail.kind;
                    primary = error.detail.status ?? -1;
                } else {
                    kind = 'trap';
                    primary = 'trap';
                }
            }
            let live = '-';
            if (cycle + 1 === cycles || primary === 'trap') {
                try { provider.close(); } catch { /* reported through observed.close */ }
                live = observed.close === 0 ? 0 : 1;
            }
            const note = `kind=${kind},vr=${observed.valueReleases},rr=${observed.resultReleases},closes=${observed.closes}`;
            process.stdout.write(`${label} ${primary} ${observed.releaseFailure} ${observed.calls} ${live} - - ${leaves[0]} ${leaves[1]} ${note}\n`);
            if (primary === 'trap') break;
        }
    }
} finally {
    WebAssembly.instantiate = originalInstantiate;
}
