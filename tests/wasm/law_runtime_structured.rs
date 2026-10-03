//! LAW-13: one bounded authored record/variant fixture through the reference
//! value, interpreter, and actually executed Core Wasm. This is differential
//! translation evidence for this program, not a preservation theorem.

use std::path::Path;
use std::process::Command;

use semaprax::{hir, interpreter, parse, verify, wasm};

const SOURCE: &str = r#"module law.structured;
@id("law.choice") variant Choice {
    @id("law.choice.left") Left,
    @id("law.choice.right") Right,
}
@id("law.pair") record Pair {
    @id("law.pair.left") left: i64,
    @id("law.pair.right") right: i64,
}
@id("app.main") fn main() -> i64 {
    let pair = Pair { left: 10, right: 23 };
    let selected = match Choice::Right {} { Choice::Left {} => 0, Choice::Right {} => 9, };
    pair.left + pair.right + selected
}
"#;

fn observe(source: &str, expected: i64, label: &str) {
    let root = std::env::temp_dir().join(format!(
        "semaprax-law-runtime-structured-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let source_path = root.join("case.spx");
    let wasm_path = root.join("case.wasm");
    std::fs::write(&source_path, source).unwrap();
    let program = parse(source, Path::new(&source_path)).unwrap();
    assert!(verify::verify(&program).is_empty());
    let resolved = hir::resolve(&program).unwrap();
    let bytes = wasm::emit_resolved_module(&resolved).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    std::fs::write(&wasm_path, bytes).unwrap();

    let interpreted = interpreter::interpret(
        &source_path,
        "app.main",
        &[],
        &interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    assert!(interpreted.returned, "{}", interpreted.envelope);
    let envelope: serde_json::Value = serde_json::from_str(&interpreted.envelope).unwrap();
    assert_eq!(
        envelope["payload"]["outcome"]["value"],
        expected.to_string()
    );

    let script = r#"
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const env = Object.fromEntries([
  'spx_add', 'spx_sub', 'spx_mul', 'spx_div', 'spx_rem', 'spx_neg', 'spx_contract_fail',
].map(name => [name, () => { throw new Error(`unexpected import ${name}`); }]));
const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[1]), { env });
assert.equal(instance.exports.semaprax_main(), BigInt(process.argv[2]));
"#;
    let output = Command::new("node")
        .args(["--input-type=module", "--eval", script])
        .arg(&wasm_path)
        .arg(expected.to_string())
        .output()
        .expect("Node is required for the LAW-13 structured runtime gate");
    assert!(
        output.status.success(),
        "Node stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn law_runtime_record_variant_interpreter_and_emitted_wasm_agree() {
    observe(SOURCE, 42, "baseline");
    let mutant = SOURCE.replace("right: 23", "right: 24");
    assert_ne!(mutant, SOURCE);
    observe(&mutant, 43, "mutant");
    assert_ne!(
        42, 43,
        "the seeded body change must be visible to both runtimes"
    );
}
