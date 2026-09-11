//! The offline agent-response example carried end to end.
//!
//! `examples/agent-response-project` is the first example project that takes a
//! bundled `[dependencies]` edge on a standard-library package, so this module
//! proves the whole closure - the project plus the vendored
//! `std.data.json.doc` source - checks, formats canonically, and returns `0`
//! from both its entry and its conformance module on the interpreter, on
//! native C11 at `-O0` and `-O2`, and on Core Wasm under Node.
//!
//! [Bounded JSON Scanner v1] owns the JSON contract the project consumes.
//!
//! [Bounded JSON Scanner v1]: ../../docs/BOUNDED-JSON-SCANNER-V1.md

use std::path::{Path, PathBuf};
use std::process::Command;

use semaprax::{codegen, format, parse, project};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/agent-response-project")
}

fn scratch(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-agent-response-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path.canonicalize().unwrap()
}

fn compile_c(source: &str, output: &Path, optimization: &str) {
    let c_path = output.with_extension("c");
    std::fs::write(&c_path, source).unwrap();
    let result = Command::new("clang")
        .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
        .arg(&c_path)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "clang {optimization} failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn run_returns_zero(path: &Path) {
    let output = Command::new(path).output().unwrap();
    assert!(output.status.success(), "{} failed", path.display());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "0",
        "{} did not report success",
        path.display()
    );
}

/// Every module of the example is canonical, so `semaprax fmt --check` over the
/// project cannot start passing because a source drifted out of the gate.
#[test]
fn every_module_is_canonical() {
    for source in [
        "src/app.spx",
        "src/report.spx",
        "src/scan.spx",
        "src/tests.spx",
        "src/verdict.spx",
    ] {
        let path = fixture().join(source);
        let text = std::fs::read_to_string(&path).unwrap();
        let program = parse(&text, &path).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            format::canonical(&program),
            text,
            "{source} is not canonical"
        );
    }
}

/// The manifest takes the bundled standard dependency, and the workspace the
/// compiler authenticates carries that package's immutable source rather than
/// anything the project vendored by hand.
#[test]
fn the_project_links_the_bundled_json_document_layer() {
    project::with_authenticated_project(&fixture().join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.data.json.doc/0.1.0/doc.spx"));
        let public = snapshot.retain_revision();
        let ids = public
            .public_api_program()
            .functions
            .iter()
            .map(|function| function.id.clone())
            .collect::<Vec<_>>();
        for selected in snapshot.manifest().web_exports() {
            assert!(
                ids.iter().any(|id| id.as_str() == selected.as_str()),
                "missing selected export root {selected}"
            );
            assert!(snapshot.semantic_graph().contains(selected));
        }
        Ok(())
    })
    .unwrap();
}

/// Entry and conformance both return `0` on all three execution lanes.
#[test]
fn entry_and_conformance_return_zero_on_interpreter_native_and_wasm() {
    let scratch = scratch("lanes");
    project::with_authenticated_project(&fixture().join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0),
            "the entry failed on the interpreter"
        );
        assert_eq!(
            snapshot.execute_test(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0),
            "conformance failed on the interpreter"
        );
        for (role, program) in [
            ("entry", snapshot.entry_program()),
            ("tests", snapshot.test_program()),
        ] {
            let c = codegen::emit_hir_c(program).map_err(|error| vec![error])?;
            for optimization in ["-O0", "-O2"] {
                let binary = scratch.join(format!("{role}{}", optimization.to_lowercase()));
                compile_c(&c, &binary, optimization);
                run_returns_zero(&binary);
            }
        }
        let wasm = snapshot.test_wasm_module()?;
        let wasm_path = scratch.join("tests.wasm");
        std::fs::write(&wasm_path, wasm).unwrap();
        let script = scratch.join("tests.mjs");
        std::fs::write(
            &script,
            r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
const bytes = await readFile("./tests.wasm");
const checked = (operation) => (a, b) => { const value = operation(a, b); if (value < -(1n<<63n) || value > (1n<<63n)-1n) throw new RangeError(); return value; };
const entries = new Map(); let next = 1; let linked;
const decode = carrier => { const word = BigInt.asUintN(64, carrier), length = Number(word & 0xffffffffn), root = Number((word >> 32n) & 0xffffffffn); return { word, length, root, tagged: (root & 0x80000000) !== 0, token: root & 0x7fffffff }; };
const read = decoded => { if ((decoded.root & 0xc0000000) === 0x40000000) { const pointer = (decoded.root & 0xffff) * 8, key = (decoded.root >>> 16) & 0x1fff, view = new DataView((linked.instance.exports.__spx_byte_memory ?? linked.instance.exports.memory).buffer); if (pointer + 32 > view.byteLength || view.getUint32(pointer, true) !== key || view.getUint32(pointer + 4, true) !== pointer || Number(view.getBigUint64(pointer + 24, true)) !== decoded.length) throw new Error("corrupt range descriptor"); const root = view.getBigInt64(pointer + 8, true), offset = Number(view.getBigUint64(pointer + 16, true)), all = read(decode(root)); if (offset > all.length || decoded.length > all.length - offset) throw new Error("byte range"); return all.slice(offset, offset + decoded.length); } if (decoded.tagged) { const value = entries.get(decoded.token); if (!(value instanceof Uint8Array) || value.length !== decoded.length) throw new Error("stale byte token"); return value; } const memory = new Uint8Array((linked.instance.exports.__spx_byte_memory ?? linked.instance.exports.memory).buffer); if (decoded.root > memory.length - decoded.length) throw new Error("byte range"); return memory.slice(decoded.root, decoded.root + decoded.length); };
const allocate = bytes => { const token = next++, owned = new Uint8Array(bytes); entries.set(token, owned); return BigInt.asIntN(64, ((0x80000000n | BigInt(token)) << 32n) | BigInt(owned.length)); };
const imports = {env:{spx_add:checked((a,b)=>a+b),spx_sub:checked((a,b)=>a-b),spx_mul:checked((a,b)=>a*b),spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:(a)=>-a,spx_contract_fail:()=>{throw new Error();},
spx_bytes_copy:c=>allocate(read(decode(c))),spx_bytes_get:(c,i)=>{ const b = read(decode(c)), u = BigInt.asUintN(64, i); return u >= BigInt(b.length) ? -1 : b[Number(u)]; },spx_bytes_drop:c=>{ const d = decode(c); read(d); entries.delete(d.token); },spx_bytes_as_slice:c=>{ const d = decode(c); read(d); return BigInt.asIntN(64, d.word); }}};
linked = await WebAssembly.instantiate(bytes, imports);
assert.equal(linked.instance.exports.semaprax_main(), 0n);
"#,
        )
        .unwrap();
        let node = Command::new("node")
            .arg(script.file_name().unwrap())
            .current_dir(&scratch)
            .output()
            .unwrap();
        assert!(
            node.status.success(),
            "Node conformance closure failed: {}",
            String::from_utf8_lossy(&node.stderr)
        );
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}
