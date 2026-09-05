//! Ordinary Core-Wasm arithmetic, separately from the aggregate status lane.
//! These authored fixtures require Node; they do not run a native compiler.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{format, graph, hir, parse, verify, wasm};
use wasmparser::{Parser, Payload, Validator};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.usize_multiplication;
@id("math.multiply")
fn multiply(left: usize, right: usize) -> usize { left * right }
@id("app.main")
fn main() -> i64 { MAIN_BODY }
"#;

const SUCCESS: &str = r#"
    let maximum = 18446744073709551615usize;
    if multiply(0usize, 0usize) == 0usize
        && multiply(maximum, 0usize) == 0usize
        && multiply(0usize, maximum) == 0usize
        && multiply(maximum, 1usize) == maximum
        && multiply(1usize, maximum) == maximum
        && multiply(9223372036854775807usize, 2usize) == 18446744073709551614usize
        && multiply(6148914691236517205usize, 3usize) == maximum
    { 0 } else { 1 }
"#;

fn ordinary_module(body: &str) -> Vec<u8> {
    let source = SOURCE.replace("MAIN_BODY", body);
    let program = parse(&source, Path::new("usize-multiplication.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert!(graph::to_json(&program)
        .unwrap()
        .contains("\"schema\":\"semaprax.graph.v17\""));
    let resolved = hir::resolve(&program).unwrap();
    let bytes = wasm::emit_resolved_module(&resolved).unwrap();
    assert_eq!(bytes, wasm::emit_module(&reparsed).unwrap());
    Validator::new().validate_all(&bytes).unwrap();

    // The ordinary route has exactly the seven scalar runtime imports and
    // sole main export: no aggregate test adapter, memory or byte arena.
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.unwrap();
                    assert_eq!(import.module, "env");
                    imports.push(import.name.to_owned());
                }
            }
            Payload::ExportSection(section) => {
                for export in section {
                    exports.push(export.unwrap().name.to_owned());
                }
            }
            Payload::MemorySection(_) => panic!("ordinary usize route has no memory"),
            _ => {}
        }
    }
    assert_eq!(
        imports,
        [
            "spx_add",
            "spx_sub",
            "spx_mul",
            "spx_div",
            "spx_rem",
            "spx_neg",
            "spx_contract_fail",
        ]
    );
    assert_eq!(exports, ["semaprax_main"]);
    bytes
}

#[test]
fn ordinary_usize_multiplication_zero_maximum_and_overflow() {
    let artifacts = [
        ("success.wasm", ordinary_module(SUCCESS)),
        (
            "overflow.wasm",
            ordinary_module(
                "if multiply(18446744073709551615usize, 2usize) == 0usize { 0 } else { 1 }",
            ),
        ),
        (
            "boundary-overflow.wasm",
            ordinary_module(
                "if multiply(9223372036854775808usize, 2usize) == 0usize { 0 } else { 1 }",
            ),
        ),
    ];
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-ordinary-usize-mul-{}-{id}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    for (name, bytes) in &artifacts {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(name))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
    let script = r#"
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
const env = Object.fromEntries([
  'spx_add', 'spx_sub', 'spx_div', 'spx_rem', 'spx_neg', 'spx_contract_fail',
].map(name => [name, () => { throw new Error(`unexpected host import ${name}`); }]));
env.spx_mul = (left, right) => {
  const value = BigInt.asUintN(64, left) * BigInt.asUintN(64, right);
  if (value > 18446744073709551615n) throw new WebAssembly.RuntimeError('usize multiplication overflow');
  return BigInt.asIntN(64, value);
};
for (const [file, succeeds] of [
  ['success.wasm', true], ['overflow.wasm', false], ['boundary-overflow.wasm', false],
]) {
  const { instance } = await WebAssembly.instantiate(readFileSync(join(process.argv[1], file)), { env });
  for (let repeat = 0; repeat < 3; repeat++) {
    if (succeeds) assert.equal(instance.exports.semaprax_main(), 0n);
    else assert.throws(() => instance.exports.semaprax_main(), WebAssembly.RuntimeError);
  }
}
"#;
    let result = Command::new("node")
        .args(["--input-type=module", "--eval", script])
        .arg(&directory)
        .output()
        .expect("Node is required for the ordinary Wasm multiplication gate");
    assert!(
        result.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());

    // Retain evidence on failure; preflight the complete fixed inventory
    // before any successful-fixture cleanup, without recursive deletion.
    let mut names = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        ["boundary-overflow.wasm", "overflow.wasm", "success.wasm"]
    );
    assert!(fs::symlink_metadata(&directory).unwrap().is_dir());
    for (name, bytes) in &artifacts {
        let path = directory.join(name);
        assert!(fs::symlink_metadata(&path).unwrap().is_file());
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
    for (name, _) in &artifacts {
        fs::remove_file(directory.join(name)).unwrap();
    }
    fs::remove_dir(directory).unwrap();
}
