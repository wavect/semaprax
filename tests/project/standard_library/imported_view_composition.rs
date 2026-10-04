//! Issue #103: one bounded imported-package view composition.
//!
//! The entry module imports offset helpers from the real `std.bytes` Project,
//! then applies them to a local borrowed view.  The hand-computed offset and
//! byte oracle is compared across the Project interpreter, native C11 O0/O2,
//! and Core Wasm routes.

use std::path::Path;
use std::process::Command;

use semaprax::{codegen, project, wasm};

use super::temporary::temporary;

const MANIFEST: &str = r#"schema = "semaprax.manifest.v1"

[package]
name = "imported-view-composition"
version = "0.1.0"
profile = "useful-data.v1"

[modules]
entry = "consumer.app"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = ["consumer.probe"]

[dependencies]
std.bytes = "=0.1.0"
"#;

const APP: &str = r#"module consumer.app;
use function @id("std.bytes.field_start") from std.bytes as field_start;
use function @id("std.bytes.field_end") from std.bytes as field_end;

@id("consumer.probe")
fn probe(view: borrow Slice<u8>) -> i64
{
    let start = field_start(view, 0usize, 44u8);
    let end = field_end(view, start, 44u8);
    let sub = byte_range(view, start, end);
    let first = match byte_get(sub, 0usize) { Option::Some { value } => value, Option::None {} => 0u8, };
    let second = match byte_get(sub, 1usize) { Option::Some { value } => value, Option::None {} => 0u8, };
    if start == 2usize && end == 3usize && first == 98u8 && second == 0u8 { 328 } else { -1 }
}

@id("consumer.main")
fn main() -> i64
{
    let data = [97u8, 44u8, 98u8, 44u8, 99u8];
    probe(array_as_slice(data))
}
"#;

const TESTS: &str =
    "module consumer.tests;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    0\n}\n";
const EXPECTED: i64 = 328;

fn assert_backend_value(backend: &str, expected: i64, actual: i64) {
    assert_eq!(
        actual, expected,
        "{backend} disagrees with the independent offset/byte oracle"
    );
}

