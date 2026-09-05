//! Scalar Core-Wasm local-index and nested-expression regressions.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{format, graph, hir, interpreter, parse, verify, wasm};
use wasmparser::Validator;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn module(source: &str) -> Vec<u8> {
    let program = parse(source, Path::new("scalar-local-layout.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert!(graph::to_json(&program).unwrap().contains("\"app.main\""));
    let bytes = wasm::emit_resolved_module(&hir::resolve(&program).unwrap()).unwrap();
    assert_eq!(bytes, wasm::emit_module(&reparsed).unwrap());
    Validator::new().validate_all(&bytes).unwrap();
    bytes
}

fn execute(source: &str) {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-scalar-local-layout-{}-{id}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let source_path = directory.join("case.spx");
    let wasm_path = directory.join("case.wasm");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&source_path)
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&wasm_path)
        .unwrap()
        .write_all(&module(source))
        .unwrap();

    let interpreted = interpreter::interpret(
        &source_path,
        "app.main",
        &[],
        &interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    assert!(interpreted.returned, "{}", interpreted.envelope);
    assert!(interpreted
        .envelope
        .contains("\"outcome\":{\"kind\":\"returned\",\"type\":\"i64\",\"value\":\"7\"}"));

    let script = r#"
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const env = Object.fromEntries([
  'spx_add', 'spx_sub', 'spx_mul', 'spx_div', 'spx_rem', 'spx_neg', 'spx_contract_fail',
].map(name => [name, () => { throw new Error(`unexpected host import ${name}`); }]));
const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[1]), { env });
for (let repeat = 0; repeat < 3; repeat++) assert.equal(instance.exports.semaprax_main(), 7n);
"#;
    let result = Command::new("node")
        .args(["--input-type=module", "--eval", script])
        .arg(&wasm_path)
        .output()
        .expect("Node is required for the scalar local-layout gate");
    assert!(
        result.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());

    assert_eq!(fs::read(&source_path).unwrap(), source.as_bytes());
    fs::remove_file(source_path).unwrap();
    fs::remove_file(wasm_path).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn parameter_offsets_keep_all_scalar_scratch_disjoint_from_parameters_and_lets() {
    execute(
        r#"
module test.scalar_offsets;

@id("case.i32_binary")
fn i32_binary(left: i32, right: i32) -> i32 {
    let first = left + right;
    let second = left * 2i32;
    first + second
}

@id("case.i32_neg")
fn i32_neg(left: i32, right: i32) -> i32 {
    let first = left + right;
    let second = -left;
    first + second
}

@id("case.u8")
fn u8_value(left: u8, right: u8) -> u8 {
    let first = left + right;
    let second = left + 1u8;
    first + second
}

@id("case.usize")
fn usize_value(left: usize, right: usize) -> usize {
    let first = left + right;
    let second = left + 1usize;
    first + second
}

@id("app.main")
fn main() -> i64 {
    if i32_binary(2i32, 3i32) == 9i32
        && i32_neg(2i32, 3i32) == 3i32
        && u8_value(1u8, 2u8) == 5u8
        && usize_value(1usize, 2usize) == 5usize
    { 7 } else { 9 }
}
"#,
    );
}
