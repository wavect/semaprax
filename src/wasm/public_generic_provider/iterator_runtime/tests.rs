use super::*;
use wasm_encoder::{
    ExportKind, ExportSection, FunctionSection, MemorySection, MemoryType, Module, RawSection,
    TypeSection,
};

#[test]
fn iterator_transfers_one_carrier_and_drops_only_the_suffix() {
    let runtime = Runtime {
        table: 65_536,
        resolve_bytes: 0,
        drop_bytes: 1,
    };
    let mut module = Module::new();
    let mut types = TypeSection::new();
    use ValType::{I32, I64};
    for (params, results) in [
        (&[I64][..], &[I32][..]),
        (&[I64][..], &[][..]),
        (&[I32, I64][..], &[I64][..]),
        (&[I64, I32, I64][..], &[I64][..]),
        (&[I64, I32][..], &[I64][..]),
        (&[I64, I32][..], &[I32][..]),
        (&[I64, I64, I32][..], &[I32][..]),
        (&[I64, I64][..], &[][..]),
        (&[][..], &[][..]),
    ] {
        types
            .ty()
            .function(params.iter().copied(), results.iter().copied());
    }
    module.section(&types);
    let mut functions = FunctionSection::new();
    for index in [0, 1, 2, 3, 4, 4, 1, 5, 6, 7, 8] {
        functions.function(index);
    }
    module.section(&functions);
    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 2,
        maximum: Some(2),
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memory);
    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    for (index, name) in [
        "make", "push", "len", "capacity", "dropVec", "into", "next", "dropIter", "reset",
    ]
    .into_iter()
    .enumerate()
    {
        exports.export(name, ExportKind::Func, index as u32 + 2);
    }
    module.section(&exports);
    let mut code = vec![11];
    // Record each drop by its carrier's low word in the first page.
    let mut resolve = Function::new([]);
    emit(&mut resolve, &[I::I32Const(0)]);
    finish(&mut code, resolve);
    let mut drop = Function::new([]);
    emit(
        &mut drop,
        &[
            I::LocalGet(0),
            I::I32WrapI64,
            I::I32Const(1),
            I::I32Store8(mem(0, 0)),
        ],
    );
    finish(&mut code, drop);
    runtime.bodies(&mut code);
    let mut reset = Function::new([]);
    runtime.reset(&mut reset);
    finish(&mut code, reset);
    module.section(&RawSection {
        id: 10,
        data: &code,
    });
    let bytes = module.finish();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    if !std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        return;
    }
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let script = r#"
const assert = require('node:assert/strict');
const module = new WebAssembly.Module(Buffer.from(process.argv[1], 'hex'));
const {memory, make, push, len, capacity, dropVec, into, next, dropIter, reset}
    = new WebAssembly.Instance(module).exports;
const m = new DataView(memory.buffer), out = 4096;
const a = -(1n << 63n) + 64n, b = a + 1n;
reset();
assert.equal(make(9, 33n), 0n);
const vector = make(9, 2n);
const one = push(vector, 9, a);
assert.throws(() => len(vector, 9), WebAssembly.RuntimeError);
const two = push(one, 9, b);
assert.equal(len(two, 9), 2n);
assert.equal(capacity(two, 9), 2n);
assert.equal(push(two, 9, a), 0n);
assert.equal(into(two, -1), 2);
assert.equal(into(two, out), 0);
const iter = m.getBigUint64(out, true);
assert.throws(() => dropVec(two), WebAssembly.RuntimeError);
m.setUint8(out, 165);
assert.equal(next(iter, 1n, out), 2);
assert.equal(m.getUint8(out), 165);
assert.equal(next(iter, 0n, out), 0);
assert.equal(m.getBigUint64(out, true), 1n);
assert.equal(m.getBigInt64(out + 8, true), a);
const rest = m.getBigUint64(out + 16, true);
assert.equal(m.getBigUint64(out + 24, true), 1n);
assert.equal(next(iter, 0n, out), 2);
dropIter(rest, 1n);
assert.equal(m.getUint8(64), 0);
assert.equal(m.getUint8(65), 1);
assert.throws(() => dropIter(rest, 1n), WebAssembly.RuntimeError);
const empty = make(9, 0n);
assert.equal(into(empty, out), 0);
assert.equal(next(m.getBigUint64(out, true), 0n, out), 0);
assert.deepEqual([...new Uint8Array(memory.buffer, out, 32)], Array(32).fill(0));
reset();
const fresh = make(9, 0n);
assert.notEqual(fresh, vector);
assert.throws(() => len(vector, 9), WebAssembly.RuntimeError);
dropVec(fresh);
const active = Array.from({length: 32}, () => make(9, 0n));
assert.equal(new Set(active).size, 32);
assert.ok(active.every(handle => handle !== 0n));
assert.equal(make(9, 0n), 0n);
dropVec(active[0]);
const replacement = make(9, 0n);
assert.notEqual(replacement, active[0]);
assert.throws(() => dropVec(active[0]), WebAssembly.RuntimeError);
dropVec(replacement);
for (const handle of active.slice(1)) dropVec(handle);
"#;
    let output = std::process::Command::new("node")
        .args(["-e", script, &hex])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}
