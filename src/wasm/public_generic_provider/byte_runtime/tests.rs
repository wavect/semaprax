use super::*;
use wasm_encoder::{
    ExportKind, ExportSection, GlobalSection, GlobalType, MemorySection, MemoryType, Module,
    RawSection, ValType,
};

#[test]
fn import_free_heap_charges_each_allocation_through_drop_and_resets_for_reentry() {
    let heap = Heap {
        table: 0,
        data: 65_536,
        resolve: 1,
    };
    let mut module = Module::new();
    let mut types = vec![4, 0x60, 1, 0x7e, 0, 0x60, 0, 0];
    append_type_entries(&mut types);
    module.section(&RawSection {
        id: 1,
        data: &types,
    });
    module.section(&RawSection {
        id: 3,
        data: &[4, 0, 2, 3, 1], // drop, resolve, alloc, reset
    });
    let pages = u64::from(heap.data + HEAP_DATA_BYTES).div_ceil(65_536);
    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: pages,
        maximum: Some(pages),
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memory);
    let mut globals = GlobalSection::new();
    for _ in 0..=GLOBAL_HEAP_NEXT {
        globals.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &wasm_encoder::ConstExpr::i32_const(0),
        );
    }
    module.section(&globals);
    let mut exports = ExportSection::new();
    for (name, index) in [("drop", 0), ("resolve", 1), ("alloc", 2), ("reset", 3)] {
        exports.export(name, ExportKind::Func, index);
    }
    module.section(&exports);

    // Emit the actual provider helpers and extract its ordinary drop body.
    // Their function indices are the indices in this test module.
    let mut runtime = vec![6];
    runtime_bodies(&mut runtime, heap);
    let reader =
        wasmparser::CodeSectionReader::new(wasmparser::BinaryReader::new(&runtime, 0)).unwrap();
    let drop_body = reader.into_iter().nth(2).unwrap().unwrap();
    let mut code = vec![4];
    u32_leb(&mut code, drop_body.as_bytes().len() as u32);
    code.extend(drop_body.as_bytes());
    helper_bodies(&mut code, heap);
    let mut reset = vec![0];
    emit_register_inputs(&mut reset, heap, 32_768, 32_784);
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
assert.deepEqual(WebAssembly.Module.imports(module), []);
const { alloc, drop, resolve, reset } = new WebAssembly.Instance(module).exports;
const trap = WebAssembly.RuntimeError;
reset();
for (let i = 0; i < 32; i++) {
  const token = alloc(0);
  assert.equal(token, BigInt.asIntN(64, (0x80000003n + BigInt(i)) << 32n));
  drop(token);
  assert.throws(() => resolve(token), trap);
}
assert.throws(() => alloc(0), trap);
reset();
for (let i = 0; i < 16; i++) drop(alloc(131072));
assert.throws(() => alloc(1), trap);
const empty = alloc(0);
assert.equal(empty, BigInt.asIntN(64, 0x80000013n << 32n));
assert.equal(resolve(empty), 65536 + 2097152);
drop(empty);
reset();
drop(alloc(131072));
"#;
    let output = std::process::Command::new("node")
        .args(["-e", script, &hex])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}
