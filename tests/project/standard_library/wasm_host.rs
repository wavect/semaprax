pub(super) fn wasm_conformance_js(
    wasm_filename: &str,
    live_entry_bound: usize,
    expected_value: i64,
    expected_peak: Option<usize>,
) -> String {
    format!(
        r#"import assert from "node:assert/strict";
import {{ readFile }} from "node:fs/promises";
const bytes = await readFile("./{}");
const checked = (operation) => (a, b) => {{ const value = operation(a, b); if (value < -(1n<<63n) || value > (1n<<63n)-1n) throw new RangeError(); return value; }};
let peak = 0; const entries = new Map(); let next = 1; const boxes = new Map(); let nextBox = 1n; let linked;
const decode = carrier => {{ const word = BigInt.asUintN(64, carrier), length = Number(word & 0xffffffffn), root = Number((word >> 32n) & 0xffffffffn); return {{ word, length, root, tagged: (root & 0x80000000) !== 0, token: root & 0x7fffffff }}; }};
const read = decoded => {{ if ((decoded.root & 0xc0000000) === 0x40000000) {{ const pointer = (decoded.root & 0xffff) * 8, key = (decoded.root >>> 16) & 0x1fff, view = new DataView((linked.instance.exports.__spx_byte_memory ?? linked.instance.exports.memory).buffer); if (pointer + 32 > view.byteLength || view.getUint32(pointer, true) !== key || view.getUint32(pointer + 4, true) !== pointer || Number(view.getBigUint64(pointer + 24, true)) !== decoded.length) throw new Error("corrupt range descriptor"); const root = view.getBigInt64(pointer + 8, true), offset = Number(view.getBigUint64(pointer + 16, true)), all = read(decode(root)); if (offset > all.length || decoded.length > all.length - offset) throw new Error("byte range"); return all.slice(offset, offset + decoded.length); }} if (decoded.tagged) {{ const value = entries.get(decoded.token); if (!(value instanceof Uint8Array) || value.length !== decoded.length) throw new Error("stale byte token"); return value; }} const memory = new Uint8Array((linked.instance.exports.__spx_byte_memory ?? linked.instance.exports.memory).buffer); if (decoded.root > memory.length - decoded.length) throw new Error("byte range"); return memory.slice(decoded.root, decoded.root + decoded.length); }};
const allocate = bytes => {{ if (entries.size >= {}) throw new Error("owned Bytes live entry limit exceeded"); const token = next++, owned = new Uint8Array(bytes); entries.set(token, owned); peak = Math.max(peak, entries.size); return BigInt.asIntN(64, ((0x80000000n | BigInt(token)) << 32n) | BigInt(owned.length)); }};
const boxKey = value => {{ if (typeof value !== "bigint" || value === 0n) throw new Error("invalid Box carrier"); return value.toString(); }};
const readBox = (value, tag) => {{ const entry = boxes.get(boxKey(value)); if (!entry || entry.tag !== tag) throw new Error("stale or mistyped Box"); return entry; }};
const setBytes = (carrier, index, values) => {{ const decoded = decode(carrier), target = read(decoded); if (!decoded.tagged || typeof index !== "bigint" || index < 0n || index > BigInt(target.length) || BigInt(target.length) - index < BigInt(values.length) || !values.every(value => Number.isInteger(value) && value >= 0 && value <= 255)) throw new Error("owned byte buffer interval invariant"); target.set(values, Number(index)); return BigInt.asIntN(64, decoded.word); }};
const setChoice = (carrier, index, one, sourceCarrier, selector, wideWidth, extended) => {{ if (typeof selector !== "bigint" || !Number.isInteger(one) || one < 0 || one > 255) throw new Error("owned byte buffer choice invariant"); const bits = BigInt.asUintN(64, selector), copy = (bits & (1n << 63n)) !== 0n, wide = extended && (bits & (1n << 62n)) !== 0n, start = bits & ((1n << (extended ? 62n : 63n)) - 1n), source = read(decode(sourceCarrier)), width = copy ? (wide ? 48 : wideWidth) : 1, values = copy ? Array.from({{length: width}}, (_, offset) => {{ const at = start + BigInt(offset); return at < BigInt(source.length) ? source[Number(at)] : 0; }}) : [one]; return setBytes(carrier, index, values); }};
const imports = {{env:{{spx_add:checked((a,b)=>a+b),spx_sub:checked((a,b)=>a-b),spx_mul:checked((a,b)=>a*b),spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:(a)=>-a,spx_contract_fail:()=>{{throw new Error();}},
spx_bytes_copy:c=>allocate(read(decode(c))),spx_bytes_get:(c,i)=>{{ const b = read(decode(c)), u = BigInt.asUintN(64, i); return u >= BigInt(b.length) ? -1 : b[Number(u)]; }},spx_bytes_drop:c=>{{ const d = decode(c); read(d); entries.delete(d.token); }},spx_bytes_as_slice:c=>{{ const d = decode(c); read(d); return BigInt.asIntN(64, d.word); }},spx_bytes_zeroed:count=>{{ if (typeof count !== "bigint" || count < 0n || count > 131072n) throw new Error("owned byte buffer capacity invariant"); return allocate(new Uint8Array(Number(count))); }},spx_bytes_set:(c,i,v)=>{{ const d = decode(c), b = read(d); if (typeof i !== "bigint" || i < 0n || i >= BigInt(b.length) || !Number.isInteger(v) || v < 0 || v > 255) throw new Error("owned byte buffer element invariant"); b[Number(i)] = v; return BigInt.asIntN(64, d.word); }},spx_bytes_set5:(c,i,a,b,d,e,f)=>setBytes(c,i,[a,b,d,e,f]),spx_bytes_set1_or5:(c,i,one,source,selector)=>setChoice(c,i,one,source,selector,5,false),spx_bytes_set1_or6_or48:(c,i,one,source,selector)=>setChoice(c,i,one,source,selector,6,true),spx_box_new:(tag,bits)=>{{ if (boxes.size >= 4096) return 0n; const token=nextBox++; boxes.set(boxKey(token),{{tag,bits}}); return token; }},spx_box_get:(value,tag)=>readBox(value,tag).bits,spx_box_into_inner:(value,tag)=>{{ const entry=readBox(value,tag); boxes.delete(boxKey(value)); return entry.bits; }},spx_box_drop:value=>{{ if (!boxes.delete(boxKey(value))) throw new Error("double Box drop"); }}}}}};
linked = await WebAssembly.instantiate(bytes, imports);
// Re-entry observes an owned buffer that outlived one call as a live entry.
// Issue #102: assert against the *other backends'* actual computed value
// (`expected_value`, the interpreter's real `Returned(n)`), not a hardcoded
// `0n` literal every backend could vacuously agree on independent of what
// it actually computed.
for (let r = 0; r < 4; ++r) {{ assert.equal(linked.instance.exports.semaprax_main(), {}n); assert.equal(entries.size, 0); assert.equal(boxes.size, 0); }}
{}
"#,
        wasm_filename,
        live_entry_bound,
        expected_value,
        expected_peak
            .map(|bound| format!("assert.equal(peak, {bound});"))
            .unwrap_or_default()
    )
}