fn node_value(path: &Path, wasm_path: &Path) -> i64 {
    let script = path.with_extension("mjs");
    // The Wasm byte ABI materializes local arrays as owned temporary carriers.
    // Validate both those owners and borrowed range descriptors, and require
    // every temporary to be released after the scalar result is returned.
    const SCRIPT: &str = r#"import {readFile} from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
let linked;
const entries = new Map();
let next = 1;
const read = (carrier, depth = 0) => {
    if (depth > 32) throw Error("descriptor cycle");
    const word = BigInt.asUintN(64, carrier);
    const length = Number(word & 0xffffffffn);
    const root = Number(word >> 32n);
    const memory = (linked.instance.exports.__spx_byte_memory ?? linked.instance.exports.memory).buffer;
    if ((root & 0xc0000000) === 0x40000000) {
        const pointer = (root & 0xffff) * 8;
        const key = (root >>> 16) & 0x1fff;
        const view = new DataView(memory);
        if (pointer + 32 > view.byteLength || view.getUint32(pointer, true) !== key ||
            view.getUint32(pointer + 4, true) !== pointer ||
            view.getBigUint64(pointer + 24, true) !== BigInt(length)) throw Error("range descriptor");
        const offset = view.getBigUint64(pointer + 16, true);
        const original = read(view.getBigInt64(pointer + 8, true), depth + 1);
        if (offset > BigInt(original.length) || BigInt(length) > BigInt(original.length) - offset) throw Error("range bounds");
        return original.subarray(Number(offset), Number(offset) + length);
    }
    if ((root & 0x80000000) !== 0) {
        const value = entries.get(root & 0x7fffffff);
        if (!(value instanceof Uint8Array) || value.length !== length) throw Error("stale owned carrier");
        return value;
    }
    if (root > memory.byteLength || length > memory.byteLength - root) throw Error("memory bounds");
    return new Uint8Array(memory, root, length);
};
const checked = f => (...args) => { const value = f(...args); if (value < -(1n<<63n) || value > (1n<<63n)-1n) throw new RangeError(); return value; };
const allocate = bytes => {
    if (entries.size >= 64 || next >= 0x40000000) throw Error("owner capacity");
    const token = next++;
    const value = new Uint8Array(bytes);
    entries.set(token, value);
    return BigInt.asIntN(64, ((0x80000000n | BigInt(token)) << 32n) | BigInt(value.length));
};
const owner = carrier => {
    const root = Number(BigInt.asUintN(64,carrier) >> 32n);
    if ((root & 0x80000000) === 0) throw Error("borrow used as owner");
    return root & 0x7fffffff;
};
const env = {
    spx_add: checked((a,b)=>a+b), spx_sub: checked((a,b)=>a-b), spx_mul: checked((a,b)=>a*b),
    spx_div: checked((a,b)=>a/b), spx_rem: (a,b)=>a%b, spx_neg: checked(a=>-a),
    spx_contract_fail: () => { throw Error("contract"); },
    spx_bytes_copy: carrier => allocate(read(carrier)),
    spx_bytes_drop: carrier => { read(carrier); if (!entries.delete(owner(carrier))) throw Error("double drop"); },
    spx_bytes_as_slice: carrier => { read(carrier); return carrier; },
    spx_bytes_zeroed: length => { if (length < 0n || length > 131072n) throw Error("byte capacity"); return allocate(new Uint8Array(Number(length))); },
    spx_bytes_set: (carrier,index,value) => { owner(carrier); const bytes = read(carrier); if (index < 0n || index >= BigInt(bytes.length) || !Number.isInteger(value) || value < 0 || value > 255) throw Error("byte element"); bytes[Number(index)] = value; return carrier; },
    spx_bytes_set5: () => { throw Error("unexpected five-byte write"); },
    spx_bytes_set1_or5: () => { throw Error("unexpected one-or-five write"); },
    spx_bytes_set1_or6_or48: () => { throw Error("unexpected one-or-six-or-forty-eight write"); },
    spx_bytes_get: (carrier,index) => { const value = read(carrier); const i = BigInt.asUintN(64,index); return i >= BigInt(value.length) ? -1 : value[Number(i)]; }
};
linked = await WebAssembly.instantiate(bytes, {env});
const value = linked.instance.exports.semaprax_main();
if (entries.size !== 0) throw Error("owned temporary leaked");
console.log(value.toString());
"#;
    std::fs::write(&script, SCRIPT).unwrap();
    let output = Command::new("node")
        .arg(script.file_name().unwrap())
        .arg(wasm_path.file_name().unwrap())
        .current_dir(path.parent().unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Core Wasm failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap()
}

#[test]
#[cfg_attr(windows, ignore = "native C11 command-line fixture is Unix-only")]
fn imported_std_bytes_view_composition_agrees_across_project_backends() {
    let root = temporary("imported-view-composition");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
    std::fs::write(root.join("src/app.spx"), APP).unwrap();
    std::fs::write(root.join("src/tests.spx"), TESTS).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        let interpreter = match snapshot.execute_entry(&options)?.outcome() {
            project::ProjectExecutionOutcome::Returned(value) => *value,
            other => panic!("imported view interpreter failed: {other:?}"),
        };
        assert_backend_value("Project interpreter", EXPECTED, interpreter);
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            let binary = root.join(format!("native{}", optimization));
            let c_path = binary.with_extension("c");
            std::fs::write(&c_path, &c).unwrap();
            let compiled = Command::new("clang")
                .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                .arg(&c_path)
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "clang {optimization} failed: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let output = Command::new(&binary).output().unwrap();
            assert!(output.status.success());
            let native: i64 = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse()
                .unwrap();
            assert_backend_value(&format!("native {optimization}"), interpreter, native);
        }
        let wasm_bytes =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        let wasm_path = root.join("imported.wasm");
        std::fs::write(&wasm_path, wasm_bytes).unwrap();
        assert_backend_value(
            "Core Wasm",
            interpreter,
            node_value(&root.join("imported"), &wasm_path),
        );
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[should_panic(expected = "Core Wasm disagrees with the independent offset/byte oracle")]
fn imported_view_oracle_rejects_a_perturbed_backend_value() {
    assert_backend_value("Core Wasm", EXPECTED, EXPECTED + 1);
}
